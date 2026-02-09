//! REST API endpoint handlers for the Observer server.
//!
//! All handlers read from the in-memory [`SimulationSnapshot`] via the
//! shared [`AppState`]. No database access is required in Phase 2.
//!
//! # Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | `GET` | `/` | Minimal HTML status page |
//! | `GET` | `/api/agents` | List all agents |
//! | `GET` | `/api/agents/:id` | Get single agent + state |
//! | `GET` | `/api/locations` | List all locations |
//! | `GET` | `/api/locations/:id` | Get single location |
//! | `GET` | `/api/events` | Query events (by tick or agent) |
//! | `GET` | `/api/world` | Current world snapshot |

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse};
use axum::Json;
use uuid::Uuid;

use crate::error::ObserverError;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Query parameter structs
// ---------------------------------------------------------------------------

/// Query parameters for the `GET /api/events` endpoint.
#[derive(Debug, serde::Deserialize)]
pub struct EventsQuery {
    /// Filter events by tick number.
    pub tick: Option<u64>,
    /// Filter events by agent ID.
    pub agent_id: Option<String>,
    /// Maximum number of events to return (default 100).
    pub limit: Option<usize>,
}

/// Query parameters for the `GET /api/agents` endpoint.
#[derive(Debug, serde::Deserialize)]
pub struct AgentsQuery {
    /// Filter by alive/dead/all status. Accepted values: `alive`, `dead`, `all`.
    pub status: Option<String>,
}

// ---------------------------------------------------------------------------
// GET / -- minimal HTML status page
// ---------------------------------------------------------------------------

