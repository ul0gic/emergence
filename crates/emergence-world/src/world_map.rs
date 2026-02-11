//! World graph: locations as nodes, routes as weighted directed edges.
//!
//! The [`WorldMap`] is the spatial backbone of the simulation. It stores all
//! [`LocationState`] nodes and [`Route`] edges, provides neighbor lookups,
//! route queries, and shortest-path computation.
//!
//! Internally, an adjacency map indexes outbound routes per location:
//! `BTreeMap<LocationId, Vec<RouteId>>`. A reverse adjacency map indexes
//! inbound routes for bidirectional traversal.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use emergence_types::{AgentId, Location, LocationId, Resource, Route, RouteId, Season, Weather};

use crate::error::WorldError;
use crate::location::LocationState;
use crate::route;

/// The world graph holding all locations and routes.
///
/// Provides spatial queries, pathfinding, and batch operations like
/// per-tick resource regeneration across all locations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorldMap {
    /// All locations indexed by their identifier.
    locations: BTreeMap<LocationId, LocationState>,
    /// All routes indexed by their identifier.
    routes: BTreeMap<RouteId, Route>,
    /// Outbound adjacency: location -> list of route IDs departing from it.
    outbound: BTreeMap<LocationId, Vec<RouteId>>,
    /// Inbound adjacency: location -> list of route IDs arriving at it.
    inbound: BTreeMap<LocationId, Vec<RouteId>>,
}

impl WorldMap {
    /// Create an empty world map.
    pub const fn new() -> Self {
        Self {
            locations: BTreeMap::new(),
            routes: BTreeMap::new(),
            outbound: BTreeMap::new(),
            inbound: BTreeMap::new(),
        }
    }

    // -------------------------------------------------------------------
    // Location operations
    // -------------------------------------------------------------------

    /// Add a location to the world map.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::DuplicateLocation`] if a location with the same
    /// ID already exists.
    pub fn add_location(&mut self, location: Location) -> Result<(), WorldError> {
        let id = location.id;
        if self.locations.contains_key(&id) {
            return Err(WorldError::DuplicateLocation(id));
        }
        self.locations.insert(id, LocationState::new(location));
        self.outbound.entry(id).or_default();
        self.inbound.entry(id).or_default();
        Ok(())
    }

    /// Get an immutable reference to a location's state.
    pub fn get_location(&self, id: LocationId) -> Option<&LocationState> {
        self.locations.get(&id)
    }

    /// Get a mutable reference to a location's state.
    pub fn get_location_mut(&mut self, id: LocationId) -> Option<&mut LocationState> {
        self.locations.get_mut(&id)
    }

    /// Return the number of locations in the map.
    pub fn location_count(&self) -> usize {
        self.locations.len()
    }

    /// Return all location IDs.
    pub fn location_ids(&self) -> Vec<LocationId> {
        self.locations.keys().copied().collect()
    }

    /// Iterate over all locations immutably.
    pub fn locations(&self) -> impl Iterator<Item = (&LocationId, &LocationState)> {
        self.locations.iter()
    }

    /// Iterate over all locations mutably.
    pub fn locations_mut(&mut self) -> impl Iterator<Item = (&LocationId, &mut LocationState)> {
        self.locations.iter_mut()
    }

    // -------------------------------------------------------------------
    // Route operations
    // -------------------------------------------------------------------

    /// Add a route to the world map.
    ///
    /// Both `from_location` and `to_location` must already exist in the map.
    /// If the route is bidirectional, the reverse direction is also indexed.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::LocationNotFound`] if either endpoint is missing,
    /// or [`WorldError::DuplicateRoute`] if the route ID already exists.
    pub fn add_route(&mut self, route_def: Route) -> Result<(), WorldError> {
        if !self.locations.contains_key(&route_def.from_location) {
            return Err(WorldError::LocationNotFound(route_def.from_location));
        }
        if !self.locations.contains_key(&route_def.to_location) {
            return Err(WorldError::LocationNotFound(route_def.to_location));
        }
        if self.routes.contains_key(&route_def.id) {
            return Err(WorldError::DuplicateRoute(route_def.id));
        }

        let id = route_def.id;
        let from = route_def.from_location;
        let to = route_def.to_location;
        let bidir = route_def.bidirectional;

        self.routes.insert(id, route_def);
        self.outbound.entry(from).or_default().push(id);
        self.inbound.entry(to).or_default().push(id);

        if bidir {
            self.outbound.entry(to).or_default().push(id);
            self.inbound.entry(from).or_default().push(id);
        }

        Ok(())
    }

