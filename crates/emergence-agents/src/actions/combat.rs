//! Combat mechanics for physical confrontation between agents.
//!
//! Implements task 6.3.5 from the build plan: agents can attack or intimidate
//! co-located agents. Combat power is computed from health, energy, personality,
//! equipment, and allies present. Damage is dealt based on power differential.
//!
//! ## Combat flow
//!
//! 1. Validate prerequisites (co-location, attacker has energy >= 20)
//! 2. Compute combat power for both participants
//! 3. Resolve based on intent:
//!    - **Attack**: compare power, deal damage, winner loots
//!    - **Intimidate**: if attacker power > 1.5x defender power, defender
//!      loses energy and relationship drops
//! 4. Emit `CombatInitiated` and `CombatResolved` events
//! 5. If health reaches 0, trigger death mechanics

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use emergence_types::{
    AgentId, AgentState, CombatInitiatedDetails, CombatIntent, CombatResolvedDetails, LocationId,
    Personality, Resource,
};

use crate::error::AgentError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Energy cost for the attacker (Attack intent).
pub const ATTACK_ENERGY_COST: u32 = 20;

/// Energy cost for the defender (reactive).
pub const DEFEND_ENERGY_COST: u32 = 10;

/// Energy cost for the attacker (Intimidate intent).
pub const INTIMIDATE_ENERGY_COST: u32 = 10;

/// Minimum damage dealt to each participant in an attack.
pub const MIN_DAMAGE: u32 = 5;

/// Maximum number of resources the winner can loot from the loser.
pub const MAX_LOOT_ITEMS: usize = 5;

/// Intimidation power threshold multiplier (attacker must have > 1.5x power).
/// Expressed as 150 (i.e. 150 percent of defender power).
const INTIMIDATION_THRESHOLD_PCT: u32 = 150;

/// Energy lost by a defender who is successfully intimidated.
const INTIMIDATION_ENERGY_LOSS: u32 = 10;

/// Relationship score set when intimidation succeeds (-0.8).
fn intimidation_relationship() -> Decimal {
    Decimal::new(-8, 1)
}

// ---------------------------------------------------------------------------
// CombatAction
// ---------------------------------------------------------------------------

/// A validated combat action ready for resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatAction {
    /// The agent who started the fight.
    pub attacker_id: AgentId,
    /// The target of the combat action.
    pub defender_id: AgentId,
    /// The type of combat action.
    pub intent: CombatIntent,
    /// The location where combat occurs.
    pub location_id: LocationId,
}

// ---------------------------------------------------------------------------
// CombatContext
// ---------------------------------------------------------------------------

/// World state needed to resolve combat.
///
/// Assembled by the tick cycle and passed into [`resolve_combat`].
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct CombatContext {
    /// Attacker's personality.
    pub attacker_personality: Personality,
    /// Defender's personality.
    pub defender_personality: Personality,
    /// Attacker's current health (0--100).
    pub attacker_health: u32,
    /// Defender's current health (0--100).
    pub defender_health: u32,
    /// Attacker's current energy (0--100).
    pub attacker_energy: u32,
    /// Defender's current energy (0--100).
    pub defender_energy: u32,
    /// Whether the attacker has a Tool in inventory.
    pub attacker_has_tool: bool,
    /// Whether the attacker has a `ToolAdvanced` in inventory.
    pub attacker_has_advanced_tool: bool,
    /// Whether the defender has a Tool in inventory.
    pub defender_has_tool: bool,
    /// Whether the defender has a `ToolAdvanced` in inventory.
    pub defender_has_advanced_tool: bool,
    /// Number of allies the attacker has at the location.
    pub attacker_allies_count: u32,
    /// Number of allies the defender has at the location.
    pub defender_allies_count: u32,
}

// ---------------------------------------------------------------------------
// CombatResult
// ---------------------------------------------------------------------------

