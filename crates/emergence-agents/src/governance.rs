//! Governance structure tracking for the Emergence simulation.
//!
//! Detects and classifies emergent governance patterns from agent behavior:
//! leadership claims, voting patterns, rule declarations, and authority
//! challenges. This module implements task 6.4.3 from the build plan.
//!
//! # Classification Logic
//!
//! The `GovernanceTracker` accumulates behavioral signals and classifies
//! the governance type based on:
//!
//! - **Anarchy**: no leadership claims at all.
//! - **Chieftain**: a single unchallenged leader with no voting.
//! - **Monarchy**: a single leader who was challenged but upheld.
//! - **Council/Oligarchy**: multiple leaders sharing power.
//! - **Democracy**: voting patterns present alongside leadership.
//! - **Theocracy**: the leader is also a religious construct adherent
//!   (detected externally; the tracker accepts an `is_religious` flag).
//! - **Dictatorship**: leadership established via force (high challenge
//!   rate, low upheld rate for challengers).
//!
//! # Scope
//!
//! Governance is tracked per group and per location. A group may have
//! governance independent of location, and a location may have governance
//! without a formal group.

use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use emergence_types::{AgentId, GroupId, LocationId};

// ---------------------------------------------------------------------------
// GovernanceType
// ---------------------------------------------------------------------------

/// The classified type of governance for a group or location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GovernanceType {
    /// No recognized leadership structure.
    Anarchy,
    /// A single unchallenged leader.
    Chieftain,
    /// Multiple leaders sharing power.
    Council,
    /// A single hereditary or established leader.
    Monarchy,
    /// Governance with voting patterns.
    Democracy,
    /// A small group of leaders controlling power.
    Oligarchy,
    /// A religious leader governing.
    Theocracy,
    /// Leadership established or maintained by force.
    Dictatorship,
}

// ---------------------------------------------------------------------------
// LeadershipClaim
// ---------------------------------------------------------------------------

/// A record of an agent claiming authority over a group or location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeadershipClaim {
    /// The agent making the claim.
    pub agent_id: AgentId,
    /// The group this claim applies to, if any.
    pub group_id: Option<GroupId>,
    /// The location this claim applies to, if any.
    pub location_id: Option<LocationId>,
    /// The tick when the claim was made.
    pub tick: u64,
    /// Whether this claim has been challenged.
    pub challenged: bool,
    /// Whether the claim was upheld after challenge.
    pub upheld: bool,
    /// Whether this leader is associated with a religious construct.
    pub is_religious_leader: bool,
}

// ---------------------------------------------------------------------------
// VoteRecord
// ---------------------------------------------------------------------------

/// A record of an agent casting a vote on a proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoteRecord {
    /// The agent who cast the vote.
    pub voter_id: AgentId,
    /// The proposal being voted on.
    pub proposal: String,
    /// Whether the agent voted in favor.
    pub in_favor: bool,
    /// The tick when the vote was cast.
    pub tick: u64,
}

// ---------------------------------------------------------------------------
// RuleDeclaration
// ---------------------------------------------------------------------------

/// A rule or law declared by a leader for a group or location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDeclaration {
    /// The agent who declared the rule.
    pub declared_by: AgentId,
    /// The tick when the rule was declared.
    pub tick: u64,
    /// The text of the rule.
    pub rule_text: String,
    /// The group this rule applies to, if any.
    pub group_scope: Option<GroupId>,
    /// The location this rule applies to, if any.
    pub location_scope: Option<LocationId>,
    /// Whether the rule is still active.
    pub active: bool,
}

// ---------------------------------------------------------------------------
// GovernanceTracker
// ---------------------------------------------------------------------------

/// Tracks governance-related actions and classifies government types.
///
/// Accumulates leadership claims, votes, rule declarations, and
/// authority challenges, then provides classification methods.
#[derive(Debug, Clone)]
pub struct GovernanceTracker {
    /// All leadership claims, keyed by a unique ID.
    claims: BTreeMap<Uuid, LeadershipClaim>,
    /// All vote records.
    votes: Vec<VoteRecord>,
    /// All rule declarations, keyed by a unique ID.
    rules: BTreeMap<Uuid, RuleDeclaration>,
    /// Authority challenge records: (challenger, `challenged_leader`, tick, success).
    challenges: Vec<(AgentId, AgentId, u64, bool)>,
}

