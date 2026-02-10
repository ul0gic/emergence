//! Diplomacy actions for the Emergence simulation.
//!
//! Enables formal diplomatic interactions between agents and groups:
//! alliances, conflicts, treaties, and tribute. This module implements
//! task 6.3.6 from the build plan.
//!
//! # Architecture
//!
//! Diplomacy operates at the **group level** for alliances, conflicts,
//! and treaties, and at the **agent level** for tribute. All diplomatic
//! actions require the involved parties to be present (co-located) or
//! to have communicated agreement via the message system.
//!
//! # Events
//!
//! - `AllianceFormed` -- emitted when two groups form an alliance.
//! - `AllianceBroken` -- emitted when an alliance is dissolved.
//! - `WarDeclared` -- emitted when a group declares conflict.
//! - `TreatyNegotiated` -- emitted when two groups agree to a treaty.
//!
//! # Invariants
//!
//! - A group cannot be allied with itself.
//! - A group cannot declare conflict against an ally (must break alliance first).
//! - Treaties require both group leaders to be co-located.
//! - Tribute transfers go through the ledger for conservation law compliance.

use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use emergence_types::{AgentId, GroupId, LocationId, Resource};

use crate::error::AgentError;

// ---------------------------------------------------------------------------
// Alliance types
// ---------------------------------------------------------------------------

/// Terms of an alliance between two groups.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllianceTerms {
    /// Whether allied groups are obligated to defend each other.
    pub mutual_defense: bool,
    /// Whether allied groups receive preferential trade rates.
    pub trade_preference: bool,
    /// Whether allied groups share access to each other's territory.
    pub shared_territory: bool,
}


/// The current status of an alliance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllianceStatus {
    /// The alliance is active and in force.
    Active,
    /// The alliance was broken by a specific group.
    Broken {
        /// The group that broke the alliance.
        broken_by: GroupId,
        /// The tick when the alliance was broken.
        tick: u64,
    },
    /// The alliance expired (if it had a duration).
    Expired,
}

/// A formal alliance between two or more groups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alliance {
    /// Unique alliance identifier.
    pub id: Uuid,
    /// The groups participating in this alliance.
    pub groups: Vec<GroupId>,
    /// The terms of the alliance.
    pub terms: AllianceTerms,
    /// The tick when the alliance was formed.
    pub formed_at_tick: u64,
    /// Current status of the alliance.
    pub status: AllianceStatus,
}

// ---------------------------------------------------------------------------
// Treaty types
// ---------------------------------------------------------------------------

/// Terms of a treaty between two groups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreatyTerms {
    /// Whether the treaty includes a ceasefire agreement.
    pub ceasefire: bool,
    /// Optional border agreement specifying shared or divided locations.
    pub border_agreement: Option<Vec<LocationId>>,
    /// Optional trade terms specifying resource exchange rates.
    pub trade_terms: Option<BTreeMap<Resource, u32>>,
    /// Optional duration in ticks (treaty expires after this many ticks).
    pub duration_ticks: Option<u64>,
}

/// A formal treaty between two groups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Treaty {
    /// Unique treaty identifier.
    pub id: Uuid,
    /// First group in the treaty.
    pub group_a: GroupId,
    /// Second group in the treaty.
    pub group_b: GroupId,
    /// The terms of the treaty.
    pub terms: TreatyTerms,
    /// The tick when the treaty was negotiated.
    pub negotiated_at_tick: u64,
    /// Whether the treaty is still active.
    pub active: bool,
}

// ---------------------------------------------------------------------------
// Conflict types
// ---------------------------------------------------------------------------

/// An active conflict between two groups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    /// Unique conflict identifier.
    pub id: Uuid,
    /// The group that declared the conflict.
    pub aggressor: GroupId,
    /// The target of the conflict declaration.
    pub target: GroupId,
    /// The stated reason for the conflict.
    pub reason: String,
    /// The tick when the conflict was declared.
    pub declared_at_tick: u64,
    /// Whether the conflict is still active.
    pub active: bool,
    /// The tick when the conflict ended, if applicable.
    pub ended_at_tick: Option<u64>,
}

