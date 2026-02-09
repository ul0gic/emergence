//! Default starting world map for the Emergence simulation.
//!
//! Creates 12 locations across 3 regions (Central Valley, Highlands,
//! Coastal Lowlands) plus 3 undiscovered locations, connected by natural
//! routes per `world-engine.md` section 3.4.

use std::collections::BTreeSet;

use chrono::Utc;
use emergence_types::{
    Location, LocationId, PathType, Resource, ResourceNode, Route, RouteId,
};
use rust_decimal::Decimal;

use crate::error::WorldError;
use crate::world_map::WorldMap;

/// Helper to build a [`ResourceNode`].
const fn node(resource: Resource, available: u32, regen: u32, max: u32) -> ResourceNode {
    ResourceNode {
        resource,
        available,
        regen_per_tick: regen,
        max_capacity: max,
    }
}

/// Helper to build a [`Location`].
fn loc(
    id: LocationId,
    name: &str,
    region: &str,
    loc_type: &str,
    desc: &str,
    capacity: u32,
    resources: Vec<(Resource, ResourceNode)>,
) -> Location {
    Location {
        id,
        name: name.to_string(),
        region: region.to_string(),
        location_type: loc_type.to_string(),
        description: desc.to_string(),
        capacity,
        base_resources: resources.into_iter().collect(),
        discovered_by: BTreeSet::new(),
        created_at: Utc::now(),
    }
}

/// Helper to build a bidirectional natural [`Route`].
fn natural_route(
    from: LocationId,
    to: LocationId,
    cost: u32,
    path_type: PathType,
) -> Route {
    Route {
        id: RouteId::new(),
        from_location: from,
        to_location: to,
        cost_ticks: cost,
        path_type,
        durability: 100,
        max_durability: 100,
        decay_per_tick: Decimal::ZERO,
        acl: None,
        bidirectional: true,
        built_by: None,
        built_at_tick: None,
    }
}

/// Identifiers for all starting locations, returned alongside the world map
/// so that callers can reference specific locations for agent placement, etc.
#[derive(Debug, Clone)]
pub struct StartingLocationIds {
    // --- Central Valley ---
    /// Riverbank: water, wood, berries, fish, stone.
    pub riverbank: LocationId,
    /// Open Field: fertile soil, potential farmland.
    pub open_field: LocationId,
    /// Forest Edge: abundant wood, some berries, wildlife.
    pub forest_edge: LocationId,

    // --- Highlands ---
    /// Rocky Outcrop: stone, ore, sparse food.
    pub rocky_outcrop: LocationId,
    /// Mountain Cave: shelter, minerals, darkness.
    pub mountain_cave: LocationId,
    /// Hilltop: visibility bonus, sparse resources.
    pub hilltop: LocationId,

    // --- Coastal Lowlands ---
    /// Beach: sand, driftwood, shellfish, salt.
    pub beach: LocationId,
    /// Tidal Pools: unique food, natural beauty.
    pub tidal_pools: LocationId,
    /// Estuary: rich fishing, reeds.
    pub estuary: LocationId,

    // --- Undiscovered ---
    /// Deep Forest: rare resources, danger.
    pub deep_forest: LocationId,
    /// Underground Spring: valuable water source.
    pub underground_spring: LocationId,
    /// Volcanic Vent: heat, rare minerals, risk.
    pub volcanic_vent: LocationId,
}

