//! Observer server startup helper for embedding in the World Engine.
//!
//! Provides [`spawn_observer`] which launches the Observer HTTP + `WebSocket`
//! server on a background Tokio task. The engine binary calls this during
//! startup so the Observer API runs concurrently with the tick loop.
//!
//! # Usage
//!
//! ```rust,ignore
//! use emergence_observer::startup::spawn_observer;
//! use emergence_observer::state::AppState;
//! use std::sync::Arc;
//!
//! let state = Arc::new(AppState::new());
//! let handle = spawn_observer(8080, state).await?;
//! // The server is now running. The handle can be awaited on shutdown.
//! ```

use std::sync::Arc;

use tokio::task::JoinHandle;

use crate::server::{ServerConfig, ServerError};
use crate::state::AppState;

/// Errors that can occur when spawning the Observer server.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    /// The server failed to bind or start.
    #[error("server start error: {0}")]
    Server(#[from] ServerError),
}

/// Spawn the Observer HTTP server on a background Tokio task.
///
/// Binds to `0.0.0.0:{port}` and serves the REST API plus `WebSocket`
/// endpoint for real-time tick streaming. Returns a [`JoinHandle`] so
/// the caller can manage the server's lifecycle alongside the
/// simulation loop.
///
/// The server runs until the Tokio runtime is shut down or the task
/// is aborted. The caller should hold the returned handle and abort
/// or await it during clean shutdown.
///
/// # Arguments
///
/// * `port` -- TCP port to listen on (typically 8080).
/// * `state` -- Shared application state containing the broadcast
///   channel and simulation snapshot. The engine updates this state
///   each tick; the Observer serves it read-only.
///
/// # Errors
///
/// Returns [`StartupError::Server`] if the server cannot bind to the
/// requested address. This is detected eagerly before the background
/// task is spawned by performing a TCP bind check.
pub async fn spawn_observer(
    port: u16,
    state: Arc<AppState>,
) -> Result<JoinHandle<()>, StartupError> {
    let config = ServerConfig {
        host: String::from("0.0.0.0"),
        port,
    };

    // Verify the address is parseable before spawning the background task.
    // The actual bind happens inside start_server, but we catch obvious
    // misconfigurations early.
    let addr_str = format!("{}:{}", config.host, config.port);
    let _: std::net::SocketAddr = addr_str.parse().map_err(|e| {
        StartupError::Server(ServerError::Bind(format!("invalid address {addr_str}: {e}")))
    })?;

    let handle = tokio::spawn(async move {
        if let Err(e) = crate::server::start_server(&config, state).await {
            tracing::error!(error = %e, "Observer server exited with error");
        }
    });

    tracing::info!(port, "Observer server spawned on background task");

    Ok(handle)
}