// ---------------------------------------------------------------------------
// Tribute types
// ---------------------------------------------------------------------------

/// A tribute offer from one agent to another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TributeRecord {
    /// Unique tribute identifier.
    pub id: Uuid,
    /// The agent offering tribute.
    pub from_agent: AgentId,
    /// The agent receiving tribute.
    pub to_agent: AgentId,
    /// The resources offered as tribute.
    pub resources: BTreeMap<Resource, u32>,
    /// The tick when the tribute was offered.
    pub tick: u64,
    /// The location where the tribute was offered.
    pub location_id: LocationId,
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// The result of a diplomacy action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiplomacyResult {
    /// An alliance was successfully formed.
    AllianceFormed {
        /// The newly created alliance.
        alliance_id: Uuid,
    },
    /// An alliance was broken.
    AllianceBroken {
        /// The alliance that was broken.
        alliance_id: Uuid,
    },
    /// A conflict was declared.
    ConflictDeclared {
        /// The newly created conflict.
        conflict_id: Uuid,
    },
    /// A treaty was successfully negotiated.
    TreatyNegotiated {
        /// The newly created treaty.
        treaty_id: Uuid,
    },
    /// A tribute was successfully offered.
    TributeOffered {
        /// The tribute record.
        tribute_id: Uuid,
    },
}

/// Errors specific to diplomacy operations.
#[derive(Debug, thiserror::Error)]
pub enum DiplomacyError {
    /// A group cannot form an alliance with itself.
    #[error("a group cannot form an alliance with itself: {0}")]
    SelfAlliance(GroupId),

    /// The groups are already allied.
    #[error("groups {0} and {1} are already allied")]
    AlreadyAllied(GroupId, GroupId),

    /// The groups are already in conflict.
    #[error("groups {0} and {1} are already in conflict")]
    AlreadyInConflict(GroupId, GroupId),

    /// Cannot declare conflict against an ally without breaking the alliance first.
    #[error("cannot declare conflict against ally {0} -- break alliance first")]
    ConflictWithAlly(GroupId),

    /// The specified alliance was not found.
    #[error("alliance not found: {0}")]
    AllianceNotFound(Uuid),

    /// The group is not part of the specified alliance.
    #[error("group {0} is not part of alliance {1}")]
    NotInAlliance(GroupId, Uuid),

    /// The groups are not in conflict (required for treaty).
    #[error("groups {0} and {1} are not in conflict")]
    NotInConflict(GroupId, GroupId),

    /// Leaders are not co-located (required for treaty negotiation).
    #[error("leaders must be co-located for treaty negotiation")]
    LeadersNotCoLocated,

    /// Agents are not co-located (required for tribute).
    #[error("agents must be co-located for tribute")]
    AgentsNotCoLocated,

    /// An underlying agent error occurred.
    #[error("agent error: {0}")]
    Agent(#[from] AgentError),
}

// ---------------------------------------------------------------------------
// DiplomacyState
// ---------------------------------------------------------------------------

/// Tracks all diplomatic relationships in the simulation.
///
/// Maintains active alliances, conflicts, and treaties between groups,
/// and records tribute offers between agents.
#[derive(Debug, Clone)]
pub struct DiplomacyState {
    /// Active alliances between groups.
    alliances: BTreeMap<Uuid, Alliance>,
    /// Active conflicts between groups.
    conflicts: BTreeMap<Uuid, Conflict>,
    /// Active treaties between groups.
    treaties: BTreeMap<Uuid, Treaty>,
    /// Historical tribute records.
    tributes: Vec<TributeRecord>,
}

impl DiplomacyState {
    /// Create a new empty diplomacy state.
    pub const fn new() -> Self {
        Self {
            alliances: BTreeMap::new(),
            conflicts: BTreeMap::new(),
            treaties: BTreeMap::new(),
            tributes: Vec::new(),
        }
    }

