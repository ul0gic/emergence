//! Reputation tracking system for the Emergence simulation.
//!
//! Agents build observable reputations through their actions. Other agents
//! who have interacted with or observed the subject can see their reputation
//! tags in perception payloads.
//!
//! # Architecture
//!
//! Reputation is **subjective per-observer**. Each observer maintains their
//! own view of a subject's reputation based on actions they have witnessed.
//! A public (aggregate) view averages all observers' scores per tag.
//!
//! Reputation entries decay over time -- old observations fade, allowing
//! agents to change their behavior and rebuild their standing.
//!
//! # Events
//!
//! - `ReputationChanged` -- emitted when an observation updates a reputation.
//!
//! # Invariants
//!
//! - Reputation scores are clamped to [0.0, 1.0].
//! - Observation deltas are clamped to [-1.0, 1.0].
//! - An agent can only see the reputation of agents they have interacted with.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use emergence_types::AgentId;

use crate::error::AgentError;
use crate::social::SocialGraph;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Minimum number of evidence observations required for a tag to appear
/// in a perception summary.
const MIN_EVIDENCE_FOR_SUMMARY: u32 = 2;

/// Minimum score required for a tag to appear in a perception summary.
const MIN_SCORE_FOR_SUMMARY: f64 = 0.3;

/// Default number of top tags to return.
const DEFAULT_TOP_TAGS: usize = 5;

/// Default decay factor applied per call to `decay_reputation`.
const DEFAULT_DECAY_FACTOR: f64 = 0.05;

// ---------------------------------------------------------------------------
// ReputationTag
// ---------------------------------------------------------------------------

/// Observable character traits that form an agent's reputation.
///
/// Tags are earned through actions and can be positive, negative, or
/// neutral depending on the observer's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ReputationTag {
    /// Gives resources freely, helps others.
    Generous,
    /// Hoards resources, refuses trades.
    Greedy,
    /// Tells the truth, keeps promises.
    Honest,
    /// Caught in deceptions.
    Liar,
    /// Caught stealing.
    Thief,
    /// Engages in combat frequently.
    Warrior,
    /// Resolves conflicts diplomatically.
    Peacemaker,
    /// Holds leadership positions.
    Leader,
    /// Provides medicine or aid.
    Healer,
    /// Constructs structures frequently.
    Builder,
    /// Discovers or spreads knowledge.
    Scholar,
    /// Avoids social interaction.
    Hermit,
    /// Has killed other agents.
    Murderer,
}

impl ReputationTag {
    /// Returns `true` if this tag is considered socially positive.
    const fn is_positive(self) -> bool {
        matches!(
            self,
            Self::Generous
                | Self::Honest
                | Self::Peacemaker
                | Self::Leader
                | Self::Healer
                | Self::Builder
                | Self::Scholar
        )
    }
}

// ---------------------------------------------------------------------------
// ReputationEntry
// ---------------------------------------------------------------------------

/// A single reputation tag with its strength and supporting evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReputationEntry {
    /// The reputation tag.
    pub tag: ReputationTag,
    /// Strength of this reputation (0.0 to 1.0).
    pub score: f64,
    /// How many observations support this tag.
    pub evidence_count: u32,
    /// The tick when this entry was last updated.
    pub last_updated_tick: u64,
}

// ---------------------------------------------------------------------------
// ReputationObservation
// ---------------------------------------------------------------------------

/// A single observation that modifies a reputation entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReputationObservation {
    /// The agent making the observation.
    pub observer: AgentId,
    /// The agent being observed.
    pub subject: AgentId,
    /// The tick when the observation was made.
    pub tick: u64,
    /// The reputation tag being updated.
    pub tag: ReputationTag,
    /// Change in score (positive strengthens, negative weakens).
    pub delta: f64,
    /// Human-readable reason for the observation.
    pub reason: String,
}

// ---------------------------------------------------------------------------
// ReputationProfile
// ---------------------------------------------------------------------------

/// A public-facing summary of an agent's reputation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReputationProfile {
    /// The agent this profile describes.
    pub agent_id: AgentId,
    /// Reputation tags sorted by score descending.
    pub tags: Vec<ReputationEntry>,
    /// Overall sentiment from -1.0 (negative) to 1.0 (positive).
    /// Computed as a weighted average of positive vs negative tags.
    pub overall_sentiment: f64,
}

// ---------------------------------------------------------------------------
// ReputationTracker
// ---------------------------------------------------------------------------

