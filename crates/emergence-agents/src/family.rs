//! Family and relationship type tracking for the Emergence simulation.
//!
//! Implements task 6.4.4 from the build plan:
//! - `FamilyBond` types (marriage, partnership, parent-child, sibling)
//! - `FamilyUnit` tracking with members, roles, and lineage depth
//! - `FamilyTracker` for recording marriages, divorces, births
//! - Lineage tracing (ancestors, descendants, siblings)
//! - Population-level family statistics (size distribution, orphan count,
//!   longest lineage)
//!
//! # Architecture
//!
//! The family tracker is a **passive observation layer** that records
//! family structure events as they occur during the simulation. It does
//! not make decisions -- it tracks the consequences of agent actions
//! (Marry, Divorce, Reproduce) and exposes queries for analytics and
//! the observer dashboard.
//!
//! Family units are identified by UUID and contain a set of members with
//! assigned roles. A single agent can belong to multiple family units
//! (e.g., a child in their parents' family and a partner in their own).

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use uuid::Uuid;

use emergence_types::AgentId;

use crate::error::AgentError;

// ---------------------------------------------------------------------------
// FamilyBond
// ---------------------------------------------------------------------------

/// The type of bond connecting two agents in a family relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FamilyBond {
    /// A formal partnership created via the Marry action.
    Marriage,
    /// An informal partnership (e.g., co-parents who are not married).
    Partnership,
    /// A parent-to-child biological or adoptive bond.
    ParentChild,
    /// A sibling bond (shared at least one parent).
    Sibling,
}

// ---------------------------------------------------------------------------
// FamilyRole
// ---------------------------------------------------------------------------

/// The role an agent plays within a family unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FamilyRole {
    /// A parent (biological or adoptive) in the family unit.
    Parent,
    /// A child in the family unit.
    Child,
    /// A partner (married or informal) in the family unit.
    Partner,
    /// An elder (grandparent or great-grandparent) in the family unit.
    Elder,
}

// ---------------------------------------------------------------------------
// FamilyUnit
// ---------------------------------------------------------------------------

/// A family unit grouping agents with assigned roles.
///
/// Family units are created when agents marry or when children are born.
/// A unit can be dissolved (e.g., via divorce) but the record persists
/// for historical analysis.
#[derive(Debug, Clone)]
pub struct FamilyUnit {
    /// Unique identifier for this family unit.
    pub id: Uuid,
    /// Members of this family unit and their roles.
    pub members: HashMap<AgentId, FamilyRole>,
    /// The tick when this family unit was formed.
    pub formed_at_tick: u64,
    /// The tick when this family unit was dissolved, if applicable.
    pub dissolved_at_tick: Option<u64>,
    /// Generation depth of the deepest lineage in this family.
    pub lineage_depth: u32,
}

// ---------------------------------------------------------------------------
// FamilyTracker
// ---------------------------------------------------------------------------

/// Tracks all family relationships in the simulation.
///
/// Provides methods to record family events (marriage, divorce, birth)
/// and query family structure (lineage, descendants, siblings, statistics).
#[derive(Debug, Clone)]
pub struct FamilyTracker {
    /// All family units, keyed by their unique ID.
    units: BTreeMap<Uuid, FamilyUnit>,
    /// Index from agent ID to the set of family unit IDs they belong to.
    agent_to_units: BTreeMap<AgentId, BTreeSet<Uuid>>,
    /// Parent-to-children mapping for lineage tracing.
    parent_to_children: BTreeMap<AgentId, BTreeSet<AgentId>>,
    /// Child-to-parents mapping for ancestry tracing.
    child_to_parents: BTreeMap<AgentId, BTreeSet<AgentId>>,
    /// Set of agents known to be alive (for orphan counting).
    alive_agents: BTreeSet<AgentId>,
}

impl FamilyTracker {
    /// Create a new empty family tracker.
    pub const fn new() -> Self {
        Self {
            units: BTreeMap::new(),
            agent_to_units: BTreeMap::new(),
            parent_to_children: BTreeMap::new(),
            child_to_parents: BTreeMap::new(),
            alive_agents: BTreeSet::new(),
        }
    }

