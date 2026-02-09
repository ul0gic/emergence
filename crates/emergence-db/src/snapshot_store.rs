//! Snapshot persistence for world and agent state.
//!
//! World snapshots are written at the end of each tick to record
//! population, economy, and environment metrics. Agent snapshots are
//! written periodically or on significant events.
//!
//! See: `data-schemas.md` sections 4.3, 9, `world-engine.md` section 10.2

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::DbError;

/// Operations on the `world_snapshots` and `agent_snapshots` tables.
pub struct SnapshotStore<'a> {
    pool: &'a PgPool,
}

impl<'a> SnapshotStore<'a> {
    /// Create a new snapshot store bound to a connection pool.
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // World Snapshots
    // =========================================================================

    /// Insert a world snapshot for the given tick.
    ///
    /// Uses `ON CONFLICT` to update if a snapshot for this tick already
    /// exists (idempotent).
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the insert fails.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_world_snapshot(
        &self,
        tick: u64,
        era: &str,
        season: &str,
        weather: &str,
        population: i32,
        births: i32,
        deaths: i32,
        total_resources: &serde_json::Value,
        wealth_distribution: &serde_json::Value,
        trades_this_tick: i32,
        discoveries_count: i32,
        summary: &serde_json::Value,
    ) -> Result<(), DbError> {
        let tick_i64 = i64::try_from(tick).unwrap_or(i64::MAX);

        sqlx::query(
            r"INSERT INTO world_snapshots
              (tick, era, season, weather, population, births, deaths, total_resources, wealth_distribution, trades_this_tick, discoveries_count, summary)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
              ON CONFLICT (tick) DO UPDATE SET
                era = EXCLUDED.era,
                season = EXCLUDED.season,
                weather = EXCLUDED.weather,
                population = EXCLUDED.population,
                births = EXCLUDED.births,
                deaths = EXCLUDED.deaths,
                total_resources = EXCLUDED.total_resources,
                wealth_distribution = EXCLUDED.wealth_distribution,
                trades_this_tick = EXCLUDED.trades_this_tick,
                discoveries_count = EXCLUDED.discoveries_count,
                summary = EXCLUDED.summary",
        )
        .bind(tick_i64)
        .bind(era)
        .bind(season)
        .bind(weather)
        .bind(population)
        .bind(births)
        .bind(deaths)
        .bind(total_resources)
        .bind(wealth_distribution)
        .bind(trades_this_tick)
        .bind(discoveries_count)
        .bind(summary)
        .execute(self.pool)
        .await?;

        tracing::debug!(tick, "Inserted world snapshot");
        Ok(())
    }

    /// Query the world snapshot for a specific tick.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the query fails.
    pub async fn get_world_snapshot(&self, tick: u64) -> Result<Option<WorldSnapshotRow>, DbError> {
        let tick_i64 = i64::try_from(tick).unwrap_or(i64::MAX);

        let row = sqlx::query_as::<_, WorldSnapshotRow>(
            r"SELECT tick, era, season, weather, population, births, deaths,
                     total_resources, wealth_distribution, trades_this_tick,
                     discoveries_count, summary, created_at
              FROM world_snapshots
              WHERE tick = $1",
        )
        .bind(tick_i64)
        .fetch_optional(self.pool)
        .await?;

        Ok(row)
    }