/// Create the default starting world map with 12 locations across 3 regions.
///
/// Returns the populated [`WorldMap`] and the [`StartingLocationIds`] for
/// referencing specific locations.
///
/// # Errors
///
/// Returns [`WorldError`] if the map construction fails (should not happen
/// with valid hard-coded data).
#[allow(clippy::too_many_lines)]
pub fn create_starting_world() -> Result<(WorldMap, StartingLocationIds), WorldError> {
    let mut map = WorldMap::new();

    // Generate all location IDs up front.
    let ids = StartingLocationIds {
        riverbank: LocationId::new(),
        open_field: LocationId::new(),
        forest_edge: LocationId::new(),
        rocky_outcrop: LocationId::new(),
        mountain_cave: LocationId::new(),
        hilltop: LocationId::new(),
        beach: LocationId::new(),
        tidal_pools: LocationId::new(),
        estuary: LocationId::new(),
        deep_forest: LocationId::new(),
        underground_spring: LocationId::new(),
        volcanic_vent: LocationId::new(),
    };

    // ---------------------------------------------------------------
    // Region: Central Valley
    // ---------------------------------------------------------------

    map.add_location(loc(
        ids.riverbank,
        "Riverbank",
        "Central Valley",
        "natural",
        "A wide riverbank with fertile soil and fresh water. Trees line the eastern edge.",
        20,
        vec![
            (Resource::Water, node(Resource::Water, 999, 50, 999)),
            (Resource::Wood, node(Resource::Wood, 45, 3, 100)),
            (Resource::FoodBerry, node(Resource::FoodBerry, 12, 2, 30)),
            (Resource::Stone, node(Resource::Stone, 8, 0, 8)),
            (Resource::FoodFish, node(Resource::FoodFish, 20, 5, 40)),
        ],
    ))?;

    map.add_location(loc(
        ids.open_field,
        "Open Field",
        "Central Valley",
        "natural",
        "A broad, sunlit field with rich soil. Wild grasses sway in the wind. This land could support farming if the knowledge were known.",
        15,
        vec![
            (Resource::FoodRoot, node(Resource::FoodRoot, 8, 2, 20)),
            (Resource::FoodBerry, node(Resource::FoodBerry, 5, 1, 15)),
            (Resource::Fiber, node(Resource::Fiber, 10, 3, 30)),
        ],
    ))?;

    map.add_location(loc(
        ids.forest_edge,
        "Forest Edge",
        "Central Valley",
        "natural",
        "Dense woodland with towering oaks and birch. Undergrowth is thick with berry bushes. The occasional deer track weaves between the trees.",
        15,
        vec![
            (Resource::Wood, node(Resource::Wood, 80, 8, 150)),
            (Resource::FoodBerry, node(Resource::FoodBerry, 15, 3, 40)),
            (Resource::FoodRoot, node(Resource::FoodRoot, 5, 1, 15)),
        ],
    ))?;

    // ---------------------------------------------------------------
    // Region: Highlands
    // ---------------------------------------------------------------

    map.add_location(loc(
        ids.rocky_outcrop,
        "Rocky Outcrop",
        "Highlands",
        "natural",
        "Jagged stone formations jut from the hillside. Veins of darker rock suggest mineral deposits beneath the surface.",
        10,
        vec![
            (Resource::Stone, node(Resource::Stone, 40, 2, 80)),
            (Resource::Ore, node(Resource::Ore, 10, 1, 30)),
            (Resource::FoodRoot, node(Resource::FoodRoot, 3, 1, 10)),
        ],
    ))?;

    map.add_location(loc(
        ids.mountain_cave,
        "Mountain Cave",
        "Highlands",
        "natural",
        "A deep cave carved into the mountainside. Cool air flows from within. The darkness hides deposits of valuable minerals.",
        8,
        vec![
            (Resource::Stone, node(Resource::Stone, 25, 1, 50)),
            (Resource::Ore, node(Resource::Ore, 15, 2, 40)),
            (Resource::Water, node(Resource::Water, 20, 3, 30)),
        ],
    ))?;

    map.add_location(loc(
        ids.hilltop,
        "Hilltop",
        "Highlands",
        "natural",
        "A windswept hilltop offering panoramic views of the surrounding regions. Little grows here, but the vantage is invaluable.",
        12,
        vec![
            (Resource::Stone, node(Resource::Stone, 5, 0, 5)),
            (Resource::FoodBerry, node(Resource::FoodBerry, 3, 1, 10)),
        ],
    ))?;

    // ---------------------------------------------------------------
    // Region: Coastal Lowlands
    // ---------------------------------------------------------------

    map.add_location(loc(
        ids.beach,
        "Beach",
        "Coastal Lowlands",
        "natural",
        "A long stretch of sandy beach littered with driftwood. Shallow waters teem with shellfish. The salt air is heavy and constant.",
        15,
        vec![
            (Resource::Wood, node(Resource::Wood, 10, 2, 25)),
            (Resource::FoodFish, node(Resource::FoodFish, 15, 4, 35)),
            (Resource::Stone, node(Resource::Stone, 3, 0, 3)),
        ],
    ))?;

    map.add_location(loc(
        ids.tidal_pools,
        "Tidal Pools",
        "Coastal Lowlands",
        "natural",
        "Rocky pools left by the receding tide, filled with small crabs, mussels, and colorful anemones. A peaceful place of natural beauty.",
        10,
        vec![
            (Resource::FoodFish, node(Resource::FoodFish, 12, 3, 25)),
            (Resource::FoodRoot, node(Resource::FoodRoot, 4, 1, 12)),
            (Resource::Water, node(Resource::Water, 10, 2, 15)),
        ],
    ))?;

    map.add_location(loc(
        ids.estuary,
        "Estuary",
        "Coastal Lowlands",
        "natural",
        "Where the river meets the sea, rich silt banks support dense reed beds. Fish gather in the warm, shallow waters.",
        15,
        vec![
            (Resource::FoodFish, node(Resource::FoodFish, 30, 8, 60)),
            (Resource::Fiber, node(Resource::Fiber, 20, 5, 40)),
            (Resource::Water, node(Resource::Water, 100, 20, 200)),
            (Resource::Clay, node(Resource::Clay, 15, 2, 30)),
        ],
    ))?;

    // ---------------------------------------------------------------
    // Undiscovered Locations
    // ---------------------------------------------------------------

    map.add_location(loc(
        ids.deep_forest,
        "Deep Forest",
        "Central Valley",
        "hidden",
        "Far beyond the forest edge, ancient trees block out the sun. Strange sounds echo in the canopy. Rare plants and creatures dwell here.",
        8,
        vec![
            (Resource::Wood, node(Resource::Wood, 120, 10, 200)),
            (Resource::FoodBerry, node(Resource::FoodBerry, 20, 4, 50)),
            (Resource::Medicine, node(Resource::Medicine, 5, 1, 10)),
            (Resource::Hide, node(Resource::Hide, 3, 1, 8)),
        ],
    ))?;

    map.add_location(loc(
        ids.underground_spring,
        "Underground Spring",
        "Highlands",
        "hidden",
        "A hidden cave chamber with a pristine natural spring. The water is crystal clear and ice cold. This could sustain a settlement.",
        6,
        vec![
            (Resource::Water, node(Resource::Water, 500, 30, 500)),
            (Resource::Stone, node(Resource::Stone, 10, 1, 20)),
        ],
    ))?;

    map.add_location(loc(
        ids.volcanic_vent,
        "Volcanic Vent",
        "Highlands",
        "hidden",
        "A fissure in the earth from which hot gases and steam escape. The surrounding rock is streaked with metallic deposits. The heat is dangerous but the resources are unmatched.",
        5,
        vec![
            (Resource::Ore, node(Resource::Ore, 30, 5, 60)),
            (Resource::Stone, node(Resource::Stone, 20, 2, 40)),
        ],
    ))?;

    // ---------------------------------------------------------------
    // Routes
    // ---------------------------------------------------------------

    // Central Valley internal routes
    map.add_route(natural_route(
        ids.riverbank,
        ids.open_field,
        2,
        PathType::WornPath,
    ))?;
    map.add_route(natural_route(
        ids.riverbank,
        ids.forest_edge,
        3,
        PathType::DirtTrail,
    ))?;
    map.add_route(natural_route(
        ids.open_field,
        ids.forest_edge,
        2,
        PathType::DirtTrail,
    ))?;

    // Highlands internal routes
    map.add_route(natural_route(
        ids.rocky_outcrop,
        ids.mountain_cave,
        4,
        PathType::None,
    ))?;
    map.add_route(natural_route(
        ids.rocky_outcrop,
        ids.hilltop,
        3,
        PathType::DirtTrail,
    ))?;
    map.add_route(natural_route(
        ids.hilltop,
        ids.mountain_cave,
        5,
        PathType::None,
    ))?;

    // Coastal Lowlands internal routes
    map.add_route(natural_route(
        ids.beach,
        ids.tidal_pools,
        2,
        PathType::WornPath,
    ))?;
    map.add_route(natural_route(
        ids.beach,
        ids.estuary,
        3,
        PathType::DirtTrail,
    ))?;
    map.add_route(natural_route(
        ids.tidal_pools,
        ids.estuary,
        4,
        PathType::None,
    ))?;

    // Cross-region routes: Central Valley <-> Highlands
    map.add_route(natural_route(
        ids.forest_edge,
        ids.rocky_outcrop,
        5,
        PathType::None,
    ))?;
    map.add_route(natural_route(
        ids.open_field,
        ids.hilltop,
        4,
        PathType::None,
    ))?;

    // Cross-region routes: Central Valley <-> Coastal Lowlands
    map.add_route(natural_route(
        ids.riverbank,
        ids.estuary,
        6,
        PathType::DirtTrail,
    ))?;
    map.add_route(natural_route(
        ids.open_field,
        ids.beach,
        5,
        PathType::None,
    ))?;

    // Cross-region routes: Highlands <-> Coastal Lowlands
    map.add_route(natural_route(
        ids.hilltop,
        ids.beach,
        7,
        PathType::None,
    ))?;

    // Undiscovered location routes (harder to reach)
    map.add_route(natural_route(
        ids.forest_edge,
        ids.deep_forest,
        8,
        PathType::None,
    ))?;
    map.add_route(natural_route(
        ids.mountain_cave,
        ids.underground_spring,
        6,
        PathType::None,
    ))?;
    map.add_route(natural_route(
        ids.rocky_outcrop,
        ids.volcanic_vent,
        8,
        PathType::None,
    ))?;

    Ok((map, ids))
}

