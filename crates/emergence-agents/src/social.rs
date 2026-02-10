//! Social graph and relationship management for agents.
//!
//! Implements the social graph from `agent-system.md` section 3.4:
//! - Relationship scores (-1.0 to 1.0) tracking affinity between agents
//! - Interaction counting and recency tracking
//! - Relationship labels for perception assembly
//! - Group formation validation
//!
//! All arithmetic uses [`Decimal`] for precision. Scores are clamped to the
//! valid range on every update -- no silent drift beyond bounds.

use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;

use emergence_types::{AgentId, Group, GroupId, InteractionCause};

use crate::error::AgentError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum relationship score (full trust / alliance).
const SCORE_MAX: Decimal = Decimal::ONE;

/// Minimum relationship score (full hostility).
const SCORE_MIN: Decimal = Decimal::NEGATIVE_ONE;

/// Default relationship score for unknown agents.
const SCORE_DEFAULT: Decimal = Decimal::ZERO;

/// Relationship delta for a successful trade (+0.1).
fn delta_trade() -> Decimal {
    Decimal::new(1, 1)
}

/// Relationship delta for a failed/rejected trade (-0.05).
///
/// Returns the positive absolute value; the sign is applied at the call site.
fn delta_trade_failed_abs() -> Decimal {
    Decimal::new(5, 2)
}

/// Relationship delta for receiving teaching (+0.15).
fn delta_teaching() -> Decimal {
    Decimal::new(15, 2)
}

/// Relationship delta for a positive communication (+0.05).
fn delta_communication() -> Decimal {
    Decimal::new(5, 2)
}

/// Minimum relationship delta for conflict (-0.2).
fn delta_conflict_min() -> Decimal {
    Decimal::new(-2, 1)
}

/// Minimum relationship score with the founder required to join a group (0.3).
fn group_relationship_threshold() -> Decimal {
    Decimal::new(3, 1)
}

// ---------------------------------------------------------------------------
// SocialGraph
// ---------------------------------------------------------------------------

/// Per-agent social graph tracking relationships with other agents.
///
/// Each agent maintains their own `SocialGraph` alongside their [`AgentState`].
/// The graph stores relationship scores, interaction counts, and last
/// interaction ticks for every agent this agent has interacted with.
///
/// [`AgentState`]: emergence_types::AgentState
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocialGraph {
    /// Relationship scores mapped by other agent's ID.
    /// Scores range from -1.0 (hostile) to 1.0 (allied).
    relationships: BTreeMap<AgentId, Decimal>,
    /// Number of interactions with each known agent.
    interaction_count: BTreeMap<AgentId, u64>,
    /// Tick of the most recent interaction with each known agent.
    last_interaction: BTreeMap<AgentId, u64>,
    /// Group IDs this agent belongs to.
    groups: BTreeSet<GroupId>,
}

impl SocialGraph {
    /// Create a new empty social graph.
    pub const fn new() -> Self {
        Self {
            relationships: BTreeMap::new(),
            interaction_count: BTreeMap::new(),
            last_interaction: BTreeMap::new(),
            groups: BTreeSet::new(),
        }
    }

    /// Create a social graph pre-populated from an existing relationship map.
    ///
    /// This is used when reconstructing a `SocialGraph` from persisted
    /// [`AgentState`] data that already has a relationships `BTreeMap`.
    ///
    /// [`AgentState`]: emergence_types::AgentState
    pub const fn from_relationships(relationships: BTreeMap<AgentId, Decimal>) -> Self {
        Self {
            relationships,
            interaction_count: BTreeMap::new(),
            last_interaction: BTreeMap::new(),
            groups: BTreeSet::new(),
        }
    }

    /// Get the relationship score with another agent.
    ///
    /// Returns 0.0 if the agent is unknown.
    pub fn get_relationship(&self, agent_id: AgentId) -> Decimal {
        self.relationships
            .get(&agent_id)
            .copied()
            .unwrap_or(SCORE_DEFAULT)
    }

