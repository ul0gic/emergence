//! Energy costs and food values for each action type.
//!
//! Per `world-engine.md` section 7.1, each action has a defined energy cost.
//! Food items have hunger reduction and energy gain values.
//!
//! All values are `u32` -- no floating point.

use emergence_types::{ActionType, Resource};

/// Return the energy cost for a given action type.
///
/// Values from `world-engine.md` section 7.1:
/// - Gather: 10
/// - Eat: 0
/// - Drink: 0
/// - Rest: 0
/// - Move: 15 (per tick of travel)
/// - Build: 25
/// - Repair: 15
/// - Demolish: 20
/// - `ImproveRoute`: 30
/// - Communicate: 2
/// - Broadcast: 5
/// - `TradeOffer`: 2
/// - `TradeAccept`: 0
/// - `TradeReject`: 0
/// - `FormGroup`: 5
/// - Teach: 10
/// - `FarmPlant`: 20
/// - `FarmHarvest`: 10
/// - Craft: 15
/// - Mine: 20
/// - Smelt: 20
/// - Write: 5
/// - Read: 5
/// - Claim: 5
/// - Legislate: 10
/// - Enforce: 15
/// - Reproduce: 30
/// - `NoAction`: 0
#[allow(clippy::match_same_arms)] // Each action has its own spec-defined cost; keeping them separate for traceability.
pub const fn energy_cost(action: ActionType) -> u32 {
    match action {
        ActionType::Gather => 10,
        ActionType::Eat => 0,
        ActionType::Drink => 0,
        ActionType::Rest => 0,
        ActionType::Move => 15,
        ActionType::Build => 25,
        ActionType::Repair => 15,
        ActionType::Demolish => 20,
        ActionType::ImproveRoute => 30,
        ActionType::Communicate => 2,
        ActionType::Broadcast => 5,
        ActionType::TradeOffer => 2,
        ActionType::TradeAccept => 0,
        ActionType::TradeReject => 0,
        ActionType::FormGroup => 5,
        ActionType::Teach => 10,
        ActionType::FarmPlant => 20,
        ActionType::FarmHarvest => 10,
        ActionType::Craft => 15,
        ActionType::Mine => 20,
        ActionType::Smelt => 20,
        ActionType::Write => 5,
        ActionType::Read => 5,
        ActionType::Claim => 5,
        ActionType::Legislate => 10,
        ActionType::Enforce => 15,
        ActionType::Reproduce => 30,
        ActionType::NoAction => 0,
    }
}

/// Hunger reduction and energy gain from eating a food resource.
///
/// Returns `(hunger_reduction, energy_gain)`.
///
/// Values from `world-engine.md` section 6.2:
/// - Berries: hunger -20, energy +5
/// - Fish: hunger -30, energy +10
/// - Roots: hunger -15, energy +5
/// - Meat: hunger -35, energy +15
/// - Farmed food: hunger -40, energy +15
/// - Cooked food: hunger -50, energy +20
///
/// Returns `None` for non-food resources.
#[allow(clippy::match_same_arms)] // Each food type has its own spec-defined values; keeping them separate for traceability.
pub const fn food_values(resource: Resource) -> Option<(u32, u32)> {
    match resource {
        Resource::FoodBerry => Some((20, 5)),
        Resource::FoodFish => Some((30, 10)),
        Resource::FoodRoot => Some((15, 5)),
        Resource::FoodMeat => Some((35, 15)),
        Resource::FoodFarmed => Some((40, 15)),
        Resource::FoodCooked => Some((50, 20)),
        _ => None,
    }
}

/// Check whether a resource is a food type that can be consumed with `eat`.
pub const fn is_food(resource: Resource) -> bool {
    food_values(resource).is_some()
}

/// Check whether a resource is a water source for `drink`.
pub const fn is_water(resource: Resource) -> bool {
    matches!(resource, Resource::Water)
}

/// Base gather yield (units per gather action).
///
/// Skill-modified yield is `base + (skill_level * 0.5)`, computed as
/// `base + skill_level / 2` in integer arithmetic.
pub const BASE_GATHER_YIELD: u32 = 3;

/// Base mining yield (units of ore per mine action).
///
/// Skill-modified yield is `base + (mining_skill / 2)`.
pub const BASE_MINE_YIELD: u32 = 2;

/// Ore consumed per smelt action.
pub const SMELT_ORE_INPUT: u32 = 2;

/// Wood consumed per smelt action (fuel).
pub const SMELT_WOOD_INPUT: u32 = 1;

/// Metal produced per smelt action.
pub const SMELT_METAL_OUTPUT: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn survival_action_costs_match_spec() {
        assert_eq!(energy_cost(ActionType::Gather), 10);
        assert_eq!(energy_cost(ActionType::Eat), 0);
        assert_eq!(energy_cost(ActionType::Drink), 0);
        assert_eq!(energy_cost(ActionType::Rest), 0);
        assert_eq!(energy_cost(ActionType::Move), 15);
    }

    #[test]
    fn no_action_is_free() {
        assert_eq!(energy_cost(ActionType::NoAction), 0);
    }

    #[test]
    fn food_values_correct() {
        assert_eq!(food_values(Resource::FoodBerry), Some((20, 5)));
        assert_eq!(food_values(Resource::FoodFish), Some((30, 10)));
        assert_eq!(food_values(Resource::FoodCooked), Some((50, 20)));
    }

    #[test]
    fn non_food_returns_none() {
        assert_eq!(food_values(Resource::Wood), None);
        assert_eq!(food_values(Resource::Stone), None);
        assert_eq!(food_values(Resource::Water), None);
    }

    #[test]
    fn is_food_checks() {
        assert!(is_food(Resource::FoodBerry));
        assert!(is_food(Resource::FoodFish));
        assert!(!is_food(Resource::Wood));
        assert!(!is_food(Resource::Water));
    }

    #[test]
    fn is_water_checks() {
        assert!(is_water(Resource::Water));
        assert!(!is_water(Resource::Wood));
        assert!(!is_water(Resource::FoodBerry));
    }
}
