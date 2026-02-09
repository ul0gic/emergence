//! Death conditions and consequences for agents.
//!
//! Per `agent-system.md` section 9.4, agents die when:
//! - Health reaches 0 (starvation, injury, illness)
//! - Age exceeds lifespan
//!
//! On death, inventory drops at the agent's current location, structures
//! become orphaned (owner set to `None`), and a social notification is
//! emitted for related agents.

use std::collections::BTreeMap;

use emergence_types::{AgentId, AgentState, LocationId, Resource, StructureId};

use crate::config::VitalsConfig;
use crate::inventory;

/// The cause of an agent's death.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeathCause {
    /// Agent's health reached 0 due to starvation (hunger-induced health loss).
    Starvation,
    /// Agent's age exceeded the configured lifespan.
    OldAge,
    /// Agent's health reached 0 due to an external injury or illness.
    Injury,
}

impl core::fmt::Display for DeathCause {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Starvation => write!(f, "starvation"),
            Self::OldAge => write!(f, "old_age"),
            Self::Injury => write!(f, "injury"),
        }
    }
}

/// Check whether an agent meets any death condition.
///
/// Returns `Some(cause)` if the agent is dead, `None` if alive.
/// This only inspects the current state -- it does not mutate anything.
pub const fn check_death(state: &AgentState, config: &VitalsConfig) -> Option<DeathCause> {
    // Age exceeds lifespan
    if state.age > config.lifespan {
        return Some(DeathCause::OldAge);
    }

    // Health at zero
    if state.health == 0 {
        // Determine the proximate cause: if hunger is at max, it was starvation.
        // Otherwise, it was injury/other.
        if state.hunger >= config.starvation_threshold {
            return Some(DeathCause::Starvation);
        }
        return Some(DeathCause::Injury);
    }

    None
}

/// Data emitted when an agent dies, used to create the death event
/// and update world state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeathConsequences {
    /// The agent who died.
    pub agent_id: AgentId,

    /// The cause of death.
    pub cause: DeathCause,

    /// The agent's age at death (in ticks).
    pub final_age: u32,

    /// The location where the agent died (inventory drops here).
    pub death_location: LocationId,

    /// Resources dropped from the agent's inventory at the death location.
    /// Empty if the agent had no inventory.
    pub dropped_inventory: BTreeMap<Resource, u32>,

    /// Structure IDs that were owned by the deceased agent and are now
    /// orphaned (owner set to `None`).
    pub orphaned_structures: Vec<StructureId>,

    /// Agent IDs that had a relationship with the deceased and should
    /// be notified in the next tick's perception.
    pub agents_to_notify: Vec<AgentId>,
}

