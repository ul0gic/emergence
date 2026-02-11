//! Agent creation and management.
//!
//! The [`AgentManager`] creates new agents with identity, personality,
//! and generation data. It produces both an immutable [`Agent`] record
//! (identity) and a mutable [`AgentState`] (vitals and inventory).
//!
//! This module covers tasks 2.1.1 (agent identity) and the factory
//! methods for task 2.1.2 (initial vitals from config).
//!
//! See `agent-system.md` sections 2 and 9.1--9.2.

use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use emergence_types::{Agent, AgentId, AgentState, LocationId, Personality, Resource, Sex};

use crate::config::VitalsConfig;
use crate::error::AgentError;

/// Parameters for creating a child agent via reproduction.
///
/// Bundles the parent data, location, and timing into a single struct
/// to keep the method signature manageable.
#[derive(Debug, Clone)]
pub struct ChildAgentParams {
    /// Display name for the child (must be unique).
    pub name: String,
    /// Biological sex of the child (randomly assigned by caller).
    pub sex: Sex,
    /// Blended personality (caller is responsible for blending + mutation).
    pub personality: Personality,
    /// ID of the first parent.
    pub first_parent: AgentId,
    /// ID of the second parent.
    pub second_parent: AgentId,
    /// Generation number of the first parent.
    pub first_parent_generation: u32,
    /// Generation number of the second parent.
    pub second_parent_generation: u32,
    /// Location where the child is born.
    pub location: LocationId,
    /// Tick when the child enters the simulation.
    pub born_at_tick: u64,
}

/// Creates and tracks agents for the simulation.
///
/// The manager enforces name uniqueness and provides factory methods
/// for both seed agents (generation 0) and child agents (reproduced).
#[derive(Debug)]
pub struct AgentManager {
    /// Set of all agent names currently in use (for uniqueness checks).
    names_in_use: BTreeSet<String>,
}

impl AgentManager {
    /// Create a new empty agent manager.
    pub const fn new() -> Self {
        Self {
            names_in_use: BTreeSet::new(),
        }
    }

    /// Create a seed agent (generation 0, no parents).
    ///
    /// Seed agents are placed at the given location with starting vitals
    /// from `config` and an optional starting inventory.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::DuplicateName`] if the name is already taken.
    #[allow(clippy::too_many_arguments)]
    pub fn create_seed_agent(
        &mut self,
        name: String,
        sex: Sex,
        personality: Personality,
        location: LocationId,
        born_at_tick: u64,
        config: &VitalsConfig,
        starting_inventory: BTreeMap<Resource, u32>,
    ) -> Result<(Agent, AgentState), AgentError> {
        if self.names_in_use.contains(&name) {
            return Err(AgentError::DuplicateName(name));
        }
        self.names_in_use.insert(name.clone());

        let id = AgentId::new();

        let agent = Agent {
            id,
            name,
            sex,
            born_at_tick,
            died_at_tick: None,
            cause_of_death: None,
            parent_a: None,
            parent_b: None,
            generation: 0,
            personality,
            created_at: Utc::now(),
        };

        let state = AgentState {
            agent_id: id,
            energy: config.starting_energy,
            health: config.starting_health,
            hunger: 0,
            thirst: 0,
            age: 0,
            born_at_tick,
            location_id: location,
            destination_id: None,
            travel_progress: 0,
            inventory: starting_inventory,
            carry_capacity: config.carry_capacity,
            knowledge: BTreeSet::new(),
            skills: BTreeMap::new(),
            skill_xp: BTreeMap::new(),
            goals: Vec::new(),
            relationships: BTreeMap::new(),
            memory: Vec::new(),
        };

        Ok((agent, state))
    }

