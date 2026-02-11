//! Vital mechanics applied to agents each tick.
//!
//! This module implements the per-tick state transitions from
//! `world-engine.md` section 6.2:
//!
//! - Hunger increases by `hunger_rate` per tick
//! - If hunger >= starvation threshold: health decreases by `starvation_damage`
//! - Energy decreases per tick (activity-dependent, handled by caller)
//! - Health regenerates if conditions are met (hunger < 50, energy > 50, sheltered)
//! - Age increments by 1 per tick
//! - Energy cap declines after 80% of lifespan
//!
//! All arithmetic uses checked operations. No panics, no silent overflow.

use emergence_types::AgentState;

use crate::config::VitalsConfig;
use crate::death::{DeathCause, check_death};
use crate::error::AgentError;

/// Result of applying one tick of vital mechanics to an agent.
///
/// If the agent died this tick, `death` will contain the cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VitalTickResult {
    /// If the agent died during this tick, the cause of death.
    pub death: Option<DeathCause>,
}

/// Apply one tick of vital mechanics to an agent's state.
///
/// This corresponds to the "World Wake" phase of the tick cycle
/// (`world-engine.md` section 2.1). The caller is responsible for
/// determining whether the agent is sheltered.
///
/// # Order of operations
///
/// 1. Increment age
/// 2. Check death by old age
/// 3. Increase hunger
/// 4. Apply starvation damage if hunger >= threshold
/// 5. Clamp energy to age-based maximum
/// 6. Apply health regeneration if conditions met
/// 7. Check death by health depletion
pub fn apply_vital_tick(
    state: &mut AgentState,
    config: &VitalsConfig,
    is_sheltered: bool,
) -> Result<VitalTickResult, AgentError> {
    // 1. Age the agent
    state.age = state.age.checked_add(1).ok_or_else(|| AgentError::ArithmeticOverflow {
        context: String::from("age increment overflow"),
    })?;

    // 2. Check death by old age (age > lifespan)
    if let Some(cause) = check_death(state, config)
        && cause == DeathCause::OldAge
    {
        return Ok(VitalTickResult { death: Some(cause) });
    }

    // 3. Increase hunger
    state.hunger = state
        .hunger
        .checked_add(config.hunger_rate)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("hunger increase overflow"),
        })?;
    // Clamp hunger to 100 max (the stat range)
    if state.hunger > 100 {
        state.hunger = 100;
    }

    // 3b. Increase thirst
    state.thirst = state
        .thirst
        .checked_add(config.thirst_per_tick)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("thirst increase overflow"),
        })?;
    // Clamp thirst to 100 max (the stat range)
    if state.thirst > 100 {
        state.thirst = 100;
    }

    // 4. Apply starvation damage when hunger >= threshold
    if state.hunger >= config.starvation_threshold {
        state.health = state.health.saturating_sub(config.starvation_damage);
    }

    // 4b. Apply dehydration damage when thirst >= threshold
    if state.thirst >= config.dehydration_threshold {
        state.health = state.health.saturating_sub(config.dehydration_health_loss);
    }

    // 5. Clamp energy to age-based maximum
    let max_energy = config
        .max_energy_for_age(state.age)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("max_energy_for_age overflow"),
        })?;
    if state.energy > max_energy {
        state.energy = max_energy;
    }

    // 6. Health regeneration: hunger < heal_hunger_threshold AND
    //    energy > heal_energy_threshold AND sheltered
    if state.hunger < config.heal_hunger_threshold
        && state.energy > config.heal_energy_threshold
        && is_sheltered
    {
        state.health = state
            .health
            .checked_add(config.natural_heal_rate)
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("health regeneration overflow"),
            })?;
        // Clamp health to 100
        if state.health > 100 {
            state.health = 100;
        }
    }

    // 7. Final death check (starvation may have reduced health to 0)
    let death = check_death(state, config);

    Ok(VitalTickResult { death })
}

