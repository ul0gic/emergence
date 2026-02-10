//! Deception tracking system for the Emergence simulation.
//!
//! Records when agents communicate false information, tracks lie histories,
//! and enables discovery of deceptions. This module implements task 6.3.4
//! from the build plan.
//!
//! # Architecture
//!
//! Deception tracking is a **passive observation layer** that sits alongside
//! the communication system. When an agent sends a message containing a
//! factual claim about the world (resource availability, relationships, etc.),
//! the system can compare that claim against ground truth.
//!
//! - **Deterministic claims** (resource levels, location facts) are verified
//!   against the world state at the time of the message.
//! - **Subjective claims** (opinions, future intentions) are marked as
//!   unverifiable and tracked for later resolution (e.g., broken promises).
//!
//! # Discovery Mechanics
//!
//! Deceptions can be discovered when:
//! - A victim visits the claimed location and observes different conditions.
//! - A third party communicates the truth to the victim.
//! - The victim's curiosity trait increases the probability of discovery
//!   each tick an active deception exists.
//!
//! # Events
//!
//! - `DeceptionCommitted` -- emitted when a lie is recorded.
//! - `DeceptionDiscovered` -- emitted when a victim or third party uncovers
//!   a deception.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use emergence_types::{AgentId, LocationId};

use crate::error::AgentError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default relationship penalty when a minor deception is discovered (0.3).
fn deception_penalty_minor() -> Decimal {
    Decimal::new(3, 1)
}

/// Default relationship penalty when a severe deception is discovered (0.5).
fn deception_penalty_severe() -> Decimal {
    Decimal::new(5, 1)
}

/// Base discovery chance per tick per 10000 (1% = 100).
const BASE_DISCOVERY_CHANCE_PER_10000: u32 = 100;

/// Curiosity multiplier for discovery chance per 10000.
/// Applied as: base + (`curiosity_per_10000` * `CURIOSITY_MULTIPLIER` / 10000).
const CURIOSITY_MULTIPLIER: u32 = 300;

// ---------------------------------------------------------------------------
// DeceptionType
// ---------------------------------------------------------------------------

/// The category of deception committed by an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeceptionType {
    /// Agent claimed resources exist at a location when they do not
    /// (or claimed scarcity when there is abundance).
    FalseResourceClaim,
    /// Agent misrepresented a relationship (e.g., "Dax is my enemy"
    /// when they are actually allies).
    FalseRelationship,
    /// Agent promised to perform an action (e.g., trade) and did not
    /// follow through within the expected timeframe.
    BrokenPromise,
    /// Agent claimed knowledge or capabilities they do not possess.
    FalseIdentity,
    /// Agent used false information to influence another agent's behavior.
    Manipulation,
    /// A deception type not covered by the standard categories.
    Other(String),
}

// ---------------------------------------------------------------------------
// DeceptionRecord
// ---------------------------------------------------------------------------

/// A record of a deceptive statement made by an agent.
///
/// Each record captures the ground truth at the time of the statement,
/// the false claim, and metadata about discovery status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeceptionRecord {
    /// Unique identifier for this deception record.
    pub id: Uuid,
    /// The tick when the deception occurred.
    pub tick: u64,
    /// The agent who committed the deception.
    pub deceiver_id: AgentId,
    /// The target of the deception. `None` if this was a broadcast lie.
    pub target_id: Option<AgentId>,
    /// The category of deception.
    pub deception_type: DeceptionType,
    /// What the agent claimed (serialized factual assertion).
    pub claimed_info: serde_json::Value,
    /// What was actually true at the time of the claim.
    pub actual_truth: serde_json::Value,
    /// The location where the deception took place.
    pub location_id: LocationId,
    /// Whether this deception has been discovered by the victim or others.
    pub discovered: bool,
    /// The tick when the deception was discovered, if applicable.
    pub discovered_at_tick: Option<u64>,
    /// The agent who discovered the deception, if applicable.
    pub discovered_by: Option<AgentId>,
}

// ---------------------------------------------------------------------------
// DeceptionDiscovery
// ---------------------------------------------------------------------------

