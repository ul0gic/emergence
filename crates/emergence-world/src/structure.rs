//! Structure blueprints, decay mechanics, repair costs, and location effects.
//!
//! Implements `world-engine.md` sections 5.1 through 5.3:
//!
//! - [`blueprint`] returns the static blueprint for each [`StructureType`]
//! - [`apply_decay`] reduces durability by `decay_per_tick`, accounting for
//!   weather and occupancy
//! - [`compute_salvage`] calculates the 30% material recovery on collapse or
//!   demolition
//! - [`compute_repair_cost`] scales materials proportional to missing durability
//! - [`structure_effects_at_location`] aggregates effects from all standing
//!   structures into a [`LocationEffects`]

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use emergence_types::{
    LocationEffects, Resource, Structure, StructureBlueprint, StructureCategory, StructureProperties,
    StructureType, Weather,
};

use crate::error::WorldError;

// ---------------------------------------------------------------------------
// Blueprints (world-engine.md section 5.2)
// ---------------------------------------------------------------------------

/// Return the canonical blueprint for a given [`StructureType`].
///
/// Blueprints define material costs, required knowledge, durability
/// settings, capacity, and properties. Values are from `world-engine.md`
/// section 5.2.
#[allow(clippy::too_many_lines)] // Each structure type has unique configuration; splitting would obscure the blueprint table.
pub fn blueprint(structure_type: StructureType) -> StructureBlueprint {
    match structure_type {
        // ---- Tier 0: Primitive ----
        StructureType::Campfire => StructureBlueprint {
            structure_type: StructureType::Campfire,
            category: StructureCategory::Utility,
            material_costs: BTreeMap::from([(Resource::Wood, 3)]),
            required_knowledge: String::from("build_campfire"),
            max_durability: 50,
            decay_per_tick: Decimal::ONE,
            capacity: 0,
            properties: StructureProperties {
                rest_bonus: Decimal::ONE,
                weather_protection: false,
                storage_slots: 0,
                production_type: None,
                production_rate: 0,
            },
        },
        StructureType::LeanTo => StructureBlueprint {
            structure_type: StructureType::LeanTo,
            category: StructureCategory::Shelter,
            material_costs: BTreeMap::from([(Resource::Wood, 8)]),
            required_knowledge: String::from("build_lean_to"),
            max_durability: 60,
            decay_per_tick: Decimal::new(8, 1), // 0.8
            capacity: 2,
            properties: StructureProperties {
                rest_bonus: Decimal::new(12, 1), // 1.2
                weather_protection: false,
                storage_slots: 0,
                production_type: None,
                production_rate: 0,
            },
        },
        StructureType::BasicHut => StructureBlueprint {
            structure_type: StructureType::BasicHut,
            category: StructureCategory::Shelter,
            material_costs: BTreeMap::from([
                (Resource::Wood, 20),
                (Resource::Stone, 10),
            ]),
            required_knowledge: String::from("build_hut"),
            max_durability: 100,
            decay_per_tick: Decimal::new(5, 1), // 0.5
            capacity: 4,
            properties: StructureProperties {
                rest_bonus: Decimal::new(15, 1), // 1.5
                weather_protection: true,
                storage_slots: 20,
                production_type: None,
                production_rate: 0,
            },
        },

        // ---- Tier 1: Developed ----
        StructureType::StoragePit => StructureBlueprint {
            structure_type: StructureType::StoragePit,
            category: StructureCategory::Storage,
            material_costs: BTreeMap::from([
                (Resource::Stone, 10),
                (Resource::Wood, 5),
            ]),
            required_knowledge: String::from("build_storage"),
            max_durability: 80,
            decay_per_tick: Decimal::new(3, 1), // 0.3
            capacity: 0,
            properties: StructureProperties {
                rest_bonus: Decimal::ONE,
                weather_protection: false,
                storage_slots: 50,
                production_type: None,
                production_rate: 0,
            },
        },
        StructureType::Well => StructureBlueprint {
            structure_type: StructureType::Well,
            category: StructureCategory::Production,
            material_costs: BTreeMap::from([(Resource::Stone, 20)]),
            required_knowledge: String::from("masonry"),
            max_durability: 120,
            decay_per_tick: Decimal::new(2, 1), // 0.2
            capacity: 0,
            properties: StructureProperties {
                rest_bonus: Decimal::ONE,
                weather_protection: false,
                storage_slots: 0,
                production_type: Some(Resource::Water),
                production_rate: 5,
            },
        },
        StructureType::FarmPlot => StructureBlueprint {
            structure_type: StructureType::FarmPlot,
            category: StructureCategory::Production,
            material_costs: BTreeMap::from([(Resource::Wood, 15)]),
            required_knowledge: String::from("agriculture"),
            max_durability: 60,
            decay_per_tick: Decimal::new(4, 1), // 0.4
            capacity: 0,
            properties: StructureProperties {
                rest_bonus: Decimal::ONE,
                weather_protection: false,
                storage_slots: 0,
                production_type: Some(Resource::FoodFarmed),
                production_rate: 2,
            },
        },
        StructureType::Workshop => StructureBlueprint {
            structure_type: StructureType::Workshop,
            category: StructureCategory::Production,
            material_costs: BTreeMap::from([
                (Resource::Wood, 30),
                (Resource::Stone, 20),
            ]),
            required_knowledge: String::from("build_workshop"),
            max_durability: 100,
            decay_per_tick: Decimal::new(4, 1), // 0.4
            capacity: 4,
            properties: StructureProperties {
                rest_bonus: Decimal::ONE,
                weather_protection: true,
                storage_slots: 10,
                production_type: None,
                production_rate: 0,
            },
        },
        StructureType::MeetingHall => StructureBlueprint {
            structure_type: StructureType::MeetingHall,
            category: StructureCategory::Social,
            material_costs: BTreeMap::from([
                (Resource::Wood, 50),
                (Resource::Stone, 30),
            ]),
            required_knowledge: String::from("group_formation"),
            max_durability: 120,
            decay_per_tick: Decimal::new(5, 1), // 0.5
            capacity: 10,
            properties: StructureProperties {
                rest_bonus: Decimal::new(11, 1), // 1.1
                weather_protection: true,
                storage_slots: 0,
                production_type: None,
                production_rate: 0,
            },
        },

        // ---- Tier 2: Advanced ----
        StructureType::Forge => StructureBlueprint {
            structure_type: StructureType::Forge,
            category: StructureCategory::Production,
            material_costs: BTreeMap::from([
                (Resource::Stone, 40),
                (Resource::Wood, 20),
            ]),
            required_knowledge: String::from("build_forge"),
            max_durability: 150,
            decay_per_tick: Decimal::new(6, 1), // 0.6
            capacity: 2,
            properties: StructureProperties {
                rest_bonus: Decimal::ONE,
                weather_protection: true,
                storage_slots: 5,
                production_type: None,
                production_rate: 0,
            },
        },
        StructureType::Library => StructureBlueprint {
            structure_type: StructureType::Library,
            category: StructureCategory::Knowledge,
            material_costs: BTreeMap::from([
                (Resource::Wood, 60),
                (Resource::Stone, 40),
            ]),
            required_knowledge: String::from("build_library"),
            max_durability: 120,
            decay_per_tick: Decimal::new(4, 1), // 0.4
            capacity: 6,
            properties: StructureProperties {
                rest_bonus: Decimal::new(11, 1), // 1.1
                weather_protection: true,
                storage_slots: 30,
                production_type: None,
                production_rate: 0,
            },
        },
        StructureType::Market => StructureBlueprint {
            structure_type: StructureType::Market,
            category: StructureCategory::Economic,
            material_costs: BTreeMap::from([
                (Resource::Wood, 50),
                (Resource::Stone, 30),
            ]),
            required_knowledge: String::from("build_market"),
            max_durability: 100,
            decay_per_tick: Decimal::new(5, 1), // 0.5
            capacity: 8,
            properties: StructureProperties {
                rest_bonus: Decimal::ONE,
                weather_protection: true,
                storage_slots: 20,
                production_type: None,
                production_rate: 0,
            },
        },
        StructureType::Wall => StructureBlueprint {
            structure_type: StructureType::Wall,
            category: StructureCategory::Defense,
            material_costs: BTreeMap::from([
                (Resource::Stone, 100),
                (Resource::Wood, 50),
            ]),
            required_knowledge: String::from("build_wall"),
            max_durability: 200,
            decay_per_tick: Decimal::new(3, 1), // 0.3
            capacity: 0,
            properties: StructureProperties {
                rest_bonus: Decimal::ONE,
                weather_protection: false,
                storage_slots: 0,
                production_type: None,
                production_rate: 0,
            },
        },
        StructureType::Bridge => StructureBlueprint {
            structure_type: StructureType::Bridge,
            category: StructureCategory::Infrastructure,
            material_costs: BTreeMap::from([
                (Resource::Wood, 80),
                (Resource::Stone, 40),
            ]),
            required_knowledge: String::from("bridge_building"),
            max_durability: 150,
            decay_per_tick: Decimal::new(5, 1), // 0.5
            capacity: 0,
            properties: StructureProperties {
                rest_bonus: Decimal::ONE,
                weather_protection: false,
                storage_slots: 0,
                production_type: None,
                production_rate: 0,
            },
        },
    }
}

