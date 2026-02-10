//! Persuasion mechanics for the Emergence simulation.
//!
//! Implements task 6.6.2: agents can attempt to change other agents' beliefs,
//! goals, or group allegiances. Success is influenced by the persuader's
//! honesty, relationship trust, reputation, target loyalty, commitment
//! duration, and shared cultural knowledge.
//!
//! # Scoring Model
//!
//! Each persuasion attempt produces a score out of 100 points:
//!
//! | Factor                     | Range         |
//! |----------------------------|---------------|
//! | Persuader honesty          | +0 to +20     |
//! | Relationship trust         | +0 to +30     |
//! | Persuader reputation       | +0 to +15     |
//! | Target loyalty (resist)    | -0 to -20     |
//! | Target commitment duration | -0 to -15     |
//! | Shared culture bonus       | +0 to +10     |
//!
//! Outcome thresholds:
//! - `>= 60` -> `Succeeded`
//! - `40..=59` -> `PartialSuccess`
//! - `< 40` -> `Failed`

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use emergence_types::AgentId;

use crate::error::AgentError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Score threshold for full success.
const SUCCESS_THRESHOLD: i64 = 60;

/// Score threshold for partial success (scores between PARTIAL and SUCCESS).
const PARTIAL_THRESHOLD: i64 = 40;

/// Maximum points from persuader honesty.
const MAX_HONESTY_POINTS: i64 = 20;

/// Maximum points from relationship trust.
const MAX_TRUST_POINTS: i64 = 30;

/// Maximum points from persuader reputation.
const MAX_REPUTATION_POINTS: i64 = 15;

/// Maximum penalty from target loyalty.
const MAX_LOYALTY_PENALTY: i64 = 20;

/// Maximum penalty from commitment duration.
const MAX_COMMITMENT_PENALTY: i64 = 15;

/// Maximum bonus from shared culture.
const MAX_CULTURE_BONUS: i64 = 10;

/// Number of ticks at which commitment penalty reaches its maximum.
const MAX_COMMITMENT_TICKS: u64 = 500;

// ---------------------------------------------------------------------------
// PersuasionType
// ---------------------------------------------------------------------------

/// The kind of persuasion being attempted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PersuasionType {
    /// Shift the target's view on a topic.
    ChangeOpinion {
        /// The topic under discussion.
        topic: String,
    },
    /// Recruit the target to join a social construct / group.
    JoinGroup {
        /// The group to join.
        group_id: Uuid,
    },
    /// Convince the target to leave a group.
    LeaveGroup {
        /// The group to leave.
        group_id: Uuid,
    },
    /// Switch the target's loyalty from one group to another.
    ChangeAllegiance {
        /// The group the target currently belongs to (if any).
        from: Option<Uuid>,
        /// The group the target should join.
        to: Uuid,
    },
    /// Convince the target to adopt a belief or cultural knowledge.
    AdoptBelief {
        /// The belief to adopt.
        belief: String,
    },
    /// Convince the target to abandon a belief.
    AbandonBelief {
        /// The belief to abandon.
        belief: String,
    },
}

// ---------------------------------------------------------------------------
// PersuasionAttempt
// ---------------------------------------------------------------------------

/// A single persuasion attempt submitted during the resolution phase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersuasionAttempt {
    /// The agent doing the persuading.
    pub persuader: AgentId,
    /// The agent being persuaded.
    pub target: AgentId,
    /// The tick when the attempt was made.
    pub tick: u64,
    /// What kind of persuasion is being attempted.
    pub persuasion_type: PersuasionType,
    /// The actual pitch / argument text.
    pub argument: String,
}

// ---------------------------------------------------------------------------
// PersuasionResult
// ---------------------------------------------------------------------------

/// The outcome of a persuasion attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PersuasionResult {
    /// The persuasion fully succeeded.
    Succeeded {
        /// The computed influence score (0-100).
        influence_score: Decimal,
    },
    /// The persuasion completely failed.
    Failed {
        /// The computed resistance score (how far below the threshold).
        resistance_score: Decimal,
    },
    /// The target shifted somewhat but did not fully convert.
    PartialSuccess {
        /// The computed influence score.
        influence_score: Decimal,
        /// Human-readable description of the partial shift.
        description: String,
    },
}

// ---------------------------------------------------------------------------
// PersuasionRecord
// ---------------------------------------------------------------------------