/// Serve a minimal HTML page showing server status and API links.
///
/// This is the placeholder dashboard until the React frontend is built
/// in Phase 4.
pub async fn index(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let snapshot = state.snapshot.read().await;
    let tick = snapshot.current_tick;
    let era = format!("{:?}", snapshot.era);
    let season = format!("{:?}", snapshot.season);
    let weather = format!("{:?}", snapshot.weather);
    let agent_count = snapshot.agent_states.len();
    let location_count = snapshot.locations.len();
    let event_count = snapshot.events.len();

    Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <title>Emergence Observer</title>
    <style>
        body {{
            background: #0d1117;
            color: #c9d1d9;
            font-family: 'Cascadia Code', 'Fira Code', 'Consolas', monospace;
            padding: 2rem;
            max-width: 800px;
            margin: 0 auto;
        }}
        h1 {{ color: #58a6ff; margin-bottom: 0.25rem; }}
        .subtitle {{ color: #8b949e; margin-top: 0; }}
        .metric {{
            display: inline-block;
            background: #161b22;
            border: 1px solid #30363d;
            border-radius: 6px;
            padding: 1rem 1.5rem;
            margin: 0.5rem 0.5rem 0.5rem 0;
            min-width: 120px;
        }}
        .metric .label {{ color: #8b949e; font-size: 0.85rem; }}
        .metric .value {{ color: #58a6ff; font-size: 1.5rem; font-weight: bold; }}
        a {{ color: #58a6ff; text-decoration: none; }}
        a:hover {{ text-decoration: underline; }}
        ul {{ list-style: none; padding: 0; }}
        li {{ padding: 0.3rem 0; }}
        li::before {{ content: "GET "; color: #7ee787; font-weight: bold; }}
        .status {{ color: #3fb950; font-weight: bold; }}
        hr {{ border: none; border-top: 1px solid #30363d; margin: 1.5rem 0; }}
    </style>
</head>
<body>
    <h1>Emergence Observer</h1>
    <p class="subtitle">Simulation monitoring server -- Phase 2 (basic)</p>

    <p>Status: <span class="status">RUNNING</span></p>

    <div>
        <div class="metric">
            <div class="label">Tick</div>
            <div class="value">{tick}</div>
        </div>
        <div class="metric">
            <div class="label">Era</div>
            <div class="value">{era}</div>
        </div>
        <div class="metric">
            <div class="label">Season</div>
            <div class="value">{season}</div>
        </div>
        <div class="metric">
            <div class="label">Weather</div>
            <div class="value">{weather}</div>
        </div>
        <div class="metric">
            <div class="label">Agents</div>
            <div class="value">{agent_count}</div>
        </div>
        <div class="metric">
            <div class="label">Locations</div>
            <div class="value">{location_count}</div>
        </div>
        <div class="metric">
            <div class="label">Events</div>
            <div class="value">{event_count}</div>
        </div>
    </div>

    <hr>

    <h2>API Endpoints</h2>
    <ul>
        <li><a href="/api/world">/api/world</a> -- Current world snapshot</li>
        <li><a href="/api/agents">/api/agents</a> -- List all agents</li>
        <li><a href="/api/agents/:id">/api/agents/:id</a> -- Single agent detail</li>
        <li><a href="/api/locations">/api/locations</a> -- List all locations</li>
        <li><a href="/api/locations/:id">/api/locations/:id</a> -- Single location detail</li>
        <li><a href="/api/events">/api/events</a> -- Query events (?tick=N or ?agent_id=X)</li>
    </ul>

    <h2>WebSocket</h2>
    <ul>
        <li style="list-style:none;"><code>ws://host:port/ws/ticks</code> -- Live tick summary stream</li>
    </ul>
</body>
</html>"#
    ))
}

// ---------------------------------------------------------------------------
// GET /api/world -- current world snapshot
// ---------------------------------------------------------------------------

/// Return the current world snapshot including tick, era, season, weather,
/// population, and economy stats.
pub async fn get_world(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ObserverError> {
    let snapshot = state.snapshot.read().await;

    if let Some(ws) = &snapshot.world_snapshot {
        Ok(Json(serde_json::to_value(ws)?))
    } else {
        // No snapshot yet -- return a minimal response.
        let body = serde_json::json!({
            "tick": snapshot.current_tick,
            "era": snapshot.era,
            "season": snapshot.season,
            "weather": snapshot.weather,
            "agents_count": snapshot.agent_states.len(),
            "locations_count": snapshot.locations.len(),
        });
        Ok(Json(body))
    }
}

// ---------------------------------------------------------------------------
// GET /api/agents -- list agents
// ---------------------------------------------------------------------------

/// List all agents, optionally filtered by alive/dead status.
///
/// # Query Parameters
///
/// - `status`: `alive` | `dead` | `all` (default: `all`)
pub async fn list_agents(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AgentsQuery>,
) -> Result<impl IntoResponse, ObserverError> {
    let snapshot = state.snapshot.read().await;

    let filter = params.status.as_deref().unwrap_or("all");

    let agents: Vec<serde_json::Value> = snapshot
        .agents
        .values()
        .filter(|agent| match filter {
            "alive" => agent.died_at_tick.is_none(),
            "dead" => agent.died_at_tick.is_some(),
            _ => true,
        })
        .map(|agent| {
            let agent_state = snapshot.agent_states.get(&agent.id);
            serde_json::json!({
                "id": agent.id,
                "name": agent.name,
                "born_at_tick": agent.born_at_tick,
                "died_at_tick": agent.died_at_tick,
                "generation": agent.generation,
                "alive": agent.died_at_tick.is_none(),
                "vitals": agent_state.map(|s| serde_json::json!({
                    "energy": s.energy,
                    "health": s.health,
                    "hunger": s.hunger,
                    "age": s.age,
                })),
                "location_id": agent_state.map(|s| s.location_id),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "count": agents.len(),
        "agents": agents,
    })))
}

// ---------------------------------------------------------------------------
// GET /api/agents/:id -- single agent detail
// ---------------------------------------------------------------------------

/// Return the full detail for a single agent including identity,
/// personality, state, inventory, skills, knowledge, and goals.
pub async fn get_agent(
    State(state): State<Arc<AppState>>,
    Path(id_str): Path<String>,
) -> Result<impl IntoResponse, ObserverError> {
    let id = parse_uuid(&id_str)?;
    let agent_id = emergence_types::AgentId::from(id);

    let snapshot = state.snapshot.read().await;

    let agent = snapshot
        .agents
        .get(&agent_id)
        .ok_or_else(|| ObserverError::NotFound(format!("agent {id}")))?;

    let agent_state = snapshot.agent_states.get(&agent_id);

    let body = serde_json::json!({
        "agent": agent,
        "state": agent_state,
    });

    Ok(Json(body))
}

// ---------------------------------------------------------------------------
// GET /api/locations -- list locations
// ---------------------------------------------------------------------------

/// List all locations in the simulation with basic metadata.
pub async fn list_locations(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ObserverError> {
    let snapshot = state.snapshot.read().await;

    let locations: Vec<serde_json::Value> = snapshot
        .locations
        .values()
        .map(|loc| {
            serde_json::json!({
                "id": loc.id,
                "name": loc.name,
                "region": loc.region,
                "location_type": loc.location_type,
                "capacity": loc.capacity,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "count": locations.len(),
        "locations": locations,
    })))
}

// ---------------------------------------------------------------------------
// GET /api/locations/:id -- single location detail
// ---------------------------------------------------------------------------

/// Return the full detail for a single location including resources,
/// capacity, and discovered-by list.
pub async fn get_location(
    State(state): State<Arc<AppState>>,
    Path(id_str): Path<String>,
) -> Result<impl IntoResponse, ObserverError> {
    let id = parse_uuid(&id_str)?;
    let location_id = emergence_types::LocationId::from(id);

    let snapshot = state.snapshot.read().await;

    let location = snapshot
        .locations
        .get(&location_id)
        .ok_or_else(|| ObserverError::NotFound(format!("location {id}")))?;

    // Find agents at this location.
    let agents_here: Vec<serde_json::Value> = snapshot
        .agent_states
        .values()
        .filter(|s| s.location_id == location_id)
        .filter_map(|s| {
            snapshot.agents.get(&s.agent_id).map(|a| {
                serde_json::json!({
                    "id": a.id,
                    "name": a.name,
                })
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "location": location,
        "agents_here": agents_here,
    })))
}

// ---------------------------------------------------------------------------
// GET /api/events -- query events
// ---------------------------------------------------------------------------

/// Query simulation events by tick or agent ID.
///
/// # Query Parameters
///
/// - `tick`: Return events for a specific tick.
/// - `agent_id`: Return events involving a specific agent (UUID).
/// - `limit`: Maximum number of events to return (default 100, max 1000).
pub async fn list_events(
    State(state): State<Arc<AppState>>,
    Query(params): Query<EventsQuery>,
) -> Result<impl IntoResponse, ObserverError> {
    let snapshot = state.snapshot.read().await;

    let limit = params.limit.unwrap_or(100).min(1000);

    let agent_filter = params
        .agent_id
        .as_deref()
        .map(parse_uuid)
        .transpose()?
        .map(emergence_types::AgentId::from);

    let events: Vec<&emergence_types::Event> = snapshot
        .events
        .iter()
        .filter(|e| {
            if let Some(tick) = params.tick
                && e.tick != tick
            {
                return false;
            }
            if let Some(ref agent_id) = agent_filter
                && e.agent_id.as_ref() != Some(agent_id)
            {
                return false;
            }
            true
        })
        .take(limit)
        .collect();

    Ok(Json(serde_json::json!({
        "count": events.len(),
        "events": events,
    })))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a UUID from a string, returning an [`ObserverError`] on failure.
fn parse_uuid(s: &str) -> Result<Uuid, ObserverError> {
    s.parse::<Uuid>()
        .map_err(|e| ObserverError::InvalidUuid(format!("{s}: {e}")))
}