// ---------------------------------------------------------------------------
// Decay (world-engine.md section 5.3)
// ---------------------------------------------------------------------------

/// Percentage multiplier constants for decay modifiers.
///
/// Weather events accelerate decay: storms add +100%, snow adds +50%.
/// Occupied structures decay slower: 75% of normal rate.
const STORM_DECAY_MULTIPLIER_PCT: u32 = 200;
const SNOW_DECAY_MULTIPLIER_PCT: u32 = 150;
const NORMAL_DECAY_MULTIPLIER_PCT: u32 = 100;
const OCCUPIED_DECAY_MULTIPLIER_PCT: u32 = 75;

/// Apply one tick of decay to a structure.
///
/// Returns `true` if the structure collapsed (durability reached 0).
///
/// The effective decay rate is:
/// - Base `decay_per_tick` from the structure
/// - Multiplied by weather factor (storm: +100%, snow: +50%)
/// - Reduced if occupied (75% of normal)
///
/// Durability is tracked as `u32` while `decay_per_tick` is [`Decimal`].
/// The decay amount is computed in [`Decimal`] arithmetic and then
/// converted to the nearest integer (minimum 0).
///
/// # Errors
///
/// Returns [`WorldError::ArithmeticOverflow`] on arithmetic failure.
pub fn apply_decay(
    structure: &mut Structure,
    weather: Weather,
) -> Result<bool, WorldError> {
    // Weather multiplier
    let weather_pct = match weather {
        Weather::Storm => STORM_DECAY_MULTIPLIER_PCT,
        Weather::Snow => SNOW_DECAY_MULTIPLIER_PCT,
        Weather::Clear | Weather::Rain | Weather::Drought => NORMAL_DECAY_MULTIPLIER_PCT,
    };

    // Occupancy multiplier (occupied structures decay slower)
    let occupancy_pct = if structure.occupants.is_empty() {
        NORMAL_DECAY_MULTIPLIER_PCT
    } else {
        OCCUPIED_DECAY_MULTIPLIER_PCT
    };

    // Effective decay = decay_per_tick * (weather_pct / 100) * (occupancy_pct / 100)
    let weather_factor = Decimal::from(weather_pct);
    let occupancy_factor = Decimal::from(occupancy_pct);
    let hundred = Decimal::from(100);

    let effective = structure
        .decay_per_tick
        .checked_mul(weather_factor)
        .ok_or(WorldError::ArithmeticOverflow)?
        .checked_div(hundred)
        .ok_or(WorldError::ArithmeticOverflow)?
        .checked_mul(occupancy_factor)
        .ok_or(WorldError::ArithmeticOverflow)?
        .checked_div(hundred)
        .ok_or(WorldError::ArithmeticOverflow)?;

    // Convert to integer decay (round down, minimum 0)
    let decay_amount = decimal_to_u32_floor(effective);

    structure.durability = structure.durability.saturating_sub(decay_amount);

    Ok(structure.durability == 0)
}