#[cfg(test)]
mod tests {
    use emergence_types::Weather;

    use super::*;

    #[test]
    fn starting_world_creates_12_locations() {
        let result = create_starting_world();
        assert!(result.is_ok());
        let (map, _) = result.unwrap_or_else(|_| {
            // This unwrap_or_else is only in a test; we use a fallback
            // that will cause a clear assertion failure below.
            (WorldMap::new(), StartingLocationIds {
                riverbank: LocationId::new(),
                open_field: LocationId::new(),
                forest_edge: LocationId::new(),
                rocky_outcrop: LocationId::new(),
                mountain_cave: LocationId::new(),
                hilltop: LocationId::new(),
                beach: LocationId::new(),
                tidal_pools: LocationId::new(),
                estuary: LocationId::new(),
                deep_forest: LocationId::new(),
                underground_spring: LocationId::new(),
                volcanic_vent: LocationId::new(),
            })
        });
        assert_eq!(map.location_count(), 12);
    }

    #[test]
    fn starting_world_has_routes() {
        let result = create_starting_world();
        assert!(result.is_ok());
        if let Ok((map, _)) = result {
            // 17 routes total (9 intra-region + 5 cross-region + 3 undiscovered)
            assert_eq!(map.route_count(), 17);
        }
    }

    #[test]
    fn starting_world_is_connected() {
        let result = create_starting_world();
        assert!(result.is_ok());
        if let Ok((map, _)) = result {
            assert!(map.is_connected());
        }
    }

