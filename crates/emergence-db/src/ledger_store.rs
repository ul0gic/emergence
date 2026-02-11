//! Ledger persistence for batch-inserting resource transfer records.
//!
//! The central ledger tracks every resource movement in the simulation.
//! Ledger entries are flushed to `PostgreSQL` at the end of each tick
//! in batches for efficiency.
//!
//! See: `data-schemas.md` section 6, `world-engine.md` section 4.2

use emergence_types::{EntityType, LedgerEntry, LedgerEntryType, Resource};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::DbError;

/// Default batch size for ledger inserts.
const DEFAULT_BATCH_SIZE: usize = 100;

/// Operations on the `ledger` table.
pub struct LedgerStore<'a> {
    pool: &'a PgPool,
    batch_size: usize,
}

impl<'a> LedgerStore<'a> {
    /// Create a new ledger store bound to a connection pool.
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

    /// Batch-insert ledger entries into the `ledger` table.
    ///
    /// Entries are inserted in batches using multi-row UNNEST for efficiency.
    /// Each batch is wrapped in a transaction for atomicity.
    ///
    /// Optimization: instead of N individual INSERT statements per batch,
    /// a single INSERT with UNNEST arrays reduces round-trips.
    ///
    /// # Arguments
    ///
    /// * `entries` - The ledger entries to persist, typically all entries from
    ///   a single tick.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the insert fails.
    pub async fn batch_insert(&self, entries: &[LedgerEntry]) -> Result<(), DbError> {
        if entries.is_empty() {
            return Ok(());
        }

        for chunk in entries.chunks(self.batch_size) {
            let mut tx = self.pool.begin().await?;

            let len = chunk.len();
            let mut ids = Vec::with_capacity(len);
            let mut ticks = Vec::with_capacity(len);
            let mut entry_types = Vec::with_capacity(len);
            let mut from_entities: Vec<Option<Uuid>> = Vec::with_capacity(len);
            let mut from_entity_types: Vec<Option<String>> = Vec::with_capacity(len);
            let mut to_entities: Vec<Option<Uuid>> = Vec::with_capacity(len);
            let mut to_entity_types: Vec<Option<String>> = Vec::with_capacity(len);
            let mut resources = Vec::with_capacity(len);
            let mut quantities = Vec::with_capacity(len);
            let mut reasons = Vec::with_capacity(len);
            let mut reference_ids: Vec<Option<Uuid>> = Vec::with_capacity(len);
            let mut timestamps = Vec::with_capacity(len);

            for entry in chunk {
                ids.push(entry.id.into_inner());
                ticks.push(i64::try_from(entry.tick).unwrap_or(i64::MAX));
                entry_types.push(ledger_entry_type_to_db(entry.entry_type).to_owned());
                from_entities.push(entry.from_entity);
                from_entity_types
                    .push(entry.from_entity_type.map(|e| entity_type_to_db(e).to_owned()));
                to_entities.push(entry.to_entity);
                to_entity_types
                    .push(entry.to_entity_type.map(|e| entity_type_to_db(e).to_owned()));
                resources.push(resource_to_db(entry.resource).to_owned());
                quantities.push(entry.quantity);
                reasons.push(entry.reason.clone());
                reference_ids.push(entry.reference_id);
                timestamps.push(entry.created_at);
            }

            sqlx::query(
                r"INSERT INTO ledger (id, tick, entry_type, from_entity, from_entity_type, to_entity, to_entity_type, resource, quantity, reason, reference_id, created_at)
                  SELECT * FROM UNNEST($1::UUID[], $2::BIGINT[], $3::ledger_entry_type[], $4::UUID[], $5::entity_type[], $6::UUID[], $7::entity_type[], $8::TEXT[], $9::NUMERIC[], $10::TEXT[], $11::UUID[], $12::TIMESTAMPTZ[])",
            )
            .bind(&ids)
            .bind(&ticks)
            .bind(&entry_types)
            .bind(&from_entities)
            .bind(&from_entity_types)
            .bind(&to_entities)
            .bind(&to_entity_types)
            .bind(&resources)
            .bind(&quantities)
            .bind(&reasons)
            .bind(&reference_ids)
            .bind(&timestamps)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;
        }

        tracing::debug!(count = entries.len(), "Inserted ledger entries (batch UNNEST)");
        Ok(())
    }