/// The result of a deception being discovered.
///
/// Contains all information needed to emit a `DeceptionDiscovered` event
/// and apply relationship penalties.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeceptionDiscovery {
    /// The deception record ID that was discovered.
    pub deception_id: Uuid,
    /// The agent who committed the deception.
    pub deceiver_id: AgentId,
    /// The agent who discovered the deception.
    pub discoverer_id: AgentId,
    /// The tick when discovery occurred.
    pub discovered_at_tick: u64,
    /// The category of deception that was discovered.
    pub deception_type: DeceptionType,
    /// The relationship penalty to apply (positive value, will be negated).
    pub relationship_penalty: Decimal,
}

// ---------------------------------------------------------------------------
// DeceptionSeverity
// ---------------------------------------------------------------------------

/// Severity classification for a deception, used to determine the
/// relationship penalty on discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeceptionSeverity {
    /// Minor deception (false resource claims, small lies). Penalty: 0.3.
    Minor,
    /// Severe deception (manipulation, broken promises, false identity).
    /// Penalty: 0.5.
    Severe,
}

impl DeceptionSeverity {
    /// Return the relationship penalty for this severity level.
    pub fn penalty(self) -> Decimal {
        match self {
            Self::Minor => deception_penalty_minor(),
            Self::Severe => deception_penalty_severe(),
        }
    }
}

/// Classify the severity of a deception type.
pub const fn classify_severity(deception_type: &DeceptionType) -> DeceptionSeverity {
    match deception_type {
        DeceptionType::FalseResourceClaim | DeceptionType::FalseRelationship => {
            DeceptionSeverity::Minor
        }
        DeceptionType::BrokenPromise
        | DeceptionType::FalseIdentity
        | DeceptionType::Manipulation
        | DeceptionType::Other(_) => DeceptionSeverity::Severe,
    }
}

// ---------------------------------------------------------------------------
// DeceptionTracker
// ---------------------------------------------------------------------------

/// Maintains the state of all deceptions in the simulation.
///
/// The tracker stores active (undiscovered) deceptions and per-agent
/// lie statistics for honesty scoring.
#[derive(Debug, Clone)]
pub struct DeceptionTracker {
    /// Active undiscovered deceptions, keyed by their unique ID.
    active_deceptions: BTreeMap<Uuid, DeceptionRecord>,
    /// Discovered deceptions archive, keyed by their unique ID.
    discovered_deceptions: BTreeMap<Uuid, DeceptionRecord>,
    /// Per-agent total lie count (including discovered).
    agent_lie_counts: BTreeMap<AgentId, u32>,
    /// Per-agent total interaction count (for honesty ratio).
    agent_interaction_counts: BTreeMap<AgentId, u32>,
}

impl DeceptionTracker {
    /// Create a new empty deception tracker.
    pub const fn new() -> Self {
        Self {
            active_deceptions: BTreeMap::new(),
            discovered_deceptions: BTreeMap::new(),
            agent_lie_counts: BTreeMap::new(),
            agent_interaction_counts: BTreeMap::new(),
        }
    }