    #[test]
    fn starting_world_all_locations_reachable() {
        let result = create_starting_world();
        assert!(result.is_ok());
        if let Ok((map, ids)) = result {
            // Every location should be reachable from the riverbank.
            let targets = [
                ids.open_field,
                ids.forest_edge,
                ids.rocky_outcrop,
                ids.mountain_cave,
                ids.hilltop,
                ids.beach,
                ids.tidal_pools,
                ids.estuary,
                ids.deep_forest,
                ids.underground_spring,
                ids.volcanic_vent,
            ];
            for target in targets {
                let path = map.shortest_path(ids.riverbank, target, Weather::Clear);
                assert!(
                    path.is_some(),
                    "No path from Riverbank to location {target}"
                );
            }
        }
    }

    #[test]
    fn starting_world_locations_have_resources() {
        let result = create_starting_world();
        assert!(result.is_ok());
        if let Ok((map, ids)) = result {
            // Riverbank should have water, wood, berries, stone, fish.
            let riverbank = map.get_location(ids.riverbank);
            assert!(riverbank.is_some());
            if let Some(rb) = riverbank {
                assert!(rb.get_resource(&Resource::Water).is_some());
                assert!(rb.get_resource(&Resource::Wood).is_some());
                assert!(rb.get_resource(&Resource::FoodBerry).is_some());
                assert!(rb.get_resource(&Resource::Stone).is_some());
                assert!(rb.get_resource(&Resource::FoodFish).is_some());
            }

            // Rocky Outcrop should have stone and ore.
            let rocky = map.get_location(ids.rocky_outcrop);
            assert!(rocky.is_some());
            if let Some(r) = rocky {
                assert!(r.get_resource(&Resource::Stone).is_some());
                assert!(r.get_resource(&Resource::Ore).is_some());
            }
        }
    }

    #[test]
    fn starting_world_regions_correct() {
        let result = create_starting_world();
        assert!(result.is_ok());
        if let Ok((map, ids)) = result {
            let check_region = |id: LocationId, expected: &str| {
                let loc = map.get_location(id);
                assert!(loc.is_some());
                if let Some(l) = loc {
                    assert_eq!(l.location.region, expected);
                }
            };
            check_region(ids.riverbank, "Central Valley");
            check_region(ids.open_field, "Central Valley");
            check_region(ids.forest_edge, "Central Valley");
            check_region(ids.rocky_outcrop, "Highlands");
            check_region(ids.mountain_cave, "Highlands");
            check_region(ids.hilltop, "Highlands");
            check_region(ids.beach, "Coastal Lowlands");
            check_region(ids.tidal_pools, "Coastal Lowlands");
            check_region(ids.estuary, "Coastal Lowlands");
        }
    }

    #[test]
    fn starting_world_capacity_positive() {
        let result = create_starting_world();
        assert!(result.is_ok());
        if let Ok((map, _)) = result {
            for (_, loc_state) in map.locations() {
                assert!(
                    loc_state.location.capacity > 0,
                    "Location {} has zero capacity",
                    loc_state.location.name
                );
            }
        }
    }
}
