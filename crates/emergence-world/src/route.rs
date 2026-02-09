//! Route implementation for directed weighted edges in the world graph.
//!
//! A route connects two locations with a travel cost in ticks, a path quality
//! level, optional access control, and durability that degrades over time.
//! Routes can be bidirectional (most natural paths are) or one-way.
//!
//! # Route Improvement
//!
//! Agents can upgrade routes through a progression chain defined in
//! `world-engine.md` section 3.3. Each upgrade level requires specific
//! resources and may require knowledge (e.g. `basic_engineering` for
//! [`PathType::Road`] and above).
//!
//! # Route Durability and Decay
//!
//! Improved routes (anything above [`PathType::None`]) have a durability
//! score from 0 to 100. Durability decays each tick at a rate that depends
//! on the path type and weather conditions. When durability reaches 0, the
//! route degrades one level (e.g. [`PathType::Road`] becomes
//! [`PathType::WornPath`]).

use std::collections::BTreeMap;

use emergence_types::{AgentId, GroupId, PathType, Resource, Route, Weather};
use rust_decimal::Decimal;

use crate::error::WorldError;

/// Check whether a specific agent is permitted to traverse a route.
///
/// The ACL evaluation order is:
/// 1. If the route has no ACL, it is open to all agents.
/// 2. If the ACL is marked `public`, the agent is allowed.
/// 3. If the agent is in the `denied_agents` set, access is denied.
/// 4. If the agent is in the `allowed_agents` set, access is granted.
/// 5. If any of the agent's groups are in `allowed_groups`, access is granted.
/// 6. Otherwise, access is denied (default-deny for non-public ACLs).
pub fn can_traverse(route: &Route, agent: AgentId, agent_groups: &[GroupId]) -> bool {
    let Some(acl) = &route.acl else {
        return true;
    };

    if acl.public {
        return true;
    }

    if acl.denied_agents.contains(&agent) {
        return false;
    }

    if acl.allowed_agents.contains(&agent) {
        return true;
    }

    agent_groups
        .iter()
        .any(|g| acl.allowed_groups.contains(g))
}

/// Calculate the effective travel cost for a route, accounting for
/// weather conditions.
///
/// Weather effects on travel (from `data-schemas.md` section 3.7):
/// - Clear: no modifier
/// - Rain: +1 tick
/// - Storm: travel blocked (returns `None`)
/// - Drought: no modifier
/// - Snow: +2 ticks
///
/// # Errors
///
/// Returns [`WorldError::ArithmeticOverflow`] if checked arithmetic fails.
pub fn effective_travel_cost(
    route: &Route,
    weather: Weather,
) -> Result<Option<u32>, WorldError> {
    match weather {
        Weather::Storm => Ok(None), // Travel blocked
        Weather::Clear | Weather::Drought => Ok(Some(route.cost_ticks)),
        Weather::Rain => {
            let cost = route
                .cost_ticks
                .checked_add(1)
                .ok_or(WorldError::ArithmeticOverflow)?;
            Ok(Some(cost))
        }
        Weather::Snow => {
            let cost = route
                .cost_ticks
                .checked_add(2)
                .ok_or(WorldError::ArithmeticOverflow)?;
            Ok(Some(cost))
        }
    }
}

/// Return the base tick cost for a given [`PathType`].
///
/// Values come from `data-schemas.md` section 3.8.
pub const fn base_cost_for_path_type(path_type: PathType) -> u32 {
    match path_type {
        PathType::None => 8,
        PathType::DirtTrail => 5,
        PathType::WornPath => 3,
        PathType::Road => 2,
        PathType::Highway => 1,
    }
}

/// Return the next upgrade level for a [`PathType`], or `None` if already
/// at the maximum level.
pub const fn next_path_upgrade(current: PathType) -> Option<PathType> {
    match current {
        PathType::None => Some(PathType::DirtTrail),
        PathType::DirtTrail => Some(PathType::WornPath),
        PathType::WornPath => Some(PathType::Road),
        PathType::Road => Some(PathType::Highway),
        PathType::Highway => None,
    }
}

