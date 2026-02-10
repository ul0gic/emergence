//! Theft mechanics for resource stealing between co-located agents.
//!
//! Implements task 6.3.3 from the build plan: agents can attempt to steal
//! resources from other agents at the same location. Success depends on the
//! thief's aggression, risk tolerance, and the victim's alertness.
//!
//! The resolution flow:
//! 1. Validate prerequisites (co-location, victim has resource)
//! 2. Compute success probability from traits and victim state
//! 3. Roll for success
//! 4. On success: transfer resources via ledger, emit `TheftOccurred` event
//! 5. On failure: roll for detection, emit `TheftFailed` event
//! 6. Apply energy cost to thief regardless of outcome

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use emergence_types::{
    AgentId, AgentState, LocationId, Personality, Resource, TheftFailedDetails,
    TheftFailureReason, TheftOccurredDetails,
};

use crate::error::AgentError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Energy cost of a theft attempt, regardless of outcome.
pub const THEFT_ENERGY_COST: u32 = 15;

/// Base success probability for a theft attempt (40 out of 100).
const BASE_SUCCESS_RATE: u32 = 40;

/// Probability that the victim detects a failed theft (70 out of 100).
const DETECTION_RATE: u32 = 70;

/// Maximum quantity stolen per attempt.
const MAX_STEAL_QUANTITY: u32 = 5;

/// Relationship damage applied when a detected theft fails (-0.5).
fn relationship_damage() -> Decimal {
    Decimal::new(-5, 1)
}

// ---------------------------------------------------------------------------
// TheftAttempt
// ---------------------------------------------------------------------------

/// A fully validated theft attempt ready for resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheftAttempt {
    /// The agent attempting the theft.
    pub thief_id: AgentId,
    /// The victim agent.
    pub victim_id: AgentId,
    /// The resource being targeted.
    pub target_resource: Resource,
    /// The location where the attempt occurs.
    pub location_id: LocationId,
}

// ---------------------------------------------------------------------------
// TheftResult
// ---------------------------------------------------------------------------

/// The outcome of a theft attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TheftResult {
    /// The theft succeeded and resources were transferred.
    Success {
        /// Event details for the successful theft.
        details: TheftOccurredDetails,
        /// Quantity actually stolen.
        quantity_stolen: u32,
    },
    /// The theft failed.
    Failed {
        /// Event details for the failed theft.
        details: TheftFailedDetails,
    },
}

// ---------------------------------------------------------------------------
// TheftContext
// ---------------------------------------------------------------------------