impl GovernanceTracker {
    /// Create a new empty governance tracker.
    pub const fn new() -> Self {
        Self {
            claims: BTreeMap::new(),
            votes: Vec::new(),
            rules: BTreeMap::new(),
            challenges: Vec::new(),
        }
    }

    /// Record a leadership claim by an agent.
    ///
    /// Returns the unique ID of the claim record.
    pub fn record_leadership_claim(
        &mut self,
        agent_id: AgentId,
        group_id: Option<GroupId>,
        location_id: Option<LocationId>,
        tick: u64,
        is_religious_leader: bool,
    ) -> Uuid {
        let id = Uuid::now_v7();
        let claim = LeadershipClaim {
            agent_id,
            group_id,
            location_id,
            tick,
            challenged: false,
            upheld: false,
            is_religious_leader,
        };
        self.claims.insert(id, claim);
        id
    }

    /// Record a vote cast by an agent on a proposal.
    pub fn record_vote(
        &mut self,
        voter_id: AgentId,
        proposal: String,
        in_favor: bool,
        tick: u64,
    ) {
        self.votes.push(VoteRecord {
            voter_id,
            proposal,
            in_favor,
            tick,
        });
    }

    /// Record a rule declared by an agent.
    ///
    /// Returns the unique ID of the rule record.
    pub fn record_rule_declaration(
        &mut self,
        declared_by: AgentId,
        rule_text: String,
        group_scope: Option<GroupId>,
        location_scope: Option<LocationId>,
        tick: u64,
    ) -> Uuid {
        let id = Uuid::now_v7();
        let rule = RuleDeclaration {
            declared_by,
            tick,
            rule_text,
            group_scope,
            location_scope,
            active: true,
        };
        self.rules.insert(id, rule);
        id
    }

    /// Record an authority challenge against an existing leader.
    ///
    /// `success` indicates whether the challenger succeeded in
    /// overthrowing the existing leader.
    pub fn record_authority_challenge(
        &mut self,
        challenger: AgentId,
        challenged_leader: AgentId,
        tick: u64,
        success: bool,
    ) {
        self.challenges.push((challenger, challenged_leader, tick, success));

        // Mark matching claims as challenged and set upheld status.
        for claim in self.claims.values_mut() {
            if claim.agent_id == challenged_leader {
                claim.challenged = true;
                // If the challenge succeeded, the leader was NOT upheld.
                // If it failed, the leader WAS upheld.
                claim.upheld = !success;
            }
        }
    }

    /// Classify the type of government for a given group.
    ///
    /// Analyzes leadership claims, voting patterns, and challenges
    /// associated with the group to determine the governance type.
    pub fn classify_government_type(
        &self,
        group_id: Option<GroupId>,
        location_id: Option<LocationId>,
    ) -> GovernanceType {
        let relevant_claims: Vec<&LeadershipClaim> = self
            .claims
            .values()
            .filter(|c| {
                let group_match = match (group_id, c.group_id) {
                    (Some(g), Some(cg)) => g == cg,
                    (None, None) => true,
                    _ => false,
                };
                let location_match = match (location_id, c.location_id) {
                    (Some(l), Some(cl)) => l == cl,
                    (None, None) => true,
                    (None, Some(_)) | (Some(_), None) => group_match,
                };
                group_match || location_match
            })
            .collect();

        if relevant_claims.is_empty() {
            return GovernanceType::Anarchy;
        }

        // Check for voting patterns
        let has_votes = !self.votes.is_empty();

        // Collect overturned leaders (successfully challenged)
        let overturned: BTreeSet<AgentId> = self
            .challenges
            .iter()
            .filter(|(_, _, _, success)| *success)
            .map(|(_, leader, _, _)| *leader)
            .collect();

        // Count unique leaders, excluding overturned ones
        let leaders: BTreeSet<AgentId> = relevant_claims
            .iter()
            .map(|c| c.agent_id)
            .filter(|a| !overturned.contains(a))
            .collect();
        let leader_count = leaders.len();

        // Check if any leader is religious
        let has_religious_leader = relevant_claims
            .iter()
            .any(|c| c.is_religious_leader);

        // Count challenges and successful challenges
        let total_challenges = relevant_claims
            .iter()
            .filter(|c| c.challenged)
            .count();

        let successful_challenges = self
            .challenges
            .iter()
            .filter(|(_, _, _, success)| *success)
            .count();

        // Classification logic
        if has_religious_leader && leader_count == 1 {
            return GovernanceType::Theocracy;
        }

        if has_votes {
            return GovernanceType::Democracy;
        }

        if leader_count > 1 {
            if leader_count <= 3 {
                return GovernanceType::Council;
            }
            return GovernanceType::Oligarchy;
        }

        // Single leader scenarios
        if total_challenges > 0 {
            // High successful challenge rate suggests force-based rule
            if successful_challenges > 0 {
                return GovernanceType::Dictatorship;
            }
            // Challenged but upheld -> Monarchy
            return GovernanceType::Monarchy;
        }

        // Single unchallenged leader
        GovernanceType::Chieftain
    }