/// Return the previous (downgrade) level for a [`PathType`], or `None` if
/// already at the lowest level.
pub const fn previous_path_level(current: PathType) -> Option<PathType> {
    match current {
        PathType::None => None,
        PathType::DirtTrail => Some(PathType::None),
        PathType::WornPath => Some(PathType::DirtTrail),
        PathType::Road => Some(PathType::WornPath),
        PathType::Highway => Some(PathType::Road),
    }
}

/// Return the resource cost to upgrade a route to the given target [`PathType`].
///
/// Costs from `world-engine.md` section 3.3:
/// - [`PathType::None`] -> [`PathType::DirtTrail`]: 10 wood
/// - [`PathType::DirtTrail`] -> [`PathType::WornPath`]: 20 wood, 10 stone
/// - [`PathType::WornPath`] -> [`PathType::Road`]: 50 wood, 30 stone
/// - [`PathType::Road`] -> [`PathType::Highway`]: 100 wood, 80 stone, 20 metal
///
/// The `target` should be the *next* level, not the current. Returns `None`
/// if the target is [`PathType::None`] (no upgrade to wilderness).
pub fn upgrade_cost(target: PathType) -> Option<BTreeMap<Resource, u32>> {
    let mut cost = BTreeMap::new();
    match target {
        PathType::None => None,
        PathType::DirtTrail => {
            cost.insert(Resource::Wood, 10);
            Some(cost)
        }
        PathType::WornPath => {
            cost.insert(Resource::Wood, 20);
            cost.insert(Resource::Stone, 10);
            Some(cost)
        }
        PathType::Road => {
            cost.insert(Resource::Wood, 50);
            cost.insert(Resource::Stone, 30);
            Some(cost)
        }
        PathType::Highway => {
            cost.insert(Resource::Wood, 100);
            cost.insert(Resource::Stone, 80);
            cost.insert(Resource::Metal, 20);
            Some(cost)
        }
    }
}

/// Check whether a specific [`PathType`] requires advanced knowledge to build.
///
/// Per `world-engine.md` section 3.3 and 8.2:
/// - [`PathType::Road`] and [`PathType::Highway`] require `basic_engineering`
///   or `bridge_building`.
/// - Lower levels do not require any knowledge.
pub const fn requires_knowledge(target: PathType) -> Option<&'static [&'static str]> {
    match target {
        PathType::Road | PathType::Highway => {
            Some(&["basic_engineering", "bridge_building"])
        }
        _ => None,
    }
}

/// Check whether the agent has the required knowledge for a route upgrade.
///
/// Returns `true` if no knowledge is required, or if the agent knows at
/// least one of the required concepts.
pub fn has_required_knowledge(
    target: PathType,
    agent_knowledge: &std::collections::BTreeSet<String>,
) -> bool {
    requires_knowledge(target)
        .is_none_or(|required| required.iter().any(|k| agent_knowledge.contains(*k)))
}

/// Base decay rate per tick for each [`PathType`].
///
/// Wilderness routes ([`PathType::None`]) do not decay.
/// Higher-quality roads decay faster because they require more maintenance.
///
/// Returns a [`Decimal`] representing the durability points lost per tick
/// under normal conditions (no weather modifier).
pub fn base_decay_rate(path_type: PathType) -> Decimal {
    match path_type {
        PathType::None => Decimal::ZERO,
        PathType::DirtTrail => Decimal::new(1, 1),      // 0.1
        PathType::WornPath => Decimal::new(2, 1),        // 0.2
        PathType::Road => Decimal::new(3, 1),             // 0.3
        PathType::Highway => Decimal::new(5, 1),          // 0.5
    }
}

/// Initial durability for a newly upgraded route at the given [`PathType`].
///
/// Wilderness routes have no durability (they cannot degrade further).
pub const fn initial_durability(path_type: PathType) -> u32 {
    match path_type {
        PathType::None => 0,
        PathType::DirtTrail | PathType::WornPath | PathType::Road | PathType::Highway => 100,
    }
}

