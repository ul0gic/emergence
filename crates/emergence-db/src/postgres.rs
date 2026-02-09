//! `PostgreSQL` persistent state operations.
//!
//! `PostgreSQL` is the cold state store for the Emergence simulation. It holds
//! the full event history, ledger, agent records, and world snapshots.
//!
//! Uses [`sqlx`] with runtime query construction (not compile-time checked)
//! to avoid requiring a live database at build time. All queries are
//! parameterized to prevent SQL injection.

use std::time::Duration;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;

use crate::error::DbError;

/// Default maximum number of connections in the pool.
const DEFAULT_MAX_CONNECTIONS: u32 = 10;

/// Default connection timeout in seconds.
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 5;

/// Default idle timeout in seconds.
const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 300;

/// Configuration for the `PostgreSQL` connection pool.
#[derive(Debug, Clone)]
pub struct PostgresConfig {
    /// `PostgreSQL` connection URL.
    ///
    /// Format: `postgresql://user:password@host:port/database`
    pub url: String,
    /// Maximum number of connections in the pool.
    pub max_connections: u32,
    /// Connection timeout.
    pub connect_timeout: Duration,
    /// Idle connection timeout.
    pub idle_timeout: Duration,
}

impl PostgresConfig {
    /// Create a new configuration from a database URL.
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_owned(),
            max_connections: DEFAULT_MAX_CONNECTIONS,
            connect_timeout: Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS),
            idle_timeout: Duration::from_secs(DEFAULT_IDLE_TIMEOUT_SECS),
        }
    }

    /// Set the maximum number of connections.
    #[must_use]
    pub const fn with_max_connections(mut self, max: u32) -> Self {
        self.max_connections = max;
        self
    }

    /// Set the connection timeout.
    #[must_use]
    pub const fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Set the idle connection timeout.
    #[must_use]
    pub const fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }
}

/// Connection pool handle to `PostgreSQL`.
///
/// Wraps a [`sqlx::PgPool`] and provides access to the event store,
/// ledger, and snapshot persistence operations.
#[derive(Clone)]
pub struct PostgresPool {
    pool: PgPool,
}

impl PostgresPool {
    /// Connect to `PostgreSQL` using the provided configuration.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Postgres`] if the connection fails.
    /// Returns [`DbError::Config`] if the URL cannot be parsed.
    pub async fn connect(config: &PostgresConfig) -> Result<Self, DbError> {
        let connect_options: PgConnectOptions = config
            .url
            .parse()
            .map_err(|e: sqlx::Error| DbError::Config(format!("Invalid database URL: {e}")))?;

        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(config.connect_timeout)
            .idle_timeout(config.idle_timeout)
            .connect_with(connect_options)
            .await?;

        tracing::info!(
            max_connections = config.max_connections,
            "Connected to PostgreSQL"
        );

        Ok(Self { pool })
    }

    /// Connect using a database URL string with default pool settings.
    ///
    /// Convenience wrapper around [`PostgresPool::connect`] with
    /// [`PostgresConfig::new`].
    ///
    /// # Errors
    ///
    /// Returns [`DbError`] if the connection fails.
    pub async fn connect_url(url: &str) -> Result<Self, DbError> {
        let config = PostgresConfig::new(url);
        Self::connect(&config).await
    }

    /// Run all pending migrations from the `migrations/` directory.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Migration`] if any migration fails.
    pub async fn run_migrations(&self) -> Result<(), DbError> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        tracing::info!("Database migrations completed");
        Ok(())
    }

    /// Return a reference to the underlying [`PgPool`].
    pub const fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Close all connections in the pool gracefully.
    pub async fn close(&self) {
        self.pool.close().await;
        tracing::info!("PostgreSQL pool closed");
    }
}