/// World state needed to resolve a theft attempt.
///
/// Assembled by the tick cycle and passed into [`resolve_theft`].
#[derive(Debug, Clone)]
pub struct TheftContext {
    /// The thief's personality traits.
    pub thief_personality: Personality,
    /// The thief's energy level before the attempt.
    pub thief_energy: u32,
    /// The victim's energy level (determines alertness).
    pub victim_energy: u32,
    /// Whether the victim is currently resting.
    pub victim_is_resting: bool,
    /// The thief's relationship score with the victim (-1.0 to 1.0).
    pub relationship_score: Decimal,
    /// How much of the target resource the victim holds.
    pub victim_resource_quantity: u32,
    /// Random roll for success (0--99 inclusive).
    pub success_roll: u32,
    /// Random roll for detection on failure (0--99 inclusive).
    pub detection_roll: u32,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Validate the prerequisites for a theft attempt.
///
/// Returns a [`TheftAttempt`] if valid, or a [`TheftResult::Failed`] with
/// the appropriate failure reason.
pub fn validate_theft(
    thief_state: &AgentState,
    victim_state: &AgentState,
    target_resource: Resource,
    agents_at_location: &[AgentId],
) -> Result<TheftAttempt, TheftResult> {
    // Victim must be at the same location
    if !agents_at_location.contains(&victim_state.agent_id) {
        return Err(TheftResult::Failed {
            details: TheftFailedDetails {
                thief_id: thief_state.agent_id,
                victim_id: victim_state.agent_id,
                target_resource,
                reason: TheftFailureReason::VictimNotPresent,
                detected: false,
                location_id: thief_state.location_id,
            },
        });
    }

    // Victim must have the resource
    let victim_qty = victim_state
        .inventory
        .get(&target_resource)
        .copied()
        .unwrap_or(0);
    if victim_qty == 0 {
        return Err(TheftResult::Failed {
            details: TheftFailedDetails {
                thief_id: thief_state.agent_id,
                victim_id: victim_state.agent_id,
                target_resource,
                reason: TheftFailureReason::VictimHasNoResource,
                detected: false,
                location_id: thief_state.location_id,
            },
        });
    }

    Ok(TheftAttempt {
        thief_id: thief_state.agent_id,
        victim_id: victim_state.agent_id,
        target_resource,
        location_id: thief_state.location_id,
    })
}

/// Compute the success probability for a theft attempt (0--100).
///
/// Formula:
/// - Start with `BASE_SUCCESS_RATE` (40)
/// - Add `risk_tolerance * 10` (higher risk tolerance = bolder thief)
/// - Subtract `aggression * 15` (higher aggression = less sneaky)
/// - If victim energy < 30: +15 (tired victim)
/// - If victim is resting: +10
/// - If relationship > 0.5: +10 (trust makes theft easier)
/// - Clamp to 5--90 range (never guaranteed, never impossible)
pub fn compute_success_probability(ctx: &TheftContext) -> Result<u32, AgentError> {
    let risk_bonus = decimal_to_scaled_u32(ctx.thief_personality.risk_tolerance, 10)?;
    let aggression_penalty = decimal_to_scaled_u32(ctx.thief_personality.aggression, 15)?;

    let mut probability = BASE_SUCCESS_RATE;
    probability = probability.saturating_add(risk_bonus);
    probability = probability.saturating_sub(aggression_penalty);

    // Tired victim bonus
    if ctx.victim_energy < 30 {
        probability = probability.saturating_add(15);
    }

    // Resting victim bonus
    if ctx.victim_is_resting {
        probability = probability.saturating_add(10);
    }

    // Trust bonus: relationship > 0.5 makes theft easier
    let trust_threshold = Decimal::new(5, 1);
    if ctx.relationship_score > trust_threshold {
        probability = probability.saturating_add(10);
    }

    // Clamp to 5--90 range
    probability = probability.clamp(5, 90);

    Ok(probability)
}

/// Resolve a theft attempt given the pre-validated attempt and context.
///
/// Returns a [`TheftResult`] indicating success or failure, with full
/// event details suitable for the event store.
pub fn resolve_theft(
    attempt: &TheftAttempt,
    ctx: &TheftContext,
) -> Result<TheftResult, AgentError> {
    let success_probability = compute_success_probability(ctx)?;

    // Roll for success: succeed if roll < probability
    if ctx.success_roll < success_probability {
        // Success: compute quantity stolen (capped by `MAX_STEAL_QUANTITY` and available)
        let quantity = ctx.victim_resource_quantity.min(MAX_STEAL_QUANTITY);

        Ok(TheftResult::Success {
            details: TheftOccurredDetails {
                thief_id: attempt.thief_id,
                victim_id: attempt.victim_id,
                resource: attempt.target_resource,
                quantity_stolen: quantity,
                detected: false, // Successful theft is undetected
                location_id: attempt.location_id,
            },
            quantity_stolen: quantity,
        })
    } else {
        // Failed: roll for detection
        let detected = ctx.detection_roll < DETECTION_RATE;

        Ok(TheftResult::Failed {
            details: TheftFailedDetails {
                thief_id: attempt.thief_id,
                victim_id: attempt.victim_id,
                target_resource: attempt.target_resource,
                reason: TheftFailureReason::Caught,
                detected,
                location_id: attempt.location_id,
            },
        })
    }
}

/// Apply the consequences of a successful theft to both agent states.
///
/// Transfers resources from victim to thief. The caller is responsible
/// for recording the ledger entry with [`LedgerEntryType::Theft`].
///
/// [`LedgerEntryType::Theft`]: emergence_types::LedgerEntryType::Theft
///
/// # Errors
///
/// Returns an error if inventory operations fail (arithmetic overflow,
/// capacity exceeded, insufficient resource).
pub fn apply_theft_success(
    thief: &mut AgentState,
    victim: &mut AgentState,
    resource: Resource,
    quantity: u32,
) -> Result<(), AgentError> {
    // Remove from victim
    crate::inventory::remove_resource(&mut victim.inventory, resource, quantity)?;

    // Add to thief (may fail if thief is at capacity)
    crate::inventory::add_resource(
        &mut thief.inventory,
        thief.carry_capacity,
        resource,
        quantity,
    )?;

    // Apply energy cost to thief
    crate::vitals::apply_energy_cost(thief, THEFT_ENERGY_COST);

    Ok(())
}

/// Apply the consequences of a failed theft attempt.
///
/// Deducts energy from the thief. If the theft was detected, returns
/// the relationship damage delta that should be applied by the caller.
///
/// Returns `Some(delta)` if the victim detected the attempt and the
/// caller should apply relationship damage, or `None` if undetected.
pub fn apply_theft_failure(
    thief: &mut AgentState,
    detected: bool,
) -> Option<Decimal> {
    // Apply energy cost regardless
    crate::vitals::apply_energy_cost(thief, THEFT_ENERGY_COST);

    if detected {
        Some(relationship_damage())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a `Decimal` trait value (0.0--1.0) to a `u32` scaled by a multiplier.
///
/// For example, `decimal_to_scaled_u32(Decimal(0.7), 10)` returns `Ok(7)`.
fn decimal_to_scaled_u32(value: Decimal, multiplier: u32) -> Result<u32, AgentError> {
    let multiplier_dec = Decimal::from(multiplier);
    let scaled = value.checked_mul(multiplier_dec).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("theft probability trait scaling overflow"),
        }
    })?;