    /// Record a new deception.
    ///
    /// The deception is added to the active (undiscovered) set, and the
    /// deceiver's lie count is incremented.
    pub fn record_deception(
        &mut self,
        record: DeceptionRecord,
    ) -> Result<(), AgentError> {
        let deceiver = record.deceiver_id;
        let id = record.id;

        self.active_deceptions.insert(id, record);

        let count = self.agent_lie_counts.entry(deceiver).or_insert(0);
        *count = count.checked_add(1).ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("deception lie count overflow"),
        })?;

        Ok(())
    }

    /// Record an agent interaction (for honesty ratio calculation).
    ///
    /// Call this for every message sent by an agent, whether truthful or not.
    pub fn record_interaction(
        &mut self,
        agent_id: AgentId,
    ) -> Result<(), AgentError> {
        let count = self.agent_interaction_counts.entry(agent_id).or_insert(0);
        *count = count.checked_add(1).ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("deception interaction count overflow"),
        })?;
        Ok(())
    }

    /// Check for deception discoveries based on agent locations and curiosity.
    ///
    /// For each active deception, checks whether the victim (or any agent
    /// at the location referenced by the deception) could discover the lie.
    ///
    /// Discovery conditions:
    /// - The victim is now at the location mentioned in the false claim
    ///   (deterministic discovery for resource claims).
    /// - A random check based on curiosity trait passes.
    ///
    /// `agent_locations` maps each agent to their current location.
    /// `agent_curiosity` maps each agent to their curiosity trait (0-10000 scale).
    ///
    /// Returns a list of newly discovered deceptions.
    pub fn check_for_discoveries(
        &mut self,
        agent_locations: &BTreeMap<AgentId, LocationId>,
        agent_curiosity: &BTreeMap<AgentId, u32>,
        current_tick: u64,
        rng: &mut impl rand::Rng,
    ) -> Vec<DeceptionDiscovery> {
        let mut discoveries = Vec::new();
        let mut discovered_ids = Vec::new();

        for (id, record) in &self.active_deceptions {
            if record.discovered {
                continue;
            }

            // Check if the target (victim) is at the location of the claim
            if let Some(target_id) = record.target_id {
                let target_location = agent_locations.get(&target_id).copied();

                // Deterministic discovery: victim visits the location
                // referenced in a false resource claim
                let location_match = target_location
                    .is_some_and(|loc| loc == record.location_id);

                if location_match
                    && record.deception_type == DeceptionType::FalseResourceClaim
                {
                    let severity = classify_severity(&record.deception_type);
                    discoveries.push(DeceptionDiscovery {
                        deception_id: *id,
                        deceiver_id: record.deceiver_id,
                        discoverer_id: target_id,
                        discovered_at_tick: current_tick,
                        deception_type: record.deception_type.clone(),
                        relationship_penalty: severity.penalty(),
                    });
                    discovered_ids.push(*id);
                    continue;
                }

                // Probabilistic discovery: based on victim's curiosity
                let curiosity = agent_curiosity
                    .get(&target_id)
                    .copied()
                    .unwrap_or(0);

                let discovery_chance = compute_discovery_chance(curiosity);
                let roll: u32 = rng.random_range(0..10000);

                if roll < discovery_chance {
                    let severity = classify_severity(&record.deception_type);
                    discoveries.push(DeceptionDiscovery {
                        deception_id: *id,
                        deceiver_id: record.deceiver_id,
                        discoverer_id: target_id,
                        discovered_at_tick: current_tick,
                        deception_type: record.deception_type.clone(),
                        relationship_penalty: severity.penalty(),
                    });
                    discovered_ids.push(*id);
                }
            }
        }

        // Move discovered deceptions from active to discovered
        for id in discovered_ids {
            if let Some(mut record) = self.active_deceptions.remove(&id) {
                record.discovered = true;
                record.discovered_at_tick = Some(current_tick);
                // Find the corresponding discovery to set discovered_by
                for disc in &discoveries {
                    if disc.deception_id == id {
                        record.discovered_by = Some(disc.discoverer_id);
                        break;
                    }
                }
                self.discovered_deceptions.insert(id, record);
            }
        }

        discoveries
    }

    /// Calculate the honesty score for an agent.
    ///
    /// Returns a value between 0.0 (always lies) and 1.0 (never lies).
    /// If the agent has no recorded interactions, returns 1.0 (benefit of
    /// the doubt).
    ///
    /// Formula: `1.0 - (lie_count / interaction_count)`
    pub fn get_agent_honesty_score(
        &self,
        agent_id: &AgentId,
    ) -> Result<Decimal, AgentError> {
        let lie_count = self
            .agent_lie_counts
            .get(agent_id)
            .copied()
            .unwrap_or(0);

        let interaction_count = self
            .agent_interaction_counts
            .get(agent_id)
            .copied()
            .unwrap_or(0);

        if interaction_count == 0 {
            return Ok(Decimal::ONE);
        }

        let lie_dec = Decimal::from(lie_count);
        let interaction_dec = Decimal::from(interaction_count);

        let ratio = lie_dec
            .checked_div(interaction_dec)
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("honesty score division overflow"),
            })?;

        Decimal::ONE
            .checked_sub(ratio)
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("honesty score subtraction overflow"),
            })
    }

    /// Get the total lie count for an agent.
    pub fn get_agent_lie_count(&self, agent_id: &AgentId) -> u32 {
        self.agent_lie_counts
            .get(agent_id)
            .copied()
            .unwrap_or(0)
    }

    /// Get the number of active (undiscovered) deceptions.
    pub fn active_deception_count(&self) -> usize {
        self.active_deceptions.len()
    }

    /// Get the number of discovered deceptions.
    pub fn discovered_deception_count(&self) -> usize {
        self.discovered_deceptions.len()
    }

    /// Get all active deceptions committed by a specific agent.
    pub fn active_deceptions_by_agent(
        &self,
        agent_id: &AgentId,
    ) -> Vec<&DeceptionRecord> {
        self.active_deceptions
            .values()
            .filter(|r| r.deceiver_id == *agent_id)
            .collect()
    }

    /// Get all discovered deceptions committed by a specific agent.
    pub fn discovered_deceptions_by_agent(
        &self,
        agent_id: &AgentId,
    ) -> Vec<&DeceptionRecord> {
        self.discovered_deceptions
            .values()
            .filter(|r| r.deceiver_id == *agent_id)
            .collect()
    }

    /// Get an active deception by its ID.
    pub fn get_active_deception(&self, id: &Uuid) -> Option<&DeceptionRecord> {
        self.active_deceptions.get(id)
    }

    /// Get a discovered deception by its ID.
    pub fn get_discovered_deception(&self, id: &Uuid) -> Option<&DeceptionRecord> {
        self.discovered_deceptions.get(id)
    }
}

