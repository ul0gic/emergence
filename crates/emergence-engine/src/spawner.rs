//! Agent spawner for seeding the simulation with initial agents.
//!
//! At simulation start, the spawner creates N seed agents with random
//! personalities, names, starting vitals, seed knowledge, and distributes
//! them evenly across available world locations. Each agent is registered
//! as an occupant at their starting location.

use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use emergence_types::{Agent, AgentId, AgentState, LocationId, MemoryEntry, Personality, Sex};
use emergence_world::WorldMap;
use rand::Rng;
use rust_decimal::Decimal;
use serde::Deserialize;
use tracing::info;

use crate::error::EngineError;

// -----------------------------------------------------------------------
// Configuration
// -----------------------------------------------------------------------

/// Configuration for the agent spawner, loaded from `emergence-config.yaml`.
///
/// Controls how many seed agents to create, their personality generation
/// mode, and which knowledge concepts they start with.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SpawnerConfig {
    /// Number of agents to spawn at simulation start.
    #[serde(default = "default_seed_count")]
    pub seed_count: u32,

    /// Personality generation mode: `random`, `balanced`, or `custom`.
    #[serde(default = "default_personality_mode")]
    pub personality_mode: String,

    /// Knowledge concepts every seed agent starts with.
    #[serde(default = "default_seed_knowledge")]
    pub seed_knowledge: Vec<String>,
}

impl Default for SpawnerConfig {
    fn default() -> Self {
        Self {
            seed_count: default_seed_count(),
            personality_mode: default_personality_mode(),
            seed_knowledge: default_seed_knowledge(),
        }
    }
}

const fn default_seed_count() -> u32 {
    5
}

fn default_personality_mode() -> String {
    String::from("random")
}

fn default_seed_knowledge() -> Vec<String> {
    vec![
        String::from("fire"),
        String::from("basic_tools"),
        String::from("foraging"),
    ]
}

// -----------------------------------------------------------------------
// Name pool
// -----------------------------------------------------------------------

/// Built-in pool of agent names. The spawner picks randomly without
/// replacement from this list to ensure uniqueness.
const NAME_POOL: &[&str] = &[
    "Alder", "Birch", "Cedar", "Dusk", "Ember", "Fern", "Grove", "Haze",
    "Iris", "Juniper", "Kestrel", "Lark", "Moss", "Nettle", "Oak", "Pine",
    "Quill", "Reed", "Sage", "Thorn", "Umber", "Vale", "Wren", "Yarrow",
    "Zephyr", "Ash", "Brook", "Clay", "Dawn", "Elm", "Flint", "Gale",
    "Heath", "Ivy", "Jay", "Kale", "Lichen", "Maple", "Nyx", "Onyx",
    "Pebble", "Quartz", "Raven", "Sable", "Terra", "Urchin", "Vole",
    "Willow", "Xylem", "Yew",
];

// -----------------------------------------------------------------------
// Spawning result
// -----------------------------------------------------------------------

/// The output of the agent spawner: names and states for all seed agents.
#[derive(Debug)]
pub struct SpawnResult {
    /// Full agent identity records keyed by agent ID.
    pub agents: BTreeMap<AgentId, Agent>,
    /// Agent identity data: agent ID to display name.
    pub agent_names: BTreeMap<AgentId, String>,
    /// Agent mutable state: agent ID to initial state.
    pub agent_states: BTreeMap<AgentId, AgentState>,
    /// Ordered list of alive agent IDs (same order as insertion).
    pub alive_agents: Vec<AgentId>,
}

/// The result of spawning a single agent mid-simulation.
#[derive(Debug)]
pub struct SingleSpawnResult {
    /// The agent identity record.
    pub agent: Agent,
    /// The agent's mutable state.
    pub agent_state: AgentState,
}

// -----------------------------------------------------------------------
// Single-agent spawn (mid-simulation injection)
// -----------------------------------------------------------------------