// ---------------------------------------------------------------------------
// Salvage (world-engine.md section 5.3)
// ---------------------------------------------------------------------------

/// Salvage recovery percentage for collapsed or demolished structures.
const SALVAGE_PERCENTAGE: u32 = 30;

/// Compute the salvageable materials from a structure (30% of original cost).
///
/// Used when a structure collapses from decay or is demolished by an agent.
///
/// # Errors
///
/// Returns [`WorldError::ArithmeticOverflow`] on arithmetic failure.
pub fn compute_salvage(
    materials_used: &BTreeMap<Resource, u32>,
) -> Result<BTreeMap<Resource, u32>, WorldError> {
    let mut salvage = BTreeMap::new();
    for (&resource, &quantity) in materials_used {
        let recovered = quantity
            .checked_mul(SALVAGE_PERCENTAGE)
            .ok_or(WorldError::ArithmeticOverflow)?
            .checked_div(100)
            .ok_or(WorldError::ArithmeticOverflow)?;
        if recovered > 0 {
            salvage.insert(resource, recovered);
        }
    }
    Ok(salvage)
}

// ---------------------------------------------------------------------------
// Repair Cost (world-engine.md section 5.3)
// ---------------------------------------------------------------------------

/// Compute the material cost to fully repair a structure.
///
/// The cost scales proportionally with missing durability:
/// `cost = original_cost * (max - current) / max`
///
/// # Errors
///
/// Returns [`WorldError::ArithmeticOverflow`] on arithmetic failure.
pub fn compute_repair_cost(
    materials_used: &BTreeMap<Resource, u32>,
    current_durability: u32,
    max_durability: u32,
) -> Result<BTreeMap<Resource, u32>, WorldError> {
    if max_durability == 0 {
        return Ok(BTreeMap::new());
    }

    let missing = max_durability
        .checked_sub(current_durability)
        .ok_or(WorldError::ArithmeticOverflow)?;

    if missing == 0 {
        return Ok(BTreeMap::new());
    }

    let mut costs = BTreeMap::new();
    for (&resource, &quantity) in materials_used {
        let cost = quantity
            .checked_mul(missing)
            .ok_or(WorldError::ArithmeticOverflow)?
            .checked_div(max_durability)
            .ok_or(WorldError::ArithmeticOverflow)?;
        if cost > 0 {
            costs.insert(resource, cost);
        }
    }
    Ok(costs)
}

