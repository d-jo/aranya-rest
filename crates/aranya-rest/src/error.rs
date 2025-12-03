//! Error handling for the Aranya REST API v4.0.0
//!
//! This module provides error types and HTTP response mapping for
//! all REST API errors.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// REST API error types
#[derive(Debug, thiserror::Error)]
pub enum RestError {
    /// RPC connection error to the daemon
    #[error("Daemon connection error: {0}")]
    DaemonConnection(#[from] tarpc::client::RpcError),

    /// Daemon API returned an error
    #[error("Daemon API error: {0}")]
    DaemonApi(#[from] aranya_daemon_api::Error),

    /// JSON serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Invalid request parameters
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Internal server error
    #[error("Internal server error: {0}")]
    Internal(#[from] anyhow::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Resource not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Conflict (resource already exists)
    #[error("Conflict: {0}")]
    Conflict(String),
}

impl IntoResponse for RestError {
    fn into_response(self) -> Response {
        let (status, error_type, details) = match &self {
            RestError::DaemonConnection(e) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "daemon_unavailable",
                format!("Failed to connect to daemon: {}", e),
            ),
            RestError::DaemonApi(e) => (
                StatusCode::BAD_REQUEST,
                "daemon_error",
                format!("Daemon returned error: {}", e),
            ),
            RestError::Serialization(e) => (
                StatusCode::BAD_REQUEST,
                "invalid_json",
                format!("JSON error: {}", e),
            ),
            RestError::InvalidRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "invalid_request",
                msg.clone(),
            ),
            RestError::Internal(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                format!("Internal error: {}", e),
            ),
            RestError::Io(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "io_error",
                format!("IO error: {}", e),
            ),
            RestError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                "not_found",
                msg.clone(),
            ),
            RestError::PermissionDenied(msg) => (
                StatusCode::FORBIDDEN,
                "permission_denied",
                msg.clone(),
            ),
            RestError::Conflict(msg) => (
                StatusCode::CONFLICT,
                "conflict",
                msg.clone(),
            ),
        };

        let body = Json(json!({
            "error": error_type,
            "message": details,
            "status": status.as_u16()
        }));

        (status, body).into_response()
    }
}