    /// Create a child agent from two parents.
    ///
    /// The child inherits a blended personality from the parents
    /// (produced by the caller -- personality blending is a separate concern).
    /// The child starts at generation = max(first\_parent, second\_parent) + 1,
    /// at the same location as the parents, with zero inventory.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::DuplicateName`] if the name is already taken.
    pub fn create_child_agent(
        &mut self,
        params: &ChildAgentParams,
        config: &VitalsConfig,
    ) -> Result<(Agent, AgentState), AgentError> {
        if self.names_in_use.contains(&params.name) {
            return Err(AgentError::DuplicateName(params.name.clone()));
        }
        self.names_in_use.insert(params.name.clone());

        let id = AgentId::new();

        let max_parent_gen =
            core::cmp::max(params.first_parent_generation, params.second_parent_generation);
        let generation = max_parent_gen.checked_add(1).ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: String::from("generation increment overflow"),
            }
        })?;

        let agent = Agent {
            id,
            name: params.name.clone(),
            sex: params.sex,
            born_at_tick: params.born_at_tick,
            died_at_tick: None,
            cause_of_death: None,
            parent_a: Some(params.first_parent),
            parent_b: Some(params.second_parent),
            generation,
            personality: params.personality.clone(),
            created_at: Utc::now(),
        };

        let state = AgentState {
            agent_id: id,
            energy: config.starting_energy,
            health: config.starting_health,
            hunger: 0,
            thirst: 0,
            age: 0,
            born_at_tick: params.born_at_tick,
            location_id: params.location,
            destination_id: None,
            travel_progress: 0,
            inventory: BTreeMap::new(),
            carry_capacity: config.carry_capacity,
            knowledge: BTreeSet::new(),
            skills: BTreeMap::new(),
            skill_xp: BTreeMap::new(),
            goals: Vec::new(),
            relationships: BTreeMap::new(),
            memory: Vec::new(),
        };

        Ok((agent, state))
    }

    /// Release a name back to the pool (e.g. after agent removal from manager).
    ///
    /// Returns `true` if the name was in use and is now released.
    pub fn release_name(&mut self, name: &str) -> bool {
        self.names_in_use.remove(name)
    }

    /// Check whether a name is currently in use.
    pub fn is_name_taken(&self, name: &str) -> bool {
        self.names_in_use.contains(name)
    }
}