/// The outcome of a combat encounter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatResult {
    /// Event details for combat initiation.
    pub initiated: CombatInitiatedDetails,
    /// Event details for combat resolution.
    pub resolved: CombatResolvedDetails,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Compute the combat power for a participant.
///
/// Formula:
/// - Base power: `health / 4` (healthier = stronger)
/// - Aggression bonus: `aggression * 5`
/// - Energy factor: `energy / 10`
/// - Tool bonus: +3 if has Tool, +5 if has `ToolAdvanced`
/// - Group bonus: +2 per ally at the same location
pub fn compute_combat_power(
    health: u32,
    energy: u32,
    personality: &Personality,
    has_tool: bool,
    has_advanced_tool: bool,
    allies_count: u32,
) -> Result<u32, AgentError> {
    // health / 4
    let base_power = health.checked_div(4).unwrap_or(0);

    // aggression * 5
    let aggression_bonus = decimal_to_scaled_u32(personality.aggression, 5)?;

    // energy / 10
    let energy_factor = energy.checked_div(10).unwrap_or(0);

    // Tool bonuses (best tool wins, not additive)
    let tool_bonus = if has_advanced_tool {
        5
    } else if has_tool {
        3
    } else {
        0
    };

    // Group bonus: +2 per ally
    let group_bonus = allies_count.saturating_mul(2);

    // Sum all components
    let mut power = base_power;
    power = power.saturating_add(aggression_bonus);
    power = power.saturating_add(energy_factor);
    power = power.saturating_add(tool_bonus);
    power = power.saturating_add(group_bonus);

    Ok(power)
}

/// Resolve a combat encounter.
///
/// Computes power for both participants, determines the winner, applies
/// damage and energy costs, and assembles the result details.
///
/// The caller is responsible for:
/// - Applying health changes to agent states
/// - Applying energy changes to agent states
/// - Transferring loot via the ledger
/// - Emitting `CombatInitiated` and `CombatResolved` events
/// - Triggering death mechanics if health reaches 0
pub fn resolve_combat(
    action: &CombatAction,
    ctx: &CombatContext,
    defender_inventory: &BTreeMap<Resource, u32>,
) -> Result<CombatResult, AgentError> {
    let initiated = CombatInitiatedDetails {
        attacker_id: action.attacker_id,
        defender_id: action.defender_id,
        intent: action.intent,
        location_id: action.location_id,
    };

    match action.intent {
        CombatIntent::Attack => resolve_attack(action, ctx, defender_inventory, initiated),
        CombatIntent::Intimidate => resolve_intimidate(action, ctx, initiated),
    }
}

/// Apply the combat result to the attacker and defender agent states.
///
/// This handles:
/// - Health reduction from damage
/// - Energy cost deduction
/// - Loot transfer (winner takes from loser)
///
/// Returns `(attacker_died, defender_died)` indicating whether either
/// agent's health reached zero.
///
/// # Errors
///
/// Returns an error if inventory operations fail.
pub fn apply_combat_result(
    attacker: &mut AgentState,
    defender: &mut AgentState,
    resolved: &CombatResolvedDetails,
) -> Result<(bool, bool), AgentError> {
    // Apply health damage
    attacker.health = attacker.health.saturating_sub(resolved.attacker_damage);
    defender.health = defender.health.saturating_sub(resolved.defender_damage);

    // Apply energy costs
    crate::vitals::apply_energy_cost(attacker, resolved.attacker_energy_cost);
    crate::vitals::apply_energy_cost(defender, resolved.defender_energy_cost);

    // Transfer loot
    if let Some(winner_id) = resolved.winner {
        let (winner, loser) = if winner_id == attacker.agent_id {
            (&mut *attacker, &mut *defender)
        } else {
            (&mut *defender, &mut *attacker)
        };

        for (&resource, &quantity) in &resolved.loot_transferred {
            // Best-effort loot: skip if loser does not have enough or winner is full
            if crate::inventory::remove_resource(&mut loser.inventory, resource, quantity).is_ok() {
                let _ = crate::inventory::add_resource(
                    &mut winner.inventory,
                    winner.carry_capacity,
                    resource,
                    quantity,
                );
            }
        }
    }

    let attacker_died = attacker.health == 0;
    let defender_died = defender.health == 0;

    Ok((attacker_died, defender_died))
}