    /// Get an immutable reference to a route.
    pub fn get_route(&self, id: RouteId) -> Option<&Route> {
        self.routes.get(&id)
    }

    /// Get a mutable reference to a route.
    pub fn get_route_mut(&mut self, id: RouteId) -> Option<&mut Route> {
        self.routes.get_mut(&id)
    }

    /// Return the number of routes in the map.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    /// Return all route IDs.
    pub fn route_ids(&self) -> Vec<RouteId> {
        self.routes.keys().copied().collect()
    }

    /// Iterate over all routes immutably.
    pub fn routes(&self) -> impl Iterator<Item = (&RouteId, &Route)> {
        self.routes.iter()
    }

    /// Iterate over all routes mutably.
    pub fn routes_mut(&mut self) -> impl Iterator<Item = (&RouteId, &mut Route)> {
        self.routes.iter_mut()
    }

    /// Find the route between two locations that is accessible from a given
    /// location. Used by the `ImproveRoute` action to look up which route
    /// the agent wants to improve.
    ///
    /// Returns the first matching route connecting the agent's location to
    /// the given destination.
    pub fn find_route_from_to(
        &self,
        from: LocationId,
        to: LocationId,
    ) -> Option<&Route> {
        self.routes_between(from, to).into_iter().next()
    }

    /// Apply one tick of decay to all routes and return any degradation events.
    ///
    /// Returns a list of `(RouteId, new PathType)` for routes that degraded.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::ArithmeticOverflow`] on arithmetic failure.
    pub fn apply_all_route_decay(
        &mut self,
        weather: Weather,
    ) -> Result<Vec<(RouteId, emergence_types::PathType)>, WorldError> {
        let mut degraded = Vec::new();
        let route_ids: Vec<RouteId> = self.routes.keys().copied().collect();
        for id in route_ids {
            if let Some(r) = self.routes.get_mut(&id)
                && let Some(new_type) = route::apply_route_decay(r, weather)?
            {
                degraded.push((id, new_type));
            }
        }
        Ok(degraded)
    }

    // -------------------------------------------------------------------
    // Graph queries
    // -------------------------------------------------------------------

    /// Return the IDs of locations directly reachable from the given location,
    /// along with the corresponding route IDs.
    ///
    /// For bidirectional routes, both the forward and reverse directions are
    /// included in the outbound index, so neighbors include both directions.
    pub fn neighbors(&self, location: LocationId) -> Vec<(LocationId, RouteId)> {
        let Some(route_ids) = self.outbound.get(&location) else {
            return Vec::new();
        };

        let mut result = Vec::new();
        for route_id in route_ids {
            if let Some(r) = self.routes.get(route_id) {
                // Determine which end is the neighbor.
                let neighbor = if r.from_location == location {
                    r.to_location
                } else {
                    r.from_location
                };
                result.push((neighbor, *route_id));
            }
        }
        result
    }

    /// Return routes connecting two specific locations (in either direction
    /// if bidirectional).
    pub fn routes_between(
        &self,
        from: LocationId,
        to: LocationId,
    ) -> Vec<&Route> {
        let Some(route_ids) = self.outbound.get(&from) else {
            return Vec::new();
        };
        route_ids
            .iter()
            .filter_map(|rid| self.routes.get(rid))
            .filter(|r| {
                (r.from_location == from && r.to_location == to)
                    || (r.bidirectional && r.from_location == to && r.to_location == from)
            })
            .collect()
    }