    /// Register an agent as alive (for orphan detection).
    pub fn register_alive(&mut self, agent_id: AgentId) {
        self.alive_agents.insert(agent_id);
    }

    /// Mark an agent as dead (for orphan detection).
    pub fn mark_dead(&mut self, agent_id: AgentId) {
        self.alive_agents.remove(&agent_id);
    }

    /// Record a marriage between two agents, creating a new family unit.
    ///
    /// Both agents are assigned the `Partner` role in the new unit.
    /// Returns the ID of the newly created family unit.
    pub fn record_marriage(
        &mut self,
        agent_a: AgentId,
        agent_b: AgentId,
        tick: u64,
    ) -> Uuid {
        let unit_id = Uuid::now_v7();
        let mut members = HashMap::new();
        members.insert(agent_a, FamilyRole::Partner);
        members.insert(agent_b, FamilyRole::Partner);

        let unit = FamilyUnit {
            id: unit_id,
            members,
            formed_at_tick: tick,
            dissolved_at_tick: None,
            lineage_depth: 0,
        };

        self.units.insert(unit_id, unit);
        self.agent_to_units
            .entry(agent_a)
            .or_default()
            .insert(unit_id);
        self.agent_to_units
            .entry(agent_b)
            .or_default()
            .insert(unit_id);

        unit_id
    }

    /// Record a divorce, dissolving the partnership in a family unit.
    ///
    /// The family unit is marked as dissolved at the given tick. Children
    /// remain linked to both parents through the parent-child mappings.
    ///
    /// Returns an error if the family unit is not found.
    pub fn record_divorce(
        &mut self,
        unit_id: Uuid,
        tick: u64,
    ) -> Result<(), AgentError> {
        let unit = self.units.get_mut(&unit_id).ok_or_else(|| {
            AgentError::GovernanceFailed {
                reason: format!("family unit {unit_id} not found for divorce"),
            }
        })?;

        unit.dissolved_at_tick = Some(tick);
        Ok(())
    }

    /// Record the birth of a child, linking it to both parents.
    ///
    /// The child is added to any active (undissolved) family unit that
    /// contains both parents as partners. If no such unit exists, a new
    /// family unit is created with the parents and child.
    ///
    /// The lineage depth of the family unit is updated based on the
    /// child's generation.
    pub fn record_birth(
        &mut self,
        child_id: AgentId,
        parent_a: AgentId,
        parent_b: AgentId,
        generation: u32,
        tick: u64,
    ) {
        // Record parent-child relationships
        self.parent_to_children
            .entry(parent_a)
            .or_default()
            .insert(child_id);
        self.parent_to_children
            .entry(parent_b)
            .or_default()
            .insert(child_id);
        self.child_to_parents
            .entry(child_id)
            .or_default()
            .insert(parent_a);
        self.child_to_parents
            .entry(child_id)
            .or_default()
            .insert(parent_b);

        // Find an active family unit containing both parents as partners
        let existing_unit = self.find_active_partner_unit(parent_a, parent_b);

        if let Some(unit_id) = existing_unit {
            if let Some(unit) = self.units.get_mut(&unit_id) {
                unit.members.insert(child_id, FamilyRole::Child);
                if generation > unit.lineage_depth {
                    unit.lineage_depth = generation;
                }
            }
            self.agent_to_units
                .entry(child_id)
                .or_default()
                .insert(unit_id);
        } else {
            // Create a new family unit for this family
            let unit_id = Uuid::now_v7();
            let mut members = HashMap::new();
            members.insert(parent_a, FamilyRole::Parent);
            members.insert(parent_b, FamilyRole::Parent);
            members.insert(child_id, FamilyRole::Child);

            let unit = FamilyUnit {
                id: unit_id,
                members,
                formed_at_tick: tick,
                dissolved_at_tick: None,
                lineage_depth: generation,
            };

            self.units.insert(unit_id, unit);
            self.agent_to_units
                .entry(parent_a)
                .or_default()
                .insert(unit_id);
            self.agent_to_units
                .entry(parent_b)
                .or_default()
                .insert(unit_id);
            self.agent_to_units
                .entry(child_id)
                .or_default()
                .insert(unit_id);
        }
    }