/// A recorded persuasion attempt and its outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersuasionRecord {
    /// The attempt that was made.
    pub attempt: PersuasionAttempt,
    /// The result of the attempt.
    pub result: PersuasionResult,
}

// ---------------------------------------------------------------------------
// PersuasionContext
// ---------------------------------------------------------------------------

/// All factors needed to evaluate a persuasion attempt.
///
/// Collected from the world state before evaluation. All `Decimal` values
/// are in the range 0.0 to 1.0 unless otherwise noted.
pub struct PersuasionContext {
    /// Persuader's honesty trait (0.0 to 1.0).
    pub persuader_honesty: Decimal,
    /// Relationship trust score between persuader and target (-1.0 to 1.0).
    pub relationship_trust: Decimal,
    /// Persuader's reputation score (0.0 to 1.0, where 0.5 is neutral).
    pub persuader_reputation: Decimal,
    /// Target's loyalty trait (0.0 to 1.0).
    pub target_loyalty: Decimal,
    /// How many ticks the target has held their current belief / allegiance.
    pub target_commitment_ticks: u64,
    /// Number of cultural knowledge items shared between persuader and target.
    pub shared_culture_count: u32,
}

// ---------------------------------------------------------------------------
// PersuasionEvaluator
// ---------------------------------------------------------------------------

/// Evaluates and records persuasion attempts.
///
/// Maintains a history of all persuasion attempts for analytics and
/// agent perception.
#[derive(Debug, Clone)]
pub struct PersuasionEvaluator {
    /// All recorded persuasion attempts, keyed by a unique ID.
    records: BTreeMap<Uuid, PersuasionRecord>,
}

impl PersuasionEvaluator {
    /// Create a new empty evaluator.
    pub const fn new() -> Self {
        Self {
            records: BTreeMap::new(),
        }
    }