    /// Find the shortest path between two locations using BFS weighted by
    /// travel cost. Returns the ordered list of location IDs from `start`
    /// to `goal` (inclusive), or `None` if no path exists.
    ///
    /// Uses Dijkstra's algorithm with a simple priority queue (`BTreeSet`
    /// as a min-heap keyed on distance).
    pub fn shortest_path(
        &self,
        start: LocationId,
        goal: LocationId,
        weather: Weather,
    ) -> Option<Vec<LocationId>> {
        if start == goal {
            return Some(vec![start]);
        }
        if !self.locations.contains_key(&start) || !self.locations.contains_key(&goal) {
            return None;
        }

        // Distance map: location -> best known distance.
        let mut dist: BTreeMap<LocationId, u32> = BTreeMap::new();
        // Predecessor map for path reconstruction.
        let mut prev: BTreeMap<LocationId, LocationId> = BTreeMap::new();
        // Unvisited set with distances. We use a BTreeMap<(distance, LocationId), ()>
        // as a poor-man's priority queue.
        let mut queue: BTreeSet<(u32, LocationId)> = BTreeSet::new();

        dist.insert(start, 0);
        queue.insert((0, start));

        while let Some(&(current_dist, current)) = queue.iter().next() {
            queue.remove(&(current_dist, current));

            if current == goal {
                break;
            }

            for (neighbor, route_id) in self.neighbors(current) {
                let Some(r) = self.routes.get(&route_id) else {
                    continue;
                };
                let Some(cost) = route::effective_travel_cost(r, weather).ok().flatten() else {
                    continue; // Storm or error -- route not traversable.
                };
                let Some(new_dist) = current_dist.checked_add(cost) else {
                    continue;
                };

                let is_shorter = dist
                    .get(&neighbor)
                    .is_none_or(|&existing| new_dist < existing);

                if is_shorter {
                    // Remove old entry from queue if present.
                    if let Some(&old_dist) = dist.get(&neighbor) {
                        queue.remove(&(old_dist, neighbor));
                    }
                    dist.insert(neighbor, new_dist);
                    prev.insert(neighbor, current);
                    queue.insert((new_dist, neighbor));
                }
            }
        }

        // Reconstruct path.
        if !prev.contains_key(&goal) {
            return None;
        }

        let mut path = VecDeque::new();
        let mut current = goal;
        path.push_front(current);
        while let Some(&predecessor) = prev.get(&current) {
            path.push_front(predecessor);
            current = predecessor;
            if current == start {
                break;
            }
        }

        Some(path.into_iter().collect())
    }

    // -------------------------------------------------------------------
    // Tick operations
    // -------------------------------------------------------------------

    /// Regenerate resources at all locations for one tick.
    ///
    /// Returns a map of location ID to resource regeneration amounts.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::ArithmeticOverflow`] on math failure.
    pub fn regenerate_all_resources(
        &mut self,
        season: Season,
    ) -> Result<BTreeMap<LocationId, BTreeMap<Resource, u32>>, WorldError> {
        let mut results = BTreeMap::new();
        // Collect keys to avoid borrow conflict.
        let ids: Vec<LocationId> = self.locations.keys().copied().collect();
        for id in ids {
            if let Some(loc_state) = self.locations.get_mut(&id) {
                let regen = loc_state.regenerate_all(season)?;
                if !regen.is_empty() {
                    results.insert(id, regen);
                }
            }
        }
        Ok(results)
    }

    /// Move an agent from one location to another.
    ///
    /// Removes the agent from the source location and adds them to the
    /// destination. Validates that both locations exist and the destination
    /// has capacity.
    ///
    /// # Errors
    ///
    /// Returns [`WorldError::LocationNotFound`], [`WorldError::AgentNotAtLocation`],
    /// or [`WorldError::LocationAtCapacity`] as appropriate.
    pub fn move_agent(
        &mut self,
        agent: AgentId,
        from: LocationId,
        to: LocationId,
    ) -> Result<(), WorldError> {
        // Validate both locations exist.
        if !self.locations.contains_key(&from) {
            return Err(WorldError::LocationNotFound(from));
        }
        if !self.locations.contains_key(&to) {
            return Err(WorldError::LocationNotFound(to));
        }

        // Check destination capacity before modifying anything.
        {
            let dest = self
                .locations
                .get(&to)
                .ok_or(WorldError::LocationNotFound(to))?;
            if !dest.has_capacity() {
                return Err(WorldError::LocationAtCapacity {
                    location: to,
                    capacity: dest.location.capacity,
                });
            }
        }

        // Remove from source.
        {
            let source = self
                .locations
                .get_mut(&from)
                .ok_or(WorldError::LocationNotFound(from))?;
            source.remove_occupant(agent)?;
        }

        // Add to destination.
        {
            let dest = self
                .locations
                .get_mut(&to)
                .ok_or(WorldError::LocationNotFound(to))?;
            dest.add_occupant(agent)?;
        }

        Ok(())
    }

