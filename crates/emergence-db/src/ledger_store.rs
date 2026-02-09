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
    /// Entries are inserted in batches for efficiency. Each batch is
    /// wrapped in a transaction for atomicity.
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

            for entry in chunk {
                let entry_type_str = ledger_entry_type_to_db(entry.entry_type);
                let from_entity: Option<Uuid> = entry.from_entity;
                let from_entity_type_str =
                    entry.from_entity_type.map(entity_type_to_db);
                let to_entity: Option<Uuid> = entry.to_entity;
                let to_entity_type_str =
                    entry.to_entity_type.map(entity_type_to_db);
                let resource_str = resource_to_db(entry.resource);

                sqlx::query(
                    r"INSERT INTO ledger (id, tick, entry_type, from_entity, from_entity_type, to_entity, to_entity_type, resource, quantity, reason, reference_id, created_at)
                      VALUES ($1, $2, $3::ledger_entry_type, $4, $5::entity_type, $6, $7::entity_type, $8, $9, $10, $11, $12)",
                )
                .bind(entry.id.into_inner())
                .bind(i64::try_from(entry.tick).unwrap_or(i64::MAX))
                .bind(entry_type_str)
                .bind(from_entity)
                .bind(from_entity_type_str)
                .bind(to_entity)
                .bind(to_entity_type_str)
                .bind(resource_str)
                .bind(entry.quantity)
                .bind(&entry.reason)
                .bind(entry.reference_id)
                .bind(entry.created_at)
                .execute(&mut *tx)
                .await?;
            }

            tx.commit().await?;
        }

        tracing::debug!(count = entries.len(), "Inserted ledger entries");
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
