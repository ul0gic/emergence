//! Simulation loop runner with operator controls.
//!
//! This module provides [`run_simulation`], the top-level async function
//! that drives the tick loop with support for:
//!
//! - **Bounded simulation**: stop after `max_ticks` or `max_real_time_seconds`
//! - **Pause/resume**: operator can halt and continue the tick loop
//! - **Variable tick speed**: tick interval adjustable at runtime
//! - **Clean shutdown**: final snapshot, `SimulationEnded` event, graceful stop
//! - **Operator stop**: immediate clean stop via REST API
//!
//! The runner wraps the single-tick [`run_tick`] function and adds the
//! control plane around it.
//!
//! [`run_tick`]: crate::tick::run_tick

use std::sync::Arc;

use tracing::{info, warn};

use crate::decision::DecisionSource;
use crate::operator::{OperatorState, SpawnRequest, SimulationEndReason};
use crate::tick::{self, SimulationState, TickError, TickSummary};

/// Errors that can occur during the simulation run.
#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    /// A tick execution failed.
    #[error("tick error: {source}")]
    Tick {
        /// The underlying tick error.
        #[from]
        source: TickError,
    },
}

/// Result of the simulation run.
#[derive(Debug)]
pub struct SimulationResult {
    /// The reason the simulation ended.
    pub end_reason: SimulationEndReason,
    /// The last tick summary, if any tick completed.
    pub final_summary: Option<TickSummary>,
    /// Total number of ticks executed.
    pub total_ticks: u64,
}

/// Callback invoked after each tick completes.
///
/// Implementations can use this to update the observer snapshot,
/// broadcast tick summaries, etc. The callback receives the tick
/// summary and the current simulation state.
pub trait TickCallback: Send {
    /// Called after a tick completes successfully.
    fn on_tick(&mut self, summary: &TickSummary, state: &SimulationState);
}

/// A no-op tick callback for testing.
pub struct NoOpCallback;

impl TickCallback for NoOpCallback {
    fn on_tick(&mut self, _summary: &TickSummary, _state: &SimulationState) {}
}

/// Handler for processing agent spawn requests.
///
/// The runner calls this to create new agents from [`SpawnRequest`]s
/// during the pre-tick phase. The handler is responsible for creating
/// the `Agent` + `AgentState`, registering the occupant on the world
/// map, and inserting into the simulation state.
///
/// Implementations live in the engine crate where they have access to
/// the spawner infrastructure (name pool, personality generation, etc.).
pub trait SpawnHandler: Send {
    /// Process a single spawn request and integrate the new agent into
    /// the simulation state.
    ///
    /// Returns `true` if the agent was successfully created and added,
    /// `false` if the spawn failed (non-fatal; the runner logs a warning
    /// and continues).
    fn handle_spawn(&mut self, request: &SpawnRequest, state: &mut SimulationState) -> bool;
}

/// A no-op spawn handler that always returns `false`.
///
/// Used in tests where spawn processing is not needed.
pub struct NoOpSpawnHandler;

impl SpawnHandler for NoOpSpawnHandler {
    fn handle_spawn(&mut self, _request: &SpawnRequest, _state: &mut SimulationState) -> bool {
        false
    }
}

/// Run the simulation loop until a termination condition is met.
///
/// This is the main entry point for a bounded simulation run. It
/// integrates the tick cycle with operator controls (pause, resume,
/// speed, stop) and simulation boundaries (max ticks, max time).
///
/// # Arguments
///
/// * `state` - Mutable simulation state (world, agents, clock)
/// * `decision_source` - Source of agent decisions (LLM, stub, etc.)
/// * `operator` - Shared operator control state
/// * `callback` - Called after each tick for observer updates
///
/// # Returns
///
/// Returns a [`SimulationResult`] describing why the simulation ended
/// and the final tick summary.
///
/// # Errors
///
/// Returns [`RunnerError`] if a tick execution fails unrecoverably.
pub async fn run_simulation(
    state: &mut SimulationState,
    decision_source: &mut dyn DecisionSource,
    operator: &Arc<OperatorState>,
    callback: &mut dyn TickCallback,
) -> Result<SimulationResult, RunnerError> {
    run_simulation_with_spawner(
        state,
        decision_source,
        operator,
        callback,
        &mut NoOpSpawnHandler,
        0,
    )
    .await
}