    /// Get a family unit by its ID.
    pub fn get_family_unit(&self, unit_id: Uuid) -> Option<&FamilyUnit> {
        self.units.get(&unit_id)
    }

    /// Get all family units an agent belongs to.
    pub fn get_agent_family(&self, agent_id: AgentId) -> Vec<&FamilyUnit> {
        self.agent_to_units
            .get(&agent_id)
            .map_or_else(Vec::new, |unit_ids| {
                unit_ids
                    .iter()
                    .filter_map(|id| self.units.get(id))
                    .collect()
            })
    }

    /// Trace the ancestry of an agent, returning a list of ancestor agent IDs.
    ///
    /// Returns parents, grandparents, great-grandparents, etc., using
    /// breadth-first traversal. The result is ordered by generation
    /// (parents first, then grandparents, etc.).
    pub fn get_lineage(&self, agent_id: AgentId) -> Vec<AgentId> {
        let mut ancestors = Vec::new();
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::new();

        // Seed with direct parents
        if let Some(parents) = self.child_to_parents.get(&agent_id) {
            for parent in parents {
                if visited.insert(*parent) {
                    queue.push_back(*parent);
                }
            }
        }

        while let Some(ancestor) = queue.pop_front() {
            ancestors.push(ancestor);
            if let Some(parents) = self.child_to_parents.get(&ancestor) {
                for parent in parents {
                    if visited.insert(*parent) {
                        queue.push_back(*parent);
                    }
                }
            }
        }

        ancestors
    }

    /// Get all descendants of an agent (children, grandchildren, etc.).
    ///
    /// Uses breadth-first traversal. The result is ordered by generation
    /// (children first, then grandchildren, etc.).
    pub fn get_descendants(&self, agent_id: AgentId) -> Vec<AgentId> {
        let mut descendants = Vec::new();
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::new();

        // Seed with direct children
        if let Some(children) = self.parent_to_children.get(&agent_id) {
            for child in children {
                if visited.insert(*child) {
                    queue.push_back(*child);
                }
            }
        }

        while let Some(descendant) = queue.pop_front() {
            descendants.push(descendant);
            if let Some(children) = self.parent_to_children.get(&descendant) {
                for child in children {
                    if visited.insert(*child) {
                        queue.push_back(*child);
                    }
                }
            }
        }

        descendants
    }

    /// Get all siblings of an agent (agents sharing at least one parent).
    ///
    /// Does not include the agent themselves.
    pub fn get_siblings(&self, agent_id: AgentId) -> BTreeSet<AgentId> {
        let mut siblings = BTreeSet::new();

        if let Some(parents) = self.child_to_parents.get(&agent_id) {
            for parent in parents {
                if let Some(children) = self.parent_to_children.get(parent) {
                    for child in children {
                        if *child != agent_id {
                            siblings.insert(*child);
                        }
                    }
                }
            }
        }

        siblings
    }

    /// Compute the distribution of family sizes across all active units.
    ///
    /// Returns a map from family size (number of members) to count of
    /// family units with that size.
    pub fn family_size_distribution(&self) -> BTreeMap<usize, u32> {
        let mut distribution = BTreeMap::new();

        for unit in self.units.values() {
            if unit.dissolved_at_tick.is_none() {
                let size = unit.members.len();
                let count = distribution.entry(size).or_insert(0_u32);
                *count = count.saturating_add(1);
            }
        }

        distribution
    }

    /// Count the number of agents with no living parents.
    ///
    /// An orphan is an agent who:
    /// - Has parent records in the tracker, AND
    /// - None of those parents are in the alive set
    ///
    /// Agents with no parent records (seed agents) are not counted as orphans.
    pub fn orphan_count(&self) -> u32 {
        let mut count: u32 = 0;

        for (child, parents) in &self.child_to_parents {
            // Only count living children
            if !self.alive_agents.contains(child) {
                continue;
            }

            let has_living_parent = parents.iter().any(|p| self.alive_agents.contains(p));
            if !has_living_parent {
                count = count.saturating_add(1);
            }
        }

        count
    }

