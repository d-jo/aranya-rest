use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum RestError {
    #[error("Daemon connection error: {0}")]
    DaemonConnection(#[from] tarpc::client::RpcError),

    #[error("Daemon API error: {0}")]
    DaemonApi(#[from] aranya_daemon_api::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Internal server error: {0}")]
    Internal(#[from] anyhow::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl IntoResponse for RestError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            RestError::DaemonConnection(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, "Daemon unavailable")
            }
            RestError::DaemonApi(_) => (StatusCode::BAD_REQUEST, "Daemon API error"),
            RestError::Serialization(_) => (StatusCode::BAD_REQUEST, "Invalid JSON"),
            RestError::InvalidRequest(_) => (StatusCode::BAD_REQUEST, "Invalid request"),
            RestError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
            RestError::Io(_) => (StatusCode::INTERNAL_SERVER_ERROR, "IO error"),
        };

        let body = Json(json!({
            "error": error_message,
            "details": self.to_string()
        }));

        (status, body).into_response()
    }
}
