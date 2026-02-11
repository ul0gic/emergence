//! End-of-tick persistence operations for both `Dragonfly` (hot state) and
//! `PostgreSQL` (cold state).
//!
//! These functions are called during Phase 5 (Persist) of the tick cycle.
//! `Dragonfly` receives the current agent states and world metadata so that
//! the Observer API and the next tick read from an up-to-date hot store.
//! `PostgreSQL` receives events and ledger entries for permanent history.
//!
//! # Architecture
//!
//! ```text
//! End of tick
//!   |
//!   +-- persist_agent_states_to_dragonfly()    --> Dragonfly
//!   +-- persist_world_state_to_dragonfly()     --> Dragonfly
//!   +-- persist_events_to_postgres()           --> PostgreSQL events table
//!   +-- persist_tick_snapshot()                 --> PostgreSQL world_snapshots table
//! ```

use std::collections::BTreeMap;

use emergence_types::{ActionResult, AgentId, AgentState, Season, Weather};
use sqlx::PgPool;

use crate::dragonfly::DragonflyPool;
use crate::error::DbError;
use crate::event_store::EventStore;
use crate::snapshot_store::SnapshotStore;

// =========================================================================
// Error type
// =========================================================================

/// Errors that can occur during tick persistence.
#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    /// A `Dragonfly` operation failed.
    #[error("Dragonfly persist error: {0}")]
    Dragonfly(#[from] DbError),

    /// A `PostgreSQL` operation failed.
    #[error("PostgreSQL persist error: {0}")]
    Postgres(String),

    /// Serialization of state data failed.
    #[error("serialization error: {0}")]
    Serialization(String),
}

// =========================================================================
// Dragonfly (hot state) persistence — Task 7.2.4
// =========================================================================

/// Write all agent states to `Dragonfly` after a tick.
///
/// Uses batch MSET to write all agent states in a single round-trip
/// instead of individual SET calls, which is significantly faster
/// for 100+ agents.
///
/// # Key Schema
///
/// | Key | Value |
/// |-----|-------|
/// | `agent:{id}:state` | JSON-serialized [`AgentState`] |
///
/// # Errors
///
/// Returns [`PersistError::Dragonfly`] if any write to `Dragonfly` fails.
/// Returns [`PersistError::Serialization`] if agent state serialization fails.
pub async fn persist_agent_states_to_dragonfly(
    dragonfly: &DragonflyPool,
    agent_states: &BTreeMap<AgentId, AgentState>,
    tick: u64,
) -> Result<(), PersistError> {
    // Build all key-value pairs for batch MSET.
    let keys: Vec<String> = agent_states
        .keys()
        .map(|id| format!("agent:{}:state", id.into_inner()))
        .collect();
    let values: Vec<&AgentState> = agent_states.values().collect();

    let entries: Vec<(&str, &AgentState)> = keys
        .iter()
        .zip(values.iter())
        .map(|(k, v)| (k.as_str(), *v))
        .collect();

    dragonfly.mset_json(&entries).await?;

    tracing::debug!(
        tick,
        agents = agent_states.len(),
        "Persisted agent states to Dragonfly (batch MSET)"
    );

    Ok(())
}

/// Write world-level summary to `Dragonfly` after a tick.
///
/// Stores the current tick number, alive agent count, season, and weather
/// in well-known keys so the Observer API can serve current world state
/// without querying `PostgreSQL`.
///
/// # Key Schema
///
/// | Key | Value |
/// |-----|-------|
/// | `world:tick` | Current tick number (integer) |
/// | `world:agents_alive` | Count of living agents (integer) |
/// | `world:season` | Current season string |
/// | `world:weather` | Current weather string |
///
/// # Errors
///
/// Returns [`PersistError::Dragonfly`] if any write to `Dragonfly` fails.
pub async fn persist_world_state_to_dragonfly(
    dragonfly: &DragonflyPool,
    tick: u64,
    agents_alive: u32,
    season: Season,
    weather: Weather,
) -> Result<(), PersistError> {
    dragonfly.set_world_tick(tick).await?;

    let season_str = format!("{season:?}");
    let weather_str = format!("{weather:?}");

    dragonfly
        .set_json("world:agents_alive", &agents_alive)
        .await?;
    dragonfly.set_json("world:season", &season_str).await?;
    dragonfly.set_json("world:weather", &weather_str).await?;

    tracing::debug!(
        tick,
        agents_alive,
        season = season_str.as_str(),
        weather = weather_str.as_str(),
        "Persisted world state to Dragonfly"
    );

    Ok(())
}

