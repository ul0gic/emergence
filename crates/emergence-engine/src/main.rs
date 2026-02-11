//! World Engine binary for the Emergence simulation.
//!
//! This is the main entry point that wires together the tick cycle,
//! starting world, agent spawner, NATS decision source, and operator
//! controls. It loads configuration, initializes all subsystems, and
//! runs the simulation loop until a termination condition is met.
//!
//! # Startup Sequence
//!
//! 1. Initialize structured logging (tracing)
//! 2. Load configuration from `emergence-config.yaml`
//! 3. Create world clock from time config
//! 4. Create starting world map (12 locations, 17 routes)
//! 5. Spawn seed agents across locations
//! 6. Connect to NATS and create decision source
//! 7. Create operator state from simulation bounds
//! 8. Run the simulation loop
//! 9. Log the result

mod error;
mod nats_decision;
mod observer_callback;
mod spawner;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use emergence_agents::actions::conflict::ConflictStrategy;
use emergence_agents::config::VitalsConfig;
use emergence_core::clock::WorldClock;
use emergence_core::config::SimulationConfig;
use emergence_core::operator::OperatorState;
use emergence_core::runner;
use emergence_core::tick::SimulationState;
use emergence_observer::state::AppState;
use emergence_world::WeatherSystem;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::error::EngineError;
use crate::nats_decision::NatsDecisionSource;
use crate::observer_callback::ObserverCallback;
use crate::spawner::SpawnerConfig;

/// Application entry point for the World Engine.
///
/// Initializes all subsystems and runs the simulation loop. Returns
/// an error code on failure.
///
/// # Errors
///
/// Returns an error if any initialization step or the simulation itself fails.
#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize structured logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(true)
        .init();

    info!("emergence-engine starting");

    // 2. Load configuration.
    let config = load_config()?;
    info!(
        world_name = config.world.name,
        seed = config.world.seed,
        tick_interval_ms = config.world.tick_interval_ms,
        agent_decision_timeout_ms = config.world.agent_decision_timeout_ms,
        "Configuration loaded"
    );

    // 3. Create world clock.
    let clock = WorldClock::new(&config.time)?;
    info!("World clock initialized");

    // 4. Create starting world map.
    let (mut world_map, location_ids) = emergence_world::create_starting_world()?;
    info!(
        location_count = world_map.location_count(),
        first_location = %location_ids.riverbank,
        "Starting world created"
    );

    // 5. Spawn seed agents.
    let spawner_config = load_spawner_config()?;
    info!(
        seed_count = spawner_config.seed_count,
        personality_mode = spawner_config.personality_mode,
        seed_knowledge_count = spawner_config.seed_knowledge.len(),
        "Spawner configuration loaded"
    );

    let spawn_result = spawner::spawn_seed_agents(&spawner_config, &mut world_map)?;
    info!(
        agents_spawned = spawn_result.agent_names.len(),
        "Seed agents spawned"
    );

    // 6. Connect to NATS and create decision source.
    let nats_url = &config.infrastructure.nats_url;
    let decision_timeout_ms = config.world.agent_decision_timeout_ms;
    let timeout = Duration::from_millis(decision_timeout_ms);

    info!(nats_url = nats_url, timeout_ms = decision_timeout_ms, "Connecting to NATS");
    let mut decision_source = NatsDecisionSource::connect(nats_url, timeout)
        .await
        .map_err(|e| EngineError::Nats {
            message: format!("{e}"),
        })?;
    info!("NATS decision source connected");

    // 7. Create operator state.
    let operator = Arc::new(OperatorState::new(
        config.world.tick_interval_ms,
        &config.simulation,
    ));
    info!(
        max_ticks = operator.max_ticks(),
        max_real_time_seconds = operator.max_real_time_seconds(),
        tick_interval_ms = operator.tick_interval_ms(),
        "Operator state initialized"
    );

    // 8. Start Observer API server.
    let observer_port = config.infrastructure.observer_port;
    let app_state = Arc::new(AppState::with_operator(Arc::clone(&operator)));
    let _observer_handle = emergence_observer::spawn_observer(observer_port, Arc::clone(&app_state))
        .await
        .map_err(|e| EngineError::Observer {
            message: format!("{e}"),
        })?;
    info!(port = observer_port, "Observer API server started");

    // 8b. Subscribe to decision records from the runner.
    //     Uses a separate NATS connection so the decision collector runs
    //     independently from the tick-cycle decision source.
    {
        let decisions_state = Arc::clone(&app_state);
        match async_nats::connect(nats_url).await {
            Ok(decisions_client) => {
                match decisions_client
                    .subscribe("emergence.decisions.>".to_owned())
                    .await
                {
                    Ok(mut sub) => {
                        tokio::spawn(async move {
                            use emergence_observer::state::MAX_DECISIONS;
                            use futures::StreamExt as _;
                            while let Some(msg) = sub.next().await {
                                match serde_json::from_slice::<
                                    emergence_types::DecisionRecord,
                                >(&msg.payload)
                                {
                                    Ok(record) => {
                                        if let Ok(mut snap) =
                                            decisions_state.snapshot.try_write()
                                        {
                                            snap.decisions.push(record);
                                            if snap.decisions.len() > MAX_DECISIONS {
                                                let drain_count = snap
                                                    .decisions
                                                    .len()
                                                    .saturating_sub(MAX_DECISIONS);
                                                snap.decisions.drain(..drain_count);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            error = %e,
                                            "failed to deserialize decision record"
                                        );
                                    }
                                }
                            }
                        });
                        info!("Decision record collector started");
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "failed to subscribe to decision records, \
                             decision logging disabled"
                        );
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "failed to connect second NATS client for decision records, \
                     decision logging disabled"
                );
            }
        }
    }

    // 9. Assemble simulation state.
    let weather_seed = config.world.seed;
    let mut sim_state = SimulationState {
        clock,
        world_map,
        weather_system: WeatherSystem::new(weather_seed),
        agents: spawn_result.agents,
        agent_names: spawn_result.agent_names,
        agent_states: spawn_result.agent_states,
        alive_agents: spawn_result.alive_agents,
        vitals_config: VitalsConfig::default(),
        conflict_strategy: ConflictStrategy::FirstComeFirstServed,
        injected_events: Vec::new(),
        active_plagues: Vec::new(),
        active_resource_booms: Vec::new(),
    };

    let mut callback = ObserverCallback::new(app_state);

    // 9b. Create spawn handler for mid-simulation agent injection.
    let mut spawn_handler =
        spawner::EngineSpawnHandler::new(spawner_config.seed_knowledge.clone());
    let min_population = config.simulation.min_population;

    info!(
        min_population = min_population,
        "Simulation state assembled, entering tick loop"
    );

    // 10. Run the simulation.
    let result = runner::run_simulation_with_spawner(
        &mut sim_state,
        &mut decision_source,
        &operator,
        &mut callback,
        &mut spawn_handler,
        min_population,
    )
    .await?;

    // 11. Log results.
    runner::log_simulation_end(&result);

    info!(
        end_reason = ?result.end_reason,
        total_ticks = result.total_ticks,
        "emergence-engine shutdown complete"
    );

    Ok(())
}