/// Spawn a single agent for mid-simulation injection.
///
/// Creates one agent with a random (or specified) name, random personality,
/// starting vitals, seed knowledge, and places them at the given (or random)
/// location. The agent is registered as an occupant at their location.
///
/// This function is used by both the auto-population recovery system and
/// the operator spawn-agent endpoint.
///
/// # Arguments
///
/// * `request` - The spawn request specifying optional name and location.
/// * `world_map` - Mutable world map for location selection and occupant registration.
/// * `current_tick` - The current simulation tick (used for `born_at_tick`).
/// * `existing_names` - Set of names already in use, to avoid duplicates.
/// * `seed_knowledge` - Knowledge concepts the new agent starts with.
/// * `preferred_sex` - If `Some`, use this sex; otherwise random 50/50.
///
/// # Errors
///
/// Returns [`EngineError::Spawner`] if no locations exist or no unused
/// names are available.
pub fn spawn_single_agent(
    request: &emergence_core::operator::SpawnRequest,
    world_map: &mut WorldMap,
    current_tick: u64,
    existing_names: &std::collections::BTreeSet<String>,
    seed_knowledge: &[String],
    preferred_sex: Option<Sex>,
) -> Result<SingleSpawnResult, EngineError> {
    let mut rng = rand::rng();

    let location_ids = world_map.location_ids();
    if location_ids.is_empty() {
        return Err(EngineError::Spawner {
            message: String::from("world map has no locations to place agents"),
        });
    }

    // Determine the agent's name.
    let name = if let Some(ref requested_name) = request.name {
        requested_name.clone()
    } else {
        pick_unused_name(&mut rng, existing_names)?
    };

    // Determine the starting location.
    let location_id = if let Some(requested_loc) = request.location_id {
        // Validate the requested location exists.
        if world_map.get_location(requested_loc).is_none() {
            return Err(EngineError::Spawner {
                message: format!("requested location {requested_loc} does not exist"),
            });
        }
        requested_loc
    } else {
        // Pick a random location.
        let idx = rng.random_range(0..location_ids.len());
        location_ids
            .get(idx)
            .copied()
            .ok_or_else(|| EngineError::Spawner {
                message: String::from("failed to select random location"),
            })?
    };

    let agent_id = AgentId::new();
    let knowledge: BTreeSet<String> = seed_knowledge.iter().cloned().collect();

    let agent_state = AgentState {
        agent_id,
        energy: 80,
        health: 100,
        hunger: 0,
        thirst: 0,
        age: 0,
        born_at_tick: current_tick,
        location_id,
        destination_id: None,
        travel_progress: 0,
        inventory: BTreeMap::new(),
        carry_capacity: 50,
        knowledge,
        skills: BTreeMap::new(),
        skill_xp: BTreeMap::new(),
        goals: Vec::new(),
        relationships: BTreeMap::new(),
        memory: Vec::<MemoryEntry>::new(),
    };

    // Use preferred sex if provided, otherwise random 50/50.
    let sex = preferred_sex.unwrap_or_else(|| {
        if rng.random_bool(0.5) { Sex::Male } else { Sex::Female }
    });

    let agent = Agent {
        id: agent_id,
        name: name.clone(),
        sex,
        born_at_tick: current_tick,
        died_at_tick: None,
        cause_of_death: None,
        parent_a: None,
        parent_b: None,
        generation: 0,
        personality: random_personality(&mut rng),
        created_at: Utc::now(),
    };

    // Register as occupant at the location.
    if let Some(loc) = world_map.get_location_mut(location_id) {
        loc.add_occupant(agent_id).map_err(|e| EngineError::Spawner {
            message: format!("failed to add agent {agent_id} to location {location_id}: {e}"),
        })?;
    }

    info!(
        agent_id = %agent_id,
        name = %name,
        sex = %sex,
        location = %location_id,
        tick = current_tick,
        "Spawned agent (mid-simulation)"
    );

    Ok(SingleSpawnResult { agent, agent_state })
}