/// Central tracker for all reputation observations in the simulation.
///
/// Stores per-observer, per-subject reputation entries and provides
/// methods for recording observations, querying reputations, and
/// generating perception-ready summaries.
#[derive(Debug, Clone)]
pub struct ReputationTracker {
    /// Nested map: observer -> subject -> tag -> entry.
    entries: BTreeMap<AgentId, BTreeMap<AgentId, BTreeMap<ReputationTag, ReputationEntry>>>,
}

impl ReputationTracker {
    /// Create a new empty reputation tracker.
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Record a single reputation observation.
    ///
    /// Updates the observer's view of the subject for the given tag.
    /// The score is clamped to [0.0, 1.0] after applying the delta.
    /// The evidence count is incremented.
    pub fn record_observation(
        &mut self,
        observation: &ReputationObservation,
    ) -> Result<(), AgentError> {
        let subject_map = self
            .entries
            .entry(observation.observer)
            .or_default();
        let tag_map = subject_map
            .entry(observation.subject)
            .or_default();

        let entry = tag_map
            .entry(observation.tag)
            .or_insert_with(|| ReputationEntry {
                tag: observation.tag,
                score: 0.0,
                evidence_count: 0,
                last_updated_tick: observation.tick,
            });

        entry.score = clamp_score(entry.score + observation.delta);
        entry.evidence_count = entry.evidence_count.saturating_add(1);
        entry.last_updated_tick = observation.tick;

        Ok(())
    }

    /// Auto-generate reputation observations from an action.
    ///
    /// Given an action descriptor, generates the appropriate reputation
    /// observation for the observer to record about the subject.
    ///
    /// Returns the observations that were generated and recorded.
    pub fn record_action_reputation(
        &mut self,
        action: &ActionReputationEvent,
    ) -> Result<Vec<ReputationObservation>, AgentError> {
        let mut observations = Vec::new();

        let (tag, delta, reason) = match action.action {
            ReputationAction::GenerousTrade => {
                (ReputationTag::Generous, 0.2, String::from("Generous trade terms"))
            }
            ReputationAction::TheftDetected => {
                (ReputationTag::Thief, 0.3, String::from("Caught stealing"))
            }
            ReputationAction::DeceptionDiscovered => {
                (ReputationTag::Liar, 0.3, String::from("Deception discovered"))
            }
            ReputationAction::CombatInitiated => {
                (ReputationTag::Warrior, 0.2, String::from("Initiated combat"))
            }
            ReputationAction::AllianceProposed => {
                (ReputationTag::Peacemaker, 0.2, String::from("Proposed alliance"))
            }
            ReputationAction::StructureBuilt => {
                (ReputationTag::Builder, 0.15, String::from("Built a structure"))
            }
            ReputationAction::DiscoveryMade => {
                (ReputationTag::Scholar, 0.2, String::from("Made a discovery"))
            }
            ReputationAction::KilledAgent => {
                (ReputationTag::Murderer, 0.5, String::from("Killed another agent"))
            }
            ReputationAction::MedicineGiven => {
                (ReputationTag::Healer, 0.2, String::from("Provided medicine"))
            }
        };

        let obs = ReputationObservation {
            observer: action.observer,
            subject: action.subject,
            tick: action.tick,
            tag,
            delta,
            reason,
        };

        self.record_observation(&obs)?;
        observations.push(obs);

        Ok(observations)
    }