    /// Propose and form an alliance between two groups.
    ///
    /// # Validation
    ///
    /// - Groups must be different (no self-alliance).
    /// - Groups must not already be allied.
    ///
    /// Returns the alliance ID on success.
    pub fn propose_alliance(
        &mut self,
        proposer_group: GroupId,
        target_group: GroupId,
        terms: AllianceTerms,
        current_tick: u64,
    ) -> Result<DiplomacyResult, DiplomacyError> {
        // Cannot ally with self
        if proposer_group == target_group {
            return Err(DiplomacyError::SelfAlliance(proposer_group));
        }

        // Cannot already be allied
        if self.are_allied(&proposer_group, &target_group) {
            return Err(DiplomacyError::AlreadyAllied(proposer_group, target_group));
        }

        let alliance_id = Uuid::now_v7();
        let alliance = Alliance {
            id: alliance_id,
            groups: vec![proposer_group, target_group],
            terms,
            formed_at_tick: current_tick,
            status: AllianceStatus::Active,
        };

        self.alliances.insert(alliance_id, alliance);

        Ok(DiplomacyResult::AllianceFormed { alliance_id })
    }

    /// Break an existing alliance.
    ///
    /// The breaking group must be a member of the alliance.
    pub fn break_alliance(
        &mut self,
        alliance_id: Uuid,
        breaking_group: GroupId,
        current_tick: u64,
    ) -> Result<DiplomacyResult, DiplomacyError> {
        let alliance = self
            .alliances
            .get_mut(&alliance_id)
            .ok_or(DiplomacyError::AllianceNotFound(alliance_id))?;

        if !alliance.groups.contains(&breaking_group) {
            return Err(DiplomacyError::NotInAlliance(breaking_group, alliance_id));
        }

        alliance.status = AllianceStatus::Broken {
            broken_by: breaking_group,
            tick: current_tick,
        };

        Ok(DiplomacyResult::AllianceBroken { alliance_id })
    }

    /// Declare a conflict against another group.
    ///
    /// # Validation
    ///
    /// - Groups must not already be in conflict.
    /// - Aggressor must not be allied with the target (must break alliance first).
    pub fn declare_conflict(
        &mut self,
        aggressor: GroupId,
        target: GroupId,
        reason: String,
        current_tick: u64,
    ) -> Result<DiplomacyResult, DiplomacyError> {
        // Cannot declare conflict against an ally
        if self.are_allied(&aggressor, &target) {
            return Err(DiplomacyError::ConflictWithAlly(target));
        }

        // Cannot already be in conflict
        if self.are_in_conflict(&aggressor, &target) {
            return Err(DiplomacyError::AlreadyInConflict(aggressor, target));
        }

        let conflict_id = Uuid::now_v7();
        let conflict = Conflict {
            id: conflict_id,
            aggressor,
            target,
            reason,
            declared_at_tick: current_tick,
            active: true,
            ended_at_tick: None,
        };

        self.conflicts.insert(conflict_id, conflict);

        Ok(DiplomacyResult::ConflictDeclared { conflict_id })
    }

    /// Negotiate a treaty between two groups that are currently in conflict.
    ///
    /// # Validation
    ///
    /// - The groups must be in active conflict.
    /// - The group leaders must be co-located.
    ///
    /// If the treaty includes a ceasefire, the active conflict is ended.
    pub fn negotiate_treaty(
        &mut self,
        group_a: GroupId,
        group_b: GroupId,
        terms: TreatyTerms,
        leaders_co_located: bool,
        current_tick: u64,
    ) -> Result<DiplomacyResult, DiplomacyError> {
        // Leaders must be co-located
        if !leaders_co_located {
            return Err(DiplomacyError::LeadersNotCoLocated);
        }

        // Must be in conflict
        if !self.are_in_conflict(&group_a, &group_b) {
            return Err(DiplomacyError::NotInConflict(group_a, group_b));
        }

        let treaty_id = Uuid::now_v7();

        // If ceasefire, end the active conflict
        if terms.ceasefire {
            self.end_conflict_between(&group_a, &group_b, current_tick);
        }

        let treaty = Treaty {
            id: treaty_id,
            group_a,
            group_b,
            terms,
            negotiated_at_tick: current_tick,
            active: true,
        };

        self.treaties.insert(treaty_id, treaty);

        Ok(DiplomacyResult::TreatyNegotiated { treaty_id })
    }