impl Default for DeceptionTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Compute the discovery chance per 10000 based on victim's curiosity.
///
/// `curiosity_per_10000` is the curiosity trait scaled to 0-10000.
///
/// Formula: `BASE + (curiosity * MULTIPLIER / 10000)`
fn compute_discovery_chance(curiosity_per_10000: u32) -> u32 {
    let scaled = curiosity_per_10000
        .saturating_mul(CURIOSITY_MULTIPLIER)
        .checked_div(10000)
        .unwrap_or(0);

    BASE_DISCOVERY_CHANCE_PER_10000.saturating_add(scaled)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rand::SeedableRng;
    use rand::rngs::SmallRng;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    use emergence_types::{AgentId, LocationId};

    use super::*;

    fn make_deception_record(
        deceiver: AgentId,
        target: Option<AgentId>,
        location: LocationId,
        tick: u64,
        deception_type: DeceptionType,
    ) -> DeceptionRecord {
        DeceptionRecord {
            id: Uuid::now_v7(),
            tick,
            deceiver_id: deceiver,
            target_id: target,
            deception_type,
            claimed_info: serde_json::json!({"resource": "food", "level": "abundant"}),
            actual_truth: serde_json::json!({"resource": "food", "level": "scarce"}),
            location_id: location,
            discovered: false,
            discovered_at_tick: None,
            discovered_by: None,
        }
    }

    // -----------------------------------------------------------------------
    // DeceptionTracker basic operations
    // -----------------------------------------------------------------------

    #[test]
    fn new_tracker_is_empty() {
        let tracker = DeceptionTracker::new();
        assert_eq!(tracker.active_deception_count(), 0);
        assert_eq!(tracker.discovered_deception_count(), 0);
    }

    #[test]
    fn record_deception_increments_counts() {
        let mut tracker = DeceptionTracker::new();
        let deceiver = AgentId::new();
        let target = AgentId::new();
        let location = LocationId::new();

        let record = make_deception_record(
            deceiver,
            Some(target),
            location,
            1,
            DeceptionType::FalseResourceClaim,
        );

        let result = tracker.record_deception(record);
        assert!(result.is_ok());
        assert_eq!(tracker.active_deception_count(), 1);
        assert_eq!(tracker.get_agent_lie_count(&deceiver), 1);
    }

    #[test]
    fn multiple_deceptions_tracked() {
        let mut tracker = DeceptionTracker::new();
        let deceiver = AgentId::new();
        let location = LocationId::new();

        for i in 0_u32..3 {
            let record = make_deception_record(
                deceiver,
                Some(AgentId::new()),
                location,
                u64::from(i),
                DeceptionType::FalseResourceClaim,
            );
            let result = tracker.record_deception(record);
            assert!(result.is_ok());
        }

        assert_eq!(tracker.active_deception_count(), 3);
        assert_eq!(tracker.get_agent_lie_count(&deceiver), 3);
    }

    // -----------------------------------------------------------------------
    // Honesty score tests
    // -----------------------------------------------------------------------

    #[test]
    fn honesty_score_no_interactions() {
        let tracker = DeceptionTracker::new();
        let agent = AgentId::new();

        let score = tracker.get_agent_honesty_score(&agent);
        assert!(score.is_ok());
        assert_eq!(score.ok(), Some(Decimal::ONE));
    }

    #[test]
    fn honesty_score_no_lies() {
        let mut tracker = DeceptionTracker::new();
        let agent = AgentId::new();

        // Record 5 interactions but no lies
        for _ in 0..5 {
            let result = tracker.record_interaction(agent);
            assert!(result.is_ok());
        }

        let score = tracker.get_agent_honesty_score(&agent);
        assert!(score.is_ok());
        assert_eq!(score.ok(), Some(Decimal::ONE));
    }

    #[test]
    fn honesty_score_all_lies() {
        let mut tracker = DeceptionTracker::new();
        let agent = AgentId::new();
        let location = LocationId::new();

        // Record 5 interactions, 5 lies
        for i in 0_u32..5 {
            let result = tracker.record_interaction(agent);
            assert!(result.is_ok());

            let record = make_deception_record(
                agent,
                Some(AgentId::new()),
                location,
                u64::from(i),
                DeceptionType::Manipulation,
            );
            let result = tracker.record_deception(record);
            assert!(result.is_ok());
        }

        let score = tracker.get_agent_honesty_score(&agent);
        assert!(score.is_ok());
        assert_eq!(score.ok(), Some(Decimal::ZERO));
    }

    #[test]
    fn honesty_score_mixed() {
        let mut tracker = DeceptionTracker::new();
        let agent = AgentId::new();
        let location = LocationId::new();

        // Record 10 interactions, 2 lies -> honesty = 1.0 - 2/10 = 0.8
        for _ in 0..10 {
            let result = tracker.record_interaction(agent);
            assert!(result.is_ok());
        }

        for i in 0_u32..2 {
            let record = make_deception_record(
                agent,
                Some(AgentId::new()),
                location,
                u64::from(i),
                DeceptionType::FalseResourceClaim,
            );
            let result = tracker.record_deception(record);
            assert!(result.is_ok());
        }

        let score = tracker.get_agent_honesty_score(&agent);
        assert!(score.is_ok());
        assert_eq!(score.ok(), Some(Decimal::new(8, 1)));
    }

    // -----------------------------------------------------------------------
    // Discovery tests
    // -----------------------------------------------------------------------

    #[test]
    fn deterministic_discovery_at_location() {
        let mut tracker = DeceptionTracker::new();
        let deceiver = AgentId::new();
        let victim = AgentId::new();
        let lie_location = LocationId::new();

        let record = make_deception_record(
            deceiver,
            Some(victim),
            lie_location,
            1,
            DeceptionType::FalseResourceClaim,
        );
        let deception_id = record.id;
        let result = tracker.record_deception(record);
        assert!(result.is_ok());

        // Victim arrives at the lied-about location
        let mut agent_locations = BTreeMap::new();
        agent_locations.insert(victim, lie_location);

        let agent_curiosity = BTreeMap::new();
        let mut rng = SmallRng::seed_from_u64(42);

        let discoveries = tracker.check_for_discoveries(
            &agent_locations,
            &agent_curiosity,
            5,
            &mut rng,
        );

        assert_eq!(discoveries.len(), 1);
        assert_eq!(discoveries.first().map(|d| d.deception_id), Some(deception_id));
        assert_eq!(discoveries.first().map(|d| d.deceiver_id), Some(deceiver));
        assert_eq!(discoveries.first().map(|d| d.discoverer_id), Some(victim));
        assert_eq!(discoveries.first().map(|d| d.discovered_at_tick), Some(5));

        // Should have moved from active to discovered
        assert_eq!(tracker.active_deception_count(), 0);
        assert_eq!(tracker.discovered_deception_count(), 1);
    }

    #[test]
    fn no_discovery_when_victim_elsewhere() {
        let mut tracker = DeceptionTracker::new();
        let deceiver = AgentId::new();
        let victim = AgentId::new();
        let lie_location = LocationId::new();
        let other_location = LocationId::new();

        let record = make_deception_record(
            deceiver,
            Some(victim),
            lie_location,
            1,
            DeceptionType::FalseResourceClaim,
        );
        let result = tracker.record_deception(record);
        assert!(result.is_ok());

        // Victim is at a different location
        let mut agent_locations = BTreeMap::new();
        agent_locations.insert(victim, other_location);

        let agent_curiosity = BTreeMap::new();
        let mut rng = SmallRng::seed_from_u64(42);

        let discoveries = tracker.check_for_discoveries(
            &agent_locations,
            &agent_curiosity,
            5,
            &mut rng,
        );

        // With base discovery chance of 1%, there might be a discovery.
        // But without curiosity, odds are low.  Still, active should be
        // either 0 or 1 depending on the roll.
        let total = tracker.active_deception_count()
            .checked_add(tracker.discovered_deception_count())
            .unwrap_or(0);
        assert_eq!(total, 1);
        assert_eq!(discoveries.len(), tracker.discovered_deception_count());
    }

    #[test]
    fn probabilistic_discovery_high_curiosity() {
        let mut tracker = DeceptionTracker::new();
        let deceiver = AgentId::new();
        let victim = AgentId::new();
        let lie_location = LocationId::new();
        let other_location = LocationId::new();

        // Use a non-resource deception so deterministic check does not fire
        let record = make_deception_record(
            deceiver,
            Some(victim),
            lie_location,
            1,
            DeceptionType::Manipulation,
        );
        let result = tracker.record_deception(record);
        assert!(result.is_ok());

        // Victim is at a different location, but has max curiosity
        let mut agent_locations = BTreeMap::new();
        agent_locations.insert(victim, other_location);

        let mut agent_curiosity = BTreeMap::new();
        agent_curiosity.insert(victim, 10000); // Max curiosity

        // With max curiosity: chance = 100 + (10000 * 300 / 10000) = 400 / 10000 = 4%
        // Over many runs, should discover at least once
        let mut discovered = false;
        for seed in 0..200_u64 {
            let mut fresh_tracker = DeceptionTracker::new();
            let record = make_deception_record(
                deceiver,
                Some(victim),
                lie_location,
                1,
                DeceptionType::Manipulation,
            );
            let result = fresh_tracker.record_deception(record);
            assert!(result.is_ok());

            let mut rng = SmallRng::seed_from_u64(seed);
            let discoveries = fresh_tracker.check_for_discoveries(
                &agent_locations,
                &agent_curiosity,
                5,
                &mut rng,
            );
            if !discoveries.is_empty() {
                discovered = true;
                break;
            }
        }
        assert!(discovered, "Expected at least one discovery over 200 attempts with 4% chance");
    }

    #[test]
    fn broadcast_deception_no_target() {
        let mut tracker = DeceptionTracker::new();
        let deceiver = AgentId::new();
        let location = LocationId::new();

        // Broadcast lie (no specific target)
        let record = make_deception_record(
            deceiver,
            None,
            location,
            1,
            DeceptionType::FalseResourceClaim,
        );
        let result = tracker.record_deception(record);
        assert!(result.is_ok());

        // No discoveries since there is no target
        let agent_locations = BTreeMap::new();
        let agent_curiosity = BTreeMap::new();
        let mut rng = SmallRng::seed_from_u64(42);

        let discoveries = tracker.check_for_discoveries(
            &agent_locations,
            &agent_curiosity,
            5,
            &mut rng,
        );

        assert!(discoveries.is_empty());
        assert_eq!(tracker.active_deception_count(), 1);
    }

    // -----------------------------------------------------------------------
    // Severity classification tests
    // -----------------------------------------------------------------------

    #[test]
    fn severity_minor_for_resource_claims() {
        assert_eq!(
            classify_severity(&DeceptionType::FalseResourceClaim),
            DeceptionSeverity::Minor
        );
    }

    #[test]
    fn severity_minor_for_relationship_claims() {
        assert_eq!(
            classify_severity(&DeceptionType::FalseRelationship),
            DeceptionSeverity::Minor
        );
    }

    #[test]
    fn severity_severe_for_manipulation() {
        assert_eq!(
            classify_severity(&DeceptionType::Manipulation),
            DeceptionSeverity::Severe
        );
    }

    #[test]
    fn severity_severe_for_broken_promise() {
        assert_eq!(
            classify_severity(&DeceptionType::BrokenPromise),
            DeceptionSeverity::Severe
        );
    }

    #[test]
    fn severity_severe_for_false_identity() {
        assert_eq!(
            classify_severity(&DeceptionType::FalseIdentity),
            DeceptionSeverity::Severe
        );
    }

    #[test]
    fn severity_penalty_values() {
        assert_eq!(DeceptionSeverity::Minor.penalty(), Decimal::new(3, 1));
        assert_eq!(DeceptionSeverity::Severe.penalty(), Decimal::new(5, 1));
    }

    // -----------------------------------------------------------------------
    // Discovery chance computation
    // -----------------------------------------------------------------------

    #[test]
    fn discovery_chance_zero_curiosity() {
        let chance = compute_discovery_chance(0);
        assert_eq!(chance, BASE_DISCOVERY_CHANCE_PER_10000);
    }

    #[test]
    fn discovery_chance_max_curiosity() {
        let chance = compute_discovery_chance(10000);
        // 100 + (10000 * 300 / 10000) = 100 + 300 = 400
        assert_eq!(chance, 400);
    }

    #[test]
    fn discovery_chance_mid_curiosity() {
        let chance = compute_discovery_chance(5000);
        // 100 + (5000 * 300 / 10000) = 100 + 150 = 250
        assert_eq!(chance, 250);
    }

    // -----------------------------------------------------------------------
    // Agent-specific query tests
    // -----------------------------------------------------------------------

    #[test]
    fn active_deceptions_by_agent_filters_correctly() {
        let mut tracker = DeceptionTracker::new();
        let agent_a = AgentId::new();
        let agent_b = AgentId::new();
        let location = LocationId::new();

        let record_a = make_deception_record(
            agent_a,
            Some(AgentId::new()),
            location,
            1,
            DeceptionType::FalseResourceClaim,
        );
        let record_b = make_deception_record(
            agent_b,
            Some(AgentId::new()),
            location,
            2,
            DeceptionType::Manipulation,
        );

        let result_a = tracker.record_deception(record_a);
        assert!(result_a.is_ok());
        let result_b = tracker.record_deception(record_b);
        assert!(result_b.is_ok());

        let a_deceptions = tracker.active_deceptions_by_agent(&agent_a);
        assert_eq!(a_deceptions.len(), 1);
        assert_eq!(a_deceptions.first().map(|d| d.deceiver_id), Some(agent_a));

        let b_deceptions = tracker.active_deceptions_by_agent(&agent_b);
        assert_eq!(b_deceptions.len(), 1);
        assert_eq!(b_deceptions.first().map(|d| d.deceiver_id), Some(agent_b));
    }

    #[test]
    fn get_deception_by_id() {
        let mut tracker = DeceptionTracker::new();
        let deceiver = AgentId::new();
        let location = LocationId::new();

        let record = make_deception_record(
            deceiver,
            Some(AgentId::new()),
            location,
            1,
            DeceptionType::BrokenPromise,
        );
        let id = record.id;
        let result = tracker.record_deception(record);
        assert!(result.is_ok());

        assert!(tracker.get_active_deception(&id).is_some());
        assert!(tracker.get_discovered_deception(&id).is_none());
    }
}