/// Pick a random unused name from the name pool.
///
/// Tries up to `NAME_POOL.len()` times to find a name not in
/// `existing_names`. Returns an error if all names are in use.
fn pick_unused_name(
    rng: &mut impl Rng,
    existing_names: &std::collections::BTreeSet<String>,
) -> Result<String, EngineError> {
    let pool_len = NAME_POOL.len();

    // Build a list of available names.
    let available: Vec<&str> = NAME_POOL
        .iter()
        .filter(|&&n| !existing_names.contains(n))
        .copied()
        .collect();

    if available.is_empty() {
        // All pool names are taken; generate a fallback name.
        let suffix: u32 = rng.random_range(1000..9999);
        return Ok(format!("Agent-{suffix}"));
    }

    let idx = rng.random_range(0..available.len());
    available
        .get(idx)
        .map(|s| String::from(*s))
        .ok_or_else(|| EngineError::Spawner {
            message: format!("name pool index {idx} out of bounds (pool size: {pool_len})"),
        })
}

// -----------------------------------------------------------------------
// Seed-agent spawn function
// -----------------------------------------------------------------------

/// Spawn seed agents and distribute them across the world map.
///
/// Creates `config.seed_count` agents with random names, random
/// personality traits, starting vitals (energy 80, health 100, hunger 0,
/// age 0), and the configured seed knowledge set. Agents are distributed
/// evenly across all locations in the world map and registered as
/// occupants.
///
/// # Errors
///
/// Returns [`EngineError::Spawner`] if the name pool is too small for the
/// requested seed count, or if a location rejects an occupant.
pub fn spawn_seed_agents(
    config: &SpawnerConfig,
    world_map: &mut WorldMap,
) -> Result<SpawnResult, EngineError> {
    let seed_count = config.seed_count;

    let name_pool_len = u32::try_from(NAME_POOL.len()).unwrap_or(u32::MAX);
    if seed_count > name_pool_len {
        return Err(EngineError::Spawner {
            message: format!(
                "requested {seed_count} agents but name pool only has {name_pool_len} entries"
            ),
        });
    }

    let location_ids = world_map.location_ids();
    if location_ids.is_empty() {
        return Err(EngineError::Spawner {
            message: String::from("world map has no locations to place agents"),
        });
    }

    // Pick unique names randomly.
    let mut rng = rand::rng();
    let names = pick_unique_names(&mut rng, seed_count)?;

    // Assign sex to each agent. When seed_count >= 2, guarantee at least 1
    // male and 1 female so reproduction is possible from the start.
    let sexes = assign_sexes(&mut rng, seed_count);

    let knowledge: BTreeSet<String> = config.seed_knowledge.iter().cloned().collect();
    let location_count = location_ids.len();

    let mut agents = BTreeMap::new();
    let mut agent_names = BTreeMap::new();
    let mut agent_states = BTreeMap::new();
    let mut alive_agents = Vec::new();

    for (i, name) in names.into_iter().enumerate() {
        let sex = sexes.get(i).copied().unwrap_or(Sex::Female);
        let agent_id = AgentId::new();

        // Distribute agents round-robin across locations.
        let loc_index = i.checked_rem(location_count).unwrap_or(0);
        let location_id = location_ids
            .get(loc_index)
            .copied()
            .unwrap_or_else(|| {
                // Fallback: use first location. This branch is unreachable
                // because we checked location_ids is non-empty and loc_index
                // is bounded by location_count, but we handle it gracefully.
                location_ids.first().copied().unwrap_or_else(LocationId::new)
            });

        let state = AgentState {
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
            knowledge: knowledge.clone(),
            skills: BTreeMap::new(),
            skill_xp: BTreeMap::new(),
            goals: Vec::new(),
            relationships: BTreeMap::new(),
            memory: Vec::<MemoryEntry>::new(),
        };

        // Build the immutable Agent identity record.
        let agent = Agent {
            id: agent_id,
            name: name.clone(),
            sex,
            born_at_tick: 0,
            died_at_tick: None,
            cause_of_death: None,
            parent_a: None,
            parent_b: None,
            generation: 0,
            personality: random_personality(&mut rng),
            created_at: Utc::now(),
        };

        // Register agent as occupant at their starting location.
        if let Some(loc) = world_map.get_location_mut(location_id) {
            loc.add_occupant(agent_id).map_err(|e| EngineError::Spawner {
                message: format!("failed to add agent {agent_id} to location {location_id}: {e}"),
            })?;
        }

        info!(
            agent_id = %agent_id,
            name = %name,
            sex = %sex,
            location = %location_id,
            "Spawned seed agent"
        );

        agents.insert(agent_id, agent);
        agent_names.insert(agent_id, name);
        agent_states.insert(agent_id, state);
        alive_agents.push(agent_id);
    }

    Ok(SpawnResult {
        agents,
        agent_names,
        agent_states,
        alive_agents,
    })
}