    /// Record a tribute offer from one agent to another.
    ///
    /// The actual resource transfer is handled by the caller via the
    /// ledger system -- this method only records the diplomatic event.
    ///
    /// # Validation
    ///
    /// - Agents must be co-located.
    pub fn offer_tribute(
        &mut self,
        from_agent: AgentId,
        to_agent: AgentId,
        resources: BTreeMap<Resource, u32>,
        location_id: LocationId,
        agents_co_located: bool,
        current_tick: u64,
    ) -> Result<DiplomacyResult, DiplomacyError> {
        if !agents_co_located {
            return Err(DiplomacyError::AgentsNotCoLocated);
        }

        let tribute_id = Uuid::now_v7();
        let tribute = TributeRecord {
            id: tribute_id,
            from_agent,
            to_agent,
            resources,
            tick: current_tick,
            location_id,
        };

        self.tributes.push(tribute);

        Ok(DiplomacyResult::TributeOffered { tribute_id })
    }

    /// Check whether two groups have an active alliance.
    pub fn are_allied(&self, group_a: &GroupId, group_b: &GroupId) -> bool {
        self.alliances.values().any(|a| {
            matches!(a.status, AllianceStatus::Active)
                && a.groups.contains(group_a)
                && a.groups.contains(group_b)
        })
    }

    /// Check whether two groups are in active conflict.
    pub fn are_in_conflict(&self, group_a: &GroupId, group_b: &GroupId) -> bool {
        self.conflicts.values().any(|c| {
            c.active
                && ((c.aggressor == *group_a && c.target == *group_b)
                    || (c.aggressor == *group_b && c.target == *group_a))
        })
    }

    /// Get all active alliances.
    pub fn active_alliances(&self) -> Vec<&Alliance> {
        self.alliances
            .values()
            .filter(|a| matches!(a.status, AllianceStatus::Active))
            .collect()
    }

    /// Get all active conflicts.
    pub fn active_conflicts(&self) -> Vec<&Conflict> {
        self.conflicts.values().filter(|c| c.active).collect()
    }

    /// Get all active treaties.
    pub fn active_treaties(&self) -> Vec<&Treaty> {
        self.treaties.values().filter(|t| t.active).collect()
    }

    /// Get all alliances involving a specific group.
    pub fn alliances_for_group(&self, group_id: &GroupId) -> Vec<&Alliance> {
        self.alliances
            .values()
            .filter(|a| {
                matches!(a.status, AllianceStatus::Active)
                    && a.groups.contains(group_id)
            })
            .collect()
    }

    /// Get all active conflicts involving a specific group.
    pub fn conflicts_for_group(&self, group_id: &GroupId) -> Vec<&Conflict> {
        self.conflicts
            .values()
            .filter(|c| {
                c.active && (c.aggressor == *group_id || c.target == *group_id)
            })
            .collect()
    }

    /// Get all allies of a group (returns the other group IDs).
    pub fn allies_of(&self, group_id: &GroupId) -> BTreeSet<GroupId> {
        let mut allies = BTreeSet::new();
        for alliance in self.alliances.values() {
            if !matches!(alliance.status, AllianceStatus::Active) {
                continue;
            }
            if alliance.groups.contains(group_id) {
                for g in &alliance.groups {
                    if g != group_id {
                        allies.insert(*g);
                    }
                }
            }
        }
        allies
    }

    /// Get all enemies of a group (groups in active conflict).
    pub fn enemies_of(&self, group_id: &GroupId) -> BTreeSet<GroupId> {
        let mut enemies = BTreeSet::new();
        for conflict in self.conflicts.values() {
            if !conflict.active {
                continue;
            }
            if conflict.aggressor == *group_id {
                enemies.insert(conflict.target);
            } else if conflict.target == *group_id {
                enemies.insert(conflict.aggressor);
            }
        }
        enemies
    }

    /// Expire treaties that have exceeded their duration.
    ///
    /// Returns the IDs of treaties that were expired.
    pub fn expire_treaties(&mut self, current_tick: u64) -> Vec<Uuid> {
        let mut expired = Vec::new();

        for (id, treaty) in &mut self.treaties {
            if !treaty.active {
                continue;
            }
            if let Some(duration) = treaty.terms.duration_ticks {
                let expiry_tick = treaty
                    .negotiated_at_tick
                    .saturating_add(duration);
                if current_tick >= expiry_tick {
                    treaty.active = false;
                    expired.push(*id);
                }
            }
        }

        expired
    }

