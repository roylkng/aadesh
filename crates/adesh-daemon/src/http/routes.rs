use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::Utc;
use uuid::Uuid;

use adesh_contracts::{
    ApiErrorBody, ApiErrorResponse, ApiSuccess, HealthResponse, Meta, RequestEnvelope,
};
use adesh_core::ports::storage::StorageProvider;

use super::AppState;

pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let storage = match state.storage.health().await {
        Ok(()) => "ok",
        Err(_) => "degraded",
    };

    let body = ApiSuccess {
        ok: true,
        data: HealthResponse {
            status: if storage == "ok" { "ok" } else { "degraded" }.to_string(),
            version: state.config.server_version.clone(),
            storage: storage.to_string(),
            model_provider: "degraded".to_string(),
            tool_provider: "degraded".to_string(),
            queue: "degraded".to_string(),
        },
        meta: Meta {
            request_id: Uuid::new_v4().to_string(),
            ts: Utc::now(),
            audit_trace_id: None,
        },
    };

    (StatusCode::OK, Json(body))
}

pub async fn submit_request(
    State(_state): State<AppState>,
    headers: HeaderMap,
    Json(_request): Json<RequestEnvelope>,
) -> impl IntoResponse {
    let request_id = headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let body = ApiErrorResponse {
        ok: false,
        error: ApiErrorBody {
            code: "PERMANENT".to_string(),
            message: "Milestone 1 scaffold: request acceptance is not implemented yet".to_string(),
            details: serde_json::json!({"endpoint": "/v1/requests"}),
        },
        meta: Meta {
            request_id,
            ts: Utc::now(),
            audit_trace_id: None,
        },
    };

    (StatusCode::NOT_IMPLEMENTED, Json(body))
}
