//! Operator REST API handlers for runtime simulation control.
//!
//! These endpoints are separate from the observer read-only API and from
//! the agent NATS communication channels. They provide one-way command
//! authority from the operator to the World Engine.
//!
//! # Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | `POST` | `/api/operator/pause` | Pause the tick loop |
//! | `POST` | `/api/operator/resume` | Resume the tick loop |
//! | `POST` | `/api/operator/speed` | Set tick interval (ms) |
//! | `GET` | `/api/operator/status` | Current simulation status |
//! | `POST` | `/api/operator/inject-event` | Queue an event for injection |
//! | `POST` | `/api/operator/stop` | Trigger clean shutdown |

use std::sync::Arc;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::error::ObserverError;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

/// Request body for `POST /api/operator/speed`.
#[derive(Debug, serde::Deserialize)]
pub struct SetSpeedRequest {
    /// New tick interval in milliseconds (minimum 100).
    pub tick_interval_ms: u64,
}

/// Request body for `POST /api/operator/spawn-agent`.
#[derive(Debug, serde::Deserialize)]
pub struct SpawnAgentRequest {
    /// Optional display name for the agent.
    pub name: Option<String>,
    /// Optional starting location (UUID string).
    pub location_id: Option<emergence_types::LocationId>,
    /// Personality generation mode (default: `"random"`).
    #[serde(default = "default_personality_mode")]
    pub personality_mode: String,
}

fn default_personality_mode() -> String {
    String::from("random")
}

/// Request body for `POST /api/operator/inject-event`.
#[derive(Debug, serde::Deserialize)]
pub struct InjectEventRequest {
    /// The type of event to inject (e.g. "plague", "resource\_boom").
    pub event_type: String,
    /// Optional target region.
    pub target_region: Option<String>,
    /// Optional severity.
    pub severity: Option<String>,
    /// Optional description.
    pub description: Option<String>,
}

/// Generic success response.
#[derive(Debug, serde::Serialize)]
struct OperatorResponse {
    /// Whether the operation succeeded.
    ok: bool,
    /// Human-readable message.
    message: String,
}

// ---------------------------------------------------------------------------
// POST /api/operator/pause
// ---------------------------------------------------------------------------