    /// Update the relationship score with another agent by a delta amount.
    ///
    /// The score is clamped to [-1.0, 1.0] after the update. The interaction
    /// count is incremented and the last interaction tick is recorded.
    ///
    /// Returns the old score and the new clamped score.
    pub fn update_relationship(
        &mut self,
        agent_id: AgentId,
        delta: Decimal,
        current_tick: u64,
    ) -> Result<(Decimal, Decimal), AgentError> {
        let old_score = self
            .relationships
            .get(&agent_id)
            .copied()
            .unwrap_or(SCORE_DEFAULT);

        let raw_new = old_score.checked_add(delta).ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: String::from("relationship score addition overflow"),
            }
        })?;

        let new_score = clamp_score(raw_new);
        self.relationships.insert(agent_id, new_score);

        // Update interaction count
        let count = self.interaction_count.entry(agent_id).or_insert(0);
        *count = count.checked_add(1).ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: String::from("interaction count overflow"),
            }
        })?;

        // Update last interaction tick
        self.last_interaction.insert(agent_id, current_tick);

        Ok((old_score, new_score))
    }

    /// Apply a relationship change for a specific interaction cause.
    ///
    /// Uses the standard delta values from `agent-system.md` section 3.4.
    /// For conflict, a `severity` parameter (0.0 to 1.0) scales the penalty
    /// between -0.2 and -0.5.
    ///
    /// Returns the old and new scores.
    pub fn apply_interaction(
        &mut self,
        agent_id: AgentId,
        cause: InteractionCause,
        current_tick: u64,
        conflict_severity: Option<Decimal>,
    ) -> Result<(Decimal, Decimal), AgentError> {
        let delta = match cause {
            InteractionCause::Trade => delta_trade(),
            InteractionCause::TradeFailed => {
                // Negate the absolute value
                SCORE_DEFAULT.checked_sub(delta_trade_failed_abs()).ok_or_else(|| {
                    AgentError::ArithmeticOverflow {
                        context: String::from("trade failed delta negation overflow"),
                    }
                })?
            }
            InteractionCause::Teaching => delta_teaching(),
            InteractionCause::Communication => delta_communication(),
            InteractionCause::Conflict => {
                compute_conflict_delta(conflict_severity.unwrap_or(SCORE_DEFAULT))?
            }
            InteractionCause::Theft => {
                // Theft has a strong negative impact on relationships
                compute_conflict_delta(conflict_severity.unwrap_or(SCORE_DEFAULT))?
            }
            InteractionCause::Intimidation => {
                // Intimidation has a moderate negative impact
                compute_conflict_delta(conflict_severity.unwrap_or(SCORE_DEFAULT))?
            }
        };

        self.update_relationship(agent_id, delta, current_tick)
    }

    /// Get the list of all known agent IDs in this social graph.
    pub fn known_agents(&self) -> Vec<AgentId> {
        self.relationships.keys().copied().collect()
    }

    /// Get the number of interactions with a specific agent.
    pub fn get_interaction_count(&self, agent_id: AgentId) -> u64 {
        self.interaction_count
            .get(&agent_id)
            .copied()
            .unwrap_or(0)
    }

    /// Get the tick of the last interaction with a specific agent.
    ///
    /// Returns `None` if no interaction has occurred.
    pub fn get_last_interaction(&self, agent_id: AgentId) -> Option<u64> {
        self.last_interaction.get(&agent_id).copied()
    }

    /// Generate a human-readable relationship label for perception.
    ///
    /// Labels follow the format specified in the task:
    /// - "friendly (0.7)" for positive scores >= 0.3
    /// - "hostile (-0.5)" for negative scores <= -0.3
    /// - "neutral (0.1)" for scores between -0.3 and 0.3
    /// - "stranger (unknown)" for agents not in the graph
    pub fn relationship_label(&self, agent_id: AgentId) -> String {
        self.relationships.get(&agent_id).map_or_else(
            || String::from("stranger (unknown)"),
            |score| {
                let friendly_threshold = Decimal::new(3, 1);
                let hostile_threshold = Decimal::new(-3, 1);

                let label = if *score >= friendly_threshold {
                    "friendly"
                } else if *score <= hostile_threshold {
                    "hostile"
                } else {
                    "neutral"
                };
                // Format: "label (score)" with score rounded to 1 decimal place
                let rounded = score.round_dp(1);
                format!("{label} ({rounded})")
            },
        )
    }

    /// Get a reference to the raw relationship map.
    ///
    /// Used when syncing back to [`AgentState`] for persistence.
    ///
    /// [`AgentState`]: emergence_types::AgentState
    pub const fn relationships_map(&self) -> &BTreeMap<AgentId, Decimal> {
        &self.relationships
    }

    /// Record that this agent joined a group.
    pub fn join_group(&mut self, group_id: GroupId) {
        self.groups.insert(group_id);
    }

    /// Remove a group membership.
    ///
    /// Returns `true` if the agent was a member and has been removed.
    pub fn leave_group(&mut self, group_id: GroupId) -> bool {
        self.groups.remove(&group_id)
    }

    /// Get all group IDs this agent belongs to.
    pub const fn group_memberships(&self) -> &BTreeSet<GroupId> {
        &self.groups
    }
}