/// Pick `count` unique names from the name pool using random sampling.
fn pick_unique_names<R: Rng>(rng: &mut R, count: u32) -> Result<Vec<String>, EngineError> {
    let pool_len = NAME_POOL.len();
    let count_usize = usize::try_from(count).map_err(|_conversion_err| EngineError::Spawner {
        message: format!("seed count {count} exceeds usize range"),
    })?;

    if count_usize > pool_len {
        return Err(EngineError::Spawner {
            message: format!(
                "requested {count} names but pool only has {pool_len}"
            ),
        });
    }

    // Fisher-Yates partial shuffle: create index array, shuffle first `count` elements.
    let mut indices: Vec<usize> = (0..pool_len).collect();
    for i in 0..count_usize {
        let j = rng.random_range(i..pool_len);
        indices.swap(i, j);
    }

    let mut names = Vec::with_capacity(count_usize);
    for &idx in indices.iter().take(count_usize) {
        let name = NAME_POOL
            .get(idx)
            .map(|s| String::from(*s))
            .ok_or_else(|| EngineError::Spawner {
                message: format!("name pool index {idx} out of bounds"),
            })?;
        names.push(name);
    }

    Ok(names)
}

/// Generate a random decimal between 0.00 and 1.00.
#[allow(clippy::arithmetic_side_effects)]
fn rand_decimal(rng: &mut impl Rng) -> Decimal {
    let raw: u32 = rng.random_range(0..=100);
    Decimal::from(raw) / Decimal::from(100u32)
}

/// Assign sexes to seed agents.
///
/// When `count >= 2`, the first two agents are guaranteed to be one male and
/// one female (in random order). The remaining agents get a random 50/50
/// assignment. When `count < 2`, each agent gets a random sex.
fn assign_sexes(rng: &mut impl Rng, count: u32) -> Vec<Sex> {
    let count_usize = count as usize;
    let mut sexes = Vec::with_capacity(count_usize);

    if count >= 2 {
        // Guarantee at least 1 male and 1 female.
        let first_male: bool = rng.random_bool(0.5);
        if first_male {
            sexes.push(Sex::Male);
            sexes.push(Sex::Female);
        } else {
            sexes.push(Sex::Female);
            sexes.push(Sex::Male);
        }
        // Remaining agents: random 50/50.
        for _ in 2..count_usize {
            if rng.random_bool(0.5) {
                sexes.push(Sex::Male);
            } else {
                sexes.push(Sex::Female);
            }
        }
    } else {
        for _ in 0..count_usize {
            if rng.random_bool(0.5) {
                sexes.push(Sex::Male);
            } else {
                sexes.push(Sex::Female);
            }
        }
    }

    sexes
}

/// Generate a random [`Personality`] with trait values between 0.0 and 1.0.
fn random_personality(rng: &mut impl Rng) -> Personality {
    Personality {
        curiosity: rand_decimal(rng),
        cooperation: rand_decimal(rng),
        aggression: rand_decimal(rng),
        risk_tolerance: rand_decimal(rng),
        industriousness: rand_decimal(rng),
        sociability: rand_decimal(rng),
        honesty: rand_decimal(rng),
        loyalty: rand_decimal(rng),
    }
}