// =========================================================================
// PostgreSQL (cold state) persistence — Task 7.2.5
// =========================================================================

/// Batch insert tick events to `PostgreSQL` from action results.
///
/// Converts each [`ActionResult`] into an [`emergence_types::Event`] and
/// delegates to the existing [`EventStore::batch_insert`] method. Events
/// record the permanent history of agent actions.
///
/// # Errors
///
/// Returns [`PersistError::Postgres`] if the batch insert fails.
/// Returns [`PersistError::Serialization`] if event construction fails.
pub async fn persist_events_to_postgres(
    pool: &PgPool,
    tick: u64,
    action_results: &BTreeMap<AgentId, ActionResult>,
) -> Result<(), PersistError> {
    if action_results.is_empty() {
        return Ok(());
    }

    let mut events = Vec::with_capacity(action_results.len());
    let now = chrono::Utc::now();

    for (agent_id, result) in action_results {
        let event_type = if result.success {
            emergence_types::EventType::ActionSucceeded
        } else {
            emergence_types::EventType::ActionRejected
        };

        let details = serde_json::to_value(result).map_err(|e| {
            PersistError::Serialization(format!("Failed to serialize action result: {e}"))
        })?;

        let world_context = emergence_types::WorldContext {
            tick,
            era: emergence_types::Era::Primitive,
            season: emergence_types::Season::Spring,
            weather: emergence_types::Weather::Clear,
            population: 0,
        };

        let event = emergence_types::Event {
            id: emergence_types::EventId::new(),
            tick,
            event_type,
            agent_id: Some(*agent_id),
            location_id: None,
            details,
            agent_state_snapshot: None,
            world_context,
            created_at: now,
        };
        events.push(event);
    }

    let store = EventStore::new(pool);
    store
        .batch_insert(&events)
        .await
        .map_err(|e| PersistError::Postgres(format!("Event batch insert failed: {e}")))?;

    tracing::debug!(
        tick,
        events = events.len(),
        "Persisted events to PostgreSQL"
    );

    Ok(())
}

/// Persist a tick summary as a world snapshot to `PostgreSQL`.
///
/// Writes a row to the `world_snapshots` table via [`SnapshotStore`].
/// This captures population, deaths, action counts, and environment
/// state at the end of each tick.
///
/// # Errors
///
/// Returns [`PersistError::Postgres`] if the snapshot insert fails.
/// Returns [`PersistError::Serialization`] if summary serialization fails.
pub async fn persist_tick_snapshot(
    pool: &PgPool,
    tick: u64,
    season: Season,
    weather: Weather,
    agents_alive: u32,
    deaths_count: u32,
    action_results_count: u32,
) -> Result<(), PersistError> {
    let store = SnapshotStore::new(pool);

    let season_str = format!("{season:?}");
    let weather_str = format!("{weather:?}");

    let agents_alive_i32 =
        i32::try_from(agents_alive).unwrap_or(i32::MAX);
    let deaths_i32 = i32::try_from(deaths_count).unwrap_or(i32::MAX);
    let actions_i32 =
        i32::try_from(action_results_count).unwrap_or(i32::MAX);

    let total_resources = serde_json::Value::Object(serde_json::Map::new());
    let wealth_distribution = serde_json::Value::Object(serde_json::Map::new());
    let summary_json = serde_json::json!({
        "tick": tick,
        "deaths": deaths_count,
        "actions_resolved": action_results_count,
    });

    store
        .insert_world_snapshot(
            tick,
            "primitive",
            &season_str,
            &weather_str,
            agents_alive_i32,
            0, // births -- not tracked in TickSummary yet
            deaths_i32,
            &total_resources,
            &wealth_distribution,
            actions_i32,
            0, // discoveries -- not tracked in TickSummary yet
            &summary_json,
        )
        .await
        .map_err(|e| PersistError::Postgres(format!("Snapshot insert failed: {e}")))?;

    tracing::debug!(tick, "Persisted tick snapshot to PostgreSQL");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persist_error_display() {
        let err = PersistError::Serialization(String::from("test error"));
        let msg = format!("{err}");
        assert!(msg.contains("test error"));
    }

    #[test]
    fn persist_error_from_db_error() {
        let db_err = DbError::KeyNotFound(String::from("world:tick"));
        let persist_err = PersistError::from(db_err);
        let msg = format!("{persist_err}");
        assert!(msg.contains("world:tick"));
    }
}
