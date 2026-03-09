use axum::{
    Json,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use uuid::Uuid;

use adesh_contracts::{ApiErrorBody, ApiErrorResponse, Meta};

use super::AppState;

pub async fn require_root_owner(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let provided = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .map(ToOwned::to_owned)
        .or_else(|| {
            request.uri().query().and_then(|query| {
                query.split('&').find_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    match (parts.next(), parts.next()) {
                        (Some("access_token"), Some(value)) => Some(value.to_string()),
                        _ => None,
                    }
                })
            })
        });

    if provided.as_deref() == Some(state.config.root_owner_token.as_str()) {
        return next.run(request).await;
    }

    let body = ApiErrorResponse {
        ok: false,
        error: ApiErrorBody {
            code: "FORBIDDEN".to_string(),
            message: "Root Owner authentication required".to_string(),
            details: serde_json::json!({}),
        },
        meta: Meta {
            request_id: Uuid::new_v4().to_string(),
            ts: Utc::now(),
            audit_trace_id: None,
        },
    };

    (StatusCode::FORBIDDEN, Json(body)).into_response()
}