    /// Get how a specific observer views a subject.
    ///
    /// Returns all reputation entries the observer has recorded for the
    /// subject, sorted by score descending.
    pub fn get_reputation(
        &self,
        observer: AgentId,
        subject: AgentId,
    ) -> Vec<ReputationEntry> {
        self.entries
            .get(&observer)
            .and_then(|subjects| subjects.get(&subject))
            .map(|tags| {
                let mut entries: Vec<ReputationEntry> = tags.values().cloned().collect();
                entries.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(core::cmp::Ordering::Equal)
                });
                entries
            })
            .unwrap_or_default()
    }

    /// Get the aggregate public reputation for an agent.
    ///
    /// Averages all observers' scores for each tag to produce a public
    /// view of the agent's reputation.
    pub fn get_public_reputation(&self, subject: AgentId) -> ReputationProfile {
        let mut tag_totals: BTreeMap<ReputationTag, (f64, u32, u32, u64)> = BTreeMap::new();

        for subjects in self.entries.values() {
            if let Some(tags) = subjects.get(&subject) {
                for (tag, entry) in tags {
                    let totals = tag_totals.entry(*tag).or_insert((0.0, 0, 0, 0));
                    totals.0 += entry.score;
                    totals.1 = totals.1.saturating_add(1); // observer count
                    totals.2 = totals.2.saturating_add(entry.evidence_count);
                    if entry.last_updated_tick > totals.3 {
                        totals.3 = entry.last_updated_tick;
                    }
                }
            }
        }

        let mut tags: Vec<ReputationEntry> = tag_totals
            .into_iter()
            .map(|(tag, (total_score, observer_count, total_evidence, last_tick))| {
                let avg_score = if observer_count > 0 {
                    total_score / f64::from(observer_count)
                } else {
                    0.0
                };
                ReputationEntry {
                    tag,
                    score: clamp_score(avg_score),
                    evidence_count: total_evidence,
                    last_updated_tick: last_tick,
                }
            })
            .collect();

        tags.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(core::cmp::Ordering::Equal)
        });

        let overall_sentiment = compute_sentiment(&tags);

        ReputationProfile {
            agent_id: subject,
            tags,
            overall_sentiment,
        }
    }

    /// Get the N strongest reputation tags for an agent (public view).
    pub fn get_top_tags(
        &self,
        subject: AgentId,
        n: Option<usize>,
    ) -> Vec<ReputationEntry> {
        let count = n.unwrap_or(DEFAULT_TOP_TAGS);
        let profile = self.get_public_reputation(subject);
        profile.tags.into_iter().take(count).collect()
    }

    /// Get all agents who have a specific tag above a score threshold.
    pub fn get_agents_with_tag(
        &self,
        tag: ReputationTag,
        threshold: f64,
    ) -> Vec<(AgentId, f64)> {
        // Collect all unique subjects
        let mut subjects: std::collections::BTreeSet<AgentId> =
            std::collections::BTreeSet::new();
        for subjects_map in self.entries.values() {
            for subject_id in subjects_map.keys() {
                subjects.insert(*subject_id);
            }
        }

        let mut results = Vec::new();
        for subject_id in subjects {
            let profile = self.get_public_reputation(subject_id);
            for entry in &profile.tags {
                if entry.tag == tag && entry.score > threshold {
                    results.push((subject_id, entry.score));
                }
            }
        }

        // Sort by score descending
        results.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(core::cmp::Ordering::Equal)
        });

        results
    }

    /// Generate a compact reputation summary string for inclusion in
    /// agent perception payloads.
    ///
    /// Only includes tags with score > 0.3 and `evidence_count` >= 2.
    /// Format: "Known as: Generous, Leader, Scholar"
    pub fn reputation_summary_for_perception(
        &self,
        subject: AgentId,
    ) -> Option<String> {
        let profile = self.get_public_reputation(subject);
        let qualifying_tags: Vec<&str> = profile
            .tags
            .iter()
            .filter(|e| e.score > MIN_SCORE_FOR_SUMMARY && e.evidence_count >= MIN_EVIDENCE_FOR_SUMMARY)
            .map(|e| tag_display_name(e.tag))
            .collect();

        if qualifying_tags.is_empty() {
            return None;
        }

        Some(format!("Known as: {}", qualifying_tags.join(", ")))
    }

    /// Check whether an agent can see the reputation of another agent.
    ///
    /// An agent can only see the reputation of agents they have interacted
    /// with, as determined by the social graph.
    pub fn can_see_reputation(
        observer: AgentId,
        subject: AgentId,
        social_graph: &SocialGraph,
    ) -> bool {
        // Check if the observer has any interaction history with the subject
        social_graph.get_interaction_count(subject) > 0
            || social_graph.known_agents().contains(&subject)
            // Also allow if observer == subject (agents can always see their own reputation)
            || observer == subject
    }

    /// Decay reputation scores for observations older than `max_age_ticks`.
    ///
    /// Reduces scores by `decay_factor` for each entry whose last update
    /// is older than `current_tick - max_age_ticks`. Entries with scores
    /// that decay to zero or below are removed.
    pub fn decay_reputation(
        &mut self,
        current_tick: u64,
        max_age_ticks: u64,
        decay_factor: Option<f64>,
    ) {
        let factor = decay_factor.unwrap_or(DEFAULT_DECAY_FACTOR);
        let threshold_tick = current_tick.saturating_sub(max_age_ticks);

        // Collect keys to avoid borrow issues
        let observer_keys: Vec<AgentId> = self.entries.keys().copied().collect();

        for observer in observer_keys {
            let subject_keys: Vec<AgentId> = self
                .entries
                .get(&observer)
                .map(|s| s.keys().copied().collect())
                .unwrap_or_default();

            for subject in subject_keys {
                let tag_keys: Vec<ReputationTag> = self
                    .entries
                    .get(&observer)
                    .and_then(|s| s.get(&subject))
                    .map(|t| t.keys().copied().collect())
                    .unwrap_or_default();

                let mut tags_to_remove = Vec::new();

                for tag in &tag_keys {
                    if let Some(entry) = self
                        .entries
                        .get_mut(&observer)
                        .and_then(|s| s.get_mut(&subject))
                        .and_then(|t| t.get_mut(tag))
                        .filter(|e| e.last_updated_tick < threshold_tick)
                    {
                        entry.score -= factor;
                        if entry.score <= 0.0 {
                            tags_to_remove.push(*tag);
                        }
                    }
                }

                for tag in tags_to_remove {
                    if let Some(subject_map) = self
                        .entries
                        .get_mut(&observer)
                        .and_then(|s| s.get_mut(&subject))
                    {
                        subject_map.remove(&tag);
                    }
                }

                // Clean up empty subject maps
                let subject_empty = self
                    .entries
                    .get(&observer)
                    .and_then(|s| s.get(&subject))
                    .is_some_and(BTreeMap::is_empty);

                if subject_empty
                    && let Some(obs_map) = self.entries.get_mut(&observer)
                {
                    obs_map.remove(&subject);
                }
            }

            // Clean up empty observer maps
            let observer_empty = self
                .entries
                .get(&observer)
                .is_some_and(BTreeMap::is_empty);

            if observer_empty {
                self.entries.remove(&observer);
            }
        }
    }
}

