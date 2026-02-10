//! Observer API server for the Emergence simulation.
//!
//! This crate provides an Axum HTTP server that exposes:
//!
//! - **`WebSocket` endpoint** (`/ws/ticks`) for real-time tick summary
//!   streaming via [`tokio::sync::broadcast`]
//! - **REST endpoints** for querying simulation state (agents, locations,
//!   events, world snapshot)
//! - **Operator REST endpoints** for runtime control (pause, resume,
//!   speed, status, event injection, stop)
//! - **Minimal HTML dashboard** (`GET /`) showing current tick, era,
//!   season, and links to API endpoints
//!
//! # Architecture
//!
//! The observer reads from an in-memory [`SimulationSnapshot`] that is
//! updated each tick by the engine. All REST reads are lock-free reads
//! against this snapshot so the observer never blocks the tick cycle.
//! `WebSocket` clients receive tick summaries via a broadcast channel
//! with automatic lag handling.
//!
//! # Phase
//!
//! This is the Phase 2 (basic) observer. The full React dashboard is
//! built in Phase 4.
//!
//! [`SimulationSnapshot`]: state::SimulationSnapshot

pub mod error;
pub mod handlers;
pub mod operator;
pub mod router;
pub mod server;
pub mod state;
pub mod ws;

// Re-export primary types for convenience.
pub use router::build_router;
pub use server::{start_server, ServerConfig, ServerError};
pub use state::{AppState, SimulationSnapshot, TickBroadcast};
