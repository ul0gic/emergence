//! Experiment framework for A/B testing and reproducible simulations.
//!
//! Provides utilities for:
//! - Creating experiment pairs with identical starting conditions but
//!   different personality distributions (A/B testing)
//! - Saving and restoring full simulation state via snapshots
//! - Associating experiment configs with simulation runs
//!
//! See: `build-plan.md` Phase 5.2

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::ExperimentConfig;
use crate::tick::SimulationState;

/// A serializable snapshot of the full simulation state.
///
/// This captures everything needed to reconstruct the simulation at
/// a specific tick: world map, all agent states, clock state, alive
/// agents, and system configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationSnapshot {
    /// The tick at which this snapshot was taken.
    pub tick: u64,
    /// Serialized clock state.
    pub clock: serde_json::Value,
    /// Serialized world map state.
    pub world_map: serde_json::Value,
    /// All agent identity records, keyed by agent ID string.
    pub agents: BTreeMap<String, serde_json::Value>,
    /// All agent mutable states, keyed by agent ID string.
    pub agent_states: BTreeMap<String, serde_json::Value>,
    /// Agent names, keyed by agent ID string.
    pub agent_names: BTreeMap<String, String>,
    /// IDs of alive agents (as strings).
    pub alive_agents: Vec<String>,
    /// Serialized vitals configuration.
    pub vitals_config: serde_json::Value,
    /// Weather system seed.
    pub weather_seed: u64,
}