/// Process the consequences of an agent's death.
///
/// This function:
/// 1. Drains the agent's inventory (dropped at death location)
/// 2. Identifies structures to orphan (passed in by caller)
/// 3. Identifies agents to notify (from the agent's relationship graph)
///
/// The caller is responsible for actually updating the world state
/// (adding resources to the location, setting structure owners to `None`,
/// emitting the death event, etc.).
pub fn process_death(
    state: &mut AgentState,
    cause: DeathCause,
    owned_structures: Vec<StructureId>,
) -> DeathConsequences {
    // Drain the agent's inventory
    let dropped_inventory = inventory::drain_all(&mut state.inventory);

    // Collect agent IDs to notify from the social graph
    let agents_to_notify: Vec<AgentId> = state.relationships.keys().copied().collect();

    DeathConsequences {
        agent_id: state.agent_id,
        cause,
        final_age: state.age,
        death_location: state.location_id,
        dropped_inventory,
        orphaned_structures: owned_structures,
        agents_to_notify,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rust_decimal::Decimal;

    use super::*;

    fn test_state() -> AgentState {
        AgentState {
            agent_id: AgentId::new(),
            energy: 80,
            health: 100,
            hunger: 0,
            age: 0,
            born_at_tick: 0,
            location_id: LocationId::new(),
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

    #[test]
    fn alive_agent_returns_none() {
        let state = test_state();
        let config = VitalsConfig::default();
        assert_eq!(check_death(&state, &config), None);
    }

    #[test]
    fn death_by_old_age() {
        let mut state = test_state();
        let config = VitalsConfig::default();
        state.age = 2501; // > 2500
        assert_eq!(check_death(&state, &config), Some(DeathCause::OldAge));
    }

    #[test]
    fn death_at_exact_lifespan_is_alive() {
        let mut state = test_state();
        let config = VitalsConfig::default();
        state.age = 2500; // == lifespan, not > lifespan
        assert_eq!(check_death(&state, &config), None);
    }

    #[test]
    fn death_by_starvation() {
        let mut state = test_state();
        let config = VitalsConfig::default();
        state.health = 0;
        state.hunger = 100;
        assert_eq!(check_death(&state, &config), Some(DeathCause::Starvation));
    }

    #[test]
    fn death_by_injury() {
        let mut state = test_state();
        let config = VitalsConfig::default();
        state.health = 0;
        state.hunger = 30; // Not starving, so it was injury
        assert_eq!(check_death(&state, &config), Some(DeathCause::Injury));
    }

    #[test]
    fn old_age_takes_priority_over_health() {
        let mut state = test_state();
        let config = VitalsConfig::default();
        state.age = 2501;
        state.health = 0;
        // Old age check comes first
        assert_eq!(check_death(&state, &config), Some(DeathCause::OldAge));
    }

    #[test]
    fn process_death_drains_inventory() {
        let mut state = test_state();
        state.inventory.insert(Resource::Wood, 10);
        state.inventory.insert(Resource::FoodBerry, 5);

        let result = process_death(&mut state, DeathCause::Starvation, vec![]);
        assert_eq!(result.dropped_inventory.get(&Resource::Wood).copied(), Some(10));
        assert_eq!(
            result.dropped_inventory.get(&Resource::FoodBerry).copied(),
            Some(5)
        );
        assert!(state.inventory.is_empty());
    }

    #[test]
    fn process_death_captures_orphaned_structures() {
        let mut state = test_state();
        let s1 = StructureId::new();
        let s2 = StructureId::new();

        let result = process_death(&mut state, DeathCause::OldAge, vec![s1, s2]);
        assert_eq!(result.orphaned_structures.len(), 2);
        assert!(result.orphaned_structures.contains(&s1));
        assert!(result.orphaned_structures.contains(&s2));
    }

    #[test]
    fn process_death_notifies_related_agents() {
        let mut state = test_state();
        let friend = AgentId::new();
        let rival = AgentId::new();
        state
            .relationships
            .insert(friend, Decimal::new(7, 1)); // 0.7
        state
            .relationships
            .insert(rival, Decimal::new(-3, 1)); // -0.3

        let result = process_death(&mut state, DeathCause::Injury, vec![]);
        assert_eq!(result.agents_to_notify.len(), 2);
        assert!(result.agents_to_notify.contains(&friend));
        assert!(result.agents_to_notify.contains(&rival));
    }

    #[test]
    fn process_death_empty_state() {
        let mut state = test_state();
        let result = process_death(&mut state, DeathCause::OldAge, vec![]);
        assert!(result.dropped_inventory.is_empty());
        assert!(result.orphaned_structures.is_empty());
        assert!(result.agents_to_notify.is_empty());
        assert_eq!(result.agent_id, state.agent_id);
        assert_eq!(result.cause, DeathCause::OldAge);
    }

    #[test]
    fn death_cause_display() {
        assert_eq!(DeathCause::Starvation.to_string(), "starvation");
        assert_eq!(DeathCause::OldAge.to_string(), "old_age");
        assert_eq!(DeathCause::Injury.to_string(), "injury");
    }
}