// -----------------------------------------------------------------------
// SpawnHandler implementation
// -----------------------------------------------------------------------

/// Concrete [`SpawnHandler`] implementation for the World Engine.
///
/// Uses [`spawn_single_agent`] to create new agents and integrate them
/// into the simulation state. Carries the seed knowledge configuration
/// so new agents receive the same starting concepts as seed agents.
pub struct EngineSpawnHandler {
    /// Knowledge concepts every new agent starts with.
    seed_knowledge: Vec<String>,
}

impl EngineSpawnHandler {
    /// Create a new spawn handler with the given seed knowledge set.
    pub const fn new(seed_knowledge: Vec<String>) -> Self {
        Self { seed_knowledge }
    }
}

impl emergence_core::runner::SpawnHandler for EngineSpawnHandler {
    fn handle_spawn(
        &mut self,
        request: &emergence_core::operator::SpawnRequest,
        state: &mut emergence_core::tick::SimulationState,
    ) -> bool {
        // Collect existing names to avoid duplicates.
        let existing_names: std::collections::BTreeSet<String> =
            state.agent_names.values().cloned().collect();
        let current_tick = state.clock.tick();

        // Determine preferred sex for gender balance.
        // Count living males and females; spawn the underrepresented sex.
        let preferred_sex = determine_balanced_sex(state);

        match spawn_single_agent(
            request,
            &mut state.world_map,
            current_tick,
            &existing_names,
            &self.seed_knowledge,
            preferred_sex,
        ) {
            Ok(result) => {
                let agent_id = result.agent.id;
                let name = result.agent.name.clone();
                state.agents.insert(agent_id, result.agent);
                state.agent_names.insert(agent_id, name);
                state.agent_states.insert(agent_id, result.agent_state);
                state.alive_agents.push(agent_id);
                true
            }
            Err(err) => {
                tracing::warn!(error = %err, "Failed to spawn agent");
                false
            }
        }
    }
}