    /// Get a specific alliance by ID.
    pub fn get_alliance(&self, id: &Uuid) -> Option<&Alliance> {
        self.alliances.get(id)
    }

    /// Get a specific conflict by ID.
    pub fn get_conflict(&self, id: &Uuid) -> Option<&Conflict> {
        self.conflicts.get(id)
    }

    /// Get a specific treaty by ID.
    pub fn get_treaty(&self, id: &Uuid) -> Option<&Treaty> {
        self.treaties.get(id)
    }

    /// Compute the relationship delta for all members of two groups
    /// entering a conflict.
    ///
    /// Returns pairs of `(agent_a, agent_b, delta)` where `delta` is
    /// the relationship penalty to apply. Uses a base penalty of -0.3
    /// for conflict declaration.
    pub fn conflict_relationship_deltas(
        aggressor_members: &BTreeSet<AgentId>,
        defender_members: &BTreeSet<AgentId>,
    ) -> Vec<(AgentId, AgentId, Decimal)> {
        let penalty = Decimal::new(-3, 1); // -0.3
        let mut deltas = Vec::new();

        for &a in aggressor_members {
            for &b in defender_members {
                deltas.push((a, b, penalty));
            }
        }

        deltas
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// End all active conflicts between two groups.
    fn end_conflict_between(
        &mut self,
        group_a: &GroupId,
        group_b: &GroupId,
        current_tick: u64,
    ) {
        for conflict in self.conflicts.values_mut() {
            if !conflict.active {
                continue;
            }
            let matches = (conflict.aggressor == *group_a && conflict.target == *group_b)
                || (conflict.aggressor == *group_b && conflict.target == *group_a);
            if matches {
                conflict.active = false;
                conflict.ended_at_tick = Some(current_tick);
            }
        }
    }
}

impl Default for DiplomacyState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use rust_decimal::Decimal;

    use emergence_types::{AgentId, GroupId, LocationId, Resource};

    use super::*;

    // -----------------------------------------------------------------------
    // Alliance tests
    // -----------------------------------------------------------------------

    #[test]
    fn propose_alliance_success() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();

        let result = state.propose_alliance(
            g1,
            g2,
            AllianceTerms::default(),
            10,
        );

        assert!(result.is_ok());
        assert!(state.are_allied(&g1, &g2));
        assert_eq!(state.active_alliances().len(), 1);
    }

    #[test]
    fn propose_alliance_rejects_self_alliance() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();

        let result = state.propose_alliance(
            g1,
            g1,
            AllianceTerms::default(),
            10,
        );

