//! Error types for the World Engine binary.
//!
//! [`EngineError`] is the top-level error type that wraps all possible
//! failure modes during engine startup and simulation execution.

/// Top-level error for the World Engine binary.
///
/// Each variant wraps a specific subsystem error, providing a single
/// error type that `main` can propagate with `?`.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// Configuration loading failed.
    #[error("config error: {source}")]
    Config {
        /// The underlying config error.
        #[from]
        source: emergence_core::config::ConfigError,
    },

    /// World clock initialization failed.
    #[error("clock error: {source}")]
    Clock {
        /// The underlying clock error.
        #[from]
        source: emergence_core::clock::ClockError,
    },

    /// World map construction failed.
    #[error("world error: {source}")]
    World {
        /// The underlying world error.
        #[from]
        source: emergence_world::WorldError,
    },

    /// Simulation runner failed.
    #[error("runner error: {source}")]
    Runner {
        /// The underlying runner error.
        #[from]
        source: emergence_core::runner::RunnerError,
    },

    /// NATS connection or messaging failed.
    #[error("NATS error: {message}")]
    Nats {
        /// Description of the NATS failure.
        message: String,
    },

    /// Agent spawning failed.
    #[error("spawner error: {message}")]
    Spawner {
        /// Description of the spawner failure.
        message: String,
    },

    /// Observer API server failed to start.
    #[error("observer error: {message}")]
    Observer {
        /// Description of the observer failure.
        message: String,
    },
}