/// Error type for experiment operations.
#[derive(Debug, thiserror::Error)]
pub enum ExperimentError {
    /// Serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Capture a full snapshot of the simulation state.
///
/// The snapshot is serialized as JSON values so it can be stored
/// in `PostgreSQL` as a JSONB blob via the experiment store.
///
/// # Errors
///
/// Returns [`ExperimentError::Serialization`] if any state component
/// fails to serialize.
pub fn capture_snapshot(state: &SimulationState) -> Result<SimulationSnapshot, ExperimentError> {
    let tick = state.clock.tick();

    let clock_json = serde_json::to_value(&state.clock)
        .map_err(|e| ExperimentError::Serialization(format!("clock: {e}")))?;

    let world_map_json = serde_json::to_value(&state.world_map)
        .map_err(|e| ExperimentError::Serialization(format!("world_map: {e}")))?;

    let mut agents = BTreeMap::new();
    for (id, agent) in &state.agents {
        let key = id.to_string();
        let val = serde_json::to_value(agent)
            .map_err(|e| ExperimentError::Serialization(format!("agent {key}: {e}")))?;
        agents.insert(key, val);
    }

    let mut agent_states = BTreeMap::new();
    for (id, agent_state) in &state.agent_states {
        let key = id.to_string();
        let val = serde_json::to_value(agent_state)
            .map_err(|e| ExperimentError::Serialization(format!("agent_state {key}: {e}")))?;
        agent_states.insert(key, val);
    }

    let agent_names: BTreeMap<String, String> = state
        .agent_names
        .iter()
        .map(|(id, name)| (id.to_string(), name.clone()))
        .collect();

    let alive_agents: Vec<String> = state
        .alive_agents
        .iter()
        .map(ToString::to_string)
        .collect();

    let vitals_config = serde_json::to_value(&state.vitals_config)
        .map_err(|e| ExperimentError::Serialization(format!("vitals_config: {e}")))?;

    Ok(SimulationSnapshot {
        tick,
        clock: clock_json,
        world_map: world_map_json,
        agents,
        agent_states,
        agent_names,
        alive_agents,
        vitals_config,
        weather_seed: 0, // TODO: extract from weather system
    })
}

/// An experiment pair for A/B testing.
///
/// Both experiments share the same world seed, agent count, and starting
/// conditions, but differ in personality distribution (or other overrides).
#[derive(Debug, Clone)]
pub struct ExperimentPair {
    /// The control experiment (group A).
    pub control: ExperimentConfig,
    /// The treatment experiment (group B).
    pub treatment: ExperimentConfig,
}

/// Create a pair of experiment configs with identical starting conditions
/// but different personality distributions.
///
/// Both experiments get the same `world_seed`, `agent_count`, and `max_ticks`.
/// The control group uses the `control_distribution` personality mode, and
/// the treatment group uses the `treatment_distribution` personality mode.
///
/// Each experiment gets a unique `experiment_id` for post-hoc comparison.
pub fn create_experiment_pair(
    base_name: &str,
    world_seed: u64,
    agent_count: u32,
    max_ticks: u64,
    control_distribution: &str,
    treatment_distribution: &str,
) -> ExperimentPair {
    let mut control = ExperimentConfig::new(&format!("{base_name} (control)"));
    control.description = format!(
        "Control group: {control_distribution} personality distribution"
    );
    control.world_seed = Some(world_seed);
    control.agent_count = Some(agent_count);
    control.max_ticks = max_ticks;
    control_distribution.clone_into(&mut control.personality_distribution);

    let mut treatment = ExperimentConfig::new(&format!("{base_name} (treatment)"));
    treatment.description = format!(
        "Treatment group: {treatment_distribution} personality distribution"
    );
    treatment.world_seed = Some(world_seed);
    treatment.agent_count = Some(agent_count);
    treatment.max_ticks = max_ticks;
    treatment_distribution.clone_into(&mut treatment.personality_distribution);

    ExperimentPair { control, treatment }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn experiment_pair_has_unique_ids() {
        let pair = create_experiment_pair(
            "test", 42, 10, 100, "balanced", "aggressive",
        );
        assert_ne!(pair.control.experiment_id, pair.treatment.experiment_id);
    }

    #[test]
    fn experiment_pair_shares_seed() {
        let pair = create_experiment_pair(
            "test", 42, 10, 100, "balanced", "aggressive",
        );
        assert_eq!(pair.control.world_seed, Some(42));
        assert_eq!(pair.treatment.world_seed, Some(42));
    }

    #[test]
    fn experiment_pair_has_different_distributions() {
        let pair = create_experiment_pair(
            "test", 42, 10, 100, "balanced", "aggressive",
        );
        assert_eq!(pair.control.personality_distribution, "balanced");
        assert_eq!(pair.treatment.personality_distribution, "aggressive");
    }

    #[test]
    fn experiment_pair_shares_agent_count() {
        let pair = create_experiment_pair(
            "test", 42, 10, 100, "balanced", "aggressive",
        );
        assert_eq!(pair.control.agent_count, Some(10));
        assert_eq!(pair.treatment.agent_count, Some(10));
    }

    #[test]
    fn experiment_pair_shares_max_ticks() {
        let pair = create_experiment_pair(
            "test", 42, 10, 100, "balanced", "aggressive",
        );
        assert_eq!(pair.control.max_ticks, 100);
        assert_eq!(pair.treatment.max_ticks, 100);
    }

    #[test]
    fn experiment_config_from_yaml() {
        let yaml = r#"
name: "Test Experiment"
description: "Testing YAML parsing"
agent_count: 20
personality_distribution: "cooperative"
world_seed: 123
max_ticks: 500
parameter_overrides:
  economy.hunger_rate: "3"
  economy.starvation_damage: "5"
"#;
        let config = ExperimentConfig::parse(yaml).unwrap();
        assert_eq!(config.name, "Test Experiment");
        assert_eq!(config.agent_count, Some(20));
        assert_eq!(config.personality_distribution, "cooperative");
        assert_eq!(config.world_seed, Some(123));
        assert_eq!(config.max_ticks, 500);
        assert_eq!(
            config.parameter_overrides.get("economy.hunger_rate"),
            Some(&"3".to_owned())
        );
    }

    #[test]
    fn experiment_config_default() {
        let config = ExperimentConfig::default();
        assert!(!config.experiment_id.is_empty());
        assert_eq!(config.personality_distribution, "random");
        assert_eq!(config.agent_count, None);
        assert_eq!(config.world_seed, None);
        assert_eq!(config.max_ticks, 0);
    }
}
