//! Geography, resources, structures, environment, and knowledge for the Emergence simulation.
//!
//! This crate models the physical world: locations as a directed weighted graph,
//! resource nodes with regeneration, routes with travel cost and access control,
//! environmental cycles (seasons, weather, day/night), cultural knowledge, and
//! knowledge diffusion tracking.
//!
//! # Modules
//!
//! - [`cultural_knowledge`] -- Non-mechanical cultural knowledge (philosophy,
//!   art, music, mythology, ethics) that influences agent behavior and social
//!   cohesion without unlocking mechanical actions.
//! - [`diffusion`] -- Technology and cultural knowledge diffusion tracking:
//!   adoption curves, resistance rates, diffusion speed, knowledge hoarders.
//! - [`environment`] -- Weather generation with season-weighted probabilities
//!   and deterministic randomness for reproducible simulations.
//! - [`error`] -- Error types for world-graph operations.
//! - [`farming`] -- Farm plot crop state tracking, planting, growth timers,
//!   and harvest yield calculation.
//! - [`innovation`] -- Open innovation proposals: agents combine knowledge
//!   to propose new inventions evaluated by the engine.
//! - [`knowledge`] -- Knowledge tree and tech progression from Primitive
//!   through Early Industrial era with prerequisite chains.
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

pub mod cultural_knowledge;
pub mod diffusion;
pub mod environment;
pub mod error;
pub mod farming;
pub mod innovation;
pub mod knowledge;
pub mod location;
pub mod resource;
pub mod route;
pub mod starting_world;
pub mod structure;
pub mod world_map;

// Re-export primary types at crate root.
pub use environment::WeatherSystem;
pub use error::WorldError;
pub use innovation::{InnovationEvaluator, InnovationProposal, InnovationResult};
pub use knowledge::{KnowledgeEra, KnowledgeItem, KnowledgeTree, build_extended_tech_tree};
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
pub use cultural_knowledge::{
    AggregateModifiers, BehavioralInfluence, CulturalCategory, CulturalKnowledge,
    CulturalRegistry, seed_cultural_knowledge,
};
pub use diffusion::{
    AdoptionCurve, DiffusionEvent, DiffusionSource, DiffusionTracker, ResistanceRecord,
    SourceBreakdown,
};
