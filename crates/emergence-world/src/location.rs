//! Location node implementation with occupant tracking and resource access.
//!
//! A [`LocationState`] wraps the canonical [`Location`] type from
//! `emergence-types` and adds mutable runtime state: the set of agents
//! currently present and the set of structures built here.
//!
//! The separation exists because [`Location`] is the persistent identity
//! (stored in `PostgreSQL`) while [`LocationState`] is the hot, per-tick
//! state (stored in Dragonfly).

use std::collections::{BTreeMap, BTreeSet};

use emergence_types::{AgentId, Location, Resource, ResourceNode, Season, StructureId};

use crate::error::WorldError;
use crate::resource;

/// Mutable runtime state for a location in the world graph.
///
/// Holds the canonical [`Location`] definition alongside the volatile
/// per-tick data: current occupants and structures. Resource nodes live
/// inside `Location.base_resources` and are mutated in place during
/// regeneration and gathering.
#[derive(Debug, Clone)]
pub struct LocationState {
    /// The canonical location data (identity + resource definitions).
    pub location: Location,
    /// Agents currently present at this location.
    pub occupants: BTreeSet<AgentId>,
    /// Structures built at this location.
    pub structures: BTreeSet<StructureId>,
}

impl LocationState {
    /// Create a new [`LocationState`] from a [`Location`] definition.
    ///
    /// Starts with no occupants and no structures.
    pub const fn new(location: Location) -> Self {
        Self {
            location,
            occupants: BTreeSet::new(),
            structures: BTreeSet::new(),
        }
    }

    /// Return the number of agents currently at this location.
    ///
    /// Returns `u32::MAX` in the (practically impossible) case where
    /// the occupant set exceeds `u32::MAX` entries.
    pub fn occupant_count(&self) -> u32 {
        // BTreeSet::len returns usize. We use try_from with a saturating
        // fallback to satisfy the cast_possible_truncation lint.
        u32::try_from(self.occupants.len()).unwrap_or(u32::MAX)
    }

    /// Return the remaining occupant capacity.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::ArithmeticOverflow`] if checked subtraction fails.
    pub fn remaining_capacity(&self) -> Result<u32, WorldError> {
        self.location
            .capacity
            .checked_sub(self.occupant_count())
            .ok_or(WorldError::ArithmeticOverflow)
    }

    /// Check whether the location can accept another occupant.
    pub fn has_capacity(&self) -> bool {
        self.occupant_count() < self.location.capacity
    }

    /// Add an agent to this location's occupant set.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::LocationAtCapacity`] if the location is full.
    pub fn add_occupant(&mut self, agent: AgentId) -> Result<(), WorldError> {
        if !self.has_capacity() {
            return Err(WorldError::LocationAtCapacity {
                location: self.location.id,
                capacity: self.location.capacity,
            });
        }
        self.occupants.insert(agent);
        Ok(())
    }

    /// Remove an agent from this location's occupant set.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::AgentNotAtLocation`] if the agent is not here.
    pub fn remove_occupant(&mut self, agent: AgentId) -> Result<(), WorldError> {
        if !self.occupants.remove(&agent) {
            return Err(WorldError::AgentNotAtLocation {
                agent,
                location: self.location.id,
            });
        }
        Ok(())
    }

    /// Check whether a specific agent is at this location.
    pub fn contains_agent(&self, agent: AgentId) -> bool {
        self.occupants.contains(&agent)
    }

    /// Add a structure to this location.
    pub fn add_structure(&mut self, structure: StructureId) {
        self.structures.insert(structure);
    }

    /// Remove a structure from this location.
    pub fn remove_structure(&mut self, structure: &StructureId) -> bool {
        self.structures.remove(structure)
    }

    /// Get an immutable reference to a resource node at this location.
    pub fn get_resource(&self, resource: &Resource) -> Option<&ResourceNode> {
        self.location.base_resources.get(resource)
    }

    /// Get a mutable reference to a resource node at this location.
    pub fn get_resource_mut(&mut self, resource: &Resource) -> Option<&mut ResourceNode> {
        self.location.base_resources.get_mut(resource)
    }

    /// Return an immutable view of all resource nodes at this location.
    pub const fn resources(&self) -> &BTreeMap<Resource, ResourceNode> {
        &self.location.base_resources
    }

    /// Return available quantities for all resources at this location.
    pub fn available_resources(&self) -> BTreeMap<Resource, u32> {
        self.location
            .base_resources
            .iter()
            .map(|(r, node)| (*r, node.available))
            .collect()
    }

    /// Regenerate all resource nodes at this location for one tick.
    ///
    /// Returns a map of resource to the number of units regenerated.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::ArithmeticOverflow`] if checked arithmetic fails.
    pub fn regenerate_all(
        &mut self,
        season: Season,
    ) -> Result<BTreeMap<Resource, u32>, WorldError> {
        let mut results = BTreeMap::new();
        // Collect keys first to avoid borrowing conflicts.
        let keys: Vec<Resource> = self.location.base_resources.keys().copied().collect();
        for key in keys {
            if let Some(node) = self.location.base_resources.get_mut(&key) {
                let added = resource::regenerate(node, season)?;
                if added > 0 {
                    results.insert(key, added);
                }
            }
        }
        Ok(results)
    }

