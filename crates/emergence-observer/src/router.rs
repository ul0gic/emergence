//! Axum router construction for the Observer API.
//!
//! Assembles all routes (REST + `WebSocket`) into a single [`Router`]
//! with CORS middleware enabled for cross-origin dashboard access.

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::alerts;
use crate::anomaly;
use crate::handlers;
use crate::operator;
use crate::social;
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
/// - `POST /api/operator/pause` -- pause the tick loop
/// - `POST /api/operator/resume` -- resume the tick loop
/// - `POST /api/operator/speed` -- set tick interval
/// - `GET /api/operator/status` -- simulation status
/// - `POST /api/operator/inject-event` -- inject an operator event
/// - `POST /api/operator/stop` -- trigger clean shutdown
/// - `POST /api/operator/spawn-agent` -- queue agent spawn
/// - `POST /api/operator/restart` -- request simulation restart
/// - `GET /api/social/beliefs` -- detected belief systems
/// - `GET /api/social/governance` -- governance structures
/// - `GET /api/social/families` -- family units and lineage
/// - `GET /api/social/economy` -- economic classification
/// - `GET /api/social/crime` -- crime and justice stats
/// - `GET /api/anomalies/clusters` -- behavior clusters (Phase 8.3)
/// - `GET /api/anomalies/flags` -- anomaly flags (Phase 8.3)
/// - `GET /api/alerts` -- alert list (Phase 5.4)
/// - `POST /api/alerts/:id/acknowledge` -- acknowledge alert (Phase 5.4)
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
        // REST API (observer, read-only)
        .route("/api/world", get(handlers::get_world))
        .route("/api/agents", get(handlers::list_agents))
        .route("/api/agents/{id}", get(handlers::get_agent))
        .route("/api/locations", get(handlers::list_locations))
        .route("/api/locations/{id}", get(handlers::get_location))
        .route("/api/events", get(handlers::list_events))
        .route("/api/routes", get(handlers::list_routes))
        .route("/api/decisions", get(handlers::list_decisions))
        // Operator API (control endpoints)
        .route("/api/operator/pause", post(operator::pause))
        .route("/api/operator/resume", post(operator::resume))
        .route("/api/operator/speed", post(operator::set_speed))
        .route("/api/operator/status", get(operator::status))
        .route("/api/operator/inject-event", post(operator::inject_event))
        .route("/api/operator/stop", post(operator::stop))
        .route("/api/operator/spawn-agent", post(operator::spawn_agent))
        .route("/api/operator/restart", post(operator::restart))
        // Social construct detection API
        .route("/api/social/beliefs", get(social::beliefs))
        .route("/api/social/governance", get(social::governance))
        .route("/api/social/families", get(social::families))
        .route("/api/social/economy", get(social::economy))
        .route("/api/social/crime", get(social::crime))
        // Anomaly detection API (Phase 8.3)
        .route("/api/anomalies/clusters", get(anomaly::get_clusters))
        .route("/api/anomalies/flags", get(anomaly::get_flags))
        // Alert system API (Phase 5.4)
        .route("/api/alerts", get(alerts::list_alerts))
        .route(
            "/api/alerts/{id}/acknowledge",
            post(alerts::acknowledge_alert),
        )
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