// ---------------------------------------------------------------------------
// Internal resolution
// ---------------------------------------------------------------------------

/// Resolve an attack intent.
fn resolve_attack(
    action: &CombatAction,
    ctx: &CombatContext,
    defender_inventory: &BTreeMap<Resource, u32>,
    initiated: CombatInitiatedDetails,
) -> Result<CombatResult, AgentError> {
    let attacker_power = compute_combat_power(
        ctx.attacker_health,
        ctx.attacker_energy,
        &ctx.attacker_personality,
        ctx.attacker_has_tool,
        ctx.attacker_has_advanced_tool,
        ctx.attacker_allies_count,
    )?;

    let defender_power = compute_combat_power(
        ctx.defender_health,
        ctx.defender_energy,
        &ctx.defender_personality,
        ctx.defender_has_tool,
        ctx.defender_has_advanced_tool,
        ctx.defender_allies_count,
    )?;

    // Determine winner and damage
    let (winner, attacker_damage, defender_damage) = match attacker_power.cmp(&defender_power) {
        std::cmp::Ordering::Greater => {
            // Attacker wins: defender takes (power_diff * 2) damage
            let power_diff = attacker_power.saturating_sub(defender_power);
            let defender_dmg = power_diff.saturating_mul(2).max(MIN_DAMAGE);
            (Some(action.attacker_id), MIN_DAMAGE, defender_dmg)
        }
        std::cmp::Ordering::Less => {
            // Defender wins: attacker takes (power_diff * 2) damage
            let power_diff = defender_power.saturating_sub(attacker_power);
            let attacker_dmg = power_diff.saturating_mul(2).max(MIN_DAMAGE);
            (Some(action.defender_id), attacker_dmg, MIN_DAMAGE)
        }
        std::cmp::Ordering::Equal => {
            // Draw: both take minimum damage
            (None, MIN_DAMAGE, MIN_DAMAGE)
        }
    };

    // Compute loot if there is a winner
    let loot_transferred = if winner.is_some() {
        compute_loot(defender_inventory)
    } else {
        BTreeMap::new()
    };

    // Check if either agent would die
    let attacker_died = ctx.attacker_health <= attacker_damage;
    let defender_died = ctx.defender_health <= defender_damage;

    let resolved = CombatResolvedDetails {
        attacker_id: action.attacker_id,
        defender_id: action.defender_id,
        intent: CombatIntent::Attack,
        winner,
        attacker_damage,
        defender_damage,
        attacker_energy_cost: ATTACK_ENERGY_COST,
        defender_energy_cost: DEFEND_ENERGY_COST,
        loot_transferred,
        attacker_died,
        defender_died,
        location_id: action.location_id,
    };

    Ok(CombatResult {
        initiated,
        resolved,
    })
}