    /// Query the most recent world snapshots, limited to `count`.
    ///
    /// Returns snapshots in descending tick order (newest first).
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the query fails.
    pub async fn get_recent_world_snapshots(
        &self,
        count: i64,
    ) -> Result<Vec<WorldSnapshotRow>, DbError> {
        let rows = sqlx::query_as::<_, WorldSnapshotRow>(
            r"SELECT tick, era, season, weather, population, births, deaths,
                     total_resources, wealth_distribution, trades_this_tick,
                     discoveries_count, summary, created_at
              FROM world_snapshots
              ORDER BY tick DESC
              LIMIT $1",
        )
        .bind(count)
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    // =========================================================================
    // Agent Snapshots
    // =========================================================================

    /// Insert an agent state snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the insert fails.
    pub async fn insert_agent_snapshot(
        &self,
        tick: u64,
        agent_id: Uuid,
        full_state: &serde_json::Value,
    ) -> Result<(), DbError> {
        let tick_i64 = i64::try_from(tick).unwrap_or(i64::MAX);

        sqlx::query(
            r"INSERT INTO agent_snapshots (tick, agent_id, full_state)
              VALUES ($1, $2, $3)",
        )
        .bind(tick_i64)
        .bind(agent_id)
        .bind(full_state)
        .execute(self.pool)
        .await?;

        tracing::debug!(tick, %agent_id, "Inserted agent snapshot");
        Ok(())
    }

    /// Batch-insert agent state snapshots.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the insert fails.
    pub async fn batch_insert_agent_snapshots(
        &self,
        snapshots: &[(u64, Uuid, serde_json::Value)],
    ) -> Result<(), DbError> {
        if snapshots.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;

        for (tick, agent_id, full_state) in snapshots {
            let tick_i64 = i64::try_from(*tick).unwrap_or(i64::MAX);
            sqlx::query(
                r"INSERT INTO agent_snapshots (tick, agent_id, full_state)
                  VALUES ($1, $2, $3)",
            )
            .bind(tick_i64)
            .bind(agent_id)
            .bind(full_state)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        tracing::debug!(count = snapshots.len(), "Inserted agent snapshots");
        Ok(())
    }

    /// Query the latest snapshot for a specific agent.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the query fails.
    pub async fn get_latest_agent_snapshot(
        &self,
        agent_id: Uuid,
    ) -> Result<Option<AgentSnapshotRow>, DbError> {
        let row = sqlx::query_as::<_, AgentSnapshotRow>(
            r"SELECT id, tick, agent_id, full_state, created_at
              FROM agent_snapshots
              WHERE agent_id = $1
              ORDER BY tick DESC
              LIMIT 1",
        )
        .bind(agent_id)
        .fetch_optional(self.pool)
        .await?;

        Ok(row)
    }

    /// Query all snapshots for a specific agent, optionally within a tick range.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the query fails.
    pub async fn get_agent_snapshots(
        &self,
        agent_id: Uuid,
        from_tick: u64,
        to_tick: u64,
    ) -> Result<Vec<AgentSnapshotRow>, DbError> {
        let from_i64 = i64::try_from(from_tick).unwrap_or(i64::MAX);
        let to_i64 = i64::try_from(to_tick).unwrap_or(i64::MAX);

        let rows = sqlx::query_as::<_, AgentSnapshotRow>(
            r"SELECT id, tick, agent_id, full_state, created_at
              FROM agent_snapshots
              WHERE agent_id = $1 AND tick >= $2 AND tick < $3
              ORDER BY tick",
        )
        .bind(agent_id)
        .bind(from_i64)
        .bind(to_i64)
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }
}

/// A row from the `world_snapshots` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorldSnapshotRow {
    /// The tick this snapshot represents.
    pub tick: i64,
    /// Current era.
    pub era: String,
    /// Current season.
    pub season: String,
    /// Current weather.
    pub weather: String,
    /// Number of living agents.
    pub population: i32,
    /// Agents born this tick.
    pub births: i32,
    /// Agents who died this tick.
    pub deaths: i32,
    /// Total resources in the simulation as JSON.
    pub total_resources: serde_json::Value,
    /// Wealth distribution as JSON.
    pub wealth_distribution: serde_json::Value,
    /// Number of trades this tick.
    pub trades_this_tick: i32,
    /// Total discoveries to date.
    pub discoveries_count: i32,
    /// Narrative summary as JSON.
    pub summary: serde_json::Value,
    /// Real-world timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// A row from the `agent_snapshots` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AgentSnapshotRow {
    /// Auto-incremented snapshot ID.
    pub id: i64,
    /// The tick this snapshot was taken at.
    pub tick: i64,
    /// The agent this snapshot belongs to.
    pub agent_id: Uuid,
    /// Full agent state as JSON.
    pub full_state: serde_json::Value,
    /// Real-world timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}
