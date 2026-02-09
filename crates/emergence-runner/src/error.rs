//! Error types for the agent runner.
//!
//! Uses `thiserror` for typed errors that surface through the entire runner
//! pipeline: NATS connectivity, LLM calls, prompt rendering, response parsing.

/// Errors that can occur during agent runner operation.
#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    /// Failed to connect to or communicate with the NATS server.
    #[error("NATS error: {0}")]
    Nats(String),

    /// Failed to render a prompt template.
    #[error("template render error: {0}")]
    Template(String),

    /// An LLM backend returned an error or was unreachable.
    #[error("LLM backend error: {0}")]
    LlmBackend(String),

    /// The LLM response could not be parsed into a valid action.
    #[error("response parse error: {0}")]
    Parse(String),

    /// The decision deadline was exceeded.
    ///
    /// Currently timeouts are handled inline via `tokio::time::timeout`
    /// in the runner module, but this variant exists for future use in
    /// explicit timeout error propagation.
    #[error("timeout: agent decision exceeded deadline")]
    #[allow(dead_code)]
    Timeout,

    /// Configuration is invalid or missing.
    #[error("config error: {0}")]
    Config(String),

    /// Serialization or deserialization failure.
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}