/// Determine the preferred sex for a new agent to maintain gender balance.
///
/// Counts living males and females in the simulation. If the ratio is
/// significantly imbalanced (more than 60/40), returns the underrepresented
/// sex. Otherwise returns `None` for random assignment.
fn determine_balanced_sex(
    state: &emergence_core::tick::SimulationState,
) -> Option<Sex> {
    let mut males: u32 = 0;
    let mut females: u32 = 0;

    for agent_id in &state.alive_agents {
        if let Some(agent) = state.agents.get(agent_id) {
            match agent.sex {
                Sex::Male => males = males.saturating_add(1),
                Sex::Female => females = females.saturating_add(1),
            }
        }
    }

    let total = males.saturating_add(females);
    if total < 2 {
        // Too few agents to determine balance; random is fine.
        return None;
    }

    // If one sex is less than 40% of the population, spawn that sex.
    // Using integer arithmetic: male_pct = males * 100 / total
    let male_pct = males.saturating_mul(100).checked_div(total).unwrap_or(50);

    if male_pct < 40 {
        Some(Sex::Male)
    } else if male_pct > 60 {
        Some(Sex::Female)
    } else {
        None // Balance is acceptable, use random.
    }
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn make_test_world() -> WorldMap {
        let (map, _ids) = emergence_world::create_starting_world().unwrap();
        map
    }

    #[test]
    fn spawns_correct_count() {
        let config = SpawnerConfig {
            seed_count: 5,
            ..SpawnerConfig::default()
        };
        let mut world_map = make_test_world();
        let result = spawn_seed_agents(&config, &mut world_map).unwrap();

        assert_eq!(result.agent_names.len(), 5);
        assert_eq!(result.agent_states.len(), 5);
        assert_eq!(result.alive_agents.len(), 5);
    }

    #[test]
    fn all_unique_names() {
        let config = SpawnerConfig {
            seed_count: 20,
            ..SpawnerConfig::default()
        };
        let mut world_map = make_test_world();
        let result = spawn_seed_agents(&config, &mut world_map).unwrap();

        let name_set: BTreeSet<&String> = result.agent_names.values().collect();
        assert_eq!(name_set.len(), 20, "all names must be unique");
    }

    #[test]
    fn all_unique_ids() {
        let config = SpawnerConfig {
            seed_count: 10,
            ..SpawnerConfig::default()
        };
        let mut world_map = make_test_world();
        let result = spawn_seed_agents(&config, &mut world_map).unwrap();

        let id_set: BTreeSet<&AgentId> = result.agent_states.keys().collect();
        assert_eq!(id_set.len(), 10, "all agent IDs must be unique");
    }

    #[test]
    fn distributed_across_locations() {
        let config = SpawnerConfig {
            seed_count: 12,
            ..SpawnerConfig::default()
        };
        let mut world_map = make_test_world();
        let result = spawn_seed_agents(&config, &mut world_map).unwrap();

        // With 12 agents and 12 locations, each location should have exactly 1.
        let mut location_counts: BTreeMap<LocationId, u32> = BTreeMap::new();
        for state in result.agent_states.values() {
            *location_counts.entry(state.location_id).or_default() = location_counts
                .get(&state.location_id)
                .copied()
                .unwrap_or(0)
                .saturating_add(1);
        }

        // Every agent should be at some location.
        let total: u32 = location_counts.values().sum();
        assert_eq!(total, 12);

        // With 12 agents and 12 locations, distribution should be even (1 each).
        for &count in location_counts.values() {
            assert_eq!(count, 1, "each location should have exactly 1 agent");
        }
    }

    #[test]
    fn agents_have_seed_knowledge() {
        let config = SpawnerConfig {
            seed_count: 3,
            seed_knowledge: vec![
                String::from("fire"),
                String::from("basic_tools"),
            ],
            ..SpawnerConfig::default()
        };
        let mut world_map = make_test_world();
        let result = spawn_seed_agents(&config, &mut world_map).unwrap();

        for state in result.agent_states.values() {
            assert!(state.knowledge.contains("fire"));
            assert!(state.knowledge.contains("basic_tools"));
        }
    }

    #[test]
    fn agents_have_correct_starting_vitals() {
        let config = SpawnerConfig {
            seed_count: 3,
            ..SpawnerConfig::default()
        };
        let mut world_map = make_test_world();
        let result = spawn_seed_agents(&config, &mut world_map).unwrap();

        for state in result.agent_states.values() {
            assert_eq!(state.energy, 80);
            assert_eq!(state.health, 100);
            assert_eq!(state.hunger, 0);
            assert_eq!(state.age, 0);
            assert_eq!(state.born_at_tick, 0);
            assert_eq!(state.carry_capacity, 50);
        }
    }

    #[test]
    fn agents_registered_as_occupants() {
        let config = SpawnerConfig {
            seed_count: 5,
            ..SpawnerConfig::default()
        };
        let mut world_map = make_test_world();
        let result = spawn_seed_agents(&config, &mut world_map).unwrap();

        for (&agent_id, state) in &result.agent_states {
            let loc = world_map.get_location(state.location_id);
            assert!(loc.is_some(), "location must exist");
            if let Some(loc_state) = loc {
                assert!(
                    loc_state.contains_agent(agent_id),
                    "agent must be registered as occupant at their location"
                );
            }
        }
    }

    #[test]
    fn too_many_agents_returns_error() {
        let config = SpawnerConfig {
            seed_count: 100, // More than NAME_POOL size (50)
            ..SpawnerConfig::default()
        };
        let mut world_map = make_test_world();
        let result = spawn_seed_agents(&config, &mut world_map);
        assert!(result.is_err());
    }

    #[test]
    fn zero_agents_returns_empty() {
        let config = SpawnerConfig {
            seed_count: 0,
            ..SpawnerConfig::default()
        };
        let mut world_map = make_test_world();
        let result = spawn_seed_agents(&config, &mut world_map).unwrap();

        assert!(result.agent_names.is_empty());
        assert!(result.agent_states.is_empty());
        assert!(result.alive_agents.is_empty());
    }
}