/// Run the simulation loop with agent spawning support.
///
/// Like [`run_simulation`], but additionally accepts a [`SpawnHandler`]
/// for processing mid-simulation agent injection and a `min_population`
/// floor for auto-recovery.
///
/// # Arguments
///
/// * `state` - Mutable simulation state (world, agents, clock)
/// * `decision_source` - Source of agent decisions (LLM, stub, etc.)
/// * `operator` - Shared operator control state
/// * `callback` - Called after each tick for observer updates
/// * `spawn_handler` - Processes spawn requests into new agents
/// * `min_population` - Auto-spawn threshold (0 = disabled)
///
/// # Returns
///
/// Returns a [`SimulationResult`] describing why the simulation ended
/// and the final tick summary.
///
/// # Errors
///
/// Returns [`RunnerError`] if a tick execution fails unrecoverably.
#[allow(clippy::too_many_lines)]
pub async fn run_simulation_with_spawner(
    state: &mut SimulationState,
    decision_source: &mut dyn DecisionSource,
    operator: &Arc<OperatorState>,
    callback: &mut dyn TickCallback,
    spawn_handler: &mut dyn SpawnHandler,
    min_population: u32,
) -> Result<SimulationResult, RunnerError> {
    let mut last_summary: Option<TickSummary> = None;
    let mut total_ticks: u64 = 0;

    info!(
        max_ticks = operator.max_ticks(),
        max_real_time_seconds = operator.max_real_time_seconds(),
        tick_interval_ms = operator.tick_interval_ms(),
        min_population = min_population,
        "Simulation starting"
    );

    loop {
        // --- Check pause ---
        if operator.is_paused() {
            info!("Simulation paused, waiting for resume...");
            operator.wait_if_paused().await;
            info!("Simulation resumed");
        }

        // --- Check stop request (before tick) ---
        if operator.is_stop_requested() {
            info!("Operator stop requested");
            let reason = SimulationEndReason::OperatorStop;
            operator.set_end_reason(reason.clone()).await;
            return Ok(SimulationResult {
                end_reason: reason,
                final_summary: last_summary,
                total_ticks,
            });
        }

        // --- Check time limit (before tick) ---
        if operator.time_limit_reached() {
            info!(
                max_seconds = operator.max_real_time_seconds(),
                elapsed = operator.elapsed_seconds(),
                "Real-time limit reached"
            );
            let reason = SimulationEndReason::MaxRealTimeReached;
            operator.set_end_reason(reason.clone()).await;
            return Ok(SimulationResult {
                end_reason: reason,
                final_summary: last_summary,
                total_ticks,
            });
        }

        // --- Process spawn queue (before tick) ---
        let spawn_requests = operator.drain_spawn_queue().await;
        if !spawn_requests.is_empty() {
            info!(count = spawn_requests.len(), "Processing spawn queue");
            for request in &spawn_requests {
                if !spawn_handler.handle_spawn(request, state) {
                    warn!(?request, "Failed to process spawn request");
                }
            }
        }

        // --- Drain injected events from operator into simulation state ---
        let injected_events = operator.drain_injected_events().await;
        if !injected_events.is_empty() {
            info!(count = injected_events.len(), "Processing injected world events");
            state.injected_events.extend(injected_events);
        }

        // --- Process migration spawn requests from previous tick's world events ---
        let migration_spawns: Vec<String> = state
            .injected_events
            .iter()
            .filter(|e| e.event_type == "_migration_spawn")
            .filter_map(|e| e.target_region.clone())
            .collect();
        state.injected_events.retain(|e| e.event_type != "_migration_spawn");
        for loc_id_str in &migration_spawns {
            if let Ok(uuid) = loc_id_str.parse::<uuid::Uuid>() {
                let location_id = emergence_types::LocationId::from(uuid);
                let request = SpawnRequest {
                    name: None,
                    location_id: Some(location_id),
                    personality_mode: String::from("random"),
                };
                if !spawn_handler.handle_spawn(&request, state) {
                    warn!("Failed to spawn migration agent");
                }
            }
        }

        // --- Execute tick ---
        let summary = tick::run_tick(state, decision_source)?;

        total_ticks = total_ticks.saturating_add(1);

        // --- Notify callback ---
        callback.on_tick(&summary, state);

        // --- Check extinction ---
        if summary.agents_alive == 0 {
            // Before declaring extinction, check if auto-recovery can save us.
            if min_population > 0 {
                let needed = min_population;
                warn!(
                    alive = 0u32,
                    min = min_population,
                    spawning = needed,
                    "Population below minimum (extinction), auto-spawning {needed} agents"
                );
                for _ in 0..needed {
                    let request = SpawnRequest {
                        name: None,
                        location_id: None,
                        personality_mode: String::from("random"),
                    };
                    if !spawn_handler.handle_spawn(&request, state) {
                        warn!("Failed to auto-spawn recovery agent");
                    }
                }
                // Re-check after auto-spawn: if we still have 0 alive, declare extinction.
                if state.alive_agents.is_empty() {
                    info!(tick = summary.tick, "All agents dead -- extinction (auto-recovery failed)");
                    let reason = SimulationEndReason::Extinction;
                    operator.set_end_reason(reason.clone()).await;
                    return Ok(SimulationResult {
                        end_reason: reason,
                        final_summary: Some(summary),
                        total_ticks,
                    });
                }
                // Auto-recovery succeeded, continue the loop.
            } else {
                info!(tick = summary.tick, "All agents dead -- extinction");
                let reason = SimulationEndReason::Extinction;
                operator.set_end_reason(reason.clone()).await;
                return Ok(SimulationResult {
                    end_reason: reason,
                    final_summary: Some(summary),
                    total_ticks,
                });
            }
        } else if min_population > 0 {
            // --- Auto-population recovery (non-extinction case) ---
            let alive = summary.agents_alive;
            if alive < min_population {
                let needed = min_population.saturating_sub(alive);
                warn!(
                    alive = alive,
                    min = min_population,
                    spawning = needed,
                    "Population below minimum ({alive}/{min_population}), auto-spawning {needed} agents"
                );
                for _ in 0..needed {
                    let request = SpawnRequest {
                        name: None,
                        location_id: None,
                        personality_mode: String::from("random"),
                    };
                    if !spawn_handler.handle_spawn(&request, state) {
                        warn!("Failed to auto-spawn recovery agent");
                    }
                }
            }
        }

        // --- Check tick limit (after tick) ---
        // run_tick advances the clock internally, so summary.tick is the
        // tick number that just ran. If max_ticks is 5, we stop after
        // tick 5 has completed (total_ticks == 5).
        if operator.tick_limit_reached(summary.tick) {
            info!(
                tick = summary.tick,
                max_ticks = operator.max_ticks(),
                "Tick limit reached"
            );
            let reason = SimulationEndReason::MaxTicksReached;
            operator.set_end_reason(reason.clone()).await;
            return Ok(SimulationResult {
                end_reason: reason,
                final_summary: Some(summary),
                total_ticks,
            });
        }

        last_summary = Some(summary);

        // --- Sleep for tick interval ---
        let interval_ms = operator.tick_interval_ms();
        if interval_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)).await;
        }
    }
}

