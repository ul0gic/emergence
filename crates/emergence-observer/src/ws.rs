//! `WebSocket` handler for real-time tick summary streaming.
//!
//! Clients connect to `GET /ws/ticks` and receive a JSON-encoded
//! [`TickBroadcast`] message each time the engine completes a tick.
//! The handler uses a [`broadcast::Receiver`] so all connected clients
//! see the same stream.
//!
//! If a client falls behind, lagged messages are silently skipped and
//! the client resumes from the most recent tick.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use tracing::{debug, warn};

use crate::state::AppState;

/// Upgrade an HTTP request to a `WebSocket` connection and begin
/// streaming tick summaries.
///
/// # Route
///
/// `GET /ws/ticks`
pub async fn ws_ticks(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

/// Handle the `WebSocket` lifecycle: subscribe to the broadcast
/// channel and forward each tick summary as a text frame.
async fn handle_ws(mut socket: WebSocket, state: Arc<AppState>) {
    debug!("WebSocket client connected");

    let mut rx = state.subscribe();

    loop {
        tokio::select! {
            // Receive a tick broadcast from the engine.
            result = rx.recv() => {
                match result {
                    Ok(tick) => {
                        let json = match serde_json::to_string(&tick) {
                            Ok(j) => j,
                            Err(e) => {
                                warn!("Failed to serialize tick broadcast: {e}");
                                continue;
                            }
                        };
                        let msg: Message = Message::Text(json.into());
                        if socket.send(msg).await.is_err() {
                            debug!("WebSocket client disconnected (send failed)");
                            return;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        debug!(skipped = n, "WebSocket client lagged, skipping ahead");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        debug!("Broadcast channel closed, shutting down WebSocket");
                        return;
                    }
                }
            }
            // Check if the client sent a close frame or disconnected.
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => {
                        debug!("WebSocket client disconnected");
                        return;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let pong = Message::Pong(data);
                        if socket.send(pong).await.is_err() {
                            debug!("WebSocket client disconnected (pong failed)");
                            return;
                        }
                    }
                    Some(Err(e)) => {
                        debug!("WebSocket error: {e}");
                        return;
                    }
                    _ => {
                        // Ignore other message types (text, binary from client).
                    }
                }
            }
        }
    }
}