    /// Query all ledger entries for a specific tick.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the query fails.
    pub async fn get_entries_by_tick(&self, tick: u64) -> Result<Vec<LedgerRow>, DbError> {
        let tick_i64 = i64::try_from(tick).unwrap_or(i64::MAX);
        let rows = sqlx::query_as::<_, LedgerRow>(
            r"SELECT id, tick, entry_type::TEXT as entry_type, from_entity, from_entity_type::TEXT as from_entity_type, to_entity, to_entity_type::TEXT as to_entity_type, resource, quantity, reason, reference_id, created_at
              FROM ledger
              WHERE tick = $1
              ORDER BY created_at",
        )
        .bind(tick_i64)
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    /// Query all ledger entries involving a specific entity (as source or destination).
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the query fails.
    pub async fn get_entries_by_entity(&self, entity_id: Uuid) -> Result<Vec<LedgerRow>, DbError> {
        let rows = sqlx::query_as::<_, LedgerRow>(
            r"SELECT id, tick, entry_type::TEXT as entry_type, from_entity, from_entity_type::TEXT as from_entity_type, to_entity, to_entity_type::TEXT as to_entity_type, resource, quantity, reason, reference_id, created_at
              FROM ledger
              WHERE from_entity = $1 OR to_entity = $1
              ORDER BY tick, created_at",
        )
        .bind(entity_id)
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }
}

/// A row from the `ledger` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LedgerRow {
    /// Ledger entry UUID.
    pub id: Uuid,
    /// Tick when the transfer occurred.
    pub tick: i64,
    /// Entry type as a string (cast from `PostgreSQL` enum).
    pub entry_type: String,
    /// Source entity UUID.
    pub from_entity: Option<Uuid>,
    /// Source entity type as a string.
    pub from_entity_type: Option<String>,
    /// Destination entity UUID.
    pub to_entity: Option<Uuid>,
    /// Destination entity type as a string.
    pub to_entity_type: Option<String>,
    /// Resource type name.
    pub resource: String,
    /// Quantity transferred.
    pub quantity: rust_decimal::Decimal,
    /// Reason for the transfer.
    pub reason: String,
    /// Related entity ID (trade, structure, etc.).
    pub reference_id: Option<Uuid>,
    /// Real-world timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Convert a [`LedgerEntryType`] to its `PostgreSQL` enum string.
const fn ledger_entry_type_to_db(entry_type: LedgerEntryType) -> &'static str {
    match entry_type {
        LedgerEntryType::Regeneration => "regeneration",
        LedgerEntryType::Gather => "gather",
        LedgerEntryType::Consume => "consume",
        LedgerEntryType::Transfer => "transfer",
        LedgerEntryType::Build => "build",
        LedgerEntryType::Salvage => "salvage",
        LedgerEntryType::Decay => "decay",
        LedgerEntryType::Drop => "drop",
        LedgerEntryType::Pickup => "pickup",
        LedgerEntryType::Theft => "theft",
        LedgerEntryType::CombatLoot => "combat_loot",
    }
}

/// Convert an [`EntityType`] to its `PostgreSQL` enum string.
const fn entity_type_to_db(entity_type: EntityType) -> &'static str {
    match entity_type {
        EntityType::Agent => "agent",
        EntityType::Location => "location",
        EntityType::Structure => "structure",
        EntityType::World => "world",
        EntityType::Void => "void",
    }
}

/// Convert a [`Resource`] to its database string representation.
const fn resource_to_db(resource: Resource) -> &'static str {
    match resource {
        Resource::Water => "water",
        Resource::FoodBerry => "food_berry",
        Resource::FoodFish => "food_fish",
        Resource::FoodRoot => "food_root",
        Resource::FoodMeat => "food_meat",
        Resource::FoodFarmed => "food_farmed",
        Resource::FoodCooked => "food_cooked",
        Resource::Wood => "wood",
        Resource::Stone => "stone",
        Resource::Fiber => "fiber",
        Resource::Clay => "clay",
        Resource::Hide => "hide",
        Resource::Ore => "ore",
        Resource::Metal => "metal",
        Resource::Medicine => "medicine",
        Resource::Tool => "tool",
        Resource::ToolAdvanced => "tool_advanced",
        Resource::CurrencyToken => "currency_token",
        Resource::WrittenRecord => "written_record",
    }
}