    /// Get the current recognized leaders for a group or location.
    ///
    /// Returns the agent IDs of agents with active (non-overturned)
    /// leadership claims.
    pub fn get_leaders(
        &self,
        group_id: Option<GroupId>,
        location_id: Option<LocationId>,
    ) -> BTreeSet<AgentId> {
        // Collect agents whose claims have been successfully challenged
        let overturned: BTreeSet<AgentId> = self
            .challenges
            .iter()
            .filter(|(_, _, _, success)| *success)
            .map(|(_, leader, _, _)| *leader)
            .collect();

        self.claims
            .values()
            .filter(|c| {
                let group_match = match (group_id, c.group_id) {
                    (Some(g), Some(cg)) => g == cg,
                    (None, None) => true,
                    _ => false,
                };
                let location_match = match (location_id, c.location_id) {
                    (Some(l), Some(cl)) => l == cl,
                    (None, None) => true,
                    (None, Some(_)) | (Some(_), None) => group_match,
                };
                (group_match || location_match) && !overturned.contains(&c.agent_id)
            })
            .map(|c| c.agent_id)
            .collect()
    }

    /// Get all active rules for a group and/or location.
    pub fn get_rules(
        &self,
        group_id: Option<GroupId>,
        location_id: Option<LocationId>,
    ) -> Vec<&RuleDeclaration> {
        self.rules
            .values()
            .filter(|r| {
                if !r.active {
                    return false;
                }
                let group_match = match (group_id, r.group_scope) {
                    (Some(g), Some(rg)) => g == rg,
                    (None | Some(_), None) | (None, Some(_)) => true,
                };
                let location_match = match (location_id, r.location_scope) {
                    (Some(l), Some(rl)) => l == rl,
                    (None | Some(_), None) | (None, Some(_)) => true,
                };
                group_match && location_match
            })
            .collect()
    }

    /// Calculate leadership stability as a ratio.
    ///
    /// Returns the fraction of leadership claims that were either
    /// unchallenged or successfully upheld. Returns 1.0 (as 100)
    /// if there are no claims.
    ///
    /// Returned as a percentage (0-100).
    pub fn leadership_stability(&self) -> u32 {
        let total_claims = self.claims.len();
        if total_claims == 0 {
            return 100;
        }

        let stable_claims = self
            .claims
            .values()
            .filter(|c| !c.challenged || c.upheld)
            .count();

        // safe: stable_claims <= total_claims, both are usize
        let pct = stable_claims
            .saturating_mul(100)
            .checked_div(total_claims)
            .unwrap_or(0);

        // Safe truncation: percentage is always 0-100
        u32::try_from(pct).unwrap_or(100)
    }

    /// Deactivate a rule by its ID.
    ///
    /// Returns `true` if the rule was found and deactivated.
    pub fn deactivate_rule(&mut self, rule_id: Uuid) -> bool {
        if let Some(rule) = self.rules.get_mut(&rule_id) {
            rule.active = false;
            return true;
        }
        false
    }

    /// Get the total number of recorded votes.
    pub const fn vote_count(&self) -> usize {
        self.votes.len()
    }

    /// Get all votes for a specific proposal.
    pub fn votes_for_proposal(&self, proposal: &str) -> Vec<&VoteRecord> {
        self.votes
            .iter()
            .filter(|v| v.proposal == proposal)
            .collect()
    }
}