impl Default for ReputationTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Action reputation event types
// ---------------------------------------------------------------------------

/// Describes an action that generates a reputation observation.
#[derive(Debug, Clone)]
pub struct ActionReputationEvent {
    /// The agent who observed the action.
    pub observer: AgentId,
    /// The agent who performed the action.
    pub subject: AgentId,
    /// The tick when the action occurred.
    pub tick: u64,
    /// The type of action for reputation purposes.
    pub action: ReputationAction,
}

/// Action categories that generate reputation changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReputationAction {
    /// Trade with generous terms.
    GenerousTrade,
    /// Theft detected by others.
    TheftDetected,
    /// Deception discovered by others.
    DeceptionDiscovered,
    /// Combat initiated by this agent.
    CombatInitiated,
    /// Alliance or peace proposal.
    AllianceProposed,
    /// Structure built by this agent.
    StructureBuilt,
    /// Knowledge discovery made.
    DiscoveryMade,
    /// Killed another agent.
    KilledAgent,
    /// Gave medicine to another agent.
    MedicineGiven,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Clamp a reputation score to the valid range [0.0, 1.0].
const fn clamp_score(score: f64) -> f64 {
    score.clamp(0.0, 1.0)
}

/// Compute overall sentiment from a set of reputation entries.
///
/// Positive tags contribute positively, negative tags contribute negatively.
/// The result is clamped to [-1.0, 1.0].
fn compute_sentiment(entries: &[ReputationEntry]) -> f64 {
    if entries.is_empty() {
        return 0.0;
    }

    let mut weighted_sum: f64 = 0.0;
    let mut total_weight: f64 = 0.0;

    for entry in entries {
        let weight = entry.score;
        let direction: f64 = if entry.tag.is_positive() {
            1.0
        } else {
            -1.0
        };

        weighted_sum += weight * direction;
        total_weight += weight;
    }

    if total_weight == 0.0 {
        return 0.0;
    }

    let sentiment = weighted_sum / total_weight;

    // Clamp to [-1.0, 1.0]
    sentiment.clamp(-1.0, 1.0)
}