    // Truncate to integer (floor)
    let truncated = scaled.trunc();

    // Convert to u32 safely
    truncated.to_u32().ok_or_else(|| AgentError::ArithmeticOverflow {
        context: String::from("theft probability conversion to u32 failed"),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use emergence_types::{AgentId, LocationId, Personality, Resource};
    use rust_decimal::Decimal;

    use super::*;

    fn make_personality(aggression: Decimal, risk_tolerance: Decimal) -> Personality {
        Personality {
            curiosity: Decimal::new(5, 1),
            cooperation: Decimal::new(5, 1),
            aggression,
            risk_tolerance,
            industriousness: Decimal::new(5, 1),
            sociability: Decimal::new(5, 1),
            honesty: Decimal::new(5, 1),
            loyalty: Decimal::new(5, 1),
        }
    }

    fn make_agent(id: AgentId, location: LocationId, energy: u32) -> AgentState {
        AgentState {
            agent_id: id,
            energy,
            health: 100,
            hunger: 0,
            age: 100,
            born_at_tick: 0,
            location_id: location,
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

    fn default_ctx(victim_qty: u32) -> TheftContext {
        TheftContext {
            thief_personality: make_personality(
                Decimal::new(3, 1), // aggression 0.3
                Decimal::new(7, 1), // risk_tolerance 0.7
            ),
            thief_energy: 80,
            victim_energy: 60,
            victim_is_resting: false,
            relationship_score: Decimal::ZERO,
            victim_resource_quantity: victim_qty,
            success_roll: 20, // Below base rate, will succeed
            detection_roll: 50,
        }
    }

    // -----------------------------------------------------------------------
    // Validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn validate_theft_victim_not_present() {
        let loc = LocationId::new();
        let thief = make_agent(AgentId::new(), loc, 80);
        let victim = make_agent(AgentId::new(), LocationId::new(), 80);

        let result = validate_theft(
            &thief,
            &victim,
            Resource::Wood,
            &[thief.agent_id], // victim not in list
        );
        assert!(result.is_err());
        if let Err(TheftResult::Failed { details }) = result {
            assert_eq!(details.reason, TheftFailureReason::VictimNotPresent);
        }
    }

    #[test]
    fn validate_theft_victim_has_no_resource() {
        let loc = LocationId::new();
        let thief = make_agent(AgentId::new(), loc, 80);
        let victim = make_agent(AgentId::new(), loc, 80);

        let result = validate_theft(
            &thief,
            &victim,
            Resource::Wood,
            &[thief.agent_id, victim.agent_id],
        );
        assert!(result.is_err());
        if let Err(TheftResult::Failed { details }) = result {
            assert_eq!(details.reason, TheftFailureReason::VictimHasNoResource);
        }
    }

    #[test]
    fn validate_theft_passes() {
        let loc = LocationId::new();
        let thief = make_agent(AgentId::new(), loc, 80);
        let mut victim = make_agent(AgentId::new(), loc, 80);
        victim.inventory.insert(Resource::Wood, 10);

        let result = validate_theft(
            &thief,
            &victim,
            Resource::Wood,
            &[thief.agent_id, victim.agent_id],
        );
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // Probability tests
    // -----------------------------------------------------------------------

    #[test]
    fn base_probability_with_moderate_traits() {
        let ctx = default_ctx(10);
        let prob = compute_success_probability(&ctx).unwrap();
        // BASE(40) + risk_tolerance*10(7) - aggression*15(4) = 43
        assert_eq!(prob, 43);
    }

    #[test]
    fn tired_victim_gives_bonus() {
        let mut ctx = default_ctx(10);
        ctx.victim_energy = 20; // Below 30, +15 bonus
        let prob = compute_success_probability(&ctx).unwrap();
        assert_eq!(prob, 58);
    }

    #[test]
    fn resting_victim_gives_bonus() {
        let mut ctx = default_ctx(10);
        ctx.victim_is_resting = true;
        let prob = compute_success_probability(&ctx).unwrap();
        assert_eq!(prob, 53);
    }

    #[test]
    fn trust_gives_bonus() {
        let mut ctx = default_ctx(10);
        ctx.relationship_score = Decimal::new(8, 1); // 0.8 > 0.5
        let prob = compute_success_probability(&ctx).unwrap();
        assert_eq!(prob, 53);
    }

    #[test]
    fn probability_never_below_min() {
        let ctx = TheftContext {
            thief_personality: make_personality(Decimal::ONE, Decimal::ZERO),
            thief_energy: 80,
            victim_energy: 90,
            victim_is_resting: false,
            relationship_score: Decimal::ZERO,
            victim_resource_quantity: 10,
            success_roll: 0,
            detection_roll: 0,
        };
        let prob = compute_success_probability(&ctx).unwrap();
        assert!(prob >= 5);
    }

    #[test]
    fn probability_never_above_max() {
        let mut ctx = default_ctx(10);
        ctx.thief_personality = make_personality(Decimal::ZERO, Decimal::ONE);
        ctx.victim_energy = 10;
        ctx.victim_is_resting = true;
        ctx.relationship_score = Decimal::ONE;
        let prob = compute_success_probability(&ctx).unwrap();
        assert!(prob <= 90);
    }

    // -----------------------------------------------------------------------
    // Resolution tests
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_theft_succeeds_with_low_roll() {
        let attempt = TheftAttempt {
            thief_id: AgentId::new(),
            victim_id: AgentId::new(),
            target_resource: Resource::Wood,
            location_id: LocationId::new(),
        };
        let ctx = TheftContext {
            success_roll: 10,
            detection_roll: 50,
            ..default_ctx(10)
        };
        let result = resolve_theft(&attempt, &ctx).unwrap();
        match result {
            TheftResult::Success { quantity_stolen, details } => {
                assert_eq!(quantity_stolen, 5); // min(10, MAX_STEAL_QUANTITY=5)
                assert_eq!(details.resource, Resource::Wood);
                assert!(!details.detected);
            }
            TheftResult::Failed { .. } => {
                panic!("Expected success but got failure");
            }
        }
    }

    #[test]
    fn resolve_theft_fails_with_high_roll_detected() {
        let attempt = TheftAttempt {
            thief_id: AgentId::new(),
            victim_id: AgentId::new(),
            target_resource: Resource::Wood,
            location_id: LocationId::new(),
        };
        let ctx = TheftContext {
            success_roll: 99,
            detection_roll: 30, // Below DETECTION_RATE -> detected
            ..default_ctx(10)
        };
        let result = resolve_theft(&attempt, &ctx).unwrap();
        match result {
            TheftResult::Success { .. } => {
                panic!("Expected failure but got success");
            }
            TheftResult::Failed { details } => {
                assert!(details.detected);
                assert_eq!(details.reason, TheftFailureReason::Caught);
            }
        }
    }

    #[test]
    fn resolve_theft_fails_undetected() {
        let attempt = TheftAttempt {
            thief_id: AgentId::new(),
            victim_id: AgentId::new(),
            target_resource: Resource::Wood,
            location_id: LocationId::new(),
        };
        let ctx = TheftContext {
            success_roll: 99,
            detection_roll: 80, // Above DETECTION_RATE -> undetected
            ..default_ctx(10)
        };
        let result = resolve_theft(&attempt, &ctx).unwrap();
        match result {
            TheftResult::Success { .. } => {
                panic!("Expected failure but got success");
            }
            TheftResult::Failed { details } => {
                assert!(!details.detected);
            }
        }
    }

    #[test]
    fn steal_quantity_capped_by_max() {
        let attempt = TheftAttempt {
            thief_id: AgentId::new(),
            victim_id: AgentId::new(),
            target_resource: Resource::FoodBerry,
            location_id: LocationId::new(),
        };
        let ctx = TheftContext {
            success_roll: 0,
            victim_resource_quantity: 100,
            ..default_ctx(100)
        };
        let result = resolve_theft(&attempt, &ctx).unwrap();
        if let TheftResult::Success { quantity_stolen, .. } = result {
            assert_eq!(quantity_stolen, MAX_STEAL_QUANTITY);
        } else {
            panic!("Expected success");
        }
    }

    #[test]
    fn steal_quantity_limited_by_victim_inventory() {
        let attempt = TheftAttempt {
            thief_id: AgentId::new(),
            victim_id: AgentId::new(),
            target_resource: Resource::FoodBerry,
            location_id: LocationId::new(),
        };
        let ctx = TheftContext {
            success_roll: 0,
            victim_resource_quantity: 2,
            ..default_ctx(2)
        };
        let result = resolve_theft(&attempt, &ctx).unwrap();
        if let TheftResult::Success { quantity_stolen, .. } = result {
            assert_eq!(quantity_stolen, 2);
        } else {
            panic!("Expected success");
        }
    }

    // -----------------------------------------------------------------------
    // Apply tests
    // -----------------------------------------------------------------------

    #[test]
    fn apply_success_transfers_and_deducts_energy() {
        let loc = LocationId::new();
        let mut thief = make_agent(AgentId::new(), loc, 80);
        let mut victim = make_agent(AgentId::new(), loc, 80);
        victim.inventory.insert(Resource::Wood, 10);

        let result = apply_theft_success(&mut thief, &mut victim, Resource::Wood, 5);
        assert!(result.is_ok());

        assert_eq!(thief.inventory.get(&Resource::Wood).copied(), Some(5));
        assert_eq!(victim.inventory.get(&Resource::Wood).copied(), Some(5));
        assert_eq!(thief.energy, 65); // 80 - 15
    }

    #[test]
    fn apply_failure_detected_returns_relationship_damage() {
        let loc = LocationId::new();
        let mut thief = make_agent(AgentId::new(), loc, 80);

        let delta = apply_theft_failure(&mut thief, true);
        assert_eq!(thief.energy, 65);
        assert!(delta.is_some());
        assert_eq!(delta.unwrap(), relationship_damage());
    }

    #[test]
    fn apply_failure_undetected_no_relationship_damage() {
        let loc = LocationId::new();
        let mut thief = make_agent(AgentId::new(), loc, 80);

        let delta = apply_theft_failure(&mut thief, false);
        assert_eq!(thief.energy, 65);
        assert!(delta.is_none());
    }

    #[test]
    fn apply_theft_fails_if_victim_has_less_than_requested() {
        let loc = LocationId::new();
        let mut thief = make_agent(AgentId::new(), loc, 80);
        let mut victim = make_agent(AgentId::new(), loc, 80);
        victim.inventory.insert(Resource::Wood, 2);

        let result = apply_theft_success(&mut thief, &mut victim, Resource::Wood, 5);
        assert!(result.is_err());
    }
}