    /// Evaluate a persuasion attempt and return the result.
    ///
    /// Does NOT record the attempt -- call [`record_attempt`] afterwards.
    ///
    /// [`record_attempt`]: Self::record_attempt
    pub fn evaluate_persuasion(
        &self,
        context: &PersuasionContext,
    ) -> Result<PersuasionResult, AgentError> {
        let mut score: i64 = 0;

        // 1. Persuader honesty: 0.0 to 1.0 maps to 0 to 20 points.
        let honesty_points = compute_scaled_points(
            context.persuader_honesty,
            MAX_HONESTY_POINTS,
        )?;
        score = score.checked_add(honesty_points).ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: String::from("persuasion honesty score overflow"),
            }
        })?;

        // 2. Relationship trust: -1.0 to 1.0 maps to 0 to 30 points.
        //    We normalize from [-1, 1] to [0, 1] first: (trust + 1) / 2.
        let trust_normalized = context
            .relationship_trust
            .checked_add(Decimal::ONE)
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("persuasion trust normalization overflow"),
            })?
            .checked_div(Decimal::from(2))
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("persuasion trust normalization division overflow"),
            })?;
        let trust_clamped = clamp_unit(trust_normalized);
        let trust_points = compute_scaled_points(trust_clamped, MAX_TRUST_POINTS)?;
        score = score.checked_add(trust_points).ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: String::from("persuasion trust score overflow"),
            }
        })?;

        // 3. Persuader reputation: 0.0 to 1.0 maps to 0 to 15 points.
        let reputation_points = compute_scaled_points(
            clamp_unit(context.persuader_reputation),
            MAX_REPUTATION_POINTS,
        )?;
        score = score.checked_add(reputation_points).ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: String::from("persuasion reputation score overflow"),
            }
        })?;

        // 4. Target loyalty: 0.0 to 1.0 maps to 0 to -20 penalty.
        //    Only applies to allegiance-related persuasion, but for simplicity
        //    we always compute it (loyalty resists all change).
        let loyalty_penalty = compute_scaled_points(
            clamp_unit(context.target_loyalty),
            MAX_LOYALTY_PENALTY,
        )?;
        score = score.checked_sub(loyalty_penalty).ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: String::from("persuasion loyalty penalty overflow"),
            }
        })?;

        // 5. Commitment duration: scales linearly from 0 to MAX_COMMITMENT_PENALTY
        //    over MAX_COMMITMENT_TICKS.
        let commitment_ratio = if context.target_commitment_ticks >= MAX_COMMITMENT_TICKS {
            Decimal::ONE
        } else {
            Decimal::from(context.target_commitment_ticks)
                .checked_div(Decimal::from(MAX_COMMITMENT_TICKS))
                .ok_or_else(|| AgentError::ArithmeticOverflow {
                    context: String::from("persuasion commitment ratio overflow"),
                })?
        };
        let commitment_penalty = compute_scaled_points(
            clamp_unit(commitment_ratio),
            MAX_COMMITMENT_PENALTY,
        )?;
        score = score.checked_sub(commitment_penalty).ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: String::from("persuasion commitment penalty overflow"),
            }
        })?;

        // 6. Shared culture bonus: each shared item adds points, capped.
        let culture_points_raw = i64::from(context.shared_culture_count)
            .checked_mul(2)
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("persuasion culture bonus overflow"),
            })?;
        let culture_points = culture_points_raw.min(MAX_CULTURE_BONUS);
        score = score.checked_add(culture_points).ok_or_else(|| {
            AgentError::ArithmeticOverflow {
                context: String::from("persuasion culture score overflow"),
            }
        })?;

        // Clamp score to 0..=100 range for clean output.
        let clamped_score = score.clamp(0, 100);
        let score_decimal = Decimal::from(clamped_score);

        if clamped_score >= SUCCESS_THRESHOLD {
            Ok(PersuasionResult::Succeeded {
                influence_score: score_decimal,
            })
        } else if clamped_score >= PARTIAL_THRESHOLD {
            Ok(PersuasionResult::PartialSuccess {
                influence_score: score_decimal,
                description: String::from(
                    "The target's view shifted but they were not fully persuaded",
                ),
            })
        } else {
            // Resistance is how far below the partial threshold the score is.
            let resistance = Decimal::from(
                PARTIAL_THRESHOLD.checked_sub(clamped_score).unwrap_or(0),
            );
            Ok(PersuasionResult::Failed {
                resistance_score: resistance,
            })
        }
    }

    /// Record a completed persuasion attempt and result.
    pub fn record_attempt(
        &mut self,
        attempt: PersuasionAttempt,
        result: PersuasionResult,
    ) -> Uuid {
        let id = Uuid::now_v7();
        let record = PersuasionRecord { attempt, result };
        self.records.insert(id, record);
        id
    }

    /// Get the full persuasion history for an agent (as persuader or target).
    pub fn get_persuasion_history(&self, agent_id: AgentId) -> Vec<&PersuasionRecord> {
        self.records
            .values()
            .filter(|r| {
                r.attempt.persuader == agent_id || r.attempt.target == agent_id
            })
            .collect()
    }

    /// Calculate the success rate for a given agent as persuader.
    ///
    /// Returns `None` if the agent has made no persuasion attempts.
    /// Returns the ratio of successful attempts (including partial) to total.
    pub fn persuasion_success_rate(&self, agent_id: AgentId) -> Option<Decimal> {
        let attempts: Vec<&PersuasionRecord> = self
            .records
            .values()
            .filter(|r| r.attempt.persuader == agent_id)
            .collect();

        let total = attempts.len();
        if total == 0 {
            return None;
        }

        let successes = attempts
            .iter()
            .filter(|r| matches!(r.result, PersuasionResult::Succeeded { .. }))
            .count();

        Decimal::from(successes)
            .checked_div(Decimal::from(total))
    }

    /// Get agents ranked by persuasion success rate (descending).
    ///
    /// Only includes agents with at least `min_attempts` attempts.
    pub fn most_persuasive_agents(&self, min_attempts: usize) -> Vec<(AgentId, Decimal)> {
        // Collect per-agent attempt counts and success counts.
        let mut agent_stats: BTreeMap<AgentId, (usize, usize)> = BTreeMap::new();

        for record in self.records.values() {
            let entry = agent_stats
                .entry(record.attempt.persuader)
                .or_insert((0, 0));
            entry.0 = entry.0.saturating_add(1);
            if matches!(record.result, PersuasionResult::Succeeded { .. }) {
                entry.1 = entry.1.saturating_add(1);
            }
        }

        let mut ranked: Vec<(AgentId, Decimal)> = agent_stats
            .into_iter()
            .filter(|(_id, (total, _successes))| *total >= min_attempts)
            .filter_map(|(id, (total, successes))| {
                let rate = Decimal::from(successes)
                    .checked_div(Decimal::from(total))?;
                Some((id, rate))
            })
            .collect();

        // Sort descending by rate.
        ranked.sort_by(|a, b| b.1.cmp(&a.1));
        ranked
    }

    /// Get agents ranked by resistance rate (how often they resist persuasion), descending.
    ///
    /// Only includes agents with at least `min_attempts` attempts against them.
    pub fn most_resistant_agents(&self, min_attempts: usize) -> Vec<(AgentId, Decimal)> {
        let mut agent_stats: BTreeMap<AgentId, (usize, usize)> = BTreeMap::new();

        for record in self.records.values() {
            let entry = agent_stats
                .entry(record.attempt.target)
                .or_insert((0, 0));
            entry.0 = entry.0.saturating_add(1);
            if matches!(record.result, PersuasionResult::Failed { .. }) {
                entry.1 = entry.1.saturating_add(1);
            }
        }

        let mut ranked: Vec<(AgentId, Decimal)> = agent_stats
            .into_iter()
            .filter(|(_id, (total, _failures))| *total >= min_attempts)
            .filter_map(|(id, (total, failures))| {
                let rate = Decimal::from(failures)
                    .checked_div(Decimal::from(total))?;
                Some((id, rate))
            })
            .collect();

        ranked.sort_by(|a, b| b.1.cmp(&a.1));
        ranked
    }

    /// Get successful persuasions in the last N ticks from `current_tick`.
    pub fn get_recent_conversions(
        &self,
        current_tick: u64,
        lookback_ticks: u64,
    ) -> Vec<&PersuasionRecord> {
        let cutoff = current_tick.saturating_sub(lookback_ticks);
        self.records
            .values()
            .filter(|r| {
                r.attempt.tick >= cutoff
                    && matches!(r.result, PersuasionResult::Succeeded { .. })
            })
            .collect()
    }

    /// Total number of recorded persuasion attempts.
    pub fn total_attempts(&self) -> usize {
        self.records.len()
    }
}