        assert!(result.is_err());
        assert!(matches!(
            result.err(),
            Some(DiplomacyError::SelfAlliance(_))
        ));
    }

    #[test]
    fn propose_alliance_rejects_duplicate() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();

        let r1 = state.propose_alliance(g1, g2, AllianceTerms::default(), 10);
        assert!(r1.is_ok());

        let r2 = state.propose_alliance(g1, g2, AllianceTerms::default(), 11);
        assert!(r2.is_err());
        assert!(matches!(
            r2.err(),
            Some(DiplomacyError::AlreadyAllied(_, _))
        ));
    }

    #[test]
    fn break_alliance_success() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();

        let result = state.propose_alliance(g1, g2, AllianceTerms::default(), 10);
        assert!(result.is_ok());

        let alliance_id = match result.ok() {
            Some(DiplomacyResult::AllianceFormed { alliance_id }) => alliance_id,
            _ => Uuid::nil(),
        };

        let break_result = state.break_alliance(alliance_id, g1, 20);
        assert!(break_result.is_ok());
        assert!(!state.are_allied(&g1, &g2));
    }

    #[test]
    fn break_alliance_not_found() {
        let mut state = DiplomacyState::new();
        let result = state.break_alliance(Uuid::nil(), GroupId::new(), 10);
        assert!(result.is_err());
        assert!(matches!(
            result.err(),
            Some(DiplomacyError::AllianceNotFound(_))
        ));
    }

    #[test]
    fn break_alliance_not_member() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();
        let g3 = GroupId::new();

        let result = state.propose_alliance(g1, g2, AllianceTerms::default(), 10);
        let alliance_id = match result.ok() {
            Some(DiplomacyResult::AllianceFormed { alliance_id }) => alliance_id,
            _ => Uuid::nil(),
        };

        let break_result = state.break_alliance(alliance_id, g3, 20);
        assert!(break_result.is_err());
        assert!(matches!(
            break_result.err(),
            Some(DiplomacyError::NotInAlliance(_, _))
        ));
    }

    // -----------------------------------------------------------------------
    // Conflict tests
    // -----------------------------------------------------------------------

    #[test]
    fn declare_conflict_success() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();

        let result = state.declare_conflict(
            g1,
            g2,
            String::from("territorial dispute"),
            10,
        );

        assert!(result.is_ok());
        assert!(state.are_in_conflict(&g1, &g2));
        assert!(state.are_in_conflict(&g2, &g1)); // bidirectional check
        assert_eq!(state.active_conflicts().len(), 1);
    }

    #[test]
    fn declare_conflict_rejects_against_ally() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();

        let alliance = state.propose_alliance(g1, g2, AllianceTerms::default(), 10);
        assert!(alliance.is_ok());

        let conflict = state.declare_conflict(
            g1,
            g2,
            String::from("backstab"),
            20,
        );
        assert!(conflict.is_err());
        assert!(matches!(
            conflict.err(),
            Some(DiplomacyError::ConflictWithAlly(_))
        ));
    }

    #[test]
    fn declare_conflict_rejects_duplicate() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();

        let r1 = state.declare_conflict(g1, g2, String::from("first"), 10);
        assert!(r1.is_ok());

        let r2 = state.declare_conflict(g1, g2, String::from("second"), 11);
        assert!(r2.is_err());
        assert!(matches!(
            r2.err(),
            Some(DiplomacyError::AlreadyInConflict(_, _))
        ));
    }

    // -----------------------------------------------------------------------
    // Treaty tests
    // -----------------------------------------------------------------------

    #[test]
    fn negotiate_treaty_success() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();

        // First declare conflict
        let conflict = state.declare_conflict(
            g1,
            g2,
            String::from("dispute"),
            10,
        );
        assert!(conflict.is_ok());

        // Then negotiate a treaty with ceasefire
        let terms = TreatyTerms {
            ceasefire: true,
            border_agreement: None,
            trade_terms: None,
            duration_ticks: Some(100),
        };

        let result = state.negotiate_treaty(g1, g2, terms, true, 20);
        assert!(result.is_ok());

        // Conflict should be ended
        assert!(!state.are_in_conflict(&g1, &g2));
        assert_eq!(state.active_treaties().len(), 1);
    }

    #[test]
    fn negotiate_treaty_rejects_not_in_conflict() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();

        let terms = TreatyTerms {
            ceasefire: true,
            border_agreement: None,
            trade_terms: None,
            duration_ticks: None,
        };

        let result = state.negotiate_treaty(g1, g2, terms, true, 10);
        assert!(result.is_err());
        assert!(matches!(
            result.err(),
            Some(DiplomacyError::NotInConflict(_, _))
        ));
    }

    #[test]
    fn negotiate_treaty_rejects_leaders_not_co_located() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();

        let conflict = state.declare_conflict(g1, g2, String::from("war"), 10);
        assert!(conflict.is_ok());

        let terms = TreatyTerms {
            ceasefire: true,
            border_agreement: None,
            trade_terms: None,
            duration_ticks: None,
        };

        let result = state.negotiate_treaty(g1, g2, terms, false, 20);
        assert!(result.is_err());
        assert!(matches!(
            result.err(),
            Some(DiplomacyError::LeadersNotCoLocated)
        ));
    }

    #[test]
    fn treaty_without_ceasefire_preserves_conflict() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();

        let conflict = state.declare_conflict(g1, g2, String::from("war"), 10);
        assert!(conflict.is_ok());

        // Treaty without ceasefire
        let terms = TreatyTerms {
            ceasefire: false,
            border_agreement: Some(vec![LocationId::new()]),
            trade_terms: None,
            duration_ticks: None,
        };

        let result = state.negotiate_treaty(g1, g2, terms, true, 20);
        assert!(result.is_ok());

        // Conflict should still be active
        assert!(state.are_in_conflict(&g1, &g2));
    }

    // -----------------------------------------------------------------------
    // Tribute tests
    // -----------------------------------------------------------------------

    #[test]
    fn offer_tribute_success() {
        let mut state = DiplomacyState::new();
        let from = AgentId::new();
        let to = AgentId::new();
        let loc = LocationId::new();

        let mut resources = BTreeMap::new();
        resources.insert(Resource::FoodBerry, 5);

        let result = state.offer_tribute(from, to, resources, loc, true, 10);
        assert!(result.is_ok());
    }

    #[test]
    fn offer_tribute_rejects_not_co_located() {
        let mut state = DiplomacyState::new();
        let from = AgentId::new();
        let to = AgentId::new();
        let loc = LocationId::new();

        let resources = BTreeMap::new();
        let result = state.offer_tribute(from, to, resources, loc, false, 10);
        assert!(result.is_err());
        assert!(matches!(
            result.err(),
            Some(DiplomacyError::AgentsNotCoLocated)
        ));
    }

    // -----------------------------------------------------------------------
    // Query tests
    // -----------------------------------------------------------------------

    #[test]
    fn allies_of_returns_correct_groups() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();
        let g3 = GroupId::new();

        let r1 = state.propose_alliance(g1, g2, AllianceTerms::default(), 10);
        assert!(r1.is_ok());
        let r2 = state.propose_alliance(g1, g3, AllianceTerms::default(), 11);
        assert!(r2.is_ok());

        let allies = state.allies_of(&g1);
        assert_eq!(allies.len(), 2);
        assert!(allies.contains(&g2));
        assert!(allies.contains(&g3));

        // g2 and g3 are not allies of each other
        let g2_allies = state.allies_of(&g2);
        assert_eq!(g2_allies.len(), 1);
        assert!(g2_allies.contains(&g1));
    }

    #[test]
    fn enemies_of_returns_correct_groups() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();
        let g3 = GroupId::new();

        let r1 = state.declare_conflict(g1, g2, String::from("war1"), 10);
        assert!(r1.is_ok());
        let r2 = state.declare_conflict(g3, g1, String::from("war2"), 11);
        assert!(r2.is_ok());

        let enemies = state.enemies_of(&g1);
        assert_eq!(enemies.len(), 2);
        assert!(enemies.contains(&g2));
        assert!(enemies.contains(&g3));
    }

    #[test]
    fn expire_treaties_works() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();

        // Create a conflict, then a treaty with 10-tick duration
        let c = state.declare_conflict(g1, g2, String::from("war"), 10);
        assert!(c.is_ok());

        let terms = TreatyTerms {
            ceasefire: true,
            border_agreement: None,
            trade_terms: None,
            duration_ticks: Some(10),
        };

        let t = state.negotiate_treaty(g1, g2, terms, true, 20);
        assert!(t.is_ok());

        // Not expired at tick 29
        let expired = state.expire_treaties(29);
        assert!(expired.is_empty());
        assert_eq!(state.active_treaties().len(), 1);

        // Expired at tick 30 (20 + 10)
        let expired = state.expire_treaties(30);
        assert_eq!(expired.len(), 1);
        assert_eq!(state.active_treaties().len(), 0);
    }

    // -----------------------------------------------------------------------
    // Conflict relationship deltas
    // -----------------------------------------------------------------------

    #[test]
    fn conflict_relationship_deltas_computed() {
        let mut group_a = BTreeSet::new();
        group_a.insert(AgentId::new());
        group_a.insert(AgentId::new());

        let mut group_b = BTreeSet::new();
        group_b.insert(AgentId::new());

        let deltas = DiplomacyState::conflict_relationship_deltas(
            &group_a,
            &group_b,
        );

        // 2 members * 1 member = 2 pairs
        assert_eq!(deltas.len(), 2);

        for (_a, _b, delta) in &deltas {
            assert_eq!(*delta, Decimal::new(-3, 1));
        }
    }

    #[test]
    fn conflict_relationship_deltas_empty_groups() {
        let group_a = BTreeSet::new();
        let group_b = BTreeSet::new();

        let deltas = DiplomacyState::conflict_relationship_deltas(
            &group_a,
            &group_b,
        );

        assert!(deltas.is_empty());
    }

    // -----------------------------------------------------------------------
    // Full diplomacy lifecycle
    // -----------------------------------------------------------------------

    #[test]
    fn full_lifecycle_alliance_break_conflict_treaty() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();

        // 1. Form alliance
        let alliance_result = state.propose_alliance(
            g1,
            g2,
            AllianceTerms {
                mutual_defense: true,
                trade_preference: true,
                shared_territory: false,
            },
            10,
        );
        assert!(alliance_result.is_ok());
        assert!(state.are_allied(&g1, &g2));

        let alliance_id = match alliance_result.ok() {
            Some(DiplomacyResult::AllianceFormed { alliance_id }) => alliance_id,
            _ => Uuid::nil(),
        };

        // 2. Cannot declare conflict while allied
        let conflict_attempt = state.declare_conflict(
            g1,
            g2,
            String::from("betrayal"),
            20,
        );
        assert!(conflict_attempt.is_err());

        // 3. Break alliance
        let break_result = state.break_alliance(alliance_id, g1, 30);
        assert!(break_result.is_ok());
        assert!(!state.are_allied(&g1, &g2));

        // 4. Now can declare conflict
        let conflict_result = state.declare_conflict(
            g1,
            g2,
            String::from("betrayal"),
            31,
        );
        assert!(conflict_result.is_ok());
        assert!(state.are_in_conflict(&g1, &g2));

        // 5. Negotiate treaty
        let treaty_terms = TreatyTerms {
            ceasefire: true,
            border_agreement: None,
            trade_terms: None,
            duration_ticks: Some(50),
        };
        let treaty_result = state.negotiate_treaty(g1, g2, treaty_terms, true, 50);
        assert!(treaty_result.is_ok());

        // Conflict should be ended
        assert!(!state.are_in_conflict(&g1, &g2));
        assert_eq!(state.active_treaties().len(), 1);
    }

    #[test]
    fn alliances_for_group_filters() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();
        let g3 = GroupId::new();
        let g4 = GroupId::new();

        let r1 = state.propose_alliance(g1, g2, AllianceTerms::default(), 10);
        assert!(r1.is_ok());
        let r2 = state.propose_alliance(g3, g4, AllianceTerms::default(), 11);
        assert!(r2.is_ok());

        let g1_alliances = state.alliances_for_group(&g1);
        assert_eq!(g1_alliances.len(), 1);

        let g4_alliances = state.alliances_for_group(&g4);
        assert_eq!(g4_alliances.len(), 1);

        // g1 should not see g3-g4 alliance
        let g1_groups: BTreeSet<GroupId> = g1_alliances
            .iter()
            .flat_map(|a| a.groups.iter().copied())
            .collect();
        assert!(!g1_groups.contains(&g3));
        assert!(!g1_groups.contains(&g4));
    }

    #[test]
    fn conflicts_for_group_filters() {
        let mut state = DiplomacyState::new();
        let g1 = GroupId::new();
        let g2 = GroupId::new();
        let g3 = GroupId::new();

        let r1 = state.declare_conflict(g1, g2, String::from("war1"), 10);
        assert!(r1.is_ok());
        let r2 = state.declare_conflict(g2, g3, String::from("war2"), 11);
        assert!(r2.is_ok());

        // g2 is in both conflicts
        let g2_conflicts = state.conflicts_for_group(&g2);
        assert_eq!(g2_conflicts.len(), 2);

        // g1 is in one conflict
        let g1_conflicts = state.conflicts_for_group(&g1);
        assert_eq!(g1_conflicts.len(), 1);

        // g3 is in one conflict
        let g3_conflicts = state.conflicts_for_group(&g3);
        assert_eq!(g3_conflicts.len(), 1);
    }
}
