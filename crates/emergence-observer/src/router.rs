//! Axum router construction for the Observer API.
//!
//! Assembles all routes (REST + `WebSocket`) into a single [`Router`]
//! with CORS middleware enabled for cross-origin dashboard access.

use std::sync::Arc;

use axum::routing::get;
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::handlers;
use crate::state::AppState;
use crate::ws;

/// Build the complete Axum router for the Observer server.
///
/// The router includes:
/// - `GET /` -- minimal HTML status page
/// - `GET /ws/ticks` -- `WebSocket` tick summary stream
/// - `GET /api/world` -- current world snapshot
/// - `GET /api/agents` -- list agents
/// - `GET /api/agents/:id` -- single agent
/// - `GET /api/locations` -- list locations
/// - `GET /api/locations/:id` -- single location
/// - `GET /api/events` -- query events
///
/// CORS is configured to allow any origin for development. In
/// production this should be restricted.
pub fn build_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Status page
        .route("/", get(handlers::index))
        // WebSocket
        .route("/ws/ticks", get(ws::ws_ticks))
        // REST API
        .route("/api/world", get(handlers::get_world))
        .route("/api/agents", get(handlers::list_agents))
        .route("/api/agents/{id}", get(handlers::get_agent))
        .route("/api/locations", get(handlers::list_locations))
        .route("/api/locations/{id}", get(handlers::get_location))
        .route("/api/events", get(handlers::list_events))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