    /// Find the deepest generation count across all family units.
    ///
    /// Returns 0 if no family units exist.
    pub fn longest_lineage(&self) -> u32 {
        self.units
            .values()
            .map(|u| u.lineage_depth)
            .max()
            .unwrap_or(0)
    }

    /// Get the total number of family units (active and dissolved).
    pub fn total_units(&self) -> usize {
        self.units.len()
    }

    /// Get the number of active (undissolved) family units.
    pub fn active_units(&self) -> usize {
        self.units
            .values()
            .filter(|u| u.dissolved_at_tick.is_none())
            .count()
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Find an active (undissolved) family unit where both agents are partners.
    fn find_active_partner_unit(
        &self,
        agent_a: AgentId,
        agent_b: AgentId,
    ) -> Option<Uuid> {
        let units_a = self.agent_to_units.get(&agent_a)?;
        let units_b = self.agent_to_units.get(&agent_b)?;

        // Intersect the two sets and find an active unit with both as partners
        for unit_id in units_a.intersection(units_b) {
            if let Some(unit) = self.units.get(unit_id)
                && unit.dissolved_at_tick.is_none()
            {
                let a_is_partner = unit
                    .members
                    .get(&agent_a)
                    .is_some_and(|r| *r == FamilyRole::Partner);
                let b_is_partner = unit
                    .members
                    .get(&agent_b)
                    .is_some_and(|r| *r == FamilyRole::Partner);
                if a_is_partner && b_is_partner {
                    return Some(*unit_id);
                }
            }
        }

        None
    }
}

impl Default for FamilyTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Marriage tests
    // -----------------------------------------------------------------------

    #[test]
    fn record_marriage_creates_unit() {
        let mut tracker = FamilyTracker::new();
        let agent_a = AgentId::new();
        let agent_b = AgentId::new();

        let unit_id = tracker.record_marriage(agent_a, agent_b, 10);

        let unit = tracker.get_family_unit(unit_id);
        assert!(unit.is_some());

        let unit = unit.unwrap_or_else(|| {
            // This branch is unreachable in test but satisfies no-panic lint.
            static EMPTY: std::sync::OnceLock<FamilyUnit> = std::sync::OnceLock::new();
            EMPTY.get_or_init(|| FamilyUnit {
                id: Uuid::nil(),
                members: HashMap::new(),
                formed_at_tick: 0,
                dissolved_at_tick: None,
                lineage_depth: 0,
            })
        });

        assert_eq!(unit.members.len(), 2);
        assert_eq!(unit.members.get(&agent_a), Some(&FamilyRole::Partner));
        assert_eq!(unit.members.get(&agent_b), Some(&FamilyRole::Partner));
        assert_eq!(unit.formed_at_tick, 10);
        assert!(unit.dissolved_at_tick.is_none());
    }

