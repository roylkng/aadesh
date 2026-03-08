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
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("forbidden")]
    Forbidden,
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
            Self::NotImplemented(msg) => (
                "PERMANENT",
                msg.to_string(),
                json!({"stage": "milestone_1_scaffold"}),
            ),
            Self::BadRequest(msg) => ("INVALID_INPUT", msg, json!({})),
            Self::Storage(err) => (
                "PERMANENT",
                err.to_string(),
                json!({"component": "storage"}),
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