/// Load the main simulation configuration from `emergence-config.yaml`.
///
/// Looks for the config file relative to the current working directory.
fn load_config() -> Result<SimulationConfig, EngineError> {
    let config_path = Path::new("emergence-config.yaml");
    if config_path.exists() {
        let config = SimulationConfig::from_file(config_path)?;
        Ok(config)
    } else {
        info!("Config file not found, using defaults");
        Ok(SimulationConfig::default())
    }
}

/// Load spawner configuration from `emergence-config.yaml`.
///
/// Reads the `agents` section from the YAML config file. If the file
/// does not exist or lacks the `agents` key, defaults are used.
fn load_spawner_config() -> Result<SpawnerConfig, EngineError> {
    let config_path = Path::new("emergence-config.yaml");
    if config_path.exists() {
        let contents = std::fs::read_to_string(config_path).map_err(|e| EngineError::Spawner {
            message: format!("failed to read config file: {e}"),
        })?;

        // Parse the full YAML and extract just the "agents" section.
        let raw: serde_yml::Value =
            serde_yml::from_str(&contents).map_err(|e| EngineError::Spawner {
                message: format!("failed to parse config YAML: {e}"),
            })?;

        if let Some(agents_value) = raw.get("agents") {
            let spawner_config: SpawnerConfig = serde_yml::from_value(agents_value.clone())
                .map_err(|e| EngineError::Spawner {
                    message: format!("failed to parse agents config: {e}"),
                })?;
            Ok(spawner_config)
        } else {
            Ok(SpawnerConfig::default())
        }
    } else {
        Ok(SpawnerConfig::default())
    }
}