    /// Harvest a quantity of a specific resource from this location.
    ///
    /// Returns the actual amount taken (may be less than requested if
    /// the node has insufficient supply).
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::ResourceNotAvailable`] if the resource does not
    /// exist at this location, or [`WorldError::ArithmeticOverflow`] on math
    /// failure.
    pub fn harvest_resource(
        &mut self,
        res: Resource,
        requested: u32,
    ) -> Result<u32, WorldError> {
        let node = self
            .location
            .base_resources
            .get_mut(&res)
            .ok_or(WorldError::ResourceNotAvailable {
                resource: res,
                location: self.location.id,
            })?;
        resource::harvest(node, requested)
    }

    /// Mark an agent as having discovered this location.
    pub fn mark_discovered_by(&mut self, agent: AgentId) {
        self.location.discovered_by.insert(agent);
    }

    /// Check whether an agent has discovered this location.
    pub fn is_discovered_by(&self, agent: AgentId) -> bool {
        self.location.discovered_by.contains(&agent)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use chrono::Utc;
    use emergence_types::{LocationId, Resource, ResourceNode};

    use super::*;

    fn make_location(capacity: u32) -> Location {
        let mut resources = BTreeMap::new();
        resources.insert(
            Resource::Wood,
            ResourceNode {
                resource: Resource::Wood,
                available: 50,
                regen_per_tick: 5,
                max_capacity: 100,
            },
        );
        resources.insert(
            Resource::Stone,
            ResourceNode {
                resource: Resource::Stone,
                available: 8,
                regen_per_tick: 0,
                max_capacity: 8,
            },
        );
        Location {
            id: LocationId::new(),
            name: "Test Location".to_string(),
            region: "Test Region".to_string(),
            location_type: "natural".to_string(),
            description: "A test location.".to_string(),
            capacity,
            base_resources: resources,
            discovered_by: BTreeSet::new(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn new_location_state_empty() {
        let loc = make_location(10);
        let state = LocationState::new(loc);
        assert_eq!(state.occupant_count(), 0);
        assert!(state.has_capacity());
        assert!(state.structures.is_empty());
    }

    #[test]
    fn add_and_remove_occupant() {
        let loc = make_location(2);
        let mut state = LocationState::new(loc);
        let a1 = AgentId::new();
        let a2 = AgentId::new();

        assert!(state.add_occupant(a1).is_ok());
        assert!(state.contains_agent(a1));
        assert_eq!(state.occupant_count(), 1);

        assert!(state.add_occupant(a2).is_ok());
        assert_eq!(state.occupant_count(), 2);

        assert!(state.remove_occupant(a1).is_ok());
        assert!(!state.contains_agent(a1));
        assert_eq!(state.occupant_count(), 1);
    }

    #[test]
    fn capacity_enforcement() {
        let loc = make_location(1);
        let mut state = LocationState::new(loc);
        let a1 = AgentId::new();
        let a2 = AgentId::new();

        assert!(state.add_occupant(a1).is_ok());
        let err = state.add_occupant(a2);
        assert!(err.is_err());
    }

    #[test]
    fn remove_absent_agent_errors() {
        let loc = make_location(5);
        let mut state = LocationState::new(loc);
        let agent = AgentId::new();
        let err = state.remove_occupant(agent);
        assert!(err.is_err());
    }

    #[test]
    fn remaining_capacity_calculation() {
        let loc = make_location(5);
        let mut state = LocationState::new(loc);
        assert_eq!(state.remaining_capacity().ok(), Some(5));

        let _ = state.add_occupant(AgentId::new());
        let _ = state.add_occupant(AgentId::new());
        assert_eq!(state.remaining_capacity().ok(), Some(3));
    }

    #[test]
    fn harvest_resource_success() {
        let loc = make_location(5);
        let mut state = LocationState::new(loc);
        let taken = state.harvest_resource(Resource::Wood, 10);
        assert!(taken.is_ok());
        assert_eq!(taken.ok(), Some(10));
        // Wood was 50, took 10, now 40.
        let node = state.get_resource(&Resource::Wood);
        assert!(node.is_some());
        if let Some(n) = node {
            assert_eq!(n.available, 40);
        }
    }

    #[test]
    fn harvest_unavailable_resource_errors() {
        let loc = make_location(5);
        let mut state = LocationState::new(loc);
        let result = state.harvest_resource(Resource::FoodFish, 5);
        assert!(result.is_err());
    }

    #[test]
    fn regenerate_all_resources() {
        let loc = make_location(5);
        let mut state = LocationState::new(loc);
        // Wood: 50, regen 5, max 100 -> should add 5 in summer
        // Stone: 8, regen 0, max 8 -> should add 0
        let results = state.regenerate_all(Season::Summer);
        assert!(results.is_ok());
        let map = results.ok().unwrap_or_default();
        assert_eq!(map.get(&Resource::Wood).copied(), Some(5));
        // Stone should not appear (0 regen)
        assert!(map.get(&Resource::Stone).is_none());
        assert_eq!(
            state
                .get_resource(&Resource::Wood)
                .map(|n| n.available),
            Some(55)
        );
    }

    #[test]
    fn discover_location() {
        let loc = make_location(5);
        let mut state = LocationState::new(loc);
        let agent = AgentId::new();
        assert!(!state.is_discovered_by(agent));
        state.mark_discovered_by(agent);
        assert!(state.is_discovered_by(agent));
    }

    #[test]
    fn available_resources_snapshot() {
        let loc = make_location(5);
        let state = LocationState::new(loc);
        let avail = state.available_resources();
        assert_eq!(avail.get(&Resource::Wood).copied(), Some(50));
        assert_eq!(avail.get(&Resource::Stone).copied(), Some(8));
    }
}