    /// Check graph connectivity: whether every location is reachable from
    /// every other location (strongly connected for bidirectional routes).
    ///
    /// Returns `true` if the graph is connected, `false` if there are
    /// isolated components.
    pub fn is_connected(&self) -> bool {
        let ids: Vec<LocationId> = self.locations.keys().copied().collect();
        if ids.is_empty() {
            return true;
        }

        let Some(&start) = ids.first() else {
            return true;
        };

        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::new();
        visited.insert(start);
        queue.push_back(start);

        while let Some(current) = queue.pop_front() {
            for (neighbor, _) in self.neighbors(current) {
                if visited.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }

        visited.len() == self.locations.len()
    }
}

impl Default for WorldMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use chrono::Utc;
    use emergence_types::{PathType, ResourceNode};
    use rust_decimal::Decimal;

    use super::*;

    fn make_location_with_id(id: LocationId, name: &str, region: &str) -> Location {
        let mut resources = BTreeMap::new();
        resources.insert(
            Resource::Wood,
            ResourceNode {
                resource: Resource::Wood,
                available: 50,
                regen_per_tick: 5,
                max_capacity: 100,
            },
        );
        Location {
            id,
            name: name.to_string(),
            region: region.to_string(),
            location_type: "natural".to_string(),
            description: format!("Test location: {name}"),
            capacity: 10,
            base_resources: resources,
            discovered_by: BTreeSet::new(),
            created_at: Utc::now(),
        }
    }

    fn make_route_between(
        from: LocationId,
        to: LocationId,
        cost: u32,
        bidir: bool,
    ) -> Route {
        Route {
            id: RouteId::new(),
            from_location: from,
            to_location: to,
            cost_ticks: cost,
            path_type: PathType::DirtTrail,
            durability: 100,
            max_durability: 100,
            decay_per_tick: Decimal::ZERO,
            acl: None,
            bidirectional: bidir,
            built_by: None,
            built_at_tick: None,
        }
    }

    fn make_triangle_world() -> (WorldMap, LocationId, LocationId, LocationId) {
        let mut map = WorldMap::new();
        let a = LocationId::new();
        let b = LocationId::new();
        let c = LocationId::new();

        let _ = map.add_location(make_location_with_id(a, "Alpha", "North"));
        let _ = map.add_location(make_location_with_id(b, "Beta", "North"));
        let _ = map.add_location(make_location_with_id(c, "Gamma", "South"));

        let _ = map.add_route(make_route_between(a, b, 3, true));
        let _ = map.add_route(make_route_between(b, c, 5, true));
        let _ = map.add_route(make_route_between(a, c, 10, true));

        (map, a, b, c)
    }

    #[test]
    fn add_locations_and_routes() {
        let (map, _, _, _) = make_triangle_world();
        assert_eq!(map.location_count(), 3);
        assert_eq!(map.route_count(), 3);
    }

    #[test]
    fn duplicate_location_rejected() {
        let mut map = WorldMap::new();
        let id = LocationId::new();
        let loc = make_location_with_id(id, "A", "R");
        assert!(map.add_location(loc.clone()).is_ok());
        assert!(map.add_location(loc).is_err());
    }

    #[test]
    fn route_requires_valid_endpoints() {
        let mut map = WorldMap::new();
        let a = LocationId::new();
        let b = LocationId::new();
        let _ = map.add_location(make_location_with_id(a, "A", "R"));
        // b doesn't exist
        let route = make_route_between(a, b, 3, true);
        assert!(map.add_route(route).is_err());
    }

    #[test]
    fn neighbors_bidirectional() {
        let (map, a, b, c) = make_triangle_world();
        let a_neighbors: Vec<LocationId> = map.neighbors(a).iter().map(|(loc, _)| *loc).collect();
        assert!(a_neighbors.contains(&b));
        assert!(a_neighbors.contains(&c));
        assert_eq!(a_neighbors.len(), 2);
    }

    #[test]
    fn neighbors_one_directional() {
        let mut map = WorldMap::new();
        let a = LocationId::new();
        let b = LocationId::new();
        let _ = map.add_location(make_location_with_id(a, "A", "R"));
        let _ = map.add_location(make_location_with_id(b, "B", "R"));
        let _ = map.add_route(make_route_between(a, b, 3, false));

        // a can reach b
        let a_neighbors: Vec<LocationId> = map.neighbors(a).iter().map(|(loc, _)| *loc).collect();
        assert!(a_neighbors.contains(&b));

        // b cannot reach a (one-directional)
        let b_neighbors: Vec<LocationId> = map.neighbors(b).iter().map(|(loc, _)| *loc).collect();
        assert!(!b_neighbors.contains(&a));
    }

