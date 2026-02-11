//! Event store operations for batch-inserting simulation events.
//!
//! Events are the source of truth for the simulation's history. Every state
//! change produces an immutable event written to `PostgreSQL`. Events are
//! partitioned by tick range (10,000 ticks per partition).
//!
//! See: `data-schemas.md` section 5, `world-engine.md` section 10.2

use emergence_types::{Event, EventType};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::DbError;

/// Default batch size for event inserts.
const DEFAULT_BATCH_SIZE: usize = 100;

/// Operations on the `events` table.
pub struct EventStore<'a> {
    pool: &'a PgPool,
    batch_size: usize,
}

impl<'a> EventStore<'a> {
    /// Create a new event store bound to a connection pool.
    pub const fn new(pool: &'a PgPool) -> Self {
        Self {
            pool,
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }

    /// Set the batch size for inserts.
    #[must_use]
    pub const fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Batch-insert events into the `events` table.
    ///
    /// Events are inserted in batches using multi-row VALUES clauses for
    /// efficiency. Each batch is wrapped in a transaction so either all
    /// events in the batch are committed or none are.
    ///
    /// Optimization: instead of N individual INSERT statements, each batch
    /// uses a single INSERT with N value tuples, reducing round-trips to
    /// `PostgreSQL` by a factor of N.
    ///
    /// # Arguments
    ///
    /// * `events` - The events to insert, typically all events from a single tick.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the insert fails.
    pub async fn batch_insert(&self, events: &[Event]) -> Result<(), DbError> {
        if events.is_empty() {
            return Ok(());
        }

        for chunk in events.chunks(self.batch_size) {
            let mut tx = self.pool.begin().await?;

            // Pre-allocate arrays for UNNEST-based batch insert.
            let len = chunk.len();
            let mut ticks = Vec::with_capacity(len);
            let mut event_types = Vec::with_capacity(len);
            let mut agent_ids: Vec<Option<Uuid>> = Vec::with_capacity(len);
            let mut location_ids: Vec<Option<Uuid>> = Vec::with_capacity(len);
            let mut details_arr = Vec::with_capacity(len);
            let mut snapshots: Vec<Option<serde_json::Value>> = Vec::with_capacity(len);
            let mut contexts = Vec::with_capacity(len);
            let mut timestamps = Vec::with_capacity(len);

            for event in chunk {
                ticks.push(i64::try_from(event.tick).unwrap_or(i64::MAX));
                event_types.push(event_type_to_db(event.event_type).to_owned());
                agent_ids.push(event.agent_id.map(emergence_types::AgentId::into_inner));
                location_ids.push(
                    event
                        .location_id
                        .map(emergence_types::LocationId::into_inner),
                );
                details_arr.push(event.details.clone());
                snapshots.push(
                    event
                        .agent_state_snapshot
                        .as_ref()
                        .map(serde_json::to_value)
                        .transpose()
                        .map_err(DbError::Serialization)?,
                );
                contexts.push(
                    serde_json::to_value(&event.world_context)
                        .map_err(DbError::Serialization)?,
                );
                timestamps.push(event.created_at);
            }

            // Multi-row INSERT using UNNEST for batch efficiency.
            sqlx::query(
                r"INSERT INTO events (tick, event_type, agent_id, location_id, details, agent_state_snapshot, world_context, created_at)
                  SELECT * FROM UNNEST($1::BIGINT[], $2::event_type[], $3::UUID[], $4::UUID[], $5::JSONB[], $6::JSONB[], $7::JSONB[], $8::TIMESTAMPTZ[])",
            )
            .bind(&ticks)
            .bind(&event_types)
            .bind(&agent_ids)
            .bind(&location_ids)
            .bind(&details_arr)
            .bind(&snapshots)
            .bind(&contexts)
            .bind(&timestamps)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;
        }

        tracing::debug!(count = events.len(), "Inserted events (batch UNNEST)");
        Ok(())
    }

    /// Query events for a specific tick.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the query fails.
    pub async fn get_events_by_tick(&self, tick: u64) -> Result<Vec<EventRow>, DbError> {
        let tick_i64 = i64::try_from(tick).unwrap_or(i64::MAX);
        let rows = sqlx::query_as::<_, EventRow>(
            r"SELECT id, tick, event_type::TEXT as event_type, agent_id, location_id, details, agent_state_snapshot, world_context, created_at
              FROM events
              WHERE tick = $1
              ORDER BY id",
        )
        .bind(tick_i64)
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    /// Query events for a specific agent within a tick range.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the query fails.
    pub async fn get_events_by_agent(
        &self,
        agent_id: Uuid,
        from_tick: u64,
        to_tick: u64,
    ) -> Result<Vec<EventRow>, DbError> {
        let from_i64 = i64::try_from(from_tick).unwrap_or(i64::MAX);
        let to_i64 = i64::try_from(to_tick).unwrap_or(i64::MAX);
        let rows = sqlx::query_as::<_, EventRow>(
            r"SELECT id, tick, event_type::TEXT as event_type, agent_id, location_id, details, agent_state_snapshot, world_context, created_at
              FROM events
              WHERE agent_id = $1 AND tick >= $2 AND tick < $3
              ORDER BY tick, id",
        )
        .bind(agent_id)
        .bind(from_i64)
        .bind(to_i64)
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }
}

/// A row from the `events` table.
///
/// Uses runtime types rather than compile-time checked types to
/// avoid requiring a live database during builds.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EventRow {
    /// Auto-incremented event ID.
    pub id: i64,
    /// The tick when this event occurred.
    pub tick: i64,
    /// Event type as a string (cast from the `PostgreSQL` enum).
    pub event_type: String,
    /// Primary agent involved, if any.
    pub agent_id: Option<Uuid>,
    /// Location where the event occurred, if any.
    pub location_id: Option<Uuid>,
    /// Type-specific payload.
    pub details: serde_json::Value,
    /// Agent state snapshot at event time.
    pub agent_state_snapshot: Option<serde_json::Value>,
    /// World context at event time.
    pub world_context: Option<serde_json::Value>,
    /// Real-world timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Convert an [`EventType`] enum variant to its `PostgreSQL` enum string.
const fn event_type_to_db(et: EventType) -> &'static str {
    match et {
        EventType::TickStart => "tick_start",
        EventType::TickEnd => "tick_end",
        EventType::AgentBorn => "agent_born",
        EventType::AgentDied => "agent_died",
        EventType::ActionSubmitted => "action_submitted",
        EventType::ActionSucceeded => "action_succeeded",
        EventType::ActionRejected => "action_rejected",
        EventType::ResourceGathered => "resource_gathered",
        EventType::ResourceConsumed => "resource_consumed",
        EventType::TradeCompleted => "trade_completed",
        EventType::TradeFailed => "trade_failed",
        EventType::StructureBuilt => "structure_built",
        EventType::StructureDestroyed => "structure_destroyed",
        EventType::StructureRepaired => "structure_repaired",
        EventType::RouteImproved => "route_improved",
        EventType::LocationDiscovered => "location_discovered",
        EventType::KnowledgeDiscovered => "knowledge_discovered",
        EventType::KnowledgeTaught => "knowledge_taught",
        EventType::MessageSent => "message_sent",
        EventType::GroupFormed => "group_formed",
        EventType::RelationshipChanged => "relationship_changed",
        EventType::WeatherChanged => "weather_changed",
        EventType::SeasonChanged => "season_changed",
        EventType::RouteDegraded => "route_degraded",
        EventType::StructureClaimed => "structure_claimed",
        EventType::RuleCreated => "rule_created",
        EventType::EnforcementApplied => "enforcement_applied",
        EventType::LedgerAnomaly => "ledger_anomaly",
        EventType::TheftOccurred => "theft_occurred",
        EventType::TheftFailed => "theft_failed",
        EventType::CombatInitiated => "combat_initiated",
        EventType::CombatResolved => "combat_resolved",
    }
}