/// Apply repair to a structure, restoring durability to maximum.
///
/// Sets `structure.durability = structure.max_durability`.
pub const fn apply_repair(structure: &mut Structure) {
    structure.durability = structure.max_durability;
}

// ---------------------------------------------------------------------------
// Location Effects (Task 4.1.5)
// ---------------------------------------------------------------------------

/// Aggregate the effects of all standing structures at a location.
///
/// Returns a [`LocationEffects`] describing the combined bonuses from
/// all provided structures. Only structures with `durability > 0` and
/// no `destroyed_at_tick` are considered "standing."
pub fn structure_effects_at_location(structures: &[Structure]) -> LocationEffects {
    let mut effects = LocationEffects {
        weather_protection: false,
        best_rest_bonus_pct: 100,
        total_storage_slots: 0,
        has_shelter: false,
        has_fire: false,
        production: BTreeMap::new(),
    };

    for s in structures {
        // Skip destroyed or fully decayed structures
        if s.destroyed_at_tick.is_some() || s.durability == 0 {
            continue;
        }

        // Weather protection
        if s.properties.weather_protection {
            effects.weather_protection = true;
        }

        // Rest bonus (keep the best one, convert Decimal to pct)
        let bonus_pct = decimal_to_pct(s.properties.rest_bonus);
        if bonus_pct > effects.best_rest_bonus_pct {
            effects.best_rest_bonus_pct = bonus_pct;
        }

        // Storage slots
        effects.total_storage_slots = effects
            .total_storage_slots
            .saturating_add(s.properties.storage_slots);

        // Shelter detection
        if matches!(
            s.structure_type,
            StructureType::LeanTo | StructureType::BasicHut
        ) {
            effects.has_shelter = true;
        }

        // Fire detection
        if s.structure_type == StructureType::Campfire {
            effects.has_fire = true;
        }

        // Production
        if let Some(prod_resource) = s.properties.production_type {
            let entry = effects.production.entry(prod_resource).or_insert(0);
            *entry = entry.saturating_add(s.properties.production_rate);
        }
    }

    effects
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a [`Decimal`] to `u32` by truncating toward zero.
///
/// Negative values and conversion failures return 0.
fn decimal_to_u32_floor(d: Decimal) -> u32 {
    let truncated = d.trunc();
    let mantissa = truncated.mantissa();
    let scale = truncated.scale();
    let divisor: i128 = 10_i128.checked_pow(scale).unwrap_or(1);
    let val = mantissa.checked_div(divisor).unwrap_or(0);
    if val < 0 {
        0
    } else if val > i128::from(u32::MAX) {
        u32::MAX
    } else {
        // Safety: we verified 0 <= val <= u32::MAX.
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let result = val as u32;
        result
    }
}

/// Convert a [`Decimal`] multiplier (e.g. 1.5) to a percentage `u32` (e.g. 150).
///
/// Returns 100 on conversion failure.
fn decimal_to_pct(d: Decimal) -> u32 {
    let hundred = Decimal::from(100);
    let scaled = d.checked_mul(hundred).unwrap_or(hundred);
    decimal_to_u32_floor(scaled)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeSet;

    use emergence_types::{
        AgentId, LocationId, Resource, Structure, StructureId, StructureType, Weather,
    };
    use rust_decimal::Decimal;

    use super::*;

    fn make_structure(st: StructureType) -> Structure {
        let bp = blueprint(st);
        Structure {
            id: StructureId::new(),
            structure_type: st,
            subtype: None,
            location_id: LocationId::new(),
            builder: AgentId::new(),
            owner: Some(AgentId::new()),
            built_at_tick: 10,
            destroyed_at_tick: None,
            materials_used: bp.material_costs.clone(),
            durability: bp.max_durability,
            max_durability: bp.max_durability,
            decay_per_tick: bp.decay_per_tick,
            capacity: bp.capacity,
            occupants: BTreeSet::new(),
            access_list: None,
            properties: bp.properties,
        }
    }

    // -----------------------------------------------------------------------
    // Blueprint tests
    // -----------------------------------------------------------------------

    #[test]
    fn campfire_blueprint_costs() {
        let bp = blueprint(StructureType::Campfire);
        assert_eq!(bp.material_costs.get(&Resource::Wood).copied(), Some(3));
        assert_eq!(bp.required_knowledge, "build_campfire");
        assert_eq!(bp.max_durability, 50);
    }

    #[test]
    fn basic_hut_blueprint_costs() {
        let bp = blueprint(StructureType::BasicHut);
        assert_eq!(bp.material_costs.get(&Resource::Wood).copied(), Some(20));
        assert_eq!(bp.material_costs.get(&Resource::Stone).copied(), Some(10));
        assert_eq!(bp.required_knowledge, "build_hut");
        assert!(bp.properties.weather_protection);
        assert_eq!(bp.properties.rest_bonus, Decimal::new(15, 1));
    }

    #[test]
    fn lean_to_blueprint_costs() {
        let bp = blueprint(StructureType::LeanTo);
        assert_eq!(bp.material_costs.get(&Resource::Wood).copied(), Some(8));
        assert_eq!(bp.required_knowledge, "build_lean_to");
        assert_eq!(bp.properties.rest_bonus, Decimal::new(12, 1));
    }

    #[test]
    fn all_13_structure_types_have_blueprints() {
        let types = [
            StructureType::Campfire,
            StructureType::LeanTo,
            StructureType::BasicHut,
            StructureType::StoragePit,
            StructureType::Well,
            StructureType::FarmPlot,
            StructureType::Workshop,
            StructureType::MeetingHall,
            StructureType::Forge,
            StructureType::Library,
            StructureType::Market,
            StructureType::Wall,
            StructureType::Bridge,
        ];
        for st in types {
            let bp = blueprint(st);
            assert_eq!(bp.structure_type, st);
            assert!(!bp.material_costs.is_empty());
            assert!(!bp.required_knowledge.is_empty());
            assert!(bp.max_durability > 0);
        }
    }

    #[test]
    fn wall_has_highest_stone_cost() {
        let bp = blueprint(StructureType::Wall);
        assert_eq!(bp.material_costs.get(&Resource::Stone).copied(), Some(100));
        assert_eq!(bp.material_costs.get(&Resource::Wood).copied(), Some(50));
    }

    #[test]
    fn well_produces_water() {
        let bp = blueprint(StructureType::Well);
        assert_eq!(bp.properties.production_type, Some(Resource::Water));
        assert_eq!(bp.properties.production_rate, 5);
    }

    #[test]
    fn farm_plot_produces_food() {
        let bp = blueprint(StructureType::FarmPlot);
        assert_eq!(bp.properties.production_type, Some(Resource::FoodFarmed));
        assert_eq!(bp.properties.production_rate, 2);
    }

    // -----------------------------------------------------------------------
    // Decay tests
    // -----------------------------------------------------------------------

    #[test]
    fn decay_reduces_durability() {
        let mut s = make_structure(StructureType::Campfire);
        // Campfire: decay_per_tick = 1, clear weather, unoccupied
        let collapsed = apply_decay(&mut s, Weather::Clear).unwrap();
        assert!(!collapsed);
        assert_eq!(s.durability, 49);
    }

    #[test]
    fn decay_storm_doubles_rate() {
        let mut s = make_structure(StructureType::Campfire);
        // Storm: 200% of 1.0 = 2 durability lost
        let collapsed = apply_decay(&mut s, Weather::Storm).unwrap();
        assert!(!collapsed);
        assert_eq!(s.durability, 48);
    }

    #[test]
    fn decay_snow_increases_rate() {
        let mut s = make_structure(StructureType::Campfire);
        // Snow: 150% of 1.0 = 1 (floor of 1.5)
        let collapsed = apply_decay(&mut s, Weather::Snow).unwrap();
        assert!(!collapsed);
        assert_eq!(s.durability, 49);
    }

    #[test]
    fn decay_occupied_slower() {
        let mut s = make_structure(StructureType::Campfire);
        s.occupants.insert(AgentId::new());
        // Occupied: 75% of 1.0 = 0 (floor of 0.75)
        let collapsed = apply_decay(&mut s, Weather::Clear).unwrap();
        assert!(!collapsed);
        assert_eq!(s.durability, 50); // No decay this tick (floor)
    }

    #[test]
    fn decay_to_zero_collapses() {
        let mut s = make_structure(StructureType::Campfire);
        s.durability = 1;
        let collapsed = apply_decay(&mut s, Weather::Clear).unwrap();
        assert!(collapsed);
        assert_eq!(s.durability, 0);
    }

    #[test]
    fn many_ticks_of_decay_collapse() {
        let mut s = make_structure(StructureType::Campfire);
        let mut collapsed = false;
        for _ in 0..100 {
            let result = apply_decay(&mut s, Weather::Clear).unwrap();
            if result {
                collapsed = true;
                break;
            }
        }
        assert!(collapsed, "Campfire should collapse within 100 ticks");
    }

    // -----------------------------------------------------------------------
    // Salvage tests
    // -----------------------------------------------------------------------

    #[test]
    fn salvage_30_percent() {
        let materials = BTreeMap::from([
            (Resource::Wood, 20),
            (Resource::Stone, 10),
        ]);
        let salvage = compute_salvage(&materials).unwrap();
        assert_eq!(salvage.get(&Resource::Wood).copied(), Some(6));
        assert_eq!(salvage.get(&Resource::Stone).copied(), Some(3));
    }

    #[test]
    fn salvage_small_amounts() {
        let materials = BTreeMap::from([(Resource::Wood, 3)]);
        let salvage = compute_salvage(&materials).unwrap();
        // 3 * 30 / 100 = 0 (truncated)
        assert!(salvage.get(&Resource::Wood).is_none());
    }

    #[test]
    fn salvage_empty_materials() {
        let materials = BTreeMap::new();
        let salvage = compute_salvage(&materials).unwrap();
        assert!(salvage.is_empty());
    }

    // -----------------------------------------------------------------------
    // Repair cost tests
    // -----------------------------------------------------------------------

    #[test]
    fn repair_cost_half_damage() {
        let materials = BTreeMap::from([
            (Resource::Wood, 20),
            (Resource::Stone, 10),
        ]);
        let cost = compute_repair_cost(&materials, 50, 100).unwrap();
        assert_eq!(cost.get(&Resource::Wood).copied(), Some(10));
        assert_eq!(cost.get(&Resource::Stone).copied(), Some(5));
    }

    #[test]
    fn repair_cost_full_health() {
        let materials = BTreeMap::from([(Resource::Wood, 20)]);
        let cost = compute_repair_cost(&materials, 100, 100).unwrap();
        assert!(cost.is_empty());
    }

    #[test]
    fn repair_cost_zero_health() {
        let materials = BTreeMap::from([(Resource::Wood, 20)]);
        let cost = compute_repair_cost(&materials, 0, 100).unwrap();
        assert_eq!(cost.get(&Resource::Wood).copied(), Some(20));
    }

    #[test]
    fn repair_cost_zero_max_durability() {
        let materials = BTreeMap::from([(Resource::Wood, 20)]);
        let cost = compute_repair_cost(&materials, 0, 0).unwrap();
        assert!(cost.is_empty());
    }

    // -----------------------------------------------------------------------
    // Location effects tests
    // -----------------------------------------------------------------------

    #[test]
    fn effects_empty_structures() {
        let effects = structure_effects_at_location(&[]);
        assert!(!effects.weather_protection);
        assert_eq!(effects.best_rest_bonus_pct, 100);
        assert_eq!(effects.total_storage_slots, 0);
        assert!(!effects.has_shelter);
        assert!(!effects.has_fire);
        assert!(effects.production.is_empty());
    }

    #[test]
    fn effects_campfire_provides_fire() {
        let s = make_structure(StructureType::Campfire);
        let effects = structure_effects_at_location(&[s]);
        assert!(effects.has_fire);
        assert!(!effects.has_shelter);
        assert!(!effects.weather_protection);
    }

    #[test]
    fn effects_basic_hut_provides_shelter_and_weather() {
        let s = make_structure(StructureType::BasicHut);
        let effects = structure_effects_at_location(&[s]);
        assert!(effects.has_shelter);
        assert!(effects.weather_protection);
        assert_eq!(effects.best_rest_bonus_pct, 150);
        assert_eq!(effects.total_storage_slots, 20);
    }

    #[test]
    fn effects_multiple_structures_aggregate() {
        let campfire = make_structure(StructureType::Campfire);
        let hut = make_structure(StructureType::BasicHut);
        let storage = make_structure(StructureType::StoragePit);
        let well = make_structure(StructureType::Well);

        let effects = structure_effects_at_location(&[campfire, hut, storage, well]);
        assert!(effects.has_fire);
        assert!(effects.has_shelter);
        assert!(effects.weather_protection);
        assert_eq!(effects.best_rest_bonus_pct, 150);
        // BasicHut: 20 + StoragePit: 50 = 70
        assert_eq!(effects.total_storage_slots, 70);
        // Well produces 5 water
        assert_eq!(
            effects.production.get(&Resource::Water).copied(),
            Some(5)
        );
    }

    #[test]
    fn effects_skip_destroyed_structures() {
        let mut s = make_structure(StructureType::BasicHut);
        s.destroyed_at_tick = Some(50);
        let effects = structure_effects_at_location(&[s]);
        assert!(!effects.has_shelter);
        assert!(!effects.weather_protection);
    }

    #[test]
    fn effects_skip_zero_durability() {
        let mut s = make_structure(StructureType::BasicHut);
        s.durability = 0;
        let effects = structure_effects_at_location(&[s]);
        assert!(!effects.has_shelter);
    }

    #[test]
    fn effects_farm_and_well_production() {
        let farm = make_structure(StructureType::FarmPlot);
        let well = make_structure(StructureType::Well);
        let effects = structure_effects_at_location(&[farm, well]);
        assert_eq!(
            effects.production.get(&Resource::FoodFarmed).copied(),
            Some(2)
        );
        assert_eq!(
            effects.production.get(&Resource::Water).copied(),
            Some(5)
        );
    }

    #[test]
    fn effects_lean_to_provides_shelter_but_no_weather_protection() {
        let s = make_structure(StructureType::LeanTo);
        let effects = structure_effects_at_location(&[s]);
        assert!(effects.has_shelter);
        assert!(!effects.weather_protection);
        assert_eq!(effects.best_rest_bonus_pct, 120);
    }

    // -----------------------------------------------------------------------
    // Helper tests
    // -----------------------------------------------------------------------

    #[test]
    fn decimal_to_u32_floor_basic() {
        assert_eq!(decimal_to_u32_floor(Decimal::new(35, 1)), 3);
        assert_eq!(decimal_to_u32_floor(Decimal::new(99, 1)), 9);
        assert_eq!(decimal_to_u32_floor(Decimal::ZERO), 0);
        assert_eq!(decimal_to_u32_floor(Decimal::ONE), 1);
    }

    #[test]
    fn decimal_to_pct_basic() {
        assert_eq!(decimal_to_pct(Decimal::new(15, 1)), 150); // 1.5 -> 150
        assert_eq!(decimal_to_pct(Decimal::new(12, 1)), 120); // 1.2 -> 120
        assert_eq!(decimal_to_pct(Decimal::ONE), 100);
    }
}
