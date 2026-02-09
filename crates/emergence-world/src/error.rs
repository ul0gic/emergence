//! Error types for the `emergence-world` crate.
//!
//! All fallible operations in this crate return [`WorldError`] through the
//! standard [`Result`] type alias.

use emergence_types::{AgentId, LocationId, Resource, RouteId};

/// Errors that can occur during world-graph operations.
#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    /// A location was not found in the world graph.
    #[error("location not found: {0}")]
    LocationNotFound(LocationId),

    /// A route was not found in the world graph.
    #[error("route not found: {0}")]
    RouteNotFound(RouteId),

    /// No route exists between the specified locations.
    #[error("no route from {from} to {to}")]
    NoRouteBetween {
        /// Origin location.
        from: LocationId,
        /// Destination location.
        to: LocationId,
    },

    /// The location has reached its maximum occupant capacity.
    #[error("location {location} is at capacity ({capacity})")]
    LocationAtCapacity {
        /// The full location.
        location: LocationId,
        /// Maximum capacity.
        capacity: u32,
    },

    /// The agent is not present at the specified location.
    #[error("agent {agent} is not at location {location}")]
    AgentNotAtLocation {
        /// The agent.
        agent: AgentId,
        /// The location.
        location: LocationId,
    },

    /// The agent is denied access to a route by its ACL.
    #[error("agent {agent} is denied access to route {route}")]
    AccessDenied {
        /// The denied agent.
        agent: AgentId,
        /// The restricted route.
        route: RouteId,
    },

    /// The requested resource is not available at the location.
    #[error("resource {resource:?} not available at location {location}")]
    ResourceNotAvailable {
        /// The requested resource.
        resource: Resource,
        /// The location.
        location: LocationId,
    },

    /// Arithmetic overflow during a checked operation.
    #[error("arithmetic overflow in world calculation")]
    ArithmeticOverflow,

    /// A duplicate entity was inserted where uniqueness is required.
    #[error("duplicate location id: {0}")]
    DuplicateLocation(LocationId),

    /// A duplicate route was inserted where uniqueness is required.
    #[error("duplicate route id: {0}")]
    DuplicateRoute(RouteId),
}