impl Default for SocialGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Group Formation
// ---------------------------------------------------------------------------

/// Validate and create a new group.
///
/// Per the task spec:
/// - All invited members must be at the same location (checked via `co_located_agents`)
/// - All invited members must have relationship > 0.3 with the founder
/// - The founder is automatically a member
///
/// Returns the new [`Group`] on success.
pub fn form_group(
    group_name: String,
    founder_id: AgentId,
    invited_members: &[AgentId],
    founder_graph: &SocialGraph,
    co_located_agents: &BTreeSet<AgentId>,
    current_tick: u64,
) -> Result<Group, AgentError> {
    let threshold = group_relationship_threshold();

    // Validate each invited member
    for member_id in invited_members {
        // Must be at the same location
        if !co_located_agents.contains(member_id) {
            return Err(AgentError::GroupFormationFailed {
                reason: format!(
                    "invited member {member_id} is not at the same location as the founder"
                ),
            });
        }

        // Must have relationship > threshold with founder
        let score = founder_graph.get_relationship(*member_id);
        if score <= threshold {
            return Err(AgentError::GroupFormationFailed {
                reason: format!(
                    "relationship with {member_id} is {score}, needs to be above {threshold}"
                ),
            });
        }
    }

    let group_id = GroupId::new();

    let mut members = BTreeSet::new();
    members.insert(founder_id);
    for member_id in invited_members {
        members.insert(*member_id);
    }

    Ok(Group {
        id: group_id,
        name: group_name,
        founder: founder_id,
        members,
        formed_at_tick: current_tick,
    })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Clamp a relationship score to the valid range [-1.0, 1.0].
fn clamp_score(score: Decimal) -> Decimal {
    if score > SCORE_MAX {
        SCORE_MAX
    } else if score < SCORE_MIN {
        SCORE_MIN
    } else {
        score
    }
}

/// Compute the conflict delta based on severity.
///
/// Severity ranges from 0.0 to 1.0 and scales the penalty between
/// -0.2 (mild) and -0.5 (severe).
///
/// Formula: delta = -0.2 - (severity * 0.3)
fn compute_conflict_delta(severity: Decimal) -> Result<Decimal, AgentError> {
    let base = delta_conflict_min(); // -0.2
    let range = Decimal::new(3, 1); // 0.3

    // Clamp severity to [0.0, 1.0]
    let clamped_severity = if severity > Decimal::ONE {
        Decimal::ONE
    } else if severity < Decimal::ZERO {
        Decimal::ZERO
    } else {
        severity
    };

    let scaled_range = clamped_severity.checked_mul(range).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("conflict severity scaling overflow"),
        }
    })?;

    base.checked_sub(scaled_range).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("conflict delta computation overflow"),
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::*;

    #[test]
    fn new_graph_is_empty() {
        let graph = SocialGraph::new();
        assert!(graph.known_agents().is_empty());
        assert!(graph.group_memberships().is_empty());
    }

    #[test]
    fn default_score_for_unknown_agent() {
        let graph = SocialGraph::new();
        let unknown = AgentId::new();
        assert_eq!(graph.get_relationship(unknown), Decimal::ZERO);
    }

    #[test]
    fn update_relationship_basic() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        let result = graph.update_relationship(other, Decimal::new(1, 1), 10);
        assert!(result.is_ok());
        let (old, new) = result.ok().unwrap_or((Decimal::ZERO, Decimal::ZERO));
        assert_eq!(old, Decimal::ZERO);
        assert_eq!(new, Decimal::new(1, 1)); // 0.1
        assert_eq!(graph.get_relationship(other), Decimal::new(1, 1));
    }

    #[test]
    fn update_relationship_accumulates() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        let _ = graph.update_relationship(other, Decimal::new(3, 1), 10);
        let _ = graph.update_relationship(other, Decimal::new(2, 1), 20);
        assert_eq!(graph.get_relationship(other), Decimal::new(5, 1)); // 0.5
    }

    #[test]
    fn update_relationship_clamps_to_max() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        let _ = graph.update_relationship(other, Decimal::new(8, 1), 10);
        let _ = graph.update_relationship(other, Decimal::new(5, 1), 20);
        // 0.8 + 0.5 = 1.3, clamped to 1.0
        assert_eq!(graph.get_relationship(other), Decimal::ONE);
    }

    #[test]
    fn update_relationship_clamps_to_min() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        let _ = graph.update_relationship(other, Decimal::new(-8, 1), 10);
        let _ = graph.update_relationship(other, Decimal::new(-5, 1), 20);
        // -0.8 + (-0.5) = -1.3, clamped to -1.0
        assert_eq!(graph.get_relationship(other), Decimal::NEGATIVE_ONE);
    }

    #[test]
    fn interaction_count_increments() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        assert_eq!(graph.get_interaction_count(other), 0);
        let _ = graph.update_relationship(other, Decimal::new(1, 1), 10);
        assert_eq!(graph.get_interaction_count(other), 1);
        let _ = graph.update_relationship(other, Decimal::new(1, 1), 20);
        assert_eq!(graph.get_interaction_count(other), 2);
    }

    #[test]
    fn last_interaction_tracks_tick() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        assert!(graph.get_last_interaction(other).is_none());
        let _ = graph.update_relationship(other, Decimal::new(1, 1), 42);
        assert_eq!(graph.get_last_interaction(other), Some(42));
        let _ = graph.update_relationship(other, Decimal::new(1, 1), 100);
        assert_eq!(graph.get_last_interaction(other), Some(100));
    }

    #[test]
    fn known_agents_returns_all() {
        let mut graph = SocialGraph::new();
        let a = AgentId::new();
        let b = AgentId::new();
        let c = AgentId::new();

        let _ = graph.update_relationship(a, Decimal::new(1, 1), 10);
        let _ = graph.update_relationship(b, Decimal::new(-1, 1), 20);
        let _ = graph.update_relationship(c, Decimal::new(5, 1), 30);

        let known = graph.known_agents();
        assert_eq!(known.len(), 3);
        assert!(known.contains(&a));
        assert!(known.contains(&b));
        assert!(known.contains(&c));
    }

    // -----------------------------------------------------------------------
    // apply_interaction tests
    // -----------------------------------------------------------------------

    #[test]
    fn apply_trade_positive() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        let result = graph.apply_interaction(other, InteractionCause::Trade, 10, None);
        assert!(result.is_ok());
        assert_eq!(graph.get_relationship(other), Decimal::new(1, 1)); // +0.1
    }

    #[test]
    fn apply_trade_failed_negative() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        let result = graph.apply_interaction(other, InteractionCause::TradeFailed, 10, None);
        assert!(result.is_ok());
        assert_eq!(graph.get_relationship(other), Decimal::new(-5, 2)); // -0.05
    }

    #[test]
    fn apply_teaching_positive() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        let result = graph.apply_interaction(other, InteractionCause::Teaching, 10, None);
        assert!(result.is_ok());
        assert_eq!(graph.get_relationship(other), Decimal::new(15, 2)); // +0.15
    }

    #[test]
    fn apply_communication_positive() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        let result = graph.apply_interaction(other, InteractionCause::Communication, 10, None);
        assert!(result.is_ok());
        assert_eq!(graph.get_relationship(other), Decimal::new(5, 2)); // +0.05
    }

    #[test]
    fn apply_conflict_mild() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        // Severity 0.0 -> delta = -0.2
        let result = graph.apply_interaction(
            other,
            InteractionCause::Conflict,
            10,
            Some(Decimal::ZERO),
        );
        assert!(result.is_ok());
        assert_eq!(graph.get_relationship(other), Decimal::new(-2, 1)); // -0.2
    }

    #[test]
    fn apply_conflict_severe() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        // Severity 1.0 -> delta = -0.2 - 0.3 = -0.5
        let result = graph.apply_interaction(
            other,
            InteractionCause::Conflict,
            10,
            Some(Decimal::ONE),
        );
        assert!(result.is_ok());
        assert_eq!(graph.get_relationship(other), Decimal::new(-5, 1)); // -0.5
    }

    #[test]
    fn apply_conflict_mid_severity() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        // Severity 0.5 -> delta = -0.2 - 0.15 = -0.35
        let result = graph.apply_interaction(
            other,
            InteractionCause::Conflict,
            10,
            Some(Decimal::new(5, 1)),
        );
        assert!(result.is_ok());
        assert_eq!(graph.get_relationship(other), Decimal::new(-35, 2)); // -0.35
    }

    // -----------------------------------------------------------------------
    // relationship_label tests
    // -----------------------------------------------------------------------

    #[test]
    fn label_stranger() {
        let graph = SocialGraph::new();
        let unknown = AgentId::new();
        assert_eq!(graph.relationship_label(unknown), "stranger (unknown)");
    }

    #[test]
    fn label_friendly() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();
        let _ = graph.update_relationship(other, Decimal::new(7, 1), 10);
        assert_eq!(graph.relationship_label(other), "friendly (0.7)");
    }

    #[test]
    fn label_hostile() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();
        let _ = graph.update_relationship(other, Decimal::new(-5, 1), 10);
        assert_eq!(graph.relationship_label(other), "hostile (-0.5)");
    }

    #[test]
    fn label_neutral() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();
        let _ = graph.update_relationship(other, Decimal::new(1, 1), 10);
        assert_eq!(graph.relationship_label(other), "neutral (0.1)");
    }

    #[test]
    fn label_at_friendly_threshold() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();
        // Score exactly 0.3 -- the threshold check uses >=, so this is friendly
        let _ = graph.update_relationship(other, Decimal::new(3, 1), 10);
        assert_eq!(graph.relationship_label(other), "friendly (0.3)");
    }

    #[test]
    fn label_just_below_friendly_threshold() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();
        let _ = graph.update_relationship(other, Decimal::new(29, 2), 10);
        assert_eq!(graph.relationship_label(other), "neutral (0.3)");
    }

    #[test]
    fn label_at_hostile_threshold() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();
        // Score exactly -0.3 -- the threshold check uses <=, so this is hostile
        let _ = graph.update_relationship(other, Decimal::new(-3, 1), 10);
        assert_eq!(graph.relationship_label(other), "hostile (-0.3)");
    }

    // -----------------------------------------------------------------------
    // Group membership tests
    // -----------------------------------------------------------------------

    #[test]
    fn join_and_leave_group() {
        let mut graph = SocialGraph::new();
        let gid = GroupId::new();

        graph.join_group(gid);
        assert!(graph.group_memberships().contains(&gid));

        assert!(graph.leave_group(gid));
        assert!(!graph.group_memberships().contains(&gid));
    }

    #[test]
    fn leave_non_member_group() {
        let mut graph = SocialGraph::new();
        let gid = GroupId::new();
        assert!(!graph.leave_group(gid));
    }

    // -----------------------------------------------------------------------
    // form_group tests
    // -----------------------------------------------------------------------

    #[test]
    fn form_group_success() {
        let founder = AgentId::new();
        let member_a = AgentId::new();
        let member_b = AgentId::new();

        let mut graph = SocialGraph::new();
        // Set relationships above threshold (> 0.3)
        let _ = graph.update_relationship(member_a, Decimal::new(5, 1), 10);
        let _ = graph.update_relationship(member_b, Decimal::new(4, 1), 10);

        let mut co_located = BTreeSet::new();
        co_located.insert(member_a);
        co_located.insert(member_b);

        let result = form_group(
            String::from("Test Group"),
            founder,
            &[member_a, member_b],
            &graph,
            &co_located,
            100,
        );
        assert!(result.is_ok());

        let group = result.ok().unwrap_or_else(|| Group {
            id: GroupId::new(),
            name: String::new(),
            founder,
            members: BTreeSet::new(),
            formed_at_tick: 0,
        });
        assert_eq!(group.name, "Test Group");
        assert_eq!(group.founder, founder);
        assert_eq!(group.members.len(), 3); // founder + 2 members
        assert!(group.members.contains(&founder));
        assert!(group.members.contains(&member_a));
        assert!(group.members.contains(&member_b));
        assert_eq!(group.formed_at_tick, 100);
    }

    #[test]
    fn form_group_member_not_co_located() {
        let founder = AgentId::new();
        let member_a = AgentId::new();

        let mut graph = SocialGraph::new();
        let _ = graph.update_relationship(member_a, Decimal::new(5, 1), 10);

        // member_a is not in the co-located set
        let co_located = BTreeSet::new();

        let result = form_group(
            String::from("Bad Group"),
            founder,
            &[member_a],
            &graph,
            &co_located,
            100,
        );
        assert!(result.is_err());
    }

    #[test]
    fn form_group_insufficient_relationship() {
        let founder = AgentId::new();
        let member_a = AgentId::new();

        let mut graph = SocialGraph::new();
        // Relationship at threshold (0.3), not above it
        let _ = graph.update_relationship(member_a, Decimal::new(3, 1), 10);

        let mut co_located = BTreeSet::new();
        co_located.insert(member_a);

        let result = form_group(
            String::from("Low Trust Group"),
            founder,
            &[member_a],
            &graph,
            &co_located,
            100,
        );
        assert!(result.is_err());
    }

    #[test]
    fn form_group_just_above_threshold() {
        let founder = AgentId::new();
        let member_a = AgentId::new();

        let mut graph = SocialGraph::new();
        // Relationship just above threshold: 0.31
        let _ = graph.update_relationship(member_a, Decimal::new(31, 2), 10);

        let mut co_located = BTreeSet::new();
        co_located.insert(member_a);

        let result = form_group(
            String::from("Friendly Group"),
            founder,
            &[member_a],
            &graph,
            &co_located,
            100,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn form_group_no_invited_members() {
        let founder = AgentId::new();
        let graph = SocialGraph::new();
        let co_located = BTreeSet::new();

        let result = form_group(
            String::from("Solo Group"),
            founder,
            &[],
            &graph,
            &co_located,
            100,
        );
        assert!(result.is_ok());

        let group = result.ok().unwrap_or_else(|| Group {
            id: GroupId::new(),
            name: String::new(),
            founder,
            members: BTreeSet::new(),
            formed_at_tick: 0,
        });
        assert_eq!(group.members.len(), 1); // Just the founder
        assert!(group.members.contains(&founder));
    }

    #[test]
    fn form_group_unknown_agent_below_threshold() {
        let founder = AgentId::new();
        let stranger = AgentId::new();

        // No relationship with stranger -> score is 0.0, below 0.3
        let graph = SocialGraph::new();

        let mut co_located = BTreeSet::new();
        co_located.insert(stranger);

        let result = form_group(
            String::from("Stranger Group"),
            founder,
            &[stranger],
            &graph,
            &co_located,
            100,
        );
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // from_relationships tests
    // -----------------------------------------------------------------------

    #[test]
    fn from_relationships_populates_scores() {
        let a = AgentId::new();
        let b = AgentId::new();

        let mut map = BTreeMap::new();
        map.insert(a, Decimal::new(5, 1));
        map.insert(b, Decimal::new(-3, 1));

        let graph = SocialGraph::from_relationships(map);
        assert_eq!(graph.get_relationship(a), Decimal::new(5, 1));
        assert_eq!(graph.get_relationship(b), Decimal::new(-3, 1));
        assert_eq!(graph.known_agents().len(), 2);
    }

    // -----------------------------------------------------------------------
    // clamp_score tests
    // -----------------------------------------------------------------------

    #[test]
    fn clamp_within_range() {
        assert_eq!(clamp_score(Decimal::new(5, 1)), Decimal::new(5, 1));
        assert_eq!(clamp_score(Decimal::ZERO), Decimal::ZERO);
        assert_eq!(clamp_score(Decimal::new(-5, 1)), Decimal::new(-5, 1));
    }

    #[test]
    fn clamp_above_max() {
        assert_eq!(clamp_score(Decimal::new(15, 1)), Decimal::ONE);
    }

    #[test]
    fn clamp_below_min() {
        assert_eq!(clamp_score(Decimal::new(-15, 1)), Decimal::NEGATIVE_ONE);
    }

    // -----------------------------------------------------------------------
    // compute_conflict_delta tests
    // -----------------------------------------------------------------------

    #[test]
    fn conflict_delta_zero_severity() {
        let delta = compute_conflict_delta(Decimal::ZERO);
        assert!(delta.is_ok());
        assert_eq!(delta.ok(), Some(Decimal::new(-2, 1))); // -0.2
    }

    #[test]
    fn conflict_delta_full_severity() {
        let delta = compute_conflict_delta(Decimal::ONE);
        assert!(delta.is_ok());
        assert_eq!(delta.ok(), Some(Decimal::new(-5, 1))); // -0.5
    }

    #[test]
    fn conflict_delta_half_severity() {
        let delta = compute_conflict_delta(Decimal::new(5, 1));
        assert!(delta.is_ok());
        assert_eq!(delta.ok(), Some(Decimal::new(-35, 2))); // -0.35
    }

    #[test]
    fn conflict_delta_clamps_negative_severity() {
        // Negative severity should be treated as 0
        let delta = compute_conflict_delta(Decimal::new(-5, 1));
        assert!(delta.is_ok());
        assert_eq!(delta.ok(), Some(Decimal::new(-2, 1))); // -0.2
    }

    #[test]
    fn conflict_delta_clamps_excessive_severity() {
        // Severity > 1.0 should be treated as 1.0
        let delta = compute_conflict_delta(Decimal::new(20, 1));
        assert!(delta.is_ok());
        assert_eq!(delta.ok(), Some(Decimal::new(-5, 1))); // -0.5
    }

    // -----------------------------------------------------------------------
    // Edge case / regression tests
    // -----------------------------------------------------------------------

    #[test]
    fn many_positive_interactions_clamp_at_one() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        for tick in 0..20 {
            let _ = graph.apply_interaction(other, InteractionCause::Trade, tick, None);
        }
        // 20 * 0.1 = 2.0, clamped to 1.0
        assert_eq!(graph.get_relationship(other), Decimal::ONE);
        assert_eq!(graph.get_interaction_count(other), 20);
    }

    #[test]
    fn many_negative_interactions_clamp_at_negative_one() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        for tick in 0..25 {
            let _ = graph.apply_interaction(
                other,
                InteractionCause::Conflict,
                tick,
                Some(Decimal::ONE),
            );
        }
        // 25 * (-0.5) = -12.5, clamped to -1.0
        assert_eq!(graph.get_relationship(other), Decimal::NEGATIVE_ONE);
    }

    #[test]
    fn mixed_interactions_converge() {
        let mut graph = SocialGraph::new();
        let other = AgentId::new();

        // Teach (+0.15) then conflict severity 0 (-0.2) -> net -0.05
        let _ = graph.apply_interaction(other, InteractionCause::Teaching, 10, None);
        assert_eq!(graph.get_relationship(other), Decimal::new(15, 2));

        let _ = graph.apply_interaction(
            other,
            InteractionCause::Conflict,
            20,
            Some(Decimal::ZERO),
        );
        assert_eq!(graph.get_relationship(other), Decimal::new(-5, 2));
    }
}