    #[test]
    fn marriage_links_agents_to_unit() {
        let mut tracker = FamilyTracker::new();
        let agent_a = AgentId::new();
        let agent_b = AgentId::new();

        let _unit_id = tracker.record_marriage(agent_a, agent_b, 10);

        let families_a = tracker.get_agent_family(agent_a);
        assert_eq!(families_a.len(), 1);

        let families_b = tracker.get_agent_family(agent_b);
        assert_eq!(families_b.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Divorce tests
    // -----------------------------------------------------------------------

    #[test]
    fn record_divorce_dissolves_unit() {
        let mut tracker = FamilyTracker::new();
        let agent_a = AgentId::new();
        let agent_b = AgentId::new();

        let unit_id = tracker.record_marriage(agent_a, agent_b, 10);
        let result = tracker.record_divorce(unit_id, 50);
        assert!(result.is_ok());

        let unit = tracker.get_family_unit(unit_id);
        assert!(unit.is_some_and(|u| u.dissolved_at_tick == Some(50)));
    }

    #[test]
    fn divorce_nonexistent_unit_fails() {
        let mut tracker = FamilyTracker::new();
        let result = tracker.record_divorce(Uuid::nil(), 50);
        assert!(result.is_err());
    }

    #[test]
    fn divorce_preserves_child_links() {
        let mut tracker = FamilyTracker::new();
        let parent_a = AgentId::new();
        let parent_b = AgentId::new();
        let child = AgentId::new();

        let unit_id = tracker.record_marriage(parent_a, parent_b, 10);
        tracker.record_birth(child, parent_a, parent_b, 1, 20);
        let result = tracker.record_divorce(unit_id, 50);
        assert!(result.is_ok());

        // Child should still have lineage to both parents
        let lineage = tracker.get_lineage(child);
        assert!(lineage.contains(&parent_a));
        assert!(lineage.contains(&parent_b));
    }

    // -----------------------------------------------------------------------
    // Birth tests
    // -----------------------------------------------------------------------

    #[test]
    fn record_birth_adds_child_to_existing_unit() {
        let mut tracker = FamilyTracker::new();
        let parent_a = AgentId::new();
        let parent_b = AgentId::new();
        let child = AgentId::new();

        let unit_id = tracker.record_marriage(parent_a, parent_b, 10);
        tracker.record_birth(child, parent_a, parent_b, 1, 20);

        let unit = tracker.get_family_unit(unit_id);
        assert!(unit.is_some_and(|u| u.members.len() == 3));
        assert!(unit.is_some_and(|u| u.members.get(&child) == Some(&FamilyRole::Child)));
    }

    #[test]
    fn record_birth_creates_unit_when_no_marriage() {
        let mut tracker = FamilyTracker::new();
        let parent_a = AgentId::new();
        let parent_b = AgentId::new();
        let child = AgentId::new();

        tracker.record_birth(child, parent_a, parent_b, 1, 20);

        // A new family unit should have been created
        let families = tracker.get_agent_family(child);
        assert_eq!(families.len(), 1);

        let unit = families.first();
        assert!(unit.is_some());
        assert!(unit.is_some_and(|u| u.members.len() == 3));
    }

    #[test]
    fn record_birth_updates_lineage_depth() {
        let mut tracker = FamilyTracker::new();
        let parent_a = AgentId::new();
        let parent_b = AgentId::new();
        let child = AgentId::new();

        let unit_id = tracker.record_marriage(parent_a, parent_b, 10);
        tracker.record_birth(child, parent_a, parent_b, 3, 20);

        let unit = tracker.get_family_unit(unit_id);
        assert!(unit.is_some_and(|u| u.lineage_depth == 3));
    }

    // -----------------------------------------------------------------------
    // Lineage tests
    // -----------------------------------------------------------------------

    #[test]
    fn get_lineage_returns_parents() {
        let mut tracker = FamilyTracker::new();
        let grandparent_a = AgentId::new();
        let grandparent_b = AgentId::new();
        let parent_a = AgentId::new();
        let parent_b = AgentId::new();
        let child = AgentId::new();

        // Set up two generations
        tracker.record_birth(parent_a, grandparent_a, grandparent_b, 1, 10);
        tracker.record_birth(child, parent_a, parent_b, 2, 20);

        let lineage = tracker.get_lineage(child);

        // Should have parent_a, parent_b, grandparent_a, grandparent_b
        assert_eq!(lineage.len(), 4);
        assert!(lineage.contains(&parent_a));
        assert!(lineage.contains(&parent_b));
        assert!(lineage.contains(&grandparent_a));
        assert!(lineage.contains(&grandparent_b));
    }

    #[test]
    fn get_lineage_empty_for_seed_agent() {
        let tracker = FamilyTracker::new();
        let seed_agent = AgentId::new();

        let lineage = tracker.get_lineage(seed_agent);
        assert!(lineage.is_empty());
    }

    #[test]
    fn get_lineage_three_generations() {
        let mut tracker = FamilyTracker::new();
        let great_gp_a = AgentId::new();
        let great_gp_b = AgentId::new();
        let gp_a = AgentId::new();
        let gp_b = AgentId::new();
        let parent_a = AgentId::new();
        let parent_b = AgentId::new();
        let child = AgentId::new();

        tracker.record_birth(gp_a, great_gp_a, great_gp_b, 1, 10);
        tracker.record_birth(parent_a, gp_a, gp_b, 2, 20);
        tracker.record_birth(child, parent_a, parent_b, 3, 30);

        let lineage = tracker.get_lineage(child);

        // parent_a, parent_b, gp_a, gp_b, great_gp_a, great_gp_b
        assert_eq!(lineage.len(), 6);
        assert!(lineage.contains(&great_gp_a));
        assert!(lineage.contains(&great_gp_b));
    }

    // -----------------------------------------------------------------------
    // Descendants tests
    // -----------------------------------------------------------------------

    #[test]
    fn get_descendants_returns_children_and_grandchildren() {
        let mut tracker = FamilyTracker::new();
        let grandparent = AgentId::new();
        let partner_1 = AgentId::new();
        let partner_2 = AgentId::new();
        let child = AgentId::new();
        let grandchild = AgentId::new();

        tracker.record_birth(child, grandparent, partner_1, 1, 10);
        tracker.record_birth(grandchild, child, partner_2, 2, 20);

        let descendants = tracker.get_descendants(grandparent);
        assert_eq!(descendants.len(), 2);
        assert!(descendants.contains(&child));
        assert!(descendants.contains(&grandchild));
    }

    #[test]
    fn get_descendants_empty_for_childless_agent() {
        let tracker = FamilyTracker::new();
        let agent = AgentId::new();

        let descendants = tracker.get_descendants(agent);
        assert!(descendants.is_empty());
    }

    // -----------------------------------------------------------------------
    // Sibling tests
    // -----------------------------------------------------------------------

    #[test]
    fn get_siblings_shared_parent() {
        let mut tracker = FamilyTracker::new();
        let parent_a = AgentId::new();
        let parent_b = AgentId::new();
        let child_1 = AgentId::new();
        let child_2 = AgentId::new();
        let child_3 = AgentId::new();

        tracker.record_birth(child_1, parent_a, parent_b, 1, 10);
        tracker.record_birth(child_2, parent_a, parent_b, 1, 20);
        tracker.record_birth(child_3, parent_a, parent_b, 1, 30);

        let siblings = tracker.get_siblings(child_1);
        assert_eq!(siblings.len(), 2);
        assert!(siblings.contains(&child_2));
        assert!(siblings.contains(&child_3));
        assert!(!siblings.contains(&child_1));
    }

    #[test]
    fn get_siblings_half_siblings() {
        let mut tracker = FamilyTracker::new();
        let parent_a = AgentId::new();
        let parent_b = AgentId::new();
        let parent_c = AgentId::new();
        let child_1 = AgentId::new();
        let child_2 = AgentId::new();

        // child_1 has parents (a, b), child_2 has parents (a, c)
        // They share parent_a so they are half-siblings
        tracker.record_birth(child_1, parent_a, parent_b, 1, 10);
        tracker.record_birth(child_2, parent_a, parent_c, 1, 20);

        let siblings = tracker.get_siblings(child_1);
        assert!(siblings.contains(&child_2));

        let siblings = tracker.get_siblings(child_2);
        assert!(siblings.contains(&child_1));
    }

    #[test]
    fn get_siblings_no_siblings() {
        let mut tracker = FamilyTracker::new();
        let parent_a = AgentId::new();
        let parent_b = AgentId::new();
        let child = AgentId::new();

        tracker.record_birth(child, parent_a, parent_b, 1, 10);

        let siblings = tracker.get_siblings(child);
        assert!(siblings.is_empty());
    }

    // -----------------------------------------------------------------------
    // Statistics tests
    // -----------------------------------------------------------------------

    #[test]
    fn family_size_distribution_counts_correctly() {
        let mut tracker = FamilyTracker::new();

        // Create two families: one with 2 members, one with 3
        let a1 = AgentId::new();
        let a2 = AgentId::new();
        let _unit1 = tracker.record_marriage(a1, a2, 10);

        let b1 = AgentId::new();
        let b2 = AgentId::new();
        let b3 = AgentId::new();
        let _unit2 = tracker.record_marriage(b1, b2, 20);
        tracker.record_birth(b3, b1, b2, 1, 30);

        let dist = tracker.family_size_distribution();
        assert_eq!(dist.get(&2), Some(&1));
        assert_eq!(dist.get(&3), Some(&1));
    }

    #[test]
    fn family_size_distribution_excludes_dissolved() {
        let mut tracker = FamilyTracker::new();
        let a1 = AgentId::new();
        let a2 = AgentId::new();

        let unit_id = tracker.record_marriage(a1, a2, 10);
        let _ = tracker.record_divorce(unit_id, 20);

        let dist = tracker.family_size_distribution();
        assert!(dist.is_empty());
    }

    #[test]
    fn orphan_count_no_orphans() {
        let mut tracker = FamilyTracker::new();
        let parent_a = AgentId::new();
        let parent_b = AgentId::new();
        let child = AgentId::new();

        tracker.register_alive(parent_a);
        tracker.register_alive(parent_b);
        tracker.register_alive(child);
        tracker.record_birth(child, parent_a, parent_b, 1, 10);

        assert_eq!(tracker.orphan_count(), 0);
    }

    #[test]
    fn orphan_count_both_parents_dead() {
        let mut tracker = FamilyTracker::new();
        let parent_a = AgentId::new();
        let parent_b = AgentId::new();
        let child = AgentId::new();

        tracker.register_alive(parent_a);
        tracker.register_alive(parent_b);
        tracker.register_alive(child);
        tracker.record_birth(child, parent_a, parent_b, 1, 10);

        tracker.mark_dead(parent_a);
        tracker.mark_dead(parent_b);

        assert_eq!(tracker.orphan_count(), 1);
    }

    #[test]
    fn orphan_count_one_parent_alive() {
        let mut tracker = FamilyTracker::new();
        let parent_a = AgentId::new();
        let parent_b = AgentId::new();
        let child = AgentId::new();

        tracker.register_alive(parent_a);
        tracker.register_alive(parent_b);
        tracker.register_alive(child);
        tracker.record_birth(child, parent_a, parent_b, 1, 10);

        tracker.mark_dead(parent_a);

        // One parent still alive, so not an orphan
        assert_eq!(tracker.orphan_count(), 0);
    }

    #[test]
    fn longest_lineage_multiple_depths() {
        let mut tracker = FamilyTracker::new();
        let p1 = AgentId::new();
        let p2 = AgentId::new();
        let c1 = AgentId::new();
        let p3 = AgentId::new();
        let c2 = AgentId::new();

        // Family 1: depth 1
        tracker.record_marriage(p1, p2, 10);
        tracker.record_birth(c1, p1, p2, 1, 20);

        // Family 2: depth 3
        tracker.record_birth(c2, c1, p3, 3, 30);

        assert_eq!(tracker.longest_lineage(), 3);
    }

    #[test]
    fn longest_lineage_no_families() {
        let tracker = FamilyTracker::new();
        assert_eq!(tracker.longest_lineage(), 0);
    }

    // -----------------------------------------------------------------------
    // Agent family query tests
    // -----------------------------------------------------------------------

    #[test]
    fn get_agent_family_multiple_units() {
        let mut tracker = FamilyTracker::new();
        let agent = AgentId::new();
        let partner_1 = AgentId::new();
        let partner_2 = AgentId::new();

        // Agent marries twice (first marriage dissolved)
        let unit_1 = tracker.record_marriage(agent, partner_1, 10);
        let _ = tracker.record_divorce(unit_1, 20);
        let _unit_2 = tracker.record_marriage(agent, partner_2, 30);

        let families = tracker.get_agent_family(agent);
        assert_eq!(families.len(), 2);
    }

    #[test]
    fn get_agent_family_unknown_agent() {
        let tracker = FamilyTracker::new();
        let unknown = AgentId::new();

        let families = tracker.get_agent_family(unknown);
        assert!(families.is_empty());
    }

    // -----------------------------------------------------------------------
    // Active/total unit counts
    // -----------------------------------------------------------------------

    #[test]
    fn active_and_total_unit_counts() {
        let mut tracker = FamilyTracker::new();
        let a1 = AgentId::new();
        let a2 = AgentId::new();
        let b1 = AgentId::new();
        let b2 = AgentId::new();

        let unit_1 = tracker.record_marriage(a1, a2, 10);
        let _unit_2 = tracker.record_marriage(b1, b2, 20);
        let _ = tracker.record_divorce(unit_1, 30);

        assert_eq!(tracker.total_units(), 2);
        assert_eq!(tracker.active_units(), 1);
    }
}