/// Weather modifier for route decay.
///
/// From `data-schemas.md` section 3.7:
/// - Storm: +100% (multiply by 2)
/// - Snow: +50% (multiply by 1.5)
/// - Others: no modifier (multiply by 1)
///
/// Returns a [`Decimal`] multiplier.
pub fn weather_decay_multiplier(weather: Weather) -> Decimal {
    match weather {
        Weather::Storm => Decimal::TWO,
        Weather::Snow => Decimal::new(15, 1), // 1.5
        Weather::Clear | Weather::Rain | Weather::Drought => Decimal::ONE,
    }
}

/// Apply one tick of decay to a route, accounting for weather.
///
/// Wilderness routes ([`PathType::None`]) do not decay. For improved routes,
/// the decay amount is `base_decay_rate(path_type) * weather_multiplier`,
/// truncated to a whole number and subtracted from durability.
///
/// If durability reaches 0, the route degrades one level and durability is
/// reset to 100 for the new level (or 0 if it degrades to [`PathType::None`]).
///
/// Returns `Some(new_path_type)` if the route degraded, `None` if it did not.
///
/// # Errors
///
/// Returns [`WorldError::ArithmeticOverflow`] on [`Decimal`] arithmetic failure.
pub fn apply_route_decay(route: &mut Route, weather: Weather) -> Result<Option<PathType>, WorldError> {
    // Wilderness routes do not decay
    if route.path_type == PathType::None {
        return Ok(None);
    }

    let base_rate = base_decay_rate(route.path_type);
    let multiplier = weather_decay_multiplier(weather);
    let decay_amount = base_rate
        .checked_mul(multiplier)
        .ok_or(WorldError::ArithmeticOverflow)?;

    // Convert to integer durability loss (floor). We use Decimal arithmetic
    // and then extract the integer part via truncation.
    let decay_u32 = u32::try_from(decay_amount.trunc().mantissa().unsigned_abs())
        .map_err(|_conversion| WorldError::ArithmeticOverflow)?;

    // Even if the truncated value is 0, accumulate fractional decay via
    // the route's decay_per_tick field as a fractional accumulator.
    let new_accum = route
        .decay_per_tick
        .checked_add(decay_amount)
        .ok_or(WorldError::ArithmeticOverflow)?;

    // Extract the integer part of accumulated decay as actual durability loss
    let total_loss = u32::try_from(new_accum.trunc().mantissa().unsigned_abs())
        .map_err(|_conversion| WorldError::ArithmeticOverflow)?;

    // Keep only the fractional remainder in the accumulator
    let fractional = new_accum
        .checked_sub(Decimal::from(total_loss))
        .ok_or(WorldError::ArithmeticOverflow)?;
    route.decay_per_tick = fractional;

    // Apply durability loss. We need to use saturating_sub to avoid underflow,
    // but we also need checked arithmetic for the lint.
    let _ = decay_u32; // used for clarity above; actual loss comes from total_loss
    if total_loss > 0 {
        route.durability = route.durability.saturating_sub(total_loss);
    }

    // Check if route should degrade
    if route.durability == 0
        && let Some(lower) = previous_path_level(route.path_type)
    {
        let old_type = route.path_type;
        route.path_type = lower;
        route.cost_ticks = base_cost_for_path_type(lower);
        route.durability = initial_durability(lower);
        route.max_durability = initial_durability(lower);
        route.decay_per_tick = Decimal::ZERO;
        // Only return degradation if we actually changed type
        if old_type != lower {
            return Ok(Some(lower));
        }
    }

    Ok(None)
}

/// Upgrade a route to the next [`PathType`] level.
///
/// Updates the route's `path_type`, `cost_ticks`, durability, and records the
/// builder and tick. This function does NOT check resources or knowledge --
/// those checks must be done by the caller before invoking this.
///
/// Returns the new [`PathType`] on success, or `None` if the route is already
/// at [`PathType::Highway`].
pub fn apply_route_upgrade(
    route: &mut Route,
    builder: AgentId,
    tick: u64,
) -> Option<PathType> {
    let next = next_path_upgrade(route.path_type)?;
    route.path_type = next;
    route.cost_ticks = base_cost_for_path_type(next);
    route.durability = initial_durability(next);
    route.max_durability = initial_durability(next);
    route.decay_per_tick = Decimal::ZERO;
    route.built_by = Some(builder);
    route.built_at_tick = Some(tick);
    Some(next)
}