/// Return a display name for a reputation tag.
const fn tag_display_name(tag: ReputationTag) -> &'static str {
    match tag {
        ReputationTag::Generous => "Generous",
        ReputationTag::Greedy => "Greedy",
        ReputationTag::Honest => "Honest",
        ReputationTag::Liar => "Liar",
        ReputationTag::Thief => "Thief",
        ReputationTag::Warrior => "Warrior",
        ReputationTag::Peacemaker => "Peacemaker",
        ReputationTag::Leader => "Leader",
        ReputationTag::Healer => "Healer",
        ReputationTag::Builder => "Builder",
        ReputationTag::Scholar => "Scholar",
        ReputationTag::Hermit => "Hermit",
        ReputationTag::Murderer => "Murderer",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use emergence_types::AgentId;

    use super::*;

    // -----------------------------------------------------------------------
    // Helper functions
    // -----------------------------------------------------------------------

    fn make_observation(
        observer: AgentId,
        subject: AgentId,
        tick: u64,
        tag: ReputationTag,
        delta: f64,
    ) -> ReputationObservation {
        ReputationObservation {
            observer,
            subject,
            tick,
            tag,
            delta,
            reason: String::from("test observation"),
        }
    }

    // -----------------------------------------------------------------------
    // Basic observation recording
    // -----------------------------------------------------------------------

    #[test]
    fn new_tracker_is_empty() {
        let tracker = ReputationTracker::new();
        let agent = AgentId::new();
        let rep = tracker.get_reputation(agent, AgentId::new());
        assert!(rep.is_empty());
    }

    #[test]
    fn record_single_observation() {
        let mut tracker = ReputationTracker::new();
        let observer = AgentId::new();
        let subject = AgentId::new();

        let obs = make_observation(observer, subject, 10, ReputationTag::Generous, 0.5);
        let result = tracker.record_observation(&obs);
        assert!(result.is_ok());

        let rep = tracker.get_reputation(observer, subject);
        assert_eq!(rep.len(), 1);
        assert_eq!(rep.first().map(|e| e.tag), Some(ReputationTag::Generous));
        assert!((rep.first().map(|e| e.score).unwrap_or(0.0) - 0.5).abs() < f64::EPSILON);
        assert_eq!(rep.first().map(|e| e.evidence_count), Some(1));
    }

    #[test]
    fn multiple_observations_accumulate() {
        let mut tracker = ReputationTracker::new();
        let observer = AgentId::new();
        let subject = AgentId::new();

        let obs1 = make_observation(observer, subject, 10, ReputationTag::Generous, 0.3);
        let obs2 = make_observation(observer, subject, 20, ReputationTag::Generous, 0.2);

        let _ = tracker.record_observation(&obs1);
        let _ = tracker.record_observation(&obs2);

        let rep = tracker.get_reputation(observer, subject);
        assert_eq!(rep.len(), 1);
        assert!((rep.first().map(|e| e.score).unwrap_or(0.0) - 0.5).abs() < f64::EPSILON);
        assert_eq!(rep.first().map(|e| e.evidence_count), Some(2));
        assert_eq!(rep.first().map(|e| e.last_updated_tick), Some(20));
    }

    #[test]
    fn score_clamps_to_max() {
        let mut tracker = ReputationTracker::new();
        let observer = AgentId::new();
        let subject = AgentId::new();

        let obs = make_observation(observer, subject, 10, ReputationTag::Builder, 1.5);
        let _ = tracker.record_observation(&obs);

        let rep = tracker.get_reputation(observer, subject);
        assert!((rep.first().map(|e| e.score).unwrap_or(0.0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn score_clamps_to_min() {
        let mut tracker = ReputationTracker::new();
        let observer = AgentId::new();
        let subject = AgentId::new();

        let obs = make_observation(observer, subject, 10, ReputationTag::Thief, -0.5);
        let _ = tracker.record_observation(&obs);

        let rep = tracker.get_reputation(observer, subject);
        // Starting from 0.0, delta of -0.5 should clamp to 0.0
        assert!((rep.first().map(|e| e.score).unwrap_or(1.0) - 0.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Action-based reputation generation
    // -----------------------------------------------------------------------

    #[test]
    fn action_reputation_generates_observation() {
        let mut tracker = ReputationTracker::new();
        let observer = AgentId::new();
        let subject = AgentId::new();

        let event = ActionReputationEvent {
            observer,
            subject,
            tick: 10,
            action: ReputationAction::TheftDetected,
        };

        let result = tracker.record_action_reputation(&event);
        assert!(result.is_ok());
        let observations = result.unwrap_or_default();
        assert_eq!(observations.len(), 1);
        assert_eq!(
            observations.first().map(|o| o.tag),
            Some(ReputationTag::Thief)
        );

        let rep = tracker.get_reputation(observer, subject);
        assert_eq!(rep.first().map(|e| e.tag), Some(ReputationTag::Thief));
    }

    #[test]
    fn action_reputation_all_actions() {
        let mut tracker = ReputationTracker::new();
        let observer = AgentId::new();
        let subject = AgentId::new();

        let actions = [
            (ReputationAction::GenerousTrade, ReputationTag::Generous),
            (ReputationAction::TheftDetected, ReputationTag::Thief),
            (ReputationAction::DeceptionDiscovered, ReputationTag::Liar),
            (ReputationAction::CombatInitiated, ReputationTag::Warrior),
            (ReputationAction::AllianceProposed, ReputationTag::Peacemaker),
            (ReputationAction::StructureBuilt, ReputationTag::Builder),
            (ReputationAction::DiscoveryMade, ReputationTag::Scholar),
            (ReputationAction::KilledAgent, ReputationTag::Murderer),
            (ReputationAction::MedicineGiven, ReputationTag::Healer),
        ];

        for (action, expected_tag) in actions {
            let event = ActionReputationEvent {
                observer,
                subject,
                tick: 10,
                action,
            };
            let result = tracker.record_action_reputation(&event);
            assert!(result.is_ok());
            let observations = result.unwrap_or_default();
            assert_eq!(observations.first().map(|o| o.tag), Some(expected_tag));
        }
    }

    // -----------------------------------------------------------------------
    // Personal vs public view
    // -----------------------------------------------------------------------

    #[test]
    fn personal_view_differs_from_public() {
        let mut tracker = ReputationTracker::new();
        let observer_a = AgentId::new();
        let observer_b = AgentId::new();
        let subject = AgentId::new();

        // Observer A thinks subject is very generous
        let obs_a = make_observation(observer_a, subject, 10, ReputationTag::Generous, 0.8);
        let _ = tracker.record_observation(&obs_a);

        // Observer B thinks subject is only slightly generous
        let obs_b = make_observation(observer_b, subject, 10, ReputationTag::Generous, 0.2);
        let _ = tracker.record_observation(&obs_b);

        // Personal views should differ
        let rep_a = tracker.get_reputation(observer_a, subject);
        let rep_b = tracker.get_reputation(observer_b, subject);
        assert!((rep_a.first().map(|e| e.score).unwrap_or(0.0) - 0.8).abs() < f64::EPSILON);
        assert!((rep_b.first().map(|e| e.score).unwrap_or(0.0) - 0.2).abs() < f64::EPSILON);

        // Public view should average
        let profile = tracker.get_public_reputation(subject);
        let generous_entry = profile.tags.iter().find(|e| e.tag == ReputationTag::Generous);
        assert!(generous_entry.is_some());
        assert!(
            (generous_entry.map(|e| e.score).unwrap_or(0.0) - 0.5).abs() < f64::EPSILON
        );
    }

    // -----------------------------------------------------------------------
    // Top tags
    // -----------------------------------------------------------------------

    #[test]
    fn get_top_tags_returns_sorted() {
        let mut tracker = ReputationTracker::new();
        let observer = AgentId::new();
        let subject = AgentId::new();

        let tags_and_scores = [
            (ReputationTag::Builder, 0.3),
            (ReputationTag::Scholar, 0.8),
            (ReputationTag::Generous, 0.5),
        ];

        for (tag, score) in tags_and_scores {
            let obs = make_observation(observer, subject, 10, tag, score);
            let _ = tracker.record_observation(&obs);
        }

        let top = tracker.get_top_tags(subject, Some(2));
        assert_eq!(top.len(), 2);
        assert_eq!(top.first().map(|e| e.tag), Some(ReputationTag::Scholar));
        assert_eq!(top.get(1).map(|e| e.tag), Some(ReputationTag::Generous));
    }

    // -----------------------------------------------------------------------
    // Perception summary string
    // -----------------------------------------------------------------------

    #[test]
    fn perception_summary_empty_when_no_tags() {
        let tracker = ReputationTracker::new();
        let subject = AgentId::new();
        assert!(tracker.reputation_summary_for_perception(subject).is_none());
    }

    #[test]
    fn perception_summary_filters_low_score() {
        let mut tracker = ReputationTracker::new();
        let observer = AgentId::new();
        let subject = AgentId::new();

        // Score is 0.2, below threshold of 0.3, and 2 pieces of evidence
        let obs1 = make_observation(observer, subject, 10, ReputationTag::Generous, 0.1);
        let obs2 = make_observation(observer, subject, 20, ReputationTag::Generous, 0.1);
        let _ = tracker.record_observation(&obs1);
        let _ = tracker.record_observation(&obs2);

        // Should be None because 0.2 < 0.3 threshold
        assert!(tracker.reputation_summary_for_perception(subject).is_none());
    }

    #[test]
    fn perception_summary_filters_low_evidence() {
        let mut tracker = ReputationTracker::new();
        let observer = AgentId::new();
        let subject = AgentId::new();

        // Good score but only 1 piece of evidence
        let obs = make_observation(observer, subject, 10, ReputationTag::Generous, 0.8);
        let _ = tracker.record_observation(&obs);

        // Should be None because evidence_count == 1 < 2
        assert!(tracker.reputation_summary_for_perception(subject).is_none());
    }

    #[test]
    fn perception_summary_includes_qualifying_tags() {
        let mut tracker = ReputationTracker::new();
        let observer = AgentId::new();
        let subject = AgentId::new();

        // Two observations for Generous (score 0.8, evidence 2) -- qualifies
        let obs1 = make_observation(observer, subject, 10, ReputationTag::Generous, 0.4);
        let obs2 = make_observation(observer, subject, 20, ReputationTag::Generous, 0.4);
        let _ = tracker.record_observation(&obs1);
        let _ = tracker.record_observation(&obs2);

        // Two observations for Scholar (score 0.6, evidence 2) -- qualifies
        let obs3 = make_observation(observer, subject, 10, ReputationTag::Scholar, 0.3);
        let obs4 = make_observation(observer, subject, 20, ReputationTag::Scholar, 0.3);
        let _ = tracker.record_observation(&obs3);
        let _ = tracker.record_observation(&obs4);

        let summary = tracker.reputation_summary_for_perception(subject);
        assert!(summary.is_some());
        let text = summary.unwrap_or_default();
        assert!(text.starts_with("Known as: "));
        assert!(text.contains("Generous"));
        assert!(text.contains("Scholar"));
    }

    // -----------------------------------------------------------------------
    // Visibility check (requires interaction)
    // -----------------------------------------------------------------------

    #[test]
    fn can_see_own_reputation() {
        let agent = AgentId::new();
        let graph = SocialGraph::new();
        assert!(ReputationTracker::can_see_reputation(agent, agent, &graph));
    }

    #[test]
    fn cannot_see_stranger_reputation() {
        let observer = AgentId::new();
        let subject = AgentId::new();
        let graph = SocialGraph::new();
        assert!(!ReputationTracker::can_see_reputation(observer, subject, &graph));
    }

    #[test]
    fn can_see_interacted_reputation() {
        let observer = AgentId::new();
        let subject = AgentId::new();
        let mut graph = SocialGraph::new();
        let _ = graph.update_relationship(
            subject,
            rust_decimal::Decimal::new(1, 1),
            10,
        );
        assert!(ReputationTracker::can_see_reputation(observer, subject, &graph));
    }

    // -----------------------------------------------------------------------
    // Decay over time
    // -----------------------------------------------------------------------

    #[test]
    fn decay_reduces_old_scores() {
        let mut tracker = ReputationTracker::new();
        let observer = AgentId::new();
        let subject = AgentId::new();

        // Observation at tick 10
        let obs = make_observation(observer, subject, 10, ReputationTag::Generous, 0.5);
        let _ = tracker.record_observation(&obs);

        // Decay at tick 200 with max_age 100 (entry at tick 10 is older)
        tracker.decay_reputation(200, 100, Some(0.1));

        let rep = tracker.get_reputation(observer, subject);
        assert!(!rep.is_empty());
        // Score should be 0.5 - 0.1 = 0.4
        assert!((rep.first().map(|e| e.score).unwrap_or(0.0) - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn decay_does_not_affect_recent() {
        let mut tracker = ReputationTracker::new();
        let observer = AgentId::new();
        let subject = AgentId::new();

        // Observation at tick 150 (recent)
        let obs = make_observation(observer, subject, 150, ReputationTag::Generous, 0.5);
        let _ = tracker.record_observation(&obs);

        // Decay at tick 200 with max_age 100 (threshold is tick 100, entry at 150 is newer)
        tracker.decay_reputation(200, 100, Some(0.1));

        let rep = tracker.get_reputation(observer, subject);
        assert!(!rep.is_empty());
        // Score should remain 0.5 (not decayed)
        assert!((rep.first().map(|e| e.score).unwrap_or(0.0) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn decay_removes_zero_score_entries() {
        let mut tracker = ReputationTracker::new();
        let observer = AgentId::new();
        let subject = AgentId::new();

        // Small score that will decay to zero
        let obs = make_observation(observer, subject, 10, ReputationTag::Generous, 0.05);
        let _ = tracker.record_observation(&obs);

        // Decay with factor 0.1 -> 0.05 - 0.1 = -0.05 -> removed
        tracker.decay_reputation(200, 100, Some(0.1));

        let rep = tracker.get_reputation(observer, subject);
        assert!(rep.is_empty());
    }

    // -----------------------------------------------------------------------
    // Sentiment calculation
    // -----------------------------------------------------------------------

    #[test]
    fn sentiment_positive_tags_only() {
        let entries = vec![
            ReputationEntry {
                tag: ReputationTag::Generous,
                score: 0.8,
                evidence_count: 5,
                last_updated_tick: 100,
            },
            ReputationEntry {
                tag: ReputationTag::Scholar,
                score: 0.6,
                evidence_count: 3,
                last_updated_tick: 90,
            },
        ];
        let sentiment = compute_sentiment(&entries);
        assert!((sentiment - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn sentiment_negative_tags_only() {
        let entries = vec![
            ReputationEntry {
                tag: ReputationTag::Thief,
                score: 0.7,
                evidence_count: 4,
                last_updated_tick: 100,
            },
            ReputationEntry {
                tag: ReputationTag::Murderer,
                score: 0.5,
                evidence_count: 2,
                last_updated_tick: 90,
            },
        ];
        let sentiment = compute_sentiment(&entries);
        assert!((sentiment - (-1.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn sentiment_mixed_tags() {
        let entries = vec![
            ReputationEntry {
                tag: ReputationTag::Generous,
                score: 0.5,
                evidence_count: 3,
                last_updated_tick: 100,
            },
            ReputationEntry {
                tag: ReputationTag::Thief,
                score: 0.5,
                evidence_count: 3,
                last_updated_tick: 100,
            },
        ];
        let sentiment = compute_sentiment(&entries);
        // Equal positive and negative with equal weight -> 0.0
        assert!(sentiment.abs() < f64::EPSILON);
    }

    #[test]
    fn sentiment_empty_entries() {
        let entries: Vec<ReputationEntry> = vec![];
        let sentiment = compute_sentiment(&entries);
        assert!(sentiment.abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Tag filtering by threshold
    // -----------------------------------------------------------------------

    #[test]
    fn get_agents_with_tag_filters_by_threshold() {
        let mut tracker = ReputationTracker::new();
        let observer = AgentId::new();
        let agent_a = AgentId::new();
        let agent_b = AgentId::new();
        let agent_c = AgentId::new();

        // Agent A: Generous with score 0.8
        let obs = make_observation(observer, agent_a, 10, ReputationTag::Generous, 0.8);
        let _ = tracker.record_observation(&obs);

        // Agent B: Generous with score 0.3
        let obs = make_observation(observer, agent_b, 10, ReputationTag::Generous, 0.3);
        let _ = tracker.record_observation(&obs);

        // Agent C: Generous with score 0.1
        let obs = make_observation(observer, agent_c, 10, ReputationTag::Generous, 0.1);
        let _ = tracker.record_observation(&obs);

        // Threshold 0.5 should only return agent A
        let results = tracker.get_agents_with_tag(ReputationTag::Generous, 0.5);
        assert_eq!(results.len(), 1);
        assert_eq!(results.first().map(|(id, _)| *id), Some(agent_a));

        // Threshold 0.0 should return A and B (> 0.0)
        let results = tracker.get_agents_with_tag(ReputationTag::Generous, 0.0);
        assert_eq!(results.len(), 3);
    }

    // -----------------------------------------------------------------------
    // Public reputation profile
    // -----------------------------------------------------------------------

    #[test]
    fn public_reputation_aggregates_multiple_observers() {
        let mut tracker = ReputationTracker::new();
        let obs_a = AgentId::new();
        let obs_b = AgentId::new();
        let obs_c = AgentId::new();
        let subject = AgentId::new();

        // Three observers with different scores for the same tag
        let o1 = make_observation(obs_a, subject, 10, ReputationTag::Leader, 0.9);
        let o2 = make_observation(obs_b, subject, 10, ReputationTag::Leader, 0.6);
        let o3 = make_observation(obs_c, subject, 10, ReputationTag::Leader, 0.3);
        let _ = tracker.record_observation(&o1);
        let _ = tracker.record_observation(&o2);
        let _ = tracker.record_observation(&o3);

        let profile = tracker.get_public_reputation(subject);
        assert_eq!(profile.agent_id, subject);
        assert_eq!(profile.tags.len(), 1);
        // Average: (0.9 + 0.6 + 0.3) / 3 = 0.6
        assert!(
            (profile.tags.first().map(|e| e.score).unwrap_or(0.0) - 0.6).abs() < f64::EPSILON
        );
        // Total evidence across all observers
        assert_eq!(profile.tags.first().map(|e| e.evidence_count), Some(3));
    }

    // -----------------------------------------------------------------------
    // Multiple tags per subject
    // -----------------------------------------------------------------------

    #[test]
    fn multiple_tags_tracked_independently() {
        let mut tracker = ReputationTracker::new();
        let observer = AgentId::new();
        let subject = AgentId::new();

        let obs1 = make_observation(observer, subject, 10, ReputationTag::Generous, 0.5);
        let obs2 = make_observation(observer, subject, 10, ReputationTag::Warrior, 0.3);
        let _ = tracker.record_observation(&obs1);
        let _ = tracker.record_observation(&obs2);

        let rep = tracker.get_reputation(observer, subject);
        assert_eq!(rep.len(), 2);
        // Should be sorted by score descending
        assert_eq!(rep.first().map(|e| e.tag), Some(ReputationTag::Generous));
        assert_eq!(rep.get(1).map(|e| e.tag), Some(ReputationTag::Warrior));
    }

    // -----------------------------------------------------------------------
    // Default trait
    // -----------------------------------------------------------------------

    #[test]
    fn default_tracker_is_empty() {
        let tracker = ReputationTracker::default();
        let agent = AgentId::new();
        let profile = tracker.get_public_reputation(agent);
        assert!(profile.tags.is_empty());
    }
}
