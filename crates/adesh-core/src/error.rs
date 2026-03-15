use serde_json::json;
use thiserror::Error;

use adesh_contracts::{ApiErrorBody, ApiErrorResponse, Meta};
use chrono::Utc;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage unavailable: {0}")]
    Unavailable(String),
    #[error("storage unsupported: {0}")]
    Unsupported(String),
    #[error("storage invalid input: {0}")]
    InvalidInput(String),
    #[error("storage not found: {0}")]
    NotFound(String),
    #[error("storage conflict: {0}")]
    Conflict(String),
    #[error("storage corruption: {0}")]
    Corruption(String),
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("forbidden")]
    Forbidden,
    #[error("rate limited")]
    RateLimited,
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

impl AppError {
    pub fn into_response_body(self, request_id: String) -> ApiErrorResponse {
        let (code, message, details) = match self {
            Self::Forbidden => (
                "FORBIDDEN",
                "Root Owner authentication required".to_string(),
                json!({}),
            ),
            Self::RateLimited => (
                "RATE_LIMITED",
                "Too many requests in the current window".to_string(),
                json!({}),
            ),
            Self::NotImplemented(msg) => (
                "PERMANENT",
                msg.to_string(),
                json!({"stage": "milestone_1_scaffold"}),
            ),
            Self::BadRequest(msg) => ("INVALID_INPUT", msg, json!({})),
            Self::Storage(err) => (
                match &err {
                    StorageError::InvalidInput(_) => "INVALID_INPUT",
                    StorageError::NotFound(_) => "NOT_FOUND",
                    StorageError::Conflict(_) => "CONFLICT",
                    StorageError::Corruption(_) => "PERMANENT",
                    StorageError::Unavailable(_) | StorageError::Unsupported(_) => "PERMANENT",
                },
                match &err {
                    StorageError::InvalidInput(message) => message.clone(),
                    StorageError::NotFound(message) => message.clone(),
                    StorageError::Conflict(message) => message.clone(),
                    StorageError::Unavailable(_) => "Dependency is unavailable".to_string(),
                    StorageError::Unsupported(_) => {
                        "Requested operation is unsupported".to_string()
                    }
                    StorageError::Corruption(_) => "Data integrity check failed".to_string(),
                },
                match &err {
                    StorageError::InvalidInput(message) => json!({
                        "component": "storage",
                        "kind": "validation",
                        "violations": [message],
                    }),
                    StorageError::Unavailable(message)
                    | StorageError::Unsupported(message)
                    | StorageError::Corruption(message) => json!({
                        "component": "storage",
                        "backend_error": message,
                    }),
                    _ => json!({"component": "storage"}),
                },
            ),
        };

        ApiErrorResponse {
            ok: false,
            error: ApiErrorBody {
                code: code.to_string(),
                message,
                details,
            },
            meta: Meta {
                request_id,
                ts: Utc::now(),
                audit_trace_id: None,
            },
        }
    }
}