impl Default for GovernanceTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use emergence_types::{AgentId, GroupId, LocationId};

    use super::*;

    // -----------------------------------------------------------------------
    // 1. Classification: Anarchy
    // -----------------------------------------------------------------------

    #[test]
    fn classify_anarchy_no_claims() {
        let tracker = GovernanceTracker::new();
        let gov_type = tracker.classify_government_type(None, None);
        assert_eq!(gov_type, GovernanceType::Anarchy);
    }

    // -----------------------------------------------------------------------
    // 2. Classification: Chieftain
    // -----------------------------------------------------------------------

    #[test]
    fn classify_chieftain_single_unchallenged() {
        let mut tracker = GovernanceTracker::new();
        let leader = AgentId::new();
        let group = GroupId::new();

        tracker.record_leadership_claim(leader, Some(group), None, 1, false);

        let gov_type = tracker.classify_government_type(Some(group), None);
        assert_eq!(gov_type, GovernanceType::Chieftain);
    }

    // -----------------------------------------------------------------------
    // 3. Classification: Monarchy
    // -----------------------------------------------------------------------

    #[test]
    fn classify_monarchy_challenged_but_upheld() {
        let mut tracker = GovernanceTracker::new();
        let leader = AgentId::new();
        let challenger = AgentId::new();
        let group = GroupId::new();

        tracker.record_leadership_claim(leader, Some(group), None, 1, false);
        // Challenge fails -> leader upheld
        tracker.record_authority_challenge(challenger, leader, 5, false);

        let gov_type = tracker.classify_government_type(Some(group), None);
        assert_eq!(gov_type, GovernanceType::Monarchy);
    }

    // -----------------------------------------------------------------------
    // 4. Classification: Council
    // -----------------------------------------------------------------------

    #[test]
    fn classify_council_multiple_leaders() {
        let mut tracker = GovernanceTracker::new();
        let leader_a = AgentId::new();
        let leader_b = AgentId::new();
        let group = GroupId::new();

        tracker.record_leadership_claim(leader_a, Some(group), None, 1, false);
        tracker.record_leadership_claim(leader_b, Some(group), None, 2, false);

        let gov_type = tracker.classify_government_type(Some(group), None);
        assert_eq!(gov_type, GovernanceType::Council);
    }

    // -----------------------------------------------------------------------
    // 5. Classification: Oligarchy
    // -----------------------------------------------------------------------

    #[test]
    fn classify_oligarchy_many_leaders() {
        let mut tracker = GovernanceTracker::new();
        let group = GroupId::new();

        for _ in 0..4 {
            tracker.record_leadership_claim(AgentId::new(), Some(group), None, 1, false);
        }

        let gov_type = tracker.classify_government_type(Some(group), None);
        assert_eq!(gov_type, GovernanceType::Oligarchy);
    }

    // -----------------------------------------------------------------------
    // 6. Classification: Democracy
    // -----------------------------------------------------------------------

    #[test]
    fn classify_democracy_with_votes() {
        let mut tracker = GovernanceTracker::new();
        let leader = AgentId::new();
        let voter = AgentId::new();
        let group = GroupId::new();

        tracker.record_leadership_claim(leader, Some(group), None, 1, false);
        tracker.record_vote(voter, String::from("Build a wall"), true, 5);

        let gov_type = tracker.classify_government_type(Some(group), None);
        assert_eq!(gov_type, GovernanceType::Democracy);
    }

    // -----------------------------------------------------------------------
    // 7. Classification: Theocracy
    // -----------------------------------------------------------------------

    #[test]
    fn classify_theocracy_religious_leader() {
        let mut tracker = GovernanceTracker::new();
        let leader = AgentId::new();
        let group = GroupId::new();

        tracker.record_leadership_claim(leader, Some(group), None, 1, true);

        let gov_type = tracker.classify_government_type(Some(group), None);
        assert_eq!(gov_type, GovernanceType::Theocracy);
    }

    // -----------------------------------------------------------------------
    // 8. Classification: Dictatorship
    // -----------------------------------------------------------------------

    #[test]
    fn classify_dictatorship_force_based() {
        let mut tracker = GovernanceTracker::new();
        let dictator = AgentId::new();
        let old_leader = AgentId::new();
        let group = GroupId::new();

        // Old leader makes a claim
        tracker.record_leadership_claim(old_leader, Some(group), None, 1, false);
        // Dictator challenges and wins
        tracker.record_authority_challenge(dictator, old_leader, 5, true);
        // Dictator makes their own claim
        tracker.record_leadership_claim(dictator, Some(group), None, 6, false);

        let gov_type = tracker.classify_government_type(Some(group), None);
        assert_eq!(gov_type, GovernanceType::Dictatorship);
    }

    // -----------------------------------------------------------------------
    // 9. Leadership tracking
    // -----------------------------------------------------------------------

    #[test]
    fn get_leaders_excludes_overturned() {
        let mut tracker = GovernanceTracker::new();
        let leader_a = AgentId::new();
        let leader_b = AgentId::new();
        let group = GroupId::new();

        tracker.record_leadership_claim(leader_a, Some(group), None, 1, false);
        tracker.record_leadership_claim(leader_b, Some(group), None, 2, false);

        // leader_a is successfully challenged
        tracker.record_authority_challenge(AgentId::new(), leader_a, 5, true);

        let leaders = tracker.get_leaders(Some(group), None);
        assert!(!leaders.contains(&leader_a));
        assert!(leaders.contains(&leader_b));
    }

    #[test]
    fn get_leaders_empty_when_no_claims() {
        let tracker = GovernanceTracker::new();
        let leaders = tracker.get_leaders(Some(GroupId::new()), None);
        assert!(leaders.is_empty());
    }

    // -----------------------------------------------------------------------
    // 10. Rule tracking
    // -----------------------------------------------------------------------

    #[test]
    fn get_rules_returns_active_only() {
        let mut tracker = GovernanceTracker::new();
        let leader = AgentId::new();
        let group = GroupId::new();

        let rule1 = tracker.record_rule_declaration(
            leader,
            String::from("No stealing"),
            Some(group),
            None,
            1,
        );
        tracker.record_rule_declaration(
            leader,
            String::from("Share food"),
            Some(group),
            None,
            2,
        );

        // Deactivate first rule
        tracker.deactivate_rule(rule1);

        let rules = tracker.get_rules(Some(group), None);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules.first().map(|r| r.rule_text.as_str()), Some("Share food"));
    }

    // -----------------------------------------------------------------------
    // 11. Leadership stability
    // -----------------------------------------------------------------------

    #[test]
    fn leadership_stability_no_claims() {
        let tracker = GovernanceTracker::new();
        assert_eq!(tracker.leadership_stability(), 100);
    }

    #[test]
    fn leadership_stability_all_unchallenged() {
        let mut tracker = GovernanceTracker::new();
        tracker.record_leadership_claim(AgentId::new(), None, None, 1, false);
        tracker.record_leadership_claim(AgentId::new(), None, None, 2, false);
        assert_eq!(tracker.leadership_stability(), 100);
    }

    #[test]
    fn leadership_stability_half_challenged() {
        let mut tracker = GovernanceTracker::new();
        let leader_a = AgentId::new();
        let leader_b = AgentId::new();

        tracker.record_leadership_claim(leader_a, None, None, 1, false);
        tracker.record_leadership_claim(leader_b, None, None, 2, false);

        // Challenge leader_a and succeed -> not upheld
        tracker.record_authority_challenge(AgentId::new(), leader_a, 5, true);

        // leader_a: challenged=true, upheld=false -> unstable
        // leader_b: challenged=false -> stable
        assert_eq!(tracker.leadership_stability(), 50);
    }

    // -----------------------------------------------------------------------
    // 12. Voting patterns
    // -----------------------------------------------------------------------

    #[test]
    fn record_and_query_votes() {
        let mut tracker = GovernanceTracker::new();
        let voter_a = AgentId::new();
        let voter_b = AgentId::new();

        tracker.record_vote(voter_a, String::from("Build wall"), true, 1);
        tracker.record_vote(voter_b, String::from("Build wall"), false, 1);
        tracker.record_vote(voter_a, String::from("Tax trade"), true, 2);

        assert_eq!(tracker.vote_count(), 3);

        let wall_votes = tracker.votes_for_proposal("Build wall");
        assert_eq!(wall_votes.len(), 2);

        let tax_votes = tracker.votes_for_proposal("Tax trade");
        assert_eq!(tax_votes.len(), 1);
    }

    // -----------------------------------------------------------------------
    // 13. Location-scoped governance
    // -----------------------------------------------------------------------

    #[test]
    fn classify_by_location() {
        let mut tracker = GovernanceTracker::new();
        let leader = AgentId::new();
        let loc = LocationId::new();

        tracker.record_leadership_claim(leader, None, Some(loc), 1, false);

        let gov_type = tracker.classify_government_type(None, Some(loc));
        assert_eq!(gov_type, GovernanceType::Chieftain);
    }

    // -----------------------------------------------------------------------
    // 14. Default trait
    // -----------------------------------------------------------------------

    #[test]
    fn default_tracker_is_empty() {
        let tracker = GovernanceTracker::default();
        assert_eq!(tracker.leadership_stability(), 100);
        assert_eq!(tracker.vote_count(), 0);
        assert!(tracker.get_leaders(None, None).is_empty());
    }
}