/// Repair a route by restoring its durability to maximum without changing
/// the [`PathType`].
///
/// This is used when an agent performs `improve_route` on a route that is
/// already at the target level -- it restores durability instead of upgrading.
///
/// Returns the durability restored (new - old).
pub const fn repair_route(route: &mut Route) -> u32 {
    let old = route.durability;
    route.durability = route.max_durability;
    route.decay_per_tick = Decimal::ZERO;
    route.max_durability.saturating_sub(old)
}

/// Check whether a route has a toll cost defined in its ACL.
///
/// Returns the toll cost map if present, or `None` if the route has no ACL
/// or the ACL has no toll.
pub fn toll_cost(route: &Route) -> Option<&BTreeMap<Resource, u32>> {
    route.acl.as_ref().and_then(|acl| acl.toll_cost.as_ref())
}

/// Check whether an agent is at one of the route's endpoint locations.
pub fn agent_at_route_endpoint(route: &Route, agent_location: emergence_types::LocationId) -> bool {
    route.from_location == agent_location || route.to_location == agent_location
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use emergence_types::{AccessControlList, LocationId, RouteId};
    use rust_decimal::Decimal;

    use super::*;

    fn make_route(cost: u32, path: PathType) -> Route {
        Route {
            id: RouteId::new(),
            from_location: LocationId::new(),
            to_location: LocationId::new(),
            cost_ticks: cost,
            path_type: path,
            durability: 100,
            max_durability: 100,
            decay_per_tick: Decimal::ZERO,
            acl: None,
            bidirectional: true,
            built_by: None,
            built_at_tick: None,
        }
    }

    fn make_route_with_acl(acl: AccessControlList) -> Route {
        Route {
            id: RouteId::new(),
            from_location: LocationId::new(),
            to_location: LocationId::new(),
            cost_ticks: 3,
            path_type: PathType::WornPath,
            durability: 100,
            max_durability: 100,
            decay_per_tick: Decimal::ZERO,
            acl: Some(acl),
            bidirectional: true,
            built_by: None,
            built_at_tick: None,
        }
    }

    #[test]
    fn no_acl_allows_everyone() {
        let route = make_route(3, PathType::WornPath);
        let agent = AgentId::new();
        assert!(can_traverse(&route, agent, &[]));
    }

    #[test]
    fn public_acl_allows_everyone() {
        let acl = AccessControlList {
            allowed_agents: BTreeSet::new(),
            allowed_groups: BTreeSet::new(),
            denied_agents: BTreeSet::new(),
            public: true,
            toll_cost: None,
        };
        let route = make_route_with_acl(acl);
        assert!(can_traverse(&route, AgentId::new(), &[]));
    }

    #[test]
    fn denied_agent_blocked() {
        let agent = AgentId::new();
        let mut denied = BTreeSet::new();
        denied.insert(agent);
        let acl = AccessControlList {
            allowed_agents: BTreeSet::new(),
            allowed_groups: BTreeSet::new(),
            denied_agents: denied,
            public: false,
            toll_cost: None,
        };
        let route = make_route_with_acl(acl);
        assert!(!can_traverse(&route, agent, &[]));
    }

    #[test]
    fn allowed_agent_granted() {
        let agent = AgentId::new();
        let mut allowed = BTreeSet::new();
        allowed.insert(agent);
        let acl = AccessControlList {
            allowed_agents: allowed,
            allowed_groups: BTreeSet::new(),
            denied_agents: BTreeSet::new(),
            public: false,
            toll_cost: None,
        };
        let route = make_route_with_acl(acl);
        assert!(can_traverse(&route, agent, &[]));
    }

    #[test]
    fn group_membership_grants_access() {
        let agent = AgentId::new();
        let group = GroupId::new();
        let mut allowed_groups = BTreeSet::new();
        allowed_groups.insert(group);
        let acl = AccessControlList {
            allowed_agents: BTreeSet::new(),
            allowed_groups,
            denied_agents: BTreeSet::new(),
            public: false,
            toll_cost: None,
        };
        let route = make_route_with_acl(acl);
        assert!(can_traverse(&route, agent, &[group]));
    }

    #[test]
    fn unknown_agent_denied_by_default() {
        let acl = AccessControlList {
            allowed_agents: BTreeSet::new(),
            allowed_groups: BTreeSet::new(),
            denied_agents: BTreeSet::new(),
            public: false,
            toll_cost: None,
        };
        let route = make_route_with_acl(acl);
        assert!(!can_traverse(&route, AgentId::new(), &[]));
    }

    #[test]
    fn denied_overrides_allowed_agents() {
        // Agent is in both allowed and denied; denied wins because
        // we check denied first.
        let agent = AgentId::new();
        let mut allowed = BTreeSet::new();
        allowed.insert(agent);
        let mut denied = BTreeSet::new();
        denied.insert(agent);
        let acl = AccessControlList {
            allowed_agents: allowed,
            allowed_groups: BTreeSet::new(),
            denied_agents: denied,
            public: false,
            toll_cost: None,
        };
        let route = make_route_with_acl(acl);
        assert!(!can_traverse(&route, agent, &[]));
    }

    #[test]
    fn travel_cost_clear_weather() {
        let route = make_route(3, PathType::WornPath);
        let cost = effective_travel_cost(&route, Weather::Clear);
        assert!(cost.is_ok());
        assert_eq!(cost.ok().flatten(), Some(3));
    }

    #[test]
    fn travel_cost_rain_adds_one() {
        let route = make_route(3, PathType::WornPath);
        let cost = effective_travel_cost(&route, Weather::Rain);
        assert_eq!(cost.ok().flatten(), Some(4));
    }

    #[test]
    fn travel_cost_snow_adds_two() {
        let route = make_route(3, PathType::WornPath);
        let cost = effective_travel_cost(&route, Weather::Snow);
        assert_eq!(cost.ok().flatten(), Some(5));
    }

    #[test]
    fn travel_cost_storm_blocks() {
        let route = make_route(3, PathType::WornPath);
        let cost = effective_travel_cost(&route, Weather::Storm);
        assert_eq!(cost.ok().flatten(), None);
    }

    #[test]
    fn travel_cost_drought_normal() {
        let route = make_route(3, PathType::WornPath);
        let cost = effective_travel_cost(&route, Weather::Drought);
        assert_eq!(cost.ok().flatten(), Some(3));
    }

    #[test]
    fn base_costs_match_spec() {
        assert_eq!(base_cost_for_path_type(PathType::None), 8);
        assert_eq!(base_cost_for_path_type(PathType::DirtTrail), 5);
        assert_eq!(base_cost_for_path_type(PathType::WornPath), 3);
        assert_eq!(base_cost_for_path_type(PathType::Road), 2);
        assert_eq!(base_cost_for_path_type(PathType::Highway), 1);
    }

    #[test]
    fn path_upgrade_chain() {
        assert_eq!(next_path_upgrade(PathType::None), Some(PathType::DirtTrail));
        assert_eq!(
            next_path_upgrade(PathType::DirtTrail),
            Some(PathType::WornPath)
        );
        assert_eq!(next_path_upgrade(PathType::WornPath), Some(PathType::Road));
        assert_eq!(next_path_upgrade(PathType::Road), Some(PathType::Highway));
        assert_eq!(next_path_upgrade(PathType::Highway), None);
    }

    // -----------------------------------------------------------------------
    // Path downgrade chain (Phase 4.3)
    // -----------------------------------------------------------------------

    #[test]
    fn path_downgrade_chain() {
        assert_eq!(previous_path_level(PathType::Highway), Some(PathType::Road));
        assert_eq!(previous_path_level(PathType::Road), Some(PathType::WornPath));
        assert_eq!(
            previous_path_level(PathType::WornPath),
            Some(PathType::DirtTrail)
        );
        assert_eq!(previous_path_level(PathType::DirtTrail), Some(PathType::None));
        assert_eq!(previous_path_level(PathType::None), None);
    }

    // -----------------------------------------------------------------------
    // Upgrade costs (Phase 4.3.1)
    // -----------------------------------------------------------------------

    #[test]
    fn upgrade_cost_none_returns_none() {
        assert!(upgrade_cost(PathType::None).is_none());
    }

    #[test]
    fn upgrade_cost_dirt_trail() {
        let cost = upgrade_cost(PathType::DirtTrail).unwrap();
        assert_eq!(cost.get(&Resource::Wood).copied(), Some(10));
        assert_eq!(cost.len(), 1);
    }

    #[test]
    fn upgrade_cost_worn_path() {
        let cost = upgrade_cost(PathType::WornPath).unwrap();
        assert_eq!(cost.get(&Resource::Wood).copied(), Some(20));
        assert_eq!(cost.get(&Resource::Stone).copied(), Some(10));
        assert_eq!(cost.len(), 2);
    }

    #[test]
    fn upgrade_cost_road() {
        let cost = upgrade_cost(PathType::Road).unwrap();
        assert_eq!(cost.get(&Resource::Wood).copied(), Some(50));
        assert_eq!(cost.get(&Resource::Stone).copied(), Some(30));
        assert_eq!(cost.len(), 2);
    }

    #[test]
    fn upgrade_cost_highway() {
        let cost = upgrade_cost(PathType::Highway).unwrap();
        assert_eq!(cost.get(&Resource::Wood).copied(), Some(100));
        assert_eq!(cost.get(&Resource::Stone).copied(), Some(80));
        assert_eq!(cost.get(&Resource::Metal).copied(), Some(20));
        assert_eq!(cost.len(), 3);
    }

    // -----------------------------------------------------------------------
    // Knowledge requirements (Phase 4.3.1)
    // -----------------------------------------------------------------------

    #[test]
    fn knowledge_not_required_for_low_levels() {
        assert!(requires_knowledge(PathType::None).is_none());
        assert!(requires_knowledge(PathType::DirtTrail).is_none());
        assert!(requires_knowledge(PathType::WornPath).is_none());
    }

    #[test]
    fn knowledge_required_for_road_and_highway() {
        let road_req = requires_knowledge(PathType::Road).unwrap();
        assert!(road_req.contains(&"basic_engineering"));
        assert!(road_req.contains(&"bridge_building"));

        let hw_req = requires_knowledge(PathType::Highway).unwrap();
        assert!(hw_req.contains(&"basic_engineering"));
        assert!(hw_req.contains(&"bridge_building"));
    }

    #[test]
    fn has_knowledge_check() {
        let mut knowledge = std::collections::BTreeSet::new();
        // No knowledge -- should fail for Road
        assert!(!has_required_knowledge(PathType::Road, &knowledge));
        // No knowledge needed for DirtTrail
        assert!(has_required_knowledge(PathType::DirtTrail, &knowledge));

        // Add basic_engineering
        knowledge.insert(String::from("basic_engineering"));
        assert!(has_required_knowledge(PathType::Road, &knowledge));
        assert!(has_required_knowledge(PathType::Highway, &knowledge));

        // bridge_building alone also works
        let mut knowledge2 = std::collections::BTreeSet::new();
        knowledge2.insert(String::from("bridge_building"));
        assert!(has_required_knowledge(PathType::Road, &knowledge2));
    }

    // -----------------------------------------------------------------------
    // Route upgrade application (Phase 4.3.1)
    // -----------------------------------------------------------------------

    #[test]
    fn apply_upgrade_from_none() {
        let mut route = make_route(8, PathType::None);
        let agent = AgentId::new();
        let result = apply_route_upgrade(&mut route, agent, 100);
        assert_eq!(result, Some(PathType::DirtTrail));
        assert_eq!(route.path_type, PathType::DirtTrail);
        assert_eq!(route.cost_ticks, 5);
        assert_eq!(route.durability, 100);
        assert_eq!(route.built_by, Some(agent));
        assert_eq!(route.built_at_tick, Some(100));
    }

    #[test]
    fn apply_upgrade_full_chain() {
        let mut route = make_route(8, PathType::None);
        let agent = AgentId::new();

        assert_eq!(apply_route_upgrade(&mut route, agent, 1), Some(PathType::DirtTrail));
        assert_eq!(route.cost_ticks, 5);

        assert_eq!(apply_route_upgrade(&mut route, agent, 2), Some(PathType::WornPath));
        assert_eq!(route.cost_ticks, 3);

        assert_eq!(apply_route_upgrade(&mut route, agent, 3), Some(PathType::Road));
        assert_eq!(route.cost_ticks, 2);

        assert_eq!(apply_route_upgrade(&mut route, agent, 4), Some(PathType::Highway));
        assert_eq!(route.cost_ticks, 1);

        // Already at max
        assert_eq!(apply_route_upgrade(&mut route, agent, 5), None);
    }

    // -----------------------------------------------------------------------
    // Route repair (Phase 4.3.3)
    // -----------------------------------------------------------------------

    #[test]
    fn repair_restores_durability() {
        let mut route = make_route(5, PathType::DirtTrail);
        route.durability = 40;
        route.max_durability = 100;

        let restored = repair_route(&mut route);
        assert_eq!(restored, 60);
        assert_eq!(route.durability, 100);
    }

    #[test]
    fn repair_at_max_restores_zero() {
        let mut route = make_route(5, PathType::DirtTrail);
        route.durability = 100;
        route.max_durability = 100;

        let restored = repair_route(&mut route);
        assert_eq!(restored, 0);
    }

    // -----------------------------------------------------------------------
    // Route decay (Phase 4.3.3)
    // -----------------------------------------------------------------------

    #[test]
    fn wilderness_does_not_decay() {
        let mut route = make_route(8, PathType::None);
        let result = apply_route_decay(&mut route, Weather::Clear);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn dirt_trail_decays_slowly() {
        let mut route = make_route(5, PathType::DirtTrail);
        route.durability = 100;
        route.max_durability = 100;
        route.decay_per_tick = Decimal::ZERO;

        // base decay is 0.1 per tick -- after 10 ticks, durability should
        // drop by 1 (accumulator hits 1.0)
        for _ in 0..9 {
            let result = apply_route_decay(&mut route, Weather::Clear);
            assert!(result.is_ok());
            assert!(result.unwrap().is_none());
        }
        // After 9 ticks: accumulator = 0.9, durability = 100
        assert_eq!(route.durability, 100);

        // 10th tick: accumulator hits 1.0, loses 1 durability
        let result = apply_route_decay(&mut route, Weather::Clear);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
        assert_eq!(route.durability, 99);
    }

    #[test]
    fn storm_doubles_decay() {
        let mut route = make_route(5, PathType::DirtTrail);
        route.durability = 100;
        route.max_durability = 100;
        route.decay_per_tick = Decimal::ZERO;

        // Storm multiplier = 2.0, so base 0.1 becomes 0.2 per tick
        // After 5 ticks, accumulator should hit 1.0
        for _ in 0..4 {
            let result = apply_route_decay(&mut route, Weather::Storm);
            assert!(result.is_ok());
        }
        assert_eq!(route.durability, 100);

        // 5th tick: 5 * 0.2 = 1.0 -- loses 1 durability
        let result = apply_route_decay(&mut route, Weather::Storm);
        assert!(result.is_ok());
        assert_eq!(route.durability, 99);
    }

    #[test]
    fn route_degrades_at_zero_durability() {
        let mut route = make_route(2, PathType::Road);
        route.durability = 1;
        route.max_durability = 100;
        // Set accumulator so next tick pushes durability to 0
        // Road decay: 0.3 per tick. We need total_loss >= 1.
        route.decay_per_tick = Decimal::new(8, 1); // 0.8 + 0.3 = 1.1 -> total_loss = 1

        let result = apply_route_decay(&mut route, Weather::Clear);
        assert!(result.is_ok());
        let degraded = result.unwrap();
        assert_eq!(degraded, Some(PathType::WornPath));
        assert_eq!(route.path_type, PathType::WornPath);
        assert_eq!(route.cost_ticks, 3);
        assert_eq!(route.durability, 100); // Reset for new level
    }

    #[test]
    fn dirt_trail_degrades_to_none() {
        let mut route = make_route(5, PathType::DirtTrail);
        route.durability = 1;
        route.max_durability = 100;
        route.decay_per_tick = Decimal::new(95, 2); // 0.95 + 0.1 = 1.05 -> loses 1

        let result = apply_route_decay(&mut route, Weather::Clear);
        assert!(result.is_ok());
        let degraded = result.unwrap();
        assert_eq!(degraded, Some(PathType::None));
        assert_eq!(route.path_type, PathType::None);
        assert_eq!(route.cost_ticks, 8);
        assert_eq!(route.durability, 0); // Wilderness has 0 durability
    }

    // -----------------------------------------------------------------------
    // Toll cost (Phase 4.3.2)
    // -----------------------------------------------------------------------

    #[test]
    fn toll_cost_none_when_no_acl() {
        let route = make_route(3, PathType::WornPath);
        assert!(toll_cost(&route).is_none());
    }

    #[test]
    fn toll_cost_none_when_acl_has_no_toll() {
        let acl = AccessControlList {
            allowed_agents: BTreeSet::new(),
            allowed_groups: BTreeSet::new(),
            denied_agents: BTreeSet::new(),
            public: true,
            toll_cost: None,
        };
        let route = make_route_with_acl(acl);
        assert!(toll_cost(&route).is_none());
    }

    #[test]
    fn toll_cost_present_when_set() {
        let mut toll = BTreeMap::new();
        toll.insert(Resource::Wood, 5);
        let acl = AccessControlList {
            allowed_agents: BTreeSet::new(),
            allowed_groups: BTreeSet::new(),
            denied_agents: BTreeSet::new(),
            public: true,
            toll_cost: Some(toll),
        };
        let route = make_route_with_acl(acl);
        let tc = toll_cost(&route);
        assert!(tc.is_some());
        let tc = tc.unwrap();
        assert_eq!(tc.get(&Resource::Wood).copied(), Some(5));
    }

    // -----------------------------------------------------------------------
    // Agent at route endpoint (Phase 4.3.1)
    // -----------------------------------------------------------------------

    #[test]
    fn agent_at_from_location() {
        let route = make_route(3, PathType::WornPath);
        assert!(agent_at_route_endpoint(&route, route.from_location));
    }

    #[test]
    fn agent_at_to_location() {
        let route = make_route(3, PathType::WornPath);
        assert!(agent_at_route_endpoint(&route, route.to_location));
    }

    #[test]
    fn agent_not_at_endpoint() {
        let route = make_route(3, PathType::WornPath);
        let other = LocationId::new();
        assert!(!agent_at_route_endpoint(&route, other));
    }

    // -----------------------------------------------------------------------
    // Weather decay multiplier
    // -----------------------------------------------------------------------

    #[test]
    fn weather_multipliers() {
        assert_eq!(weather_decay_multiplier(Weather::Clear), Decimal::ONE);
        assert_eq!(weather_decay_multiplier(Weather::Rain), Decimal::ONE);
        assert_eq!(weather_decay_multiplier(Weather::Drought), Decimal::ONE);
        assert_eq!(weather_decay_multiplier(Weather::Storm), Decimal::TWO);
        assert_eq!(weather_decay_multiplier(Weather::Snow), Decimal::new(15, 1));
    }

    // -----------------------------------------------------------------------
    // Base decay rates
    // -----------------------------------------------------------------------

    #[test]
    fn base_decay_rates() {
        assert_eq!(base_decay_rate(PathType::None), Decimal::ZERO);
        assert_eq!(base_decay_rate(PathType::DirtTrail), Decimal::new(1, 1));
        assert_eq!(base_decay_rate(PathType::WornPath), Decimal::new(2, 1));
        assert_eq!(base_decay_rate(PathType::Road), Decimal::new(3, 1));
        assert_eq!(base_decay_rate(PathType::Highway), Decimal::new(5, 1));
    }

    // -----------------------------------------------------------------------
    // Initial durability
    // -----------------------------------------------------------------------

    #[test]
    fn initial_durability_values() {
        assert_eq!(initial_durability(PathType::None), 0);
        assert_eq!(initial_durability(PathType::DirtTrail), 100);
        assert_eq!(initial_durability(PathType::WornPath), 100);
        assert_eq!(initial_durability(PathType::Road), 100);
        assert_eq!(initial_durability(PathType::Highway), 100);
    }
}