/// Apply energy cost for an action.
///
/// The energy cost is subtracted from the agent's current energy.
/// Energy cannot drop below 0 (saturating subtraction).
pub const fn apply_energy_cost(state: &mut AgentState, cost: u32) {
    state.energy = state.energy.saturating_sub(cost);
}

/// Apply rest recovery to an agent.
///
/// Recovery amount is `config.rest_recovery`, modified by the shelter
/// bonus multiplier. The result is clamped to the age-based energy cap.
///
/// `shelter_bonus_pct` is expressed as a percentage (e.g. 150 means 1.5x).
/// A value of 100 means no bonus.
pub fn apply_rest(
    state: &mut AgentState,
    config: &VitalsConfig,
    shelter_bonus_pct: u32,
) -> Result<(), AgentError> {
    // recovery = rest_recovery * shelter_bonus_pct / 100
    let scaled = config
        .rest_recovery
        .checked_mul(shelter_bonus_pct)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("rest recovery scaling overflow"),
        })?;
    let recovery = scaled.checked_div(100).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("rest recovery division by zero"),
        }
    })?;

    state.energy = state.energy.checked_add(recovery).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("energy addition overflow"),
        }
    })?;

    // Clamp to age-based energy cap
    let max_energy = config
        .max_energy_for_age(state.age)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("max_energy_for_age overflow in rest"),
        })?;
    if state.energy > max_energy {
        state.energy = max_energy;
    }

    Ok(())
}

/// Apply drinking effects: reduce thirst and restore minor energy.
///
/// `thirst_reduction` is the thirst value removed by the water.
/// `energy_gain` is the energy value restored.
/// Both are clamped to valid ranges.
pub fn apply_drink(
    state: &mut AgentState,
    config: &VitalsConfig,
    thirst_reduction: u32,
    energy_gain: u32,
) -> Result<(), AgentError> {
    // Reduce thirst (floor at 0)
    state.thirst = state.thirst.saturating_sub(thirst_reduction);

    // Add energy
    state.energy = state.energy.checked_add(energy_gain).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("energy gain overflow in drink"),
        }
    })?;

    // Clamp energy to age-based cap
    let max_energy = config
        .max_energy_for_age(state.age)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("max_energy_for_age overflow in drink"),
        })?;
    if state.energy > max_energy {
        state.energy = max_energy;
    }

    Ok(())
}