    #[test]
    fn routes_between_locations() {
        let (map, a, b, _) = make_triangle_world();
        let routes = map.routes_between(a, b);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes.first().map(|r| r.cost_ticks), Some(3));
    }

    #[test]
    fn shortest_path_direct() {
        let (map, a, _, c) = make_triangle_world();
        // a -> c direct is 10, a -> b -> c is 3+5=8 (shorter)
        let path = map.shortest_path(a, c, Weather::Clear);
        assert!(path.is_some());
        let path = path.unwrap_or_default();
        assert_eq!(path.len(), 3); // a -> b -> c
    }

    #[test]
    fn shortest_path_same_node() {
        let (map, a, _, _) = make_triangle_world();
        let path = map.shortest_path(a, a, Weather::Clear);
        assert_eq!(path, Some(vec![a]));
    }

    #[test]
    fn shortest_path_no_route() {
        let mut map = WorldMap::new();
        let a = LocationId::new();
        let b = LocationId::new();
        let _ = map.add_location(make_location_with_id(a, "A", "R"));
        let _ = map.add_location(make_location_with_id(b, "B", "R"));
        // No route between them.
        let path = map.shortest_path(a, b, Weather::Clear);
        assert!(path.is_none());
    }

    #[test]
    fn shortest_path_storm_blocks() {
        let mut map = WorldMap::new();
        let a = LocationId::new();
        let b = LocationId::new();
        let _ = map.add_location(make_location_with_id(a, "A", "R"));
        let _ = map.add_location(make_location_with_id(b, "B", "R"));
        let _ = map.add_route(make_route_between(a, b, 3, true));
        // Storm blocks travel.
        let path = map.shortest_path(a, b, Weather::Storm);
        assert!(path.is_none());
    }

    #[test]
    fn connectivity_check() {
        let (map, _, _, _) = make_triangle_world();
        assert!(map.is_connected());
    }

    #[test]
    fn disconnected_graph() {
        let mut map = WorldMap::new();
        let a = LocationId::new();
        let b = LocationId::new();
        let _ = map.add_location(make_location_with_id(a, "A", "R"));
        let _ = map.add_location(make_location_with_id(b, "B", "R"));
        // No routes.
        assert!(!map.is_connected());
    }

    #[test]
    fn regenerate_all_resources() {
        let (mut map, a, _, _) = make_triangle_world();
        let results = map.regenerate_all_resources(Season::Summer);
        assert!(results.is_ok());
        let results = results.unwrap_or_default();
        // Each location should regenerate 5 wood.
        let a_regen = results.get(&a);
        assert!(a_regen.is_some());
        assert_eq!(a_regen.and_then(|m| m.get(&Resource::Wood)).copied(), Some(5));
    }

    #[test]
    fn move_agent_between_locations() {
        let (mut map, a, b, _) = make_triangle_world();
        let agent = AgentId::new();

        // Place agent at a.
        let loc_a = map.get_location_mut(a);
        assert!(loc_a.is_some());
        if let Some(loc) = loc_a {
            let _ = loc.add_occupant(agent);
        }

        // Move to b.
        assert!(map.move_agent(agent, a, b).is_ok());

        // Verify.
        assert!(
            map.get_location(a)
                .map_or(false, |l| !l.contains_agent(agent))
        );
        assert!(
            map.get_location(b)
                .map_or(false, |l| l.contains_agent(agent))
        );
    }

    #[test]
    fn move_agent_capacity_check() {
        let mut map = WorldMap::new();
        let a = LocationId::new();
        let b_id = LocationId::new();

        // Location b has capacity 0.
        let mut b_loc = make_location_with_id(b_id, "B", "R");
        b_loc.capacity = 0;

        let _ = map.add_location(make_location_with_id(a, "A", "R"));
        let _ = map.add_location(b_loc);
        let _ = map.add_route(make_route_between(a, b_id, 1, true));

        let agent = AgentId::new();
        if let Some(loc) = map.get_location_mut(a) {
            let _ = loc.add_occupant(agent);
        }

        let result = map.move_agent(agent, a, b_id);
        assert!(result.is_err());
    }

    #[test]
    fn empty_map_is_connected() {
        let map = WorldMap::new();
        assert!(map.is_connected());
    }
}
