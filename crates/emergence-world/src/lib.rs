//! Geography, resources, structures, and environment for the Emergence simulation.
//!
//! This crate models the physical world: locations as a directed weighted graph,
//! resource nodes with regeneration, routes with travel cost and access control,
//! and environmental cycles (seasons, weather, day/night).
//!
//! # Modules
//!
//! - [`environment`] -- Weather generation with season-weighted probabilities
//!   and deterministic randomness for reproducible simulations.
//! - [`error`] -- Error types for world-graph operations.
//! - [`farming`] -- Farm plot crop state tracking, planting, growth timers,
//!   and harvest yield calculation.
//! - [`location`] -- [`LocationState`] wraps the canonical [`Location`] type
//!   with mutable runtime state (occupants, structures).
//! - [`resource`] -- Regeneration and harvesting logic for resource nodes.
//! - [`route`] -- Traversal checks, travel cost calculation with weather.
//! - [`world_map`] -- The world graph: locations as nodes, routes as edges,
//!   with pathfinding, neighbor queries, and batch operations.
//! - [`starting_world`] -- Default 12-location starting map across 3 regions.
//!
//! [`Location`]: emergence_types::Location
//! [`LocationState`]: location::LocationState

pub mod environment;
pub mod error;
pub mod farming;
pub mod location;
pub mod resource;
pub mod route;
pub mod starting_world;
pub mod structure;
pub mod world_map;

// Re-export primary types at crate root.
pub use environment::WeatherSystem;
pub use error::WorldError;
pub use location::LocationState;
pub use starting_world::{StartingLocationIds, create_starting_world};
pub use structure::{
    apply_decay, apply_repair, blueprint, compute_repair_cost, compute_salvage,
    structure_effects_at_location,
};
pub use farming::{
    BASE_HARVEST_YIELD, DEFAULT_GROWTH_TICKS, FarmCropState, FarmRegistry, harvest_yield,
};
pub use world_map::WorldMap;
