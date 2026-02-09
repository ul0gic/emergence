//! Error types for the Observer API server.
//!
//! [`ObserverError`] unifies all failure modes into a single enum that
//! can be converted into an Axum HTTP response via its
//! [`IntoResponse`](axum::response::IntoResponse) implementation.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// Errors that can occur in the Observer API layer.
#[derive(Debug, thiserror::Error)]
pub enum ObserverError {
    /// The requested resource was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// A serialization or deserialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// An invalid query parameter was provided.
    #[error("invalid query: {0}")]
    InvalidQuery(String),

    /// An internal error occurred.
    #[error("internal error: {0}")]
    Internal(String),

    /// A UUID could not be parsed from the request path.
    #[error("invalid UUID: {0}")]
    InvalidUuid(String),
}

impl IntoResponse for ObserverError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            Self::Serialization(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("JSON error: {e}"))
            }
            Self::InvalidQuery(msg) | Self::InvalidUuid(msg) => {
                (StatusCode::BAD_REQUEST, msg.clone())
            }
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let body = serde_json::json!({
            "error": message,
            "status": status.as_u16(),
        });

        (status, axum::Json(body)).into_response()
    }
}