/// Log the simulation end sequence.
///
/// This should be called after [`run_simulation`] returns to perform
/// the final snapshot and logging. The HTTP server should remain
/// running after this returns.
pub fn log_simulation_end(result: &SimulationResult) {
    info!(
        reason = ?result.end_reason,
        total_ticks = result.total_ticks,
        final_tick = result.final_summary.as_ref().map(|s| s.tick),
        final_agents_alive = result.final_summary.as_ref().map(|s| s.agents_alive),
        "Simulation ended"
    );

    if let Some(ref summary) = result.final_summary {
        info!(
            tick = summary.tick,
            agents_alive = summary.agents_alive,
            season = ?summary.season,
            weather = ?summary.weather,
            "Final tick summary"
        );
    } else {
        warn!("Simulation ended with no ticks executed");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;

    use chrono::Utc;
    use emergence_types::*;
    use rust_decimal::Decimal;

    use super::*;
    use crate::clock::WorldClock;
    use crate::config::{SimulationBoundsConfig, TimeConfig};
    use crate::decision::StubDecisionSource;

    fn default_time_config() -> TimeConfig {
        TimeConfig {
            ticks_per_season: 90,
            seasons: vec![
                "spring".to_owned(),
                "summer".to_owned(),
                "autumn".to_owned(),
                "winter".to_owned(),
            ],
            day_night: true,
        }
    }

    fn make_agent_state(agent_id: AgentId, location_id: LocationId) -> AgentState {
        AgentState {
            agent_id,
            energy: 80,
            health: 100,
            hunger: 0,
            thirst: 0,
            age: 0,
            born_at_tick: 0,
            location_id,
            destination_id: None,
            travel_progress: 0,
            inventory: BTreeMap::new(),
            carry_capacity: 50,
            knowledge: BTreeSet::new(),
            skills: BTreeMap::new(),
            skill_xp: BTreeMap::new(),
            goals: Vec::new(),
            relationships: BTreeMap::new(),
            memory: Vec::new(),
        }
    }

    fn make_location(id: LocationId, name: &str) -> Location {
        let mut resources = BTreeMap::new();
        resources.insert(
            Resource::FoodBerry,
            ResourceNode {
                resource: Resource::FoodBerry,
                available: 30,
                regen_per_tick: 3,
                max_capacity: 50,
            },
        );
        Location {
            id,
            name: name.to_string(),
            region: String::from("Test"),
            location_type: String::from("natural"),
            description: format!("Test location: {name}"),
            capacity: 20,
            base_resources: resources,
            discovered_by: BTreeSet::new(),
            created_at: Utc::now(),
        }
    }

    fn make_route(from: LocationId, to: LocationId) -> Route {
        Route {
            id: RouteId::new(),
            from_location: from,
            to_location: to,
            cost_ticks: 3,
            path_type: PathType::DirtTrail,
            durability: 100,
            max_durability: 100,
            decay_per_tick: Decimal::ZERO,
            acl: None,
            bidirectional: true,
            built_by: None,
            built_at_tick: None,
        }
    }

    fn make_simulation_state() -> SimulationState {
        let time_config = default_time_config();
        let clock = WorldClock::new(&time_config).unwrap();

        let mut world_map = emergence_world::WorldMap::new();
        let loc_a = LocationId::new();
        let loc_b = LocationId::new();

        let _ = world_map.add_location(make_location(loc_a, "Meadow"));
        let _ = world_map.add_location(make_location(loc_b, "Forest"));
        let _ = world_map.add_route(make_route(loc_a, loc_b));

        let agent_id = AgentId::new();
        let agent_state = make_agent_state(agent_id, loc_a);

        if let Some(loc) = world_map.get_location_mut(loc_a) {
            let _ = loc.add_occupant(agent_id);
        }

        let mut agent_names = BTreeMap::new();
        agent_names.insert(agent_id, String::from("Alpha"));

        let mut agent_states = BTreeMap::new();
        agent_states.insert(agent_id, agent_state);

        SimulationState {
            clock,
            world_map,
            weather_system: emergence_world::WeatherSystem::new(42),
            agents: BTreeMap::new(),
            agent_names,
            agent_states,
            alive_agents: vec![agent_id],
            vitals_config: emergence_agents::config::VitalsConfig::default(),
            conflict_strategy: emergence_agents::actions::conflict::ConflictStrategy::FirstComeFirstServed,
            injected_events: Vec::new(),
            active_plagues: Vec::new(),
            active_resource_booms: Vec::new(),
        }
    }

    #[tokio::test]
    async fn bounded_by_max_ticks() {
        let mut state = make_simulation_state();
        let mut decisions = StubDecisionSource::new();
        let bounds = SimulationBoundsConfig {
            max_ticks: 5,
            max_real_time_seconds: 0,
            end_condition: String::from("time_limit"),
            min_population: 0,
        };
        let operator = Arc::new(OperatorState::new(0, &bounds));
        let mut cb = NoOpCallback;

        let result = run_simulation(&mut state, &mut decisions, &operator, &mut cb)
            .await
            .unwrap();

        assert_eq!(result.end_reason, SimulationEndReason::MaxTicksReached);
        assert_eq!(result.total_ticks, 5);
    }

    #[tokio::test]
    async fn operator_stop() {
        let mut state = make_simulation_state();
        let mut decisions = StubDecisionSource::new();
        let bounds = SimulationBoundsConfig {
            max_ticks: 0,
            max_real_time_seconds: 0,
            end_condition: String::from("manual"),
            min_population: 0,
        };
        let operator = Arc::new(OperatorState::new(0, &bounds));
        operator.request_stop();
        let mut cb = NoOpCallback;

        let result = run_simulation(&mut state, &mut decisions, &operator, &mut cb)
            .await
            .unwrap();

        assert_eq!(result.end_reason, SimulationEndReason::OperatorStop);
        assert_eq!(result.total_ticks, 0);
    }

    #[tokio::test]
    async fn extinction_stops_simulation() {
        let mut state = make_simulation_state();
        let mut decisions = StubDecisionSource::new();
        let bounds = SimulationBoundsConfig {
            max_ticks: 0,
            max_real_time_seconds: 0,
            end_condition: String::from("extinction"),
            min_population: 0,
        };
        let operator = Arc::new(OperatorState::new(0, &bounds));
        let mut cb = NoOpCallback;

        // Kill the agent by setting extreme hunger
        let agent_id = *state.alive_agents.first().unwrap();
        if let Some(agent_state) = state.agent_states.get_mut(&agent_id) {
            agent_state.hunger = 96;
            agent_state.health = 5;
        }

        let result = run_simulation(&mut state, &mut decisions, &operator, &mut cb)
            .await
            .unwrap();

        assert_eq!(result.end_reason, SimulationEndReason::Extinction);
        // Agent should die in the first tick
        assert_eq!(result.total_ticks, 1);
    }

    #[tokio::test]
    async fn tick_callback_is_called() {
        struct CountCallback {
            count: u64,
        }
        impl TickCallback for CountCallback {
            fn on_tick(&mut self, _summary: &TickSummary, _state: &SimulationState) {
                self.count = self.count.saturating_add(1);
            }
        }

        let mut state = make_simulation_state();
        let mut decisions = StubDecisionSource::new();
        let bounds = SimulationBoundsConfig {
            max_ticks: 3,
            max_real_time_seconds: 0,
            end_condition: String::from("time_limit"),
            min_population: 0,
        };
        let operator = Arc::new(OperatorState::new(0, &bounds));
        let mut cb = CountCallback { count: 0 };

        let _ = run_simulation(&mut state, &mut decisions, &operator, &mut cb)
            .await
            .unwrap();

        assert_eq!(cb.count, 3);
    }

    #[tokio::test]
    async fn variable_speed_changes_interval() {
        let bounds = SimulationBoundsConfig {
            max_ticks: 0,
            max_real_time_seconds: 0,
            end_condition: String::from("manual"),
            min_population: 0,
        };
        let operator = Arc::new(OperatorState::new(1000, &bounds));

        assert_eq!(operator.tick_interval_ms(), 1000);
        let _ = operator.set_tick_interval_ms(500);
        assert_eq!(operator.tick_interval_ms(), 500);
    }
}