/// Resolve an intimidate intent.
fn resolve_intimidate(
    action: &CombatAction,
    ctx: &CombatContext,
    initiated: CombatInitiatedDetails,
) -> Result<CombatResult, AgentError> {
    let attacker_power = compute_combat_power(
        ctx.attacker_health,
        ctx.attacker_energy,
        &ctx.attacker_personality,
        ctx.attacker_has_tool,
        ctx.attacker_has_advanced_tool,
        ctx.attacker_allies_count,
    )?;

    let defender_power = compute_combat_power(
        ctx.defender_health,
        ctx.defender_energy,
        &ctx.defender_personality,
        ctx.defender_has_tool,
        ctx.defender_has_advanced_tool,
        ctx.defender_allies_count,
    )?;

    // Intimidation succeeds if attacker_power > defender_power * 1.5
    // Using integer math: attacker_power * 100 > defender_power * 150
    let attacker_scaled = attacker_power.checked_mul(100).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("intimidation attacker power scaling overflow"),
        }
    })?;
    let defender_scaled = defender_power.checked_mul(INTIMIDATION_THRESHOLD_PCT).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("intimidation defender power scaling overflow"),
        }
    })?;

    let intimidation_succeeded = attacker_scaled > defender_scaled;

    let (winner, defender_energy_cost) = if intimidation_succeeded {
        // Defender loses energy from fear
        (Some(action.attacker_id), INTIMIDATION_ENERGY_LOSS)
    } else {
        // Intimidation failed -- no effect on defender
        (None, 0)
    };

    let resolved = CombatResolvedDetails {
        attacker_id: action.attacker_id,
        defender_id: action.defender_id,
        intent: CombatIntent::Intimidate,
        winner,
        attacker_damage: 0, // No damage in intimidation
        defender_damage: 0,
        attacker_energy_cost: INTIMIDATE_ENERGY_COST,
        defender_energy_cost,
        loot_transferred: BTreeMap::new(), // No loot in intimidation
        attacker_died: false,
        defender_died: false,
        location_id: action.location_id,
    };

    Ok(CombatResult {
        initiated,
        resolved,
    })
}

