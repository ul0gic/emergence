//! Error types for the data layer.
//!
//! All errors are propagated via [`DbError`] which wraps the underlying
//! [`sqlx`] and [`fred`] errors with additional context about which
//! operation failed.

/// Errors that can occur in the data layer.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// A `PostgreSQL` operation failed.
    #[error("PostgreSQL error: {0}")]
    Postgres(#[from] sqlx::Error),

    /// A `PostgreSQL` migration failed.
    #[error("PostgreSQL migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    /// A `Dragonfly`/Redis operation failed.
    #[error("Dragonfly error: {0}")]
    Dragonfly(#[from] fred::error::Error),

    /// A serialization or deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// A key was not found in `Dragonfly`.
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    /// A configuration error.
    #[error("Configuration error: {0}")]
    Config(String),
}