/// Apply eating effects: reduce hunger and restore energy.
///
/// `hunger_reduction` is the hunger value removed by the food.
/// `energy_gain` is the energy value restored by the food.
/// Both are clamped to valid ranges.
pub fn apply_eat(
    state: &mut AgentState,
    config: &VitalsConfig,
    hunger_reduction: u32,
    energy_gain: u32,
) -> Result<(), AgentError> {
    // Reduce hunger (floor at 0)
    state.hunger = state.hunger.saturating_sub(hunger_reduction);

    // Add energy
    state.energy = state.energy.checked_add(energy_gain).ok_or_else(|| {
        AgentError::ArithmeticOverflow {
            context: String::from("energy gain overflow in eat"),
        }
    })?;

    // Clamp energy to age-based cap
    let max_energy = config
        .max_energy_for_age(state.age)
        .ok_or_else(|| AgentError::ArithmeticOverflow {
            context: String::from("max_energy_for_age overflow in eat"),
        })?;
    if state.energy > max_energy {
        state.energy = max_energy;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use emergence_types::{AgentId, LocationId};

    use super::*;

    /// Create a fresh agent state with default vitals for testing.
    fn test_state() -> AgentState {
        AgentState {
            agent_id: AgentId::new(),
            energy: 80,
            health: 100,
            hunger: 0,
            thirst: 0,
            age: 0,
            born_at_tick: 0,
            location_id: LocationId::new(),
            destination_id: None,
            travel_progress: 0,
            inventory: BTreeMap::new(),
            carry_capacity: 50,
            knowledge: std::collections::BTreeSet::new(),
            skills: BTreeMap::new(),
            skill_xp: BTreeMap::new(),
            goals: Vec::new(),
            relationships: BTreeMap::new(),
            memory: Vec::new(),
        }
    }

    #[test]
    fn age_increments_each_tick() {
        let mut state = test_state();
        let config = VitalsConfig::default();
        let result = apply_vital_tick(&mut state, &config, false);
        assert!(result.is_ok());
        assert_eq!(state.age, 1);
    }

    #[test]
    fn hunger_increases_each_tick() {
        let mut state = test_state();
        let config = VitalsConfig::default();
        let _ = apply_vital_tick(&mut state, &config, false);
        assert_eq!(state.hunger, 5);
    }

    #[test]
    fn hunger_clamped_to_100() {
        let mut state = test_state();
        state.hunger = 98;
        let config = VitalsConfig::default();
        let _ = apply_vital_tick(&mut state, &config, false);
        assert_eq!(state.hunger, 100);
    }

    #[test]
    fn starvation_damage_at_max_hunger() {
        let mut state = test_state();
        state.hunger = 96; // Will become 101 -> clamped to 100
        let config = VitalsConfig::default();
        let _ = apply_vital_tick(&mut state, &config, false);
        assert_eq!(state.hunger, 100);
        assert_eq!(state.health, 90); // 100 - 10 starvation
    }

    #[test]
    fn no_starvation_below_threshold() {
        let mut state = test_state();
        state.hunger = 50;
        let config = VitalsConfig::default();
        let _ = apply_vital_tick(&mut state, &config, false);
        assert_eq!(state.hunger, 55);
        assert_eq!(state.health, 100); // No starvation damage
    }

    #[test]
    fn health_regeneration_when_conditions_met() {
        let mut state = test_state();
        state.hunger = 0;
        state.energy = 80;
        state.health = 90;
        let config = VitalsConfig::default();
        // Sheltered = true
        let _ = apply_vital_tick(&mut state, &config, true);
        // hunger=5 < 50, energy=80 > 50, sheltered: heal +2
        assert_eq!(state.health, 92);
    }

    #[test]
    fn no_health_regeneration_when_not_sheltered() {
        let mut state = test_state();
        state.hunger = 0;
        state.energy = 80;
        state.health = 90;
        let config = VitalsConfig::default();
        let _ = apply_vital_tick(&mut state, &config, false);
        assert_eq!(state.health, 90);
    }

    #[test]
    fn no_health_regeneration_when_too_hungry() {
        let mut state = test_state();
        state.hunger = 46; // Will become 51 after tick, which is >= 50
        state.energy = 80;
        state.health = 90;
        let config = VitalsConfig::default();
        let _ = apply_vital_tick(&mut state, &config, true);
        // hunger after tick = 51 >= 50, so no healing
        assert_eq!(state.health, 90);
    }

    #[test]
    fn no_health_regeneration_when_low_energy() {
        let mut state = test_state();
        state.hunger = 0;
        state.energy = 50; // Not > 50
        state.health = 90;
        let config = VitalsConfig::default();
        let _ = apply_vital_tick(&mut state, &config, true);
        assert_eq!(state.health, 90);
    }

    #[test]
    fn health_clamped_to_100_on_regen() {
        let mut state = test_state();
        state.hunger = 0;
        state.energy = 80;
        state.health = 99;
        let config = VitalsConfig::default();
        let _ = apply_vital_tick(&mut state, &config, true);
        assert_eq!(state.health, 100);
    }

    #[test]
    fn death_by_old_age() {
        let mut state = test_state();
        let config = VitalsConfig::default();
        // Set age to lifespan so after +1 it exceeds
        state.age = config.lifespan;
        let result = apply_vital_tick(&mut state, &config, false);
        assert!(result.is_ok());
        let vr = result.ok();
        assert!(vr.is_some());
        let vr = vr.unwrap_or(VitalTickResult { death: None });
        assert_eq!(vr.death, Some(DeathCause::OldAge));
    }

    #[test]
    fn death_by_starvation() {
        let mut state = test_state();
        state.health = 5;
        state.hunger = 96; // Will cross 100
        let config = VitalsConfig::default();
        let result = apply_vital_tick(&mut state, &config, false);
        assert!(result.is_ok());
        let vr = result.ok().unwrap_or(VitalTickResult { death: None });
        assert_eq!(vr.death, Some(DeathCause::Starvation));
    }

    #[test]
    fn energy_cost_applied() {
        let mut state = test_state();
        state.energy = 50;
        apply_energy_cost(&mut state, 10);
        assert_eq!(state.energy, 40);
    }

    #[test]
    fn energy_cost_saturates_at_zero() {
        let mut state = test_state();
        state.energy = 5;
        apply_energy_cost(&mut state, 20);
        assert_eq!(state.energy, 0);
    }

    #[test]
    fn rest_recovery_no_bonus() {
        let mut state = test_state();
        state.energy = 20;
        let config = VitalsConfig::default();
        let result = apply_rest(&mut state, &config, 100);
        assert!(result.is_ok());
        assert_eq!(state.energy, 50); // 20 + 30
    }

    #[test]
    fn rest_recovery_with_shelter_bonus() {
        let mut state = test_state();
        state.energy = 20;
        let config = VitalsConfig::default();
        let result = apply_rest(&mut state, &config, 150); // 1.5x
        assert!(result.is_ok());
        assert_eq!(state.energy, 65); // 20 + 45
    }

    #[test]
    fn rest_recovery_clamped_to_max_energy() {
        let mut state = test_state();
        state.energy = 90;
        let config = VitalsConfig::default();
        let result = apply_rest(&mut state, &config, 150);
        assert!(result.is_ok());
        assert_eq!(state.energy, 100); // Clamped
    }

    #[test]
    fn eat_reduces_hunger_and_adds_energy() {
        let mut state = test_state();
        state.hunger = 60;
        state.energy = 50;
        let config = VitalsConfig::default();
        let result = apply_eat(&mut state, &config, 30, 10);
        assert!(result.is_ok());
        assert_eq!(state.hunger, 30);
        assert_eq!(state.energy, 60);
    }

    #[test]
    fn eat_hunger_floors_at_zero() {
        let mut state = test_state();
        state.hunger = 10;
        state.energy = 50;
        let config = VitalsConfig::default();
        let result = apply_eat(&mut state, &config, 30, 5);
        assert!(result.is_ok());
        assert_eq!(state.hunger, 0);
    }

    #[test]
    fn eat_energy_clamped() {
        let mut state = test_state();
        state.hunger = 20;
        state.energy = 95;
        let config = VitalsConfig::default();
        let result = apply_eat(&mut state, &config, 20, 20);
        assert!(result.is_ok());
        assert_eq!(state.energy, 100);
    }

    #[test]
    fn multi_tick_starvation_death() {
        let mut state = test_state();
        let config = VitalsConfig::default();
        // Simulate enough ticks to starve to death
        let mut dead = false;
        for _ in 0..100 {
            let result = apply_vital_tick(&mut state, &config, false);
            if let Ok(vr) = result
                && vr.death.is_some()
            {
                dead = true;
                break;
            }
        }
        assert!(dead, "Agent should have died from starvation within 100 ticks");
    }

    #[test]
    fn energy_cap_declines_in_old_age() {
        let mut state = test_state();
        state.age = 2249; // Will become 2250 after tick (halfway through decline)
        state.energy = 100;
        let config = VitalsConfig::default();
        let _ = apply_vital_tick(&mut state, &config, false);
        // max_energy at age 2250 = 75
        assert_eq!(state.energy, 75);
    }
}
