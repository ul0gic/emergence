//! Experiment snapshot persistence for save/restore of simulation state.
//!
//! The experiment store allows capturing a full simulation snapshot (all
//! agent states, world map state, clock state, alive agents) as a single
//! JSON blob in `PostgreSQL`. This enables:
//!
//! - Save/restore of experiments for reproducibility
//! - A/B testing with identical starting conditions
//! - Post-hoc analysis of experiment branches
//!
//! See: `build-plan.md` Phase 5.2

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::DbError;

/// Operations on the `experiment_snapshots` table.
pub struct ExperimentStore<'a> {
    pool: &'a PgPool,
}

impl<'a> ExperimentStore<'a> {
    /// Create a new experiment store bound to a connection pool.
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Save a full experiment snapshot.
    ///
    /// The `state_blob` should contain all simulation state serialized
    /// as JSON: agent states, world map, clock, alive agents, etc.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the insert fails.
    pub async fn save_snapshot(
        &self,
        experiment_id: Option<Uuid>,
        name: &str,
        description: &str,
        tick: u64,
        config: &serde_json::Value,
        state_blob: &serde_json::Value,
    ) -> Result<Uuid, DbError> {
        let tick_i64 = i64::try_from(tick).unwrap_or(i64::MAX);

        let row: (Uuid,) = sqlx::query_as(
            r"INSERT INTO experiment_snapshots (experiment_id, name, description, tick, config, state_blob)
              VALUES ($1, $2, $3, $4, $5, $6)
              RETURNING id",
        )
        .bind(experiment_id)
        .bind(name)
        .bind(description)
        .bind(tick_i64)
        .bind(config)
        .bind(state_blob)
        .fetch_one(self.pool)
        .await?;

        tracing::info!(
            snapshot_id = %row.0,
            tick,
            name,
            "Saved experiment snapshot"
        );

        Ok(row.0)
    }

    /// Load a specific experiment snapshot by ID.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the query fails.
    pub async fn load_snapshot(
        &self,
        snapshot_id: Uuid,
    ) -> Result<Option<ExperimentSnapshotRow>, DbError> {
        let row = sqlx::query_as::<_, ExperimentSnapshotRow>(
            r"SELECT id, experiment_id, name, description, tick, config, state_blob, created_at
              FROM experiment_snapshots
              WHERE id = $1",
        )
        .bind(snapshot_id)
        .fetch_optional(self.pool)
        .await?;

        Ok(row)
    }

    /// Load the most recent snapshot for an experiment.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the query fails.
    pub async fn load_latest_for_experiment(
        &self,
        experiment_id: Uuid,
    ) -> Result<Option<ExperimentSnapshotRow>, DbError> {
        let row = sqlx::query_as::<_, ExperimentSnapshotRow>(
            r"SELECT id, experiment_id, name, description, tick, config, state_blob, created_at
              FROM experiment_snapshots
              WHERE experiment_id = $1
              ORDER BY tick DESC
              LIMIT 1",
        )
        .bind(experiment_id)
        .fetch_optional(self.pool)
        .await?;

        Ok(row)
    }

    /// List all snapshots, ordered by creation time (newest first).
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the query fails.
    pub async fn list_snapshots(
        &self,
        limit: i64,
    ) -> Result<Vec<ExperimentSnapshotRow>, DbError> {
        let rows = sqlx::query_as::<_, ExperimentSnapshotRow>(
            r"SELECT id, experiment_id, name, description, tick, config, state_blob, created_at
              FROM experiment_snapshots
              ORDER BY created_at DESC
              LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    /// Delete a snapshot by ID.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the delete fails.
    pub async fn delete_snapshot(&self, snapshot_id: Uuid) -> Result<bool, DbError> {
        let result = sqlx::query(
            r"DELETE FROM experiment_snapshots WHERE id = $1",
        )
        .bind(snapshot_id)
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

/// A row from the `experiment_snapshots` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExperimentSnapshotRow {
    /// Snapshot UUID.
    pub id: Uuid,
    /// Experiment this snapshot belongs to (if any).
    pub experiment_id: Option<Uuid>,
    /// Human-readable snapshot name.
    pub name: String,
    /// Description of what this snapshot captures.
    pub description: String,
    /// The tick at which this snapshot was taken.
    pub tick: i64,
    /// Experiment configuration at snapshot time.
    pub config: serde_json::Value,
    /// Full simulation state as a JSON blob.
    pub state_blob: serde_json::Value,
    /// Real-world timestamp when snapshot was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
}