impl Default for AgentManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::*;

    fn test_personality() -> Personality {
        Personality {
            curiosity: Decimal::new(5, 1),
            cooperation: Decimal::new(5, 1),
            aggression: Decimal::new(3, 1),
            risk_tolerance: Decimal::new(4, 1),
            industriousness: Decimal::new(6, 1),
            sociability: Decimal::new(7, 1),
            honesty: Decimal::new(8, 1),
            loyalty: Decimal::new(5, 1),
        }
    }

    #[test]
    fn create_seed_agent_success() {
        let mut manager = AgentManager::new();
        let config = VitalsConfig::default();
        let location = LocationId::new();

        let result = manager.create_seed_agent(
            String::from("Kora"),
            Sex::Female,
            test_personality(),
            location,
            0,
            &config,
            BTreeMap::new(),
        );
        assert!(result.is_ok());

        if let Ok((agent, state)) = result {
            assert_eq!(agent.name, "Kora");
            assert_eq!(agent.generation, 0);
            assert!(agent.parent_a.is_none());
            assert!(agent.parent_b.is_none());
            assert_eq!(agent.born_at_tick, 0);

            assert_eq!(state.energy, config.starting_energy);
            assert_eq!(state.health, config.starting_health);
            assert_eq!(state.hunger, 0);
            assert_eq!(state.age, 0);
            assert_eq!(state.location_id, location);
            assert_eq!(state.carry_capacity, config.carry_capacity);
        }
    }

    #[test]
    fn create_seed_agent_with_starting_inventory() {
        let mut manager = AgentManager::new();
        let config = VitalsConfig::default();
        let mut inv = BTreeMap::new();
        inv.insert(Resource::FoodBerry, 10);
        inv.insert(Resource::Water, 5);

        let result = manager.create_seed_agent(
            String::from("Dax"),
            Sex::Male,
            test_personality(),
            LocationId::new(),
            0,
            &config,
            inv,
        );
        assert!(result.is_ok());

        if let Ok((_, state)) = result {
            assert_eq!(state.inventory.get(&Resource::FoodBerry).copied(), Some(10));
            assert_eq!(state.inventory.get(&Resource::Water).copied(), Some(5));
        }
    }

    #[test]
    fn duplicate_name_rejected() {
        let mut manager = AgentManager::new();
        let config = VitalsConfig::default();

        let r1 = manager.create_seed_agent(
            String::from("Kora"),
            Sex::Female,
            test_personality(),
            LocationId::new(),
            0,
            &config,
            BTreeMap::new(),
        );
        assert!(r1.is_ok());

        let r2 = manager.create_seed_agent(
            String::from("Kora"),
            Sex::Male,
            test_personality(),
            LocationId::new(),
            0,
            &config,
            BTreeMap::new(),
        );
        assert!(r2.is_err());
    }

    #[test]
    fn create_child_agent_success() {
        let mut manager = AgentManager::new();
        let config = VitalsConfig::default();
        let location = LocationId::new();
        let pa = AgentId::new();
        let pb = AgentId::new();

        let params = ChildAgentParams {
            name: String::from("Junior"),
            sex: Sex::Male,
            personality: test_personality(),
            first_parent: pa,
            second_parent: pb,
            first_parent_generation: 0,
            second_parent_generation: 1,
            location,
            born_at_tick: 100,
        };

        let result = manager.create_child_agent(&params, &config);
        assert!(result.is_ok());

        if let Ok((agent, state)) = result {
            assert_eq!(agent.name, "Junior");
            assert_eq!(agent.generation, 2); // max(0, 1) + 1
            assert_eq!(agent.parent_a, Some(pa));
            assert_eq!(agent.parent_b, Some(pb));
            assert_eq!(agent.born_at_tick, 100);
            assert_eq!(state.location_id, location);
            assert!(state.inventory.is_empty());
        }
    }

    #[test]
    fn child_generation_increments_correctly() {
        let mut manager = AgentManager::new();
        let config = VitalsConfig::default();

        let params = ChildAgentParams {
            name: String::from("Gen3"),
            sex: Sex::Female,
            personality: test_personality(),
            first_parent: AgentId::new(),
            second_parent: AgentId::new(),
            first_parent_generation: 2,
            second_parent_generation: 2,
            location: LocationId::new(),
            born_at_tick: 200,
        };

        let result = manager.create_child_agent(&params, &config);
        assert!(result.is_ok());

        if let Ok((agent, _)) = result {
            assert_eq!(agent.generation, 3);
        }
    }

    #[test]
    fn release_name_allows_reuse() {
        let mut manager = AgentManager::new();
        let config = VitalsConfig::default();

        let _ = manager.create_seed_agent(
            String::from("Kora"),
            Sex::Female,
            test_personality(),
            LocationId::new(),
            0,
            &config,
            BTreeMap::new(),
        );
        assert!(manager.is_name_taken("Kora"));

        assert!(manager.release_name("Kora"));
        assert!(!manager.is_name_taken("Kora"));

        // Can now create another agent with the same name
        let r = manager.create_seed_agent(
            String::from("Kora"),
            Sex::Female,
            test_personality(),
            LocationId::new(),
            100,
            &config,
            BTreeMap::new(),
        );
        assert!(r.is_ok());
    }

    #[test]
    fn is_name_taken_false_for_unknown() {
        let manager = AgentManager::new();
        assert!(!manager.is_name_taken("Unknown"));
    }

    #[test]
    fn agent_ids_are_unique() {
        let mut manager = AgentManager::new();
        let config = VitalsConfig::default();

        let r1 = manager.create_seed_agent(
            String::from("A"),
            Sex::Female,
            test_personality(),
            LocationId::new(),
            0,
            &config,
            BTreeMap::new(),
        );
        let r2 = manager.create_seed_agent(
            String::from("B"),
            Sex::Male,
            test_personality(),
            LocationId::new(),
            0,
            &config,
            BTreeMap::new(),
        );

        if let (Ok((a1, _)), Ok((a2, _))) = (r1, r2) {
            assert_ne!(a1.id, a2.id);
        }
    }
}
