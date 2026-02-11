//! Reproduction, maturity, and aging mechanics for agents.
//!
//! Implements Phase 3.6 of the build plan:
//! - Personality blending with mutation for child agents
//! - Knowledge inheritance (intersection of parent knowledge, tier 0--1 only)
//! - Maturity period with restricted actions
//! - Aging effects (energy cap decline, movement cost increase)
//! - Population cap enforcement
//!
//! See `agent-system.md` section 9 and `world-engine.md` section 7.1.

use std::collections::BTreeSet;

use rand::Rng;
use rust_decimal::Decimal;

use emergence_types::{ActionType, AgentId, Personality, Sex};

use crate::error::AgentError;
use crate::knowledge::{self, KnowledgeBase};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Minimum relationship score required for reproduction (0.7).
fn reproduction_relationship_threshold() -> Decimal {
    Decimal::new(7, 1)
}

/// Minimum health required for reproduction.
const REPRODUCTION_MIN_HEALTH: u32 = 50;

/// Energy cost for each parent during reproduction.
const REPRODUCTION_ENERGY_COST: u32 = 30;

/// Reduced energy cap for immature agents.
const IMMATURE_ENERGY_CAP: u32 = 60;

/// Gather yield multiplier for immature agents (50%, expressed as pct).
const IMMATURE_GATHER_YIELD_PCT: u32 = 50;

/// Default maturity period in ticks.
const DEFAULT_MATURITY_TICKS: u64 = 200;

// ---------------------------------------------------------------------------
// Personality Blending (Task 3.6.2)
// ---------------------------------------------------------------------------

/// Blend two parent personalities into a child personality with mutation.
///
/// Each trait is computed as `(parent_a_trait + parent_b_trait) / 2 + mutation`,
/// where mutation is a random value in the range `[-mutation_range, +mutation_range]`.
/// All traits are clamped to the valid 0.0--1.0 range.
///
/// `mutation_range` is specified as a [`Decimal`] (default: 0.1).
pub fn blend_personality(
    parent_a: &Personality,
    parent_b: &Personality,
    mutation_range: Decimal,
    rng: &mut impl Rng,
) -> Result<Personality, AgentError> {
    let two = Decimal::from(2);

    let mut blend_trait = |a: Decimal, b: Decimal| -> Result<Decimal, AgentError> {
        let sum = a.checked_add(b).ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("personality trait sum overflow"),
        })?;
        let avg = sum.checked_div(two).ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("personality trait average overflow"),
        })?;

        // Generate mutation: random integer in [-1000, 1000], then scale by mutation_range / 1000
        let roll: i32 = rng.random_range(-1000..=1000);
        let roll_dec = Decimal::from(roll);
        let thousand = Decimal::from(1000);
        let mutation_frac = roll_dec
            .checked_div(thousand)
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("personality mutation fraction overflow"),
            })?;
        let mutation = mutation_frac
            .checked_mul(mutation_range)
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("personality mutation scaling overflow"),
            })?;

        let raw = avg.checked_add(mutation).ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("personality trait mutation application overflow"),
        })?;

        // Clamp to [0.0, 1.0]
        Ok(clamp_trait(raw))
    };

    Ok(Personality {
        curiosity: blend_trait(parent_a.curiosity, parent_b.curiosity)?,
        cooperation: blend_trait(parent_a.cooperation, parent_b.cooperation)?,
        aggression: blend_trait(parent_a.aggression, parent_b.aggression)?,
        risk_tolerance: blend_trait(parent_a.risk_tolerance, parent_b.risk_tolerance)?,
        industriousness: blend_trait(parent_a.industriousness, parent_b.industriousness)?,
        sociability: blend_trait(parent_a.sociability, parent_b.sociability)?,
        honesty: blend_trait(parent_a.honesty, parent_b.honesty)?,
        loyalty: blend_trait(parent_a.loyalty, parent_b.loyalty)?,
    })
}

/// Clamp a [`Decimal`] personality trait value to the 0.0--1.0 range.
fn clamp_trait(value: Decimal) -> Decimal {
    if value < Decimal::ZERO {
        Decimal::ZERO
    } else if value > Decimal::ONE {
        Decimal::ONE
    } else {
        value
    }
}