/// Compute the relationship damage delta for a successful intimidation.
///
/// Returns the absolute relationship score that the victim's relationship
/// with the intimidator should be set to (-0.8).
pub fn intimidation_relationship_target() -> Decimal {
    intimidation_relationship()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute loot from the loser's inventory (up to `MAX_LOOT_ITEMS` resources).
///
/// Takes up to 1 unit of each resource type, up to the limit.
fn compute_loot(inventory: &BTreeMap<Resource, u32>) -> BTreeMap<Resource, u32> {
    let mut loot = BTreeMap::new();
    let mut count: usize = 0;

    for (&resource, &quantity) in inventory {
        if count >= MAX_LOOT_ITEMS {
            break;
        }
        if quantity > 0 {
            // Take 1 unit of each resource type
            loot.insert(resource, 1);
            count = count.saturating_add(1);
        }
    }

    loot
}

/// Convert a `Decimal` trait value (0.0--1.0) to a `u32` scaled by a multiplier.
fn decimal_to_scaled_u32(value: Decimal, multiplier: u32) -> Result<u32, AgentError> {
    let multiplier_dec = Decimal::from(multiplier);
    let scaled = value.checked_mul(multiplier_dec).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("combat power trait scaling overflow"),
        }
    })?;

    let truncated = scaled.trunc();

    truncated.to_u32().ok_or_else(|| AgentError::ArithmeticOverflow {
        context: String::from("combat power conversion to u32 failed"),
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

    fn make_personality(aggression: Decimal) -> Personality {
        Personality {
            curiosity: Decimal::new(5, 1),
            cooperation: Decimal::new(5, 1),
            aggression,
            risk_tolerance: Decimal::new(5, 1),
            industriousness: Decimal::new(5, 1),
            sociability: Decimal::new(5, 1),
            honesty: Decimal::new(5, 1),
            loyalty: Decimal::new(5, 1),
        }
    }

    fn make_agent(id: AgentId, location: LocationId, health: u32, energy: u32) -> AgentState {
        AgentState {
            agent_id: id,
            energy,
            health,
            hunger: 0,
            thirst: 0,
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

    fn default_combat_ctx() -> CombatContext {
        CombatContext {
            attacker_personality: make_personality(Decimal::new(7, 1)), // aggression 0.7
            defender_personality: make_personality(Decimal::new(3, 1)), // aggression 0.3
            attacker_health: 100,
            defender_health: 100,
            attacker_energy: 80,
            defender_energy: 60,
            attacker_has_tool: false,
            attacker_has_advanced_tool: false,
            defender_has_tool: false,
            defender_has_advanced_tool: false,
            attacker_allies_count: 0,
            defender_allies_count: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Combat power tests
    // -----------------------------------------------------------------------

    #[test]
    fn combat_power_basic() {
        let power = compute_combat_power(
            100,    // health -> 25
            80,     // energy -> 8
            &make_personality(Decimal::new(5, 1)), // aggression 0.5 -> 2
            false,
            false,
            0,
        )
        .unwrap();
        // 25 + 2 + 8 + 0 + 0 = 35
        assert_eq!(power, 35);
    }

    #[test]
    fn combat_power_with_tool() {
        let power_tool = compute_combat_power(
            100, 80,
            &make_personality(Decimal::new(5, 1)),
            true, false, 0,
        ).unwrap();
        let power_no_tool = compute_combat_power(
            100, 80,
            &make_personality(Decimal::new(5, 1)),
            false, false, 0,
        ).unwrap();
        assert_eq!(power_tool, power_no_tool.saturating_add(3));
    }

    #[test]
    fn combat_power_with_advanced_tool() {
        let power = compute_combat_power(
            100, 80,
            &make_personality(Decimal::new(5, 1)),
            true, true, 0, // both tools, advanced takes precedence
        ).unwrap();
        let power_base = compute_combat_power(
            100, 80,
            &make_personality(Decimal::new(5, 1)),
            false, false, 0,
        ).unwrap();
        assert_eq!(power, power_base.saturating_add(5));
    }

    #[test]
    fn combat_power_with_allies() {
        let power = compute_combat_power(
            100, 80,
            &make_personality(Decimal::new(5, 1)),
            false, false, 3, // 3 allies -> +6
        ).unwrap();
        let power_base = compute_combat_power(
            100, 80,
            &make_personality(Decimal::new(5, 1)),
            false, false, 0,
        ).unwrap();
        assert_eq!(power, power_base.saturating_add(6));
    }

    #[test]
    fn combat_power_high_aggression() {
        let power_high = compute_combat_power(
            100, 80,
            &make_personality(Decimal::ONE), // aggression 1.0 -> 5
            false, false, 0,
        ).unwrap();
        let power_low = compute_combat_power(
            100, 80,
            &make_personality(Decimal::ZERO), // aggression 0.0 -> 0
            false, false, 0,
        ).unwrap();
        assert_eq!(power_high.saturating_sub(power_low), 5);
    }

    // -----------------------------------------------------------------------
    // Attack resolution tests
    // -----------------------------------------------------------------------

    #[test]
    fn attacker_wins_when_stronger() {
        let loc = LocationId::new();
        let action = CombatAction {
            attacker_id: AgentId::new(),
            defender_id: AgentId::new(),
            intent: CombatIntent::Attack,
            location_id: loc,
        };
        let ctx = CombatContext {
            attacker_personality: make_personality(Decimal::new(9, 1)),
            defender_personality: make_personality(Decimal::new(1, 1)),
            attacker_health: 100,
            defender_health: 50, // weaker
            attacker_energy: 80,
            defender_energy: 30,
            ..default_combat_ctx()
        };

        let result = resolve_combat(&action, &ctx, &BTreeMap::new()).unwrap();
        assert_eq!(result.resolved.winner, Some(action.attacker_id));
        assert!(result.resolved.defender_damage >= MIN_DAMAGE);
        assert_eq!(result.resolved.attacker_energy_cost, ATTACK_ENERGY_COST);
        assert_eq!(result.resolved.defender_energy_cost, DEFEND_ENERGY_COST);
    }

    #[test]
    fn defender_wins_when_stronger() {
        let loc = LocationId::new();
        let action = CombatAction {
            attacker_id: AgentId::new(),
            defender_id: AgentId::new(),
            intent: CombatIntent::Attack,
            location_id: loc,
        };
        let ctx = CombatContext {
            attacker_personality: make_personality(Decimal::new(1, 1)),
            defender_personality: make_personality(Decimal::new(9, 1)),
            attacker_health: 50,
            defender_health: 100,
            attacker_energy: 30,
            defender_energy: 80,
            ..default_combat_ctx()
        };

        let result = resolve_combat(&action, &ctx, &BTreeMap::new()).unwrap();
        assert_eq!(result.resolved.winner, Some(action.defender_id));
        assert!(result.resolved.attacker_damage >= MIN_DAMAGE);
    }

    #[test]
    fn draw_when_equal_power() {
        let loc = LocationId::new();
        let action = CombatAction {
            attacker_id: AgentId::new(),
            defender_id: AgentId::new(),
            intent: CombatIntent::Attack,
            location_id: loc,
        };
        let ctx = CombatContext {
            attacker_personality: make_personality(Decimal::new(5, 1)),
            defender_personality: make_personality(Decimal::new(5, 1)),
            attacker_health: 100,
            defender_health: 100,
            attacker_energy: 80,
            defender_energy: 80,
            attacker_has_tool: false,
            attacker_has_advanced_tool: false,
            defender_has_tool: false,
            defender_has_advanced_tool: false,
            attacker_allies_count: 0,
            defender_allies_count: 0,
        };

        let result = resolve_combat(&action, &ctx, &BTreeMap::new()).unwrap();
        assert_eq!(result.resolved.winner, None);
        assert_eq!(result.resolved.attacker_damage, MIN_DAMAGE);
        assert_eq!(result.resolved.defender_damage, MIN_DAMAGE);
    }

    #[test]
    fn loot_transferred_on_win() {
        let loc = LocationId::new();
        let action = CombatAction {
            attacker_id: AgentId::new(),
            defender_id: AgentId::new(),
            intent: CombatIntent::Attack,
            location_id: loc,
        };
        let ctx = CombatContext {
            attacker_personality: make_personality(Decimal::ONE),
            defender_personality: make_personality(Decimal::ZERO),
            attacker_health: 100,
            defender_health: 30, // much weaker
            attacker_energy: 80,
            defender_energy: 20,
            ..default_combat_ctx()
        };

        let mut defender_inv = BTreeMap::new();
        defender_inv.insert(Resource::Wood, 10);
        defender_inv.insert(Resource::Stone, 5);

        let result = resolve_combat(&action, &ctx, &defender_inv).unwrap();
        assert_eq!(result.resolved.winner, Some(action.attacker_id));
        assert!(!result.resolved.loot_transferred.is_empty());
        assert!(result.resolved.loot_transferred.len() <= MAX_LOOT_ITEMS);
    }

    #[test]
    fn death_flagged_when_health_depleted() {
        let loc = LocationId::new();
        let action = CombatAction {
            attacker_id: AgentId::new(),
            defender_id: AgentId::new(),
            intent: CombatIntent::Attack,
            location_id: loc,
        };
        let ctx = CombatContext {
            attacker_personality: make_personality(Decimal::ONE),
            defender_personality: make_personality(Decimal::ZERO),
            attacker_health: 100,
            defender_health: 3, // Will die from MIN_DAMAGE (5)
            attacker_energy: 80,
            defender_energy: 20,
            ..default_combat_ctx()
        };

        let result = resolve_combat(&action, &ctx, &BTreeMap::new()).unwrap();
        assert!(result.resolved.defender_died);
        assert!(!result.resolved.attacker_died);
    }

    // -----------------------------------------------------------------------
    // Intimidation tests
    // -----------------------------------------------------------------------

    #[test]
    fn intimidation_succeeds_when_much_stronger() {
        let loc = LocationId::new();
        let action = CombatAction {
            attacker_id: AgentId::new(),
            defender_id: AgentId::new(),
            intent: CombatIntent::Intimidate,
            location_id: loc,
        };
        // Make attacker very strong, defender very weak
        let ctx = CombatContext {
            attacker_personality: make_personality(Decimal::ONE),
            defender_personality: make_personality(Decimal::ZERO),
            attacker_health: 100,
            defender_health: 30,
            attacker_energy: 80,
            defender_energy: 20,
            attacker_has_tool: true,
            attacker_has_advanced_tool: false,
            defender_has_tool: false,
            defender_has_advanced_tool: false,
            attacker_allies_count: 2,
            defender_allies_count: 0,
        };

        let result = resolve_combat(&action, &ctx, &BTreeMap::new()).unwrap();
        assert_eq!(result.resolved.winner, Some(action.attacker_id));
        assert_eq!(result.resolved.attacker_damage, 0); // No damage
        assert_eq!(result.resolved.defender_damage, 0); // No damage
        assert_eq!(result.resolved.defender_energy_cost, INTIMIDATION_ENERGY_LOSS);
        assert_eq!(result.resolved.attacker_energy_cost, INTIMIDATE_ENERGY_COST);
    }

    #[test]
    fn intimidation_fails_when_not_strong_enough() {
        let loc = LocationId::new();
        let action = CombatAction {
            attacker_id: AgentId::new(),
            defender_id: AgentId::new(),
            intent: CombatIntent::Intimidate,
            location_id: loc,
        };
        // Equal power -- intimidation requires > 1.5x
        let ctx = CombatContext {
            attacker_personality: make_personality(Decimal::new(5, 1)),
            defender_personality: make_personality(Decimal::new(5, 1)),
            attacker_health: 100,
            defender_health: 100,
            attacker_energy: 80,
            defender_energy: 80,
            attacker_has_tool: false,
            attacker_has_advanced_tool: false,
            defender_has_tool: false,
            defender_has_advanced_tool: false,
            attacker_allies_count: 0,
            defender_allies_count: 0,
        };

        let result = resolve_combat(&action, &ctx, &BTreeMap::new()).unwrap();
        assert_eq!(result.resolved.winner, None); // Failed
        assert_eq!(result.resolved.defender_energy_cost, 0); // No effect
    }

    #[test]
    fn intimidation_no_loot() {
        let loc = LocationId::new();
        let action = CombatAction {
            attacker_id: AgentId::new(),
            defender_id: AgentId::new(),
            intent: CombatIntent::Intimidate,
            location_id: loc,
        };
        let ctx = CombatContext {
            attacker_personality: make_personality(Decimal::ONE),
            defender_personality: make_personality(Decimal::ZERO),
            attacker_health: 100,
            defender_health: 20,
            attacker_energy: 80,
            defender_energy: 10,
            attacker_has_tool: true,
            attacker_has_advanced_tool: true,
            defender_has_tool: false,
            defender_has_advanced_tool: false,
            attacker_allies_count: 3,
            defender_allies_count: 0,
        };

        let mut defender_inv = BTreeMap::new();
        defender_inv.insert(Resource::Wood, 10);

        let result = resolve_combat(&action, &ctx, &defender_inv).unwrap();
        assert!(result.resolved.loot_transferred.is_empty());
    }

    // -----------------------------------------------------------------------
    // Apply tests
    // -----------------------------------------------------------------------

    #[test]
    fn apply_combat_reduces_health_and_energy() {
        let loc = LocationId::new();
        let attacker_id = AgentId::new();
        let defender_id = AgentId::new();
        let mut attacker = make_agent(attacker_id, loc, 100, 80);
        let mut defender = make_agent(defender_id, loc, 100, 60);

        let resolved = CombatResolvedDetails {
            attacker_id,
            defender_id,
            intent: CombatIntent::Attack,
            winner: Some(attacker_id),
            attacker_damage: 5,
            defender_damage: 20,
            attacker_energy_cost: ATTACK_ENERGY_COST,
            defender_energy_cost: DEFEND_ENERGY_COST,
            loot_transferred: BTreeMap::new(),
            attacker_died: false,
            defender_died: false,
            location_id: loc,
        };

        let (a_died, d_died) = apply_combat_result(&mut attacker, &mut defender, &resolved).unwrap();

        assert!(!a_died);
        assert!(!d_died);
        assert_eq!(attacker.health, 95);  // 100 - 5
        assert_eq!(defender.health, 80);  // 100 - 20
        assert_eq!(attacker.energy, 60);  // 80 - 20
        assert_eq!(defender.energy, 50);  // 60 - 10
    }

    #[test]
    fn apply_combat_with_loot_transfer() {
        let loc = LocationId::new();
        let attacker_id = AgentId::new();
        let defender_id = AgentId::new();
        let mut attacker = make_agent(attacker_id, loc, 100, 80);
        let mut defender = make_agent(defender_id, loc, 100, 60);
        defender.inventory.insert(Resource::Wood, 5);
        defender.inventory.insert(Resource::Stone, 3);

        let mut loot = BTreeMap::new();
        loot.insert(Resource::Wood, 1);
        loot.insert(Resource::Stone, 1);

        let resolved = CombatResolvedDetails {
            attacker_id,
            defender_id,
            intent: CombatIntent::Attack,
            winner: Some(attacker_id),
            attacker_damage: 5,
            defender_damage: 10,
            attacker_energy_cost: ATTACK_ENERGY_COST,
            defender_energy_cost: DEFEND_ENERGY_COST,
            loot_transferred: loot,
            attacker_died: false,
            defender_died: false,
            location_id: loc,
        };

        let _ = apply_combat_result(&mut attacker, &mut defender, &resolved).unwrap();

        assert_eq!(attacker.inventory.get(&Resource::Wood).copied(), Some(1));
        assert_eq!(attacker.inventory.get(&Resource::Stone).copied(), Some(1));
        assert_eq!(defender.inventory.get(&Resource::Wood).copied(), Some(4));
        assert_eq!(defender.inventory.get(&Resource::Stone).copied(), Some(2));
    }

    #[test]
    fn apply_combat_defender_death() {
        let loc = LocationId::new();
        let attacker_id = AgentId::new();
        let defender_id = AgentId::new();
        let mut attacker = make_agent(attacker_id, loc, 100, 80);
        let mut defender = make_agent(defender_id, loc, 5, 60); // Low health

        let resolved = CombatResolvedDetails {
            attacker_id,
            defender_id,
            intent: CombatIntent::Attack,
            winner: Some(attacker_id),
            attacker_damage: 5,
            defender_damage: 10,
            attacker_energy_cost: ATTACK_ENERGY_COST,
            defender_energy_cost: DEFEND_ENERGY_COST,
            loot_transferred: BTreeMap::new(),
            attacker_died: false,
            defender_died: true,
            location_id: loc,
        };

        let (a_died, d_died) = apply_combat_result(&mut attacker, &mut defender, &resolved).unwrap();
        assert!(!a_died);
        assert!(d_died);
        assert_eq!(defender.health, 0);
    }

    // -----------------------------------------------------------------------
    // Loot computation tests
    // -----------------------------------------------------------------------

    #[test]
    fn loot_limited_to_max_items() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::Wood, 10);
        inv.insert(Resource::Stone, 10);
        inv.insert(Resource::Water, 10);
        inv.insert(Resource::FoodBerry, 10);
        inv.insert(Resource::FoodFish, 10);
        inv.insert(Resource::FoodRoot, 10);
        inv.insert(Resource::Fiber, 10);

        let loot = compute_loot(&inv);
        assert!(loot.len() <= MAX_LOOT_ITEMS);
    }

    #[test]
    fn loot_empty_inventory() {
        let inv = BTreeMap::new();
        let loot = compute_loot(&inv);
        assert!(loot.is_empty());
    }

    #[test]
    fn loot_takes_one_per_resource() {
        let mut inv = BTreeMap::new();
        inv.insert(Resource::Wood, 10);
        inv.insert(Resource::Stone, 5);

        let loot = compute_loot(&inv);
        assert_eq!(loot.get(&Resource::Wood).copied(), Some(1));
        assert_eq!(loot.get(&Resource::Stone).copied(), Some(1));
    }
}