/// Pause the simulation tick loop.
///
/// The tick loop will sleep until resumed. All state is preserved in
/// memory (and Dragonfly in production). Returns an error if no
/// operator state is attached.
pub async fn pause(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ObserverError> {
    let operator = state
        .operator_state
        .as_ref()
        .ok_or_else(|| ObserverError::Internal("operator state not available".to_owned()))?;

    operator.pause();

    Ok(Json(OperatorResponse {
        ok: true,
        message: "Simulation paused".to_owned(),
    }))
}

// ---------------------------------------------------------------------------
// POST /api/operator/resume
// ---------------------------------------------------------------------------

/// Resume the simulation tick loop after a pause.
pub async fn resume(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ObserverError> {
    let operator = state
        .operator_state
        .as_ref()
        .ok_or_else(|| ObserverError::Internal("operator state not available".to_owned()))?;

    operator.resume();

    Ok(Json(OperatorResponse {
        ok: true,
        message: "Simulation resumed".to_owned(),
    }))
}

// ---------------------------------------------------------------------------
// POST /api/operator/speed
// ---------------------------------------------------------------------------

/// Change the tick interval at runtime.
///
/// The new interval takes effect before the next tick's sleep. Minimum
/// 100ms to prevent runaway ticks.
pub async fn set_speed(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetSpeedRequest>,
) -> Result<impl IntoResponse, ObserverError> {
    let operator = state
        .operator_state
        .as_ref()
        .ok_or_else(|| ObserverError::Internal("operator state not available".to_owned()))?;

    operator.set_tick_interval_ms(body.tick_interval_ms).map_or_else(
        || {
            Err(ObserverError::InvalidQuery(
                "tick_interval_ms must be at least 100".to_owned(),
            ))
        },
        |prev| {
            Ok(Json(serde_json::json!({
                "ok": true,
                "message": format!("Tick interval changed from {}ms to {}ms", prev, body.tick_interval_ms),
                "previous_interval_ms": prev,
                "new_interval_ms": body.tick_interval_ms,
            })))
        },
    )
}

// ---------------------------------------------------------------------------
// GET /api/operator/status
// ---------------------------------------------------------------------------

/// Return the current simulation status including tick, elapsed time,
/// pause state, speed, and agent counts.
pub async fn status(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ObserverError> {
    let operator = state
        .operator_state
        .as_ref()
        .ok_or_else(|| ObserverError::Internal("operator state not available".to_owned()))?;

    let snapshot = state.snapshot.read().await;

    let agents_alive = snapshot
        .agent_states
        .values()
        .filter(|s| {
            snapshot
                .agents
                .get(&s.agent_id)
                .is_some_and(|a| a.died_at_tick.is_none())
        })
        .count();
    let agents_alive_u64 = u64::try_from(agents_alive).unwrap_or(u64::MAX);

    let agents_total = u64::try_from(snapshot.agents.len()).unwrap_or(u64::MAX);

    let end_reason = operator.end_reason().await;

    let status = emergence_core::operator::SimulationStatus {
        tick: snapshot.current_tick,
        paused: operator.is_paused(),
        stop_requested: operator.is_stop_requested(),
        tick_interval_ms: operator.tick_interval_ms(),
        elapsed_seconds: operator.elapsed_seconds(),
        max_ticks: operator.max_ticks(),
        max_real_time_seconds: operator.max_real_time_seconds(),
        agents_alive: agents_alive_u64,
        agents_total,
        end_reason,
        started_at: operator.started_at().to_rfc3339(),
    };

    Ok(Json(status))
}

// ---------------------------------------------------------------------------
// POST /api/operator/inject-event
// ---------------------------------------------------------------------------

/// Queue an operator event for injection at the next tick.
///
/// The event will be processed during the World Wake phase of the
/// next tick cycle.
pub async fn inject_event(
    State(state): State<Arc<AppState>>,
    Json(body): Json<InjectEventRequest>,
) -> Result<impl IntoResponse, ObserverError> {
    let operator = state
        .operator_state
        .as_ref()
        .ok_or_else(|| ObserverError::Internal("operator state not available".to_owned()))?;

    let event = emergence_core::operator::InjectedEvent {
        event_type: body.event_type.clone(),
        target_region: body.target_region,
        severity: body.severity,
        description: body.description,
    };

    operator.inject_event(event).await;

    Ok(Json(OperatorResponse {
        ok: true,
        message: format!("Event '{}' queued for next tick", body.event_type),
    }))
}

// ---------------------------------------------------------------------------
// POST /api/operator/stop
// ---------------------------------------------------------------------------

/// Trigger a clean simulation shutdown.
///
/// The tick loop will finish its current tick, take a final snapshot,
/// emit a `SimulationEnded` event, and stop. The HTTP server continues
/// running so the observer can still query historical data.
pub async fn stop(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ObserverError> {
    let operator = state
        .operator_state
        .as_ref()
        .ok_or_else(|| ObserverError::Internal("operator state not available".to_owned()))?;

    operator.request_stop();

    Ok(Json(OperatorResponse {
        ok: true,
        message: "Stop requested -- simulation will end after current tick".to_owned(),
    }))
}

// ---------------------------------------------------------------------------
// POST /api/operator/restart
// ---------------------------------------------------------------------------

/// Request a simulation restart.
///
/// Sets a restart flag on the operator state that the engine checks.
/// The engine will cleanly stop the current simulation and the
/// orchestrator is expected to re-initialize and restart.
pub async fn restart(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ObserverError> {
    let operator = state
        .operator_state
        .as_ref()
        .ok_or_else(|| ObserverError::Internal("operator state not available".to_owned()))?;

    operator.request_restart();

    Ok(Json(serde_json::json!({
        "status": "restarting",
    })))
}

// ---------------------------------------------------------------------------
// POST /api/operator/spawn-agent
// ---------------------------------------------------------------------------

/// Queue an agent spawn request for the next tick.
///
/// The agent will be created during the pre-tick spawn processing phase
/// and will participate in the simulation starting from the following
/// perception cycle.
pub async fn spawn_agent(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SpawnAgentRequest>,
) -> Result<impl IntoResponse, ObserverError> {
    let operator = state
        .operator_state
        .as_ref()
        .ok_or_else(|| ObserverError::Internal("operator state not available".to_owned()))?;

    let request = emergence_core::operator::SpawnRequest {
        name: body.name,
        location_id: body.location_id,
        personality_mode: body.personality_mode,
    };

    operator.queue_agent_spawn(request).await;

    Ok(Json(serde_json::json!({
        "status": "queued",
        "message": "Agent spawn queued for next tick",
    })))
}
