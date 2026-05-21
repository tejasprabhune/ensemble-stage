use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("not found")]
    NotFound,

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("not implemented")]
    NotImplemented,

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::Database(e) => {
                tracing::error!("database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "database error".to_string(),
                )
            }
            AppError::NotFound => (StatusCode::NOT_FOUND, "not_found", "not found".to_string()),
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "authentication required".to_string(),
            ),
            AppError::Forbidden => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "insufficient permissions".to_string(),
            ),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg.clone()),
            AppError::NotImplemented => (
                StatusCode::NOT_IMPLEMENTED,
                "not_implemented",
                "not yet implemented".to_string(),
            ),
            AppError::Internal(e) => {
                tracing::error!("internal error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    e.to_string(),
                )
            }
        };

        (
            status,
            Json(json!({ "error": { "code": code, "message": message } })),
        )
            .into_response()
    }
}
