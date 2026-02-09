//! Observer HTTP server lifecycle management.
//!
//! Provides [`start_server`] which binds to a TCP port and runs the
//! Axum server until the provided [`CancellationToken`](tokio::sync)
//! or `Ctrl-C` is received.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::info;

use crate::router::build_router;
use crate::state::AppState;

/// Configuration for the Observer server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// The host address to bind to (e.g. `0.0.0.0`).
    pub host: String,
    /// The TCP port to listen on.
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: String::from("0.0.0.0"),
            port: 8080,
        }
    }
}

/// Start the Observer HTTP server.
///
/// Binds to the configured address, builds the router, and serves
/// requests until the process is terminated. Returns `Ok(())` on
/// clean shutdown, or an error if binding or serving fails.
///
/// # Errors
///
/// Returns an error if the TCP listener cannot bind or the server
/// encounters a fatal I/O error.
pub async fn start_server(config: &ServerConfig, state: Arc<AppState>) -> Result<(), ServerError> {
    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .map_err(|e| ServerError::Bind(format!("invalid address: {e}")))?;

    let router = build_router(state);

    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| ServerError::Bind(format!("bind failed on {addr}: {e}")))?;

    info!(%addr, "Observer server listening");

    axum::serve(listener, router)
        .await
        .map_err(|e| ServerError::Serve(format!("serve error: {e}")))?;

    Ok(())
}

/// Errors that can occur when starting or running the Observer server.
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    /// Failed to bind to the network address.
    #[error("bind error: {0}")]
    Bind(String),

    /// The server encountered a fatal error while serving.
    #[error("serve error: {0}")]
    Serve(String),
}