impl Default for PersuasionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Scale a 0.0-1.0 `Decimal` value to `0..max_points` (integer).
fn compute_scaled_points(value: Decimal, max_points: i64) -> Result<i64, AgentError> {
    let clamped = clamp_unit(value);
    let scaled = clamped
        .checked_mul(Decimal::from(max_points))
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("persuasion scaling overflow"),
        })?;
    // Truncate to integer (floor towards zero).
    let truncated = scaled.trunc();

    // Convert Decimal integer to i64.
    // The mantissa of a truncated Decimal N (with scale s) = N * 10^s.
    let scale = truncated.scale();
    let mantissa = truncated.mantissa();
    let divisor: i128 = 10_i128.checked_pow(scale).unwrap_or(1);
    let integer_val = mantissa.checked_div(divisor).unwrap_or(0);

    i64::try_from(integer_val).map_err(|_overflow| AgentError::ArithmeticOverflow {
        context: String::from("persuasion point conversion overflow"),
    })
}

/// Clamp a `Decimal` to the 0.0-1.0 range.
fn clamp_unit(value: Decimal) -> Decimal {
    if value < Decimal::ZERO {
        Decimal::ZERO
    } else if value > Decimal::ONE {
        Decimal::ONE
    } else {
        value
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use emergence_types::AgentId;

    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn default_context() -> PersuasionContext {
        PersuasionContext {
            persuader_honesty: Decimal::new(5, 1),   // 0.5
            relationship_trust: Decimal::ZERO,        // neutral
            persuader_reputation: Decimal::new(5, 1), // 0.5
            target_loyalty: Decimal::new(5, 1),       // 0.5
            target_commitment_ticks: 0,
            shared_culture_count: 0,
        }
    }

    fn make_attempt(persuader: AgentId, target: AgentId, tick: u64) -> PersuasionAttempt {
        PersuasionAttempt {
            persuader,
            target,
            tick,
            persuasion_type: PersuasionType::ChangeOpinion {
                topic: String::from("resource sharing"),
            },
            argument: String::from("We should share resources for mutual benefit"),
        }
    }

    // -----------------------------------------------------------------------
    // 1. High trust + honesty succeeds
    // -----------------------------------------------------------------------

    #[test]
    fn high_trust_and_honesty_succeeds() {
        let evaluator = PersuasionEvaluator::new();
        let ctx = PersuasionContext {
            persuader_honesty: Decimal::ONE,       // +20
            relationship_trust: Decimal::ONE,      // +30
            persuader_reputation: Decimal::ONE,    // +15
            target_loyalty: Decimal::ZERO,         // -0
            target_commitment_ticks: 0,            // -0
            shared_culture_count: 5,               // +10
        };
        // Total: 20 + 30 + 15 + 0 + 0 + 10 = 75

        let result = evaluator.evaluate_persuasion(&ctx);
        assert!(result.is_ok());
        match result.ok() {
            Some(PersuasionResult::Succeeded { influence_score }) => {
                assert_eq!(influence_score, Decimal::from(75));
            }
            other => {
                // Force test failure with a descriptive message
                assert!(false, "Expected Succeeded, got {other:?}");
            }
        }
    }

    // -----------------------------------------------------------------------
    // 2. Low trust fails
    // -----------------------------------------------------------------------

    #[test]
    fn low_trust_fails() {
        let evaluator = PersuasionEvaluator::new();
        let ctx = PersuasionContext {
            persuader_honesty: Decimal::ZERO,              // +0
            relationship_trust: Decimal::NEGATIVE_ONE,     // +0 (normalized to 0)
            persuader_reputation: Decimal::ZERO,           // +0
            target_loyalty: Decimal::ONE,                  // -20
            target_commitment_ticks: 500,                  // -15
            shared_culture_count: 0,                       // +0
        };
        // Total: 0 + 0 + 0 - 20 - 15 + 0 = -35, clamped to 0

        let result = evaluator.evaluate_persuasion(&ctx);
        assert!(result.is_ok());
        match result.ok() {
            Some(PersuasionResult::Failed { resistance_score }) => {
                // Score is 0, resistance = 40 - 0 = 40
                assert_eq!(resistance_score, Decimal::from(40));
            }
            other => {
                assert!(false, "Expected Failed, got {other:?}");
            }
        }
    }

    // -----------------------------------------------------------------------
    // 3. Loyalty resists allegiance change
    // -----------------------------------------------------------------------

    #[test]
    fn loyalty_resists_allegiance_change() {
        let evaluator = PersuasionEvaluator::new();
        // Moderate persuader, but target has max loyalty.
        let ctx = PersuasionContext {
            persuader_honesty: Decimal::new(7, 1),     // +14
            relationship_trust: Decimal::new(5, 1),    // trust 0.5 -> norm 0.75 -> +22
            persuader_reputation: Decimal::new(6, 1),  // +9
            target_loyalty: Decimal::ONE,              // -20
            target_commitment_ticks: 300,              // -9
            shared_culture_count: 0,                   // +0
        };
        // Total: 14 + 22 + 9 - 20 - 9 + 0 = 16 -> Failed

        let result = evaluator.evaluate_persuasion(&ctx);
        assert!(result.is_ok());
        assert!(matches!(
            result.ok(),
            Some(PersuasionResult::Failed { .. })
        ));
    }

    // -----------------------------------------------------------------------
    // 4. Partial success
    // -----------------------------------------------------------------------

    #[test]
    fn moderate_factors_partial_success() {
        let evaluator = PersuasionEvaluator::new();
        let ctx = PersuasionContext {
            persuader_honesty: Decimal::ONE,           // +20
            relationship_trust: Decimal::new(3, 1),    // trust 0.3 -> norm 0.65 -> +19
            persuader_reputation: Decimal::new(5, 1),  // +7
            target_loyalty: Decimal::new(3, 1),        // -6
            target_commitment_ticks: 0,                // -0
            shared_culture_count: 0,                   // +0
        };
        // Total: 20 + 19 + 7 - 6 - 0 + 0 = 40 -> PartialSuccess (>= 40, < 60)

        let result = evaluator.evaluate_persuasion(&ctx);
        assert!(result.is_ok());
        assert!(matches!(
            result.ok(),
            Some(PersuasionResult::PartialSuccess { .. })
        ));
    }

    // -----------------------------------------------------------------------
    // 5. Shared culture bonus
    // -----------------------------------------------------------------------

    #[test]
    fn shared_culture_adds_bonus() {
        let evaluator = PersuasionEvaluator::new();

        // Without culture.
        let ctx_no_culture = PersuasionContext {
            persuader_honesty: Decimal::new(8, 1),     // +16
            relationship_trust: Decimal::new(4, 1),    // trust 0.4 -> norm 0.7 -> +21
            persuader_reputation: Decimal::new(5, 1),  // +7
            target_loyalty: Decimal::new(2, 1),        // -4
            target_commitment_ticks: 0,                // -0
            shared_culture_count: 0,                   // +0
        };
        // Total: 16 + 21 + 7 - 4 = 40

        let result_no = evaluator.evaluate_persuasion(&ctx_no_culture);
        assert!(result_no.is_ok());

        // With shared culture (5 items * 2 = 10 points).
        let ctx_with_culture = PersuasionContext {
            shared_culture_count: 5,
            ..ctx_no_culture
        };
        // Total: 16 + 21 + 7 - 4 + 10 = 50

        let result_with = evaluator.evaluate_persuasion(&ctx_with_culture);
        assert!(result_with.is_ok());

        // The score with culture should be higher.
        let score_no = match result_no.ok() {
            Some(PersuasionResult::PartialSuccess { influence_score, .. }) => influence_score,
            Some(PersuasionResult::Failed { .. }) => Decimal::ZERO,
            _ => Decimal::ZERO,
        };
        let score_with = match result_with.ok() {
            Some(PersuasionResult::PartialSuccess { influence_score, .. }) => influence_score,
            Some(PersuasionResult::Succeeded { influence_score }) => influence_score,
            _ => Decimal::ZERO,
        };
        assert!(score_with > score_no);
    }

    // -----------------------------------------------------------------------
    // 6. Success rate calculation
    // -----------------------------------------------------------------------

    #[test]
    fn success_rate_calculation() {
        let mut evaluator = PersuasionEvaluator::new();
        let persuader = AgentId::new();
        let target = AgentId::new();

        // Record 2 successes and 1 failure.
        evaluator.record_attempt(
            make_attempt(persuader, target, 1),
            PersuasionResult::Succeeded {
                influence_score: Decimal::from(70),
            },
        );
        evaluator.record_attempt(
            make_attempt(persuader, target, 2),
            PersuasionResult::Succeeded {
                influence_score: Decimal::from(65),
            },
        );
        evaluator.record_attempt(
            make_attempt(persuader, target, 3),
            PersuasionResult::Failed {
                resistance_score: Decimal::from(10),
            },
        );

        let rate = evaluator.persuasion_success_rate(persuader);
        assert!(rate.is_some());
        // 2 / 3 = 0.666...
        let r = rate.unwrap_or(Decimal::ZERO);
        // Check it is approximately 0.67.
        assert!(r > Decimal::new(66, 2));
        assert!(r < Decimal::new(67, 2));
    }

    #[test]
    fn success_rate_no_attempts_returns_none() {
        let evaluator = PersuasionEvaluator::new();
        let agent = AgentId::new();
        assert!(evaluator.persuasion_success_rate(agent).is_none());
    }

    // -----------------------------------------------------------------------
    // 7. Most persuasive ranking
    // -----------------------------------------------------------------------

    #[test]
    fn most_persuasive_agents_ranking() {
        let mut evaluator = PersuasionEvaluator::new();
        let agent_a = AgentId::new();
        let agent_b = AgentId::new();
        let target = AgentId::new();

        // Agent A: 3 attempts, 3 successes (100%).
        for tick in 1..=3_u64 {
            evaluator.record_attempt(
                make_attempt(agent_a, target, tick),
                PersuasionResult::Succeeded {
                    influence_score: Decimal::from(70),
                },
            );
        }

        // Agent B: 3 attempts, 1 success (33%).
        evaluator.record_attempt(
            make_attempt(agent_b, target, 4),
            PersuasionResult::Succeeded {
                influence_score: Decimal::from(60),
            },
        );
        evaluator.record_attempt(
            make_attempt(agent_b, target, 5),
            PersuasionResult::Failed {
                resistance_score: Decimal::from(20),
            },
        );
        evaluator.record_attempt(
            make_attempt(agent_b, target, 6),
            PersuasionResult::Failed {
                resistance_score: Decimal::from(15),
            },
        );

        let ranked = evaluator.most_persuasive_agents(3);
        assert_eq!(ranked.len(), 2);
        // Agent A should be first (100% > 33%).
        assert_eq!(ranked.first().map(|(id, _)| *id), Some(agent_a));
        assert_eq!(ranked.get(1).map(|(id, _)| *id), Some(agent_b));
    }

    #[test]
    fn most_persuasive_agents_respects_min_attempts() {
        let mut evaluator = PersuasionEvaluator::new();
        let agent = AgentId::new();
        let target = AgentId::new();

        // Only 2 attempts, minimum is 3.
        evaluator.record_attempt(
            make_attempt(agent, target, 1),
            PersuasionResult::Succeeded {
                influence_score: Decimal::from(70),
            },
        );
        evaluator.record_attempt(
            make_attempt(agent, target, 2),
            PersuasionResult::Succeeded {
                influence_score: Decimal::from(70),
            },
        );

        let ranked = evaluator.most_persuasive_agents(3);
        assert!(ranked.is_empty());
    }

    // -----------------------------------------------------------------------
    // 8. Commitment duration penalty
    // -----------------------------------------------------------------------

    #[test]
    fn commitment_duration_increases_penalty() {
        let evaluator = PersuasionEvaluator::new();

        // No commitment.
        let ctx_fresh = PersuasionContext {
            persuader_honesty: Decimal::new(5, 1),
            relationship_trust: Decimal::new(5, 1),
            persuader_reputation: Decimal::new(5, 1),
            target_loyalty: Decimal::ZERO,
            target_commitment_ticks: 0,
            shared_culture_count: 0,
        };

        // Max commitment.
        let ctx_committed = PersuasionContext {
            target_commitment_ticks: 500,
            ..ctx_fresh
        };

        let result_fresh = evaluator.evaluate_persuasion(&ctx_fresh);
        let result_committed = evaluator.evaluate_persuasion(&ctx_committed);
        assert!(result_fresh.is_ok());
        assert!(result_committed.is_ok());

        let score_fresh = extract_score(&result_fresh.ok());
        let score_committed = extract_score(&result_committed.ok());

        // Committed agent should have a lower score.
        assert!(score_fresh > score_committed);
    }

    // -----------------------------------------------------------------------
    // 9. Reputation bonus
    // -----------------------------------------------------------------------

    #[test]
    fn reputation_bonus_improves_score() {
        let evaluator = PersuasionEvaluator::new();

        let ctx_no_rep = PersuasionContext {
            persuader_honesty: Decimal::new(5, 1),
            relationship_trust: Decimal::ZERO,
            persuader_reputation: Decimal::ZERO,
            target_loyalty: Decimal::ZERO,
            target_commitment_ticks: 0,
            shared_culture_count: 0,
        };

        let ctx_high_rep = PersuasionContext {
            persuader_reputation: Decimal::ONE,
            ..ctx_no_rep
        };

        let result_no = evaluator.evaluate_persuasion(&ctx_no_rep);
        let result_high = evaluator.evaluate_persuasion(&ctx_high_rep);
        assert!(result_no.is_ok());
        assert!(result_high.is_ok());

        let score_no = extract_score(&result_no.ok());
        let score_high = extract_score(&result_high.ok());

        assert!(score_high > score_no);
    }

    // -----------------------------------------------------------------------
    // 10. Most resistant agents
    // -----------------------------------------------------------------------

    #[test]
    fn most_resistant_agents_ranking() {
        let mut evaluator = PersuasionEvaluator::new();
        let persuader = AgentId::new();
        let tough = AgentId::new();
        let weak = AgentId::new();

        // Tough resists 3/3.
        for tick in 1..=3_u64 {
            evaluator.record_attempt(
                make_attempt(persuader, tough, tick),
                PersuasionResult::Failed {
                    resistance_score: Decimal::from(30),
                },
            );
        }

        // Weak resists 1/3.
        evaluator.record_attempt(
            make_attempt(persuader, weak, 4),
            PersuasionResult::Failed {
                resistance_score: Decimal::from(10),
            },
        );
        evaluator.record_attempt(
            make_attempt(persuader, weak, 5),
            PersuasionResult::Succeeded {
                influence_score: Decimal::from(60),
            },
        );
        evaluator.record_attempt(
            make_attempt(persuader, weak, 6),
            PersuasionResult::Succeeded {
                influence_score: Decimal::from(65),
            },
        );

        let ranked = evaluator.most_resistant_agents(3);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked.first().map(|(id, _)| *id), Some(tough));
    }

    // -----------------------------------------------------------------------
    // 11. Get recent conversions
    // -----------------------------------------------------------------------

    #[test]
    fn get_recent_conversions_filters_by_tick() {
        let mut evaluator = PersuasionEvaluator::new();
        let persuader = AgentId::new();
        let target = AgentId::new();

        // Old conversion at tick 5.
        evaluator.record_attempt(
            make_attempt(persuader, target, 5),
            PersuasionResult::Succeeded {
                influence_score: Decimal::from(70),
            },
        );

        // Recent conversion at tick 95.
        evaluator.record_attempt(
            make_attempt(persuader, target, 95),
            PersuasionResult::Succeeded {
                influence_score: Decimal::from(65),
            },
        );

        // Recent failure at tick 96 (should not appear).
        evaluator.record_attempt(
            make_attempt(persuader, target, 96),
            PersuasionResult::Failed {
                resistance_score: Decimal::from(20),
            },
        );

        // Get conversions in last 10 ticks from tick 100.
        let recent = evaluator.get_recent_conversions(100, 10);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent.first().map(|r| r.attempt.tick), Some(95));
    }

    // -----------------------------------------------------------------------
    // 12. Persuasion history query
    // -----------------------------------------------------------------------

    #[test]
    fn persuasion_history_includes_both_roles() {
        let mut evaluator = PersuasionEvaluator::new();
        let agent_a = AgentId::new();
        let agent_b = AgentId::new();
        let agent_c = AgentId::new();

        // A persuades B.
        evaluator.record_attempt(
            make_attempt(agent_a, agent_b, 1),
            PersuasionResult::Succeeded {
                influence_score: Decimal::from(70),
            },
        );

        // C persuades A.
        evaluator.record_attempt(
            make_attempt(agent_c, agent_a, 2),
            PersuasionResult::Failed {
                resistance_score: Decimal::from(20),
            },
        );

        // B persuades C (A not involved).
        evaluator.record_attempt(
            make_attempt(agent_b, agent_c, 3),
            PersuasionResult::Succeeded {
                influence_score: Decimal::from(60),
            },
        );

        // A should see 2 records (persuader in first, target in second).
        let history = evaluator.get_persuasion_history(agent_a);
        assert_eq!(history.len(), 2);
    }

    // -----------------------------------------------------------------------
    // 13. Boundary: all max positive factors
    // -----------------------------------------------------------------------

    #[test]
    fn max_factors_produce_max_score() {
        let evaluator = PersuasionEvaluator::new();
        let ctx = PersuasionContext {
            persuader_honesty: Decimal::ONE,
            relationship_trust: Decimal::ONE,
            persuader_reputation: Decimal::ONE,
            target_loyalty: Decimal::ZERO,
            target_commitment_ticks: 0,
            shared_culture_count: 10,
        };
        // 20 + 30 + 15 + 0 + 0 + 10 = 75

        let result = evaluator.evaluate_persuasion(&ctx);
        assert!(result.is_ok());
        let score = extract_score(&result.ok());
        assert_eq!(score, Decimal::from(75));
    }

    // -----------------------------------------------------------------------
    // 14. Boundary: all max negative factors
    // -----------------------------------------------------------------------

    #[test]
    fn min_factors_produce_zero_clamped_score() {
        let evaluator = PersuasionEvaluator::new();
        let ctx = PersuasionContext {
            persuader_honesty: Decimal::ZERO,
            relationship_trust: Decimal::NEGATIVE_ONE,
            persuader_reputation: Decimal::ZERO,
            target_loyalty: Decimal::ONE,
            target_commitment_ticks: 1000,
            shared_culture_count: 0,
        };
        // 0 + 0 + 0 - 20 - 15 + 0 = -35, clamped to 0

        let result = evaluator.evaluate_persuasion(&ctx);
        assert!(result.is_ok());
        let score = extract_score(&result.ok());
        assert_eq!(score, Decimal::ZERO);
    }

    // -----------------------------------------------------------------------
    // Helper to extract score from result
    // -----------------------------------------------------------------------

    fn extract_score(result: &Option<PersuasionResult>) -> Decimal {
        match result {
            Some(PersuasionResult::Succeeded { influence_score }) => *influence_score,
            Some(PersuasionResult::PartialSuccess {
                influence_score, ..
            }) => *influence_score,
            // For Failed results, recover the original score from the resistance.
            // resistance_score = PARTIAL_THRESHOLD - clamped_score, so
            // clamped_score = PARTIAL_THRESHOLD - resistance_score.
            Some(PersuasionResult::Failed { resistance_score }) => {
                Decimal::from(40i64).saturating_sub(*resistance_score)
            }
            None => Decimal::ZERO,
        }
    }

    // -----------------------------------------------------------------------
    // 15. Empty evaluator defaults
    // -----------------------------------------------------------------------

    #[test]
    fn empty_evaluator_has_zero_attempts() {
        let evaluator = PersuasionEvaluator::new();
        assert_eq!(evaluator.total_attempts(), 0);
        assert!(evaluator.most_persuasive_agents(1).is_empty());
        assert!(evaluator.most_resistant_agents(1).is_empty());
    }
}