// ---------------------------------------------------------------------------
// Knowledge Inheritance (Task 3.6.2)
// ---------------------------------------------------------------------------

/// Tier threshold for knowledge inheritance.
///
/// Concepts from seed levels 0 and 1 are tier 0--1 (inheritable).
/// Concepts from seed levels 2+ are advanced and not inherited.
fn tier_0_1_concepts() -> BTreeSet<String> {
    knowledge::seed_knowledge(1).into_iter().collect()
}

/// Compute the knowledge a child inherits from two parents.
///
/// The child receives the *intersection* of parent knowledge, filtered to
/// only include tier 0--1 concepts (seed knowledge levels 0 and 1).
/// Advanced concepts (tier 2+) are excluded even if both parents know them.
///
/// Returns a set of concept names the child should learn at birth.
#[allow(clippy::similar_names)]
pub fn inherit_knowledge(
    parent_a_knowledge: &KnowledgeBase,
    parent_b_knowledge: &KnowledgeBase,
) -> BTreeSet<String> {
    let allowed = tier_0_1_concepts();

    parent_a_knowledge
        .known_concepts()
        .intersection(parent_b_knowledge.known_concepts())
        .filter(|concept| allowed.contains(*concept))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Maturity (Task 3.6.3)
// ---------------------------------------------------------------------------

/// Check whether an agent has reached maturity based on their birth tick.
///
/// An agent is mature if `current_tick - born_at_tick >= maturity_ticks`.
/// Seed agents (born at tick 0 or with `maturity_ticks == 0`) are always mature.
pub const fn is_mature(born_at_tick: u64, current_tick: u64, maturity_ticks: u64) -> bool {
    if maturity_ticks == 0 {
        return true;
    }
    current_tick.saturating_sub(born_at_tick) >= maturity_ticks
}

/// Check whether an action is restricted for immature agents.
///
/// Immature agents cannot perform: build, trade (offer/accept/reject),
/// teach, reproduce, or any advanced action. They can only: gather
/// (at reduced yield), eat, drink, rest, move, and communicate.
pub const fn is_action_restricted_for_immature(action: ActionType) -> bool {
    matches!(
        action,
        ActionType::Build
            | ActionType::Repair
            | ActionType::Demolish
            | ActionType::ImproveRoute
            | ActionType::TradeOffer
            | ActionType::TradeAccept
            | ActionType::TradeReject
            | ActionType::FormGroup
            | ActionType::Teach
            | ActionType::FarmPlant
            | ActionType::FarmHarvest
            | ActionType::Craft
            | ActionType::Mine
            | ActionType::Smelt
            | ActionType::Write
            | ActionType::Read
            | ActionType::Claim
            | ActionType::Legislate
            | ActionType::Enforce
            | ActionType::Reproduce
            | ActionType::Steal
            | ActionType::Attack
            | ActionType::Intimidate
            | ActionType::Propose
            | ActionType::Vote
            | ActionType::Marry
            | ActionType::Divorce
            | ActionType::Conspire
            | ActionType::Freeform
    )
}

/// Return the energy cap for an immature agent.
///
/// Immature agents have a reduced energy cap of 60 instead of 100.
pub const fn immature_energy_cap() -> u32 {
    IMMATURE_ENERGY_CAP
}

/// Return the gather yield percentage for immature agents.
///
/// Immature agents gather at 50% yield.
pub const fn immature_gather_yield_pct() -> u32 {
    IMMATURE_GATHER_YIELD_PCT
}

/// Return the default maturity period in ticks.
pub const fn default_maturity_ticks() -> u64 {
    DEFAULT_MATURITY_TICKS
}

// ---------------------------------------------------------------------------
// Aging Effects (Task 3.6.4)
// ---------------------------------------------------------------------------

/// Compute the maximum energy for an agent based on their age and lifespan.
///
/// Before 80% of lifespan: max energy is 100.
/// From 80% to 100% of lifespan: linearly decreases from 100 to 50.
///
/// Formula from `world-engine.md` section 6.2:
/// ```text
/// max_energy = 100 * (1 - ((age - lifespan * 0.8) / (lifespan * 0.2)) * 0.5)
/// ```
///
/// Returns `None` if arithmetic overflows.
pub fn energy_cap(age: u32, lifespan: u32) -> Option<u32> {
    // Compute the aging threshold: 80% of lifespan
    let threshold = lifespan.checked_mul(80)?.checked_div(100)?;

    if age <= threshold {
        return Some(100);
    }

    let age_beyond = age.checked_sub(threshold)?;
    let old_age_window = lifespan.checked_sub(threshold)?;

    if old_age_window == 0 {
        return Some(100);
    }

    // decline = age_beyond * 50 / old_age_window
    let decline_numerator = age_beyond.checked_mul(50)?;
    let decline = decline_numerator.checked_div(old_age_window)?;

    let clamped_decline = if decline > 50 { 50 } else { decline };

    100_u32.checked_sub(clamped_decline)
}

/// Compute the movement cost multiplier based on age and lifespan.
///
/// Before 90% of lifespan: multiplier is 1.0 (no extra cost).
/// After 90% of lifespan: multiplier is 1.5 (+50% energy cost for movement).
///
/// Returns the multiplier as a [`Decimal`].
pub fn movement_cost_multiplier(age: u32, lifespan: u32) -> Decimal {
    let threshold = lifespan
        .checked_mul(90)
        .and_then(|v| v.checked_div(100))
        .unwrap_or(lifespan);

    if age >= threshold {
        Decimal::new(15, 1) // 1.5
    } else {
        Decimal::ONE // 1.0
    }
}

// ---------------------------------------------------------------------------
// Reproduction Validation (Task 3.6.1)
// ---------------------------------------------------------------------------

/// Check whether the population cap allows a new agent.
///
/// Returns `true` if `current_population < max_population`.
pub const fn can_add_agent(current_population: u32, max_population: u32) -> bool {
    current_population < max_population
}

/// Context needed to validate a reproduction attempt between two agents.
///
/// Bundles the vitals, relationship scores, co-location status, and
/// population data into a single struct to keep the function signature
/// manageable.
#[derive(Debug, Clone)]
pub struct ReproductionContext {
    /// Initiating agent's biological sex.
    pub initiator_sex: Sex,
    /// Partner agent's biological sex.
    pub partner_sex: Sex,
    /// Initiating agent's current health.
    pub initiator_health: u32,
    /// Initiating agent's current energy.
    pub initiator_energy: u32,
    /// Partner agent's current health.
    pub partner_health: u32,
    /// Partner agent's current energy.
    pub partner_energy: u32,
    /// Initiating agent's relationship score toward the partner.
    pub relationship_initiator_to_partner: Decimal,
    /// Partner's relationship score toward the initiating agent.
    pub relationship_partner_to_initiator: Decimal,
    /// Whether both agents are at the same location.
    pub co_located: bool,
    /// Current number of alive agents in the simulation.
    pub current_population: u32,
    /// Maximum allowed agents.
    pub max_population: u32,
}

/// Validate that two agents can reproduce.
///
/// Requirements per `agent-system.md` section 9.2 and `world-engine.md` section 7.1:
/// - Both agents at the same location (`co_located` must be `true`)
/// - Both agents have relationship > 0.7 with each other
/// - Both agents have health > 50
/// - Both agents have energy >= 30
/// - Population is below the maximum
///
/// Returns `Ok(())` if all conditions are met, or an error describing the failure.
pub fn validate_reproduction(ctx: &ReproductionContext) -> Result<(), AgentError> {
    if !ctx.co_located {
        return Err(AgentError::ReproductionFailed {
            reason: String::from("agents are not at the same location"),
        });
    }

    // Reproduction requires one male and one female partner.
    if ctx.initiator_sex == ctx.partner_sex {
        return Err(AgentError::ReproductionFailed {
            reason: format!(
                "reproduction requires one male and one female partner, but both are {}",
                ctx.initiator_sex
            ),
        });
    }

    let threshold = reproduction_relationship_threshold();

    if ctx.relationship_initiator_to_partner <= threshold {
        return Err(AgentError::ReproductionFailed {
            reason: format!(
                "initiator's relationship with partner is {}, needs to be above {threshold}",
                ctx.relationship_initiator_to_partner
            ),
        });
    }

    if ctx.relationship_partner_to_initiator <= threshold {
        return Err(AgentError::ReproductionFailed {
            reason: format!(
                "partner's relationship with initiator is {}, needs to be above {threshold}",
                ctx.relationship_partner_to_initiator
            ),
        });
    }

    if ctx.initiator_health <= REPRODUCTION_MIN_HEALTH {
        return Err(AgentError::ReproductionFailed {
            reason: format!(
                "initiator health is {}, needs to be above {REPRODUCTION_MIN_HEALTH}",
                ctx.initiator_health
            ),
        });
    }

    if ctx.partner_health <= REPRODUCTION_MIN_HEALTH {
        return Err(AgentError::ReproductionFailed {
            reason: format!(
                "partner health is {}, needs to be above {REPRODUCTION_MIN_HEALTH}",
                ctx.partner_health
            ),
        });
    }

    if ctx.initiator_energy < REPRODUCTION_ENERGY_COST {
        return Err(AgentError::ReproductionFailed {
            reason: format!(
                "initiator energy is {}, needs at least {REPRODUCTION_ENERGY_COST}",
                ctx.initiator_energy
            ),
        });
    }

    if ctx.partner_energy < REPRODUCTION_ENERGY_COST {
        return Err(AgentError::ReproductionFailed {
            reason: format!(
                "partner energy is {}, needs at least {REPRODUCTION_ENERGY_COST}",
                ctx.partner_energy
            ),
        });
    }

    if !can_add_agent(ctx.current_population, ctx.max_population) {
        return Err(AgentError::ReproductionFailed {
            reason: format!(
                "population cap reached: {}/{}",
                ctx.current_population, ctx.max_population
            ),
        });
    }

    Ok(())
}

/// Return the energy cost deducted from each parent during reproduction.
pub const fn reproduction_energy_cost() -> u32 {
    REPRODUCTION_ENERGY_COST
}

/// Generate a default child name from the parent names.
///
/// Format: "Child of {first\_parent} and {second\_parent}".
pub fn generate_child_name(first_parent: &str, second_parent: &str) -> String {
    format!("Child of {first_parent} and {second_parent}")
}

// ---------------------------------------------------------------------------
// AgentBorn Event Details (Task 3.6.6)
// ---------------------------------------------------------------------------

/// Details for an [`AgentBorn`] event emitted when a child is created.
///
/// [`AgentBorn`]: emergence_types::EventType::AgentBorn
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentBornDetails {
    /// The new agent's ID.
    pub child_id: AgentId,
    /// The new agent's display name.
    pub child_name: String,
    /// The child's biological sex.
    pub child_sex: Sex,
    /// First parent ID.
    pub parent_a: AgentId,
    /// First parent's biological sex.
    pub parent_a_sex: Sex,
    /// Second parent ID.
    pub parent_b: AgentId,
    /// Second parent's biological sex.
    pub parent_b_sex: Sex,
    /// The child's generation number.
    pub generation: u32,
    /// The tick when the child was born.
    pub born_at_tick: u64,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand::rngs::SmallRng;
    use rust_decimal::Decimal;

    use super::*;
    use crate::knowledge::{DiscoveryMethod, KnowledgeBase};

    fn test_personality() -> Personality {
        Personality {
            curiosity: Decimal::new(6, 1),     // 0.6
            cooperation: Decimal::new(4, 1),   // 0.4
            aggression: Decimal::new(3, 1),    // 0.3
            risk_tolerance: Decimal::new(5, 1), // 0.5
            industriousness: Decimal::new(7, 1), // 0.7
            sociability: Decimal::new(8, 1),   // 0.8
            honesty: Decimal::new(9, 1),       // 0.9
            loyalty: Decimal::new(2, 1),       // 0.2
        }
    }

    fn other_personality() -> Personality {
        Personality {
            curiosity: Decimal::new(4, 1),     // 0.4
            cooperation: Decimal::new(6, 1),   // 0.6
            aggression: Decimal::new(7, 1),    // 0.7
            risk_tolerance: Decimal::new(3, 1), // 0.3
            industriousness: Decimal::new(5, 1), // 0.5
            sociability: Decimal::new(2, 1),   // 0.2
            honesty: Decimal::new(1, 1),       // 0.1
            loyalty: Decimal::new(8, 1),       // 0.8
        }
    }

    // -----------------------------------------------------------------------
    // Personality blending
    // -----------------------------------------------------------------------

    #[test]
    fn blend_personality_averages_traits() {
        let a = test_personality();
        let b = other_personality();
        // Use zero mutation to test pure average
        let mut rng = SmallRng::seed_from_u64(42);
        let child = blend_personality(&a, &b, Decimal::ZERO, &mut rng);
        assert!(child.is_ok());
        let child = child.ok().unwrap_or_else(test_personality);

        // curiosity: (0.6 + 0.4) / 2 = 0.5
        assert_eq!(child.curiosity, Decimal::new(5, 1));
        // cooperation: (0.4 + 0.6) / 2 = 0.5
        assert_eq!(child.cooperation, Decimal::new(5, 1));
        // aggression: (0.3 + 0.7) / 2 = 0.5
        assert_eq!(child.aggression, Decimal::new(5, 1));
        // loyalty: (0.2 + 0.8) / 2 = 0.5
        assert_eq!(child.loyalty, Decimal::new(5, 1));
    }

    #[test]
    fn blend_personality_with_mutation_stays_in_range() {
        let a = test_personality();
        let b = other_personality();
        let mut rng = SmallRng::seed_from_u64(42);
        let mutation = Decimal::new(1, 1); // 0.1

        // Run many times to test clamping
        for seed in 0..100_u64 {
            let mut r = SmallRng::seed_from_u64(seed);
            let child = blend_personality(&a, &b, mutation, &mut r);
            assert!(child.is_ok());
            let child = child.ok().unwrap_or_else(test_personality);

            // All traits must be in [0.0, 1.0]
            for trait_val in [
                child.curiosity,
                child.cooperation,
                child.aggression,
                child.risk_tolerance,
                child.industriousness,
                child.sociability,
                child.honesty,
                child.loyalty,
            ] {
                assert!(
                    trait_val >= Decimal::ZERO,
                    "trait {trait_val} below 0"
                );
                assert!(
                    trait_val <= Decimal::ONE,
                    "trait {trait_val} above 1"
                );
            }
        }

        // Also verify that mutation actually creates some variance
        let child1 = blend_personality(&a, &b, mutation, &mut rng);
        let mut rng2 = SmallRng::seed_from_u64(999);
        let child2 = blend_personality(&a, &b, mutation, &mut rng2);
        assert!(child1.is_ok());
        assert!(child2.is_ok());
        let c1 = child1.ok().unwrap_or_else(test_personality);
        let c2 = child2.ok().unwrap_or_else(test_personality);
        // At least some traits should differ between different RNG seeds
        let same_count = [
            c1.curiosity == c2.curiosity,
            c1.cooperation == c2.cooperation,
            c1.aggression == c2.aggression,
            c1.risk_tolerance == c2.risk_tolerance,
            c1.industriousness == c2.industriousness,
            c1.sociability == c2.sociability,
            c1.honesty == c2.honesty,
            c1.loyalty == c2.loyalty,
        ]
        .iter()
        .filter(|&&same| same)
        .count();
        assert!(same_count < 8, "Expected some variance between different seeds");
    }

    #[test]
    fn blend_personality_extreme_parents_clamped() {
        let high = Personality {
            curiosity: Decimal::ONE,
            cooperation: Decimal::ONE,
            aggression: Decimal::ONE,
            risk_tolerance: Decimal::ONE,
            industriousness: Decimal::ONE,
            sociability: Decimal::ONE,
            honesty: Decimal::ONE,
            loyalty: Decimal::ONE,
        };
        let low = Personality {
            curiosity: Decimal::ZERO,
            cooperation: Decimal::ZERO,
            aggression: Decimal::ZERO,
            risk_tolerance: Decimal::ZERO,
            industriousness: Decimal::ZERO,
            sociability: Decimal::ZERO,
            honesty: Decimal::ZERO,
            loyalty: Decimal::ZERO,
        };
        let mut rng = SmallRng::seed_from_u64(42);
        let mutation = Decimal::new(1, 1);

        let child = blend_personality(&high, &low, mutation, &mut rng);
        assert!(child.is_ok());
        let child = child.ok().unwrap_or_else(test_personality);

        for trait_val in [
            child.curiosity,
            child.cooperation,
            child.aggression,
            child.risk_tolerance,
            child.industriousness,
            child.sociability,
            child.honesty,
            child.loyalty,
        ] {
            assert!(trait_val >= Decimal::ZERO);
            assert!(trait_val <= Decimal::ONE);
        }
    }

    // -----------------------------------------------------------------------
    // Knowledge inheritance
    // -----------------------------------------------------------------------

    #[test]
    fn inherit_knowledge_intersection_tier_0_1() {
        let mut parent_a = KnowledgeBase::with_seed_knowledge(1);
        let parent_b = KnowledgeBase::with_seed_knowledge(1);

        // Both know tier 0-1 concepts: should inherit all of them
        let inherited = inherit_knowledge(&parent_a, &parent_b);
        assert!(!inherited.is_empty());
        assert!(inherited.contains("exist"));
        assert!(inherited.contains("gather_food"));
        assert!(inherited.contains("basic_trade"));

        // Add advanced knowledge to parent_a only
        parent_a.learn("agriculture", 100, DiscoveryMethod::Experimentation);
        let inherited = inherit_knowledge(&parent_a, &parent_b);
        // agriculture is tier 2, not in intersection since parent_b doesn't know it
        assert!(!inherited.contains("agriculture"));
    }

    #[test]
    fn inherit_knowledge_excludes_advanced_even_if_both_know() {
        let parent_a = KnowledgeBase::with_seed_knowledge(2);
        let parent_b = KnowledgeBase::with_seed_knowledge(2);

        // Both know tier 2 concepts like "agriculture"
        let inherited = inherit_knowledge(&parent_a, &parent_b);

        // Tier 0-1 concepts should be present
        assert!(inherited.contains("exist"));
        assert!(inherited.contains("gather_food"));

        // Tier 2 concepts should NOT be inherited
        assert!(!inherited.contains("agriculture"));
        assert!(!inherited.contains("build_hut"));
        assert!(!inherited.contains("pottery"));
    }

    #[test]
    fn inherit_knowledge_partial_overlap() {
        let mut parent_a = KnowledgeBase::new();
        let mut parent_b = KnowledgeBase::new();

        // Parent A knows: exist, gather_food
        parent_a.learn("exist", 0, DiscoveryMethod::Seed);
        parent_a.learn("gather_food", 0, DiscoveryMethod::Seed);

        // Parent B knows: exist, basic_trade
        parent_b.learn("exist", 0, DiscoveryMethod::Seed);
        parent_b.learn("basic_trade", 0, DiscoveryMethod::Seed);

        let inherited = inherit_knowledge(&parent_a, &parent_b);

        // Only the intersection: "exist"
        assert_eq!(inherited.len(), 1);
        assert!(inherited.contains("exist"));
    }

    #[test]
    fn inherit_knowledge_empty_parents() {
        let parent_a = KnowledgeBase::new();
        let parent_b = KnowledgeBase::new();

        let inherited = inherit_knowledge(&parent_a, &parent_b);
        assert!(inherited.is_empty());
    }

    // -----------------------------------------------------------------------
    // Maturity
    // -----------------------------------------------------------------------

    #[test]
    fn is_mature_at_birth() {
        assert!(!is_mature(100, 100, 200)); // Born tick 100, current tick 100, needs 200
    }

    #[test]
    fn is_mature_during_childhood() {
        assert!(!is_mature(100, 200, 200)); // 100 ticks old, needs 200
    }

    #[test]
    fn is_mature_at_threshold() {
        assert!(is_mature(100, 300, 200)); // Exactly 200 ticks old
    }

    #[test]
    fn is_mature_after_threshold() {
        assert!(is_mature(100, 500, 200)); // 400 ticks old
    }

    #[test]
    fn is_mature_zero_maturity_always_mature() {
        assert!(is_mature(100, 100, 0)); // Zero maturity = always mature
    }

    #[test]
    fn is_mature_seed_agent_lifecycle() {
        // Seed agents born at tick 0 are NOT mature at tick 0 when maturity_ticks = 200.
        assert!(!is_mature(0, 0, 200));
        // They become mature once the maturity period has elapsed.
        assert!(is_mature(0, 200, 200));
        // Well past maturity threshold, still mature.
        assert!(is_mature(0, 500, 200));
    }

    #[test]
    fn immature_action_restrictions() {
        // Allowed actions for immature agents
        assert!(!is_action_restricted_for_immature(ActionType::Gather));
        assert!(!is_action_restricted_for_immature(ActionType::Eat));
        assert!(!is_action_restricted_for_immature(ActionType::Drink));
        assert!(!is_action_restricted_for_immature(ActionType::Rest));
        assert!(!is_action_restricted_for_immature(ActionType::Move));
        assert!(!is_action_restricted_for_immature(ActionType::Communicate));
        assert!(!is_action_restricted_for_immature(ActionType::Broadcast));
        assert!(!is_action_restricted_for_immature(ActionType::NoAction));

        // Restricted actions
        assert!(is_action_restricted_for_immature(ActionType::Build));
        assert!(is_action_restricted_for_immature(ActionType::TradeOffer));
        assert!(is_action_restricted_for_immature(ActionType::Teach));
        assert!(is_action_restricted_for_immature(ActionType::Reproduce));
        assert!(is_action_restricted_for_immature(ActionType::FarmPlant));
        assert!(is_action_restricted_for_immature(ActionType::Mine));
    }

    // -----------------------------------------------------------------------
    // Aging effects
    // -----------------------------------------------------------------------

    #[test]
    fn energy_cap_before_threshold() {
        assert_eq!(energy_cap(0, 2500), Some(100));
        assert_eq!(energy_cap(1000, 2500), Some(100));
        assert_eq!(energy_cap(2000, 2500), Some(100));
    }

    #[test]
    fn energy_cap_at_threshold() {
        assert_eq!(energy_cap(2000, 2500), Some(100));
    }

    #[test]
    fn energy_cap_halfway_decline() {
        // At age 2250: age_beyond = 250, window = 500, decline = 250*50/500 = 25
        assert_eq!(energy_cap(2250, 2500), Some(75));
    }

    #[test]
    fn energy_cap_at_lifespan() {
        assert_eq!(energy_cap(2500, 2500), Some(50));
    }

    #[test]
    fn energy_cap_beyond_lifespan() {
        assert_eq!(energy_cap(3000, 2500), Some(50));
    }

    #[test]
    fn movement_cost_before_threshold() {
        let mult = movement_cost_multiplier(0, 2500);
        assert_eq!(mult, Decimal::ONE);

        let mult = movement_cost_multiplier(2000, 2500);
        assert_eq!(mult, Decimal::ONE);
    }

    #[test]
    fn movement_cost_at_90_pct() {
        // 90% of 2500 = 2250
        let mult = movement_cost_multiplier(2250, 2500);
        assert_eq!(mult, Decimal::new(15, 1)); // 1.5
    }

    #[test]
    fn movement_cost_after_90_pct() {
        let mult = movement_cost_multiplier(2400, 2500);
        assert_eq!(mult, Decimal::new(15, 1)); // 1.5
    }

    #[test]
    fn movement_cost_just_below_90_pct() {
        // 2249 is below 2250
        let mult = movement_cost_multiplier(2249, 2500);
        assert_eq!(mult, Decimal::ONE);
    }

    // -----------------------------------------------------------------------
    // Population cap
    // -----------------------------------------------------------------------

    #[test]
    fn can_add_agent_below_cap() {
        assert!(can_add_agent(100, 200));
    }

    #[test]
    fn can_add_agent_at_cap() {
        assert!(!can_add_agent(200, 200));
    }

    #[test]
    fn can_add_agent_above_cap() {
        assert!(!can_add_agent(201, 200));
    }

    // -----------------------------------------------------------------------
    // Reproduction validation
    // -----------------------------------------------------------------------

    fn make_repro_ctx() -> ReproductionContext {
        ReproductionContext {
            initiator_sex: Sex::Male,
            partner_sex: Sex::Female,
            initiator_health: 100,
            initiator_energy: 80,
            partner_health: 100,
            partner_energy: 80,
            relationship_initiator_to_partner: Decimal::new(8, 1), // 0.8
            relationship_partner_to_initiator: Decimal::new(75, 2), // 0.75
            co_located: true,
            current_population: 50,
            max_population: 200,
        }
    }

    #[test]
    fn validate_reproduction_all_conditions_met() {
        let ctx = make_repro_ctx();
        assert!(validate_reproduction(&ctx).is_ok());
    }

    #[test]
    fn validate_reproduction_not_co_located() {
        let mut ctx = make_repro_ctx();
        ctx.co_located = false;
        assert!(validate_reproduction(&ctx).is_err());
    }

    #[test]
    fn validate_reproduction_low_relationship_initiator() {
        let mut ctx = make_repro_ctx();
        ctx.relationship_initiator_to_partner = Decimal::new(5, 1); // 0.5
        assert!(validate_reproduction(&ctx).is_err());
    }

    #[test]
    fn validate_reproduction_low_relationship_partner() {
        let mut ctx = make_repro_ctx();
        ctx.relationship_partner_to_initiator = Decimal::new(7, 1); // 0.7 -- at threshold, not above
        assert!(validate_reproduction(&ctx).is_err());
    }

    #[test]
    fn validate_reproduction_low_health_initiator() {
        let mut ctx = make_repro_ctx();
        ctx.initiator_health = 50; // at threshold, not above
        assert!(validate_reproduction(&ctx).is_err());
    }

    #[test]
    fn validate_reproduction_low_health_partner() {
        let mut ctx = make_repro_ctx();
        ctx.partner_health = 40;
        assert!(validate_reproduction(&ctx).is_err());
    }

    #[test]
    fn validate_reproduction_insufficient_energy_initiator() {
        let mut ctx = make_repro_ctx();
        ctx.initiator_energy = 20; // 20 < 30 cost
        assert!(validate_reproduction(&ctx).is_err());
    }

    #[test]
    fn validate_reproduction_insufficient_energy_partner() {
        let mut ctx = make_repro_ctx();
        ctx.partner_energy = 25;
        assert!(validate_reproduction(&ctx).is_err());
    }

    #[test]
    fn validate_reproduction_population_cap_reached() {
        let mut ctx = make_repro_ctx();
        ctx.current_population = 200;
        ctx.max_population = 200;
        assert!(validate_reproduction(&ctx).is_err());
    }

    #[test]
    fn validate_reproduction_at_exact_energy_threshold() {
        let mut ctx = make_repro_ctx();
        ctx.initiator_energy = 30;
        ctx.partner_energy = 30;
        assert!(validate_reproduction(&ctx).is_ok());
    }

    #[test]
    fn validate_reproduction_same_sex_male() {
        let mut ctx = make_repro_ctx();
        ctx.initiator_sex = Sex::Male;
        ctx.partner_sex = Sex::Male;
        assert!(validate_reproduction(&ctx).is_err());
    }

    #[test]
    fn validate_reproduction_same_sex_female() {
        let mut ctx = make_repro_ctx();
        ctx.initiator_sex = Sex::Female;
        ctx.partner_sex = Sex::Female;
        assert!(validate_reproduction(&ctx).is_err());
    }

    #[test]
    fn validate_reproduction_opposite_sex_passes() {
        let ctx = make_repro_ctx(); // Male + Female by default
        assert!(validate_reproduction(&ctx).is_ok());
    }

    // -----------------------------------------------------------------------
    // Child name generation
    // -----------------------------------------------------------------------

    #[test]
    fn generate_child_name_format() {
        let name = generate_child_name("Kora", "Dax");
        assert_eq!(name, "Child of Kora and Dax");
    }
}
