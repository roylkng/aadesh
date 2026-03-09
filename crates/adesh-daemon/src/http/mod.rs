pub mod auth;
pub mod routes;
pub mod ui;
pub mod ws;

use std::sync::Arc;

use adesh_core::{
    AppConfig,
    ports::{model::ModelProvider, tool::ToolProvider},
};
use adesh_storage_sqlite::SqliteStorage;
use axum::{
    Router, middleware,
    routing::{get, post},
};
use tokio::sync::broadcast;

use crate::{modeling::build_model_provider, tooling::build_tool_provider};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub storage: Arc<SqliteStorage>,
    pub models: Arc<dyn ModelProvider>,
    pub tools: Arc<dyn ToolProvider>,
    pub events: broadcast::Sender<String>,
}

pub fn app(
    config: AppConfig,
    storage: Arc<SqliteStorage>,
) -> Result<Router, adesh_core::StorageError> {
    let (events, _) = broadcast::channel(128);
    let state = AppState {
        models: build_model_provider(&config)?,
        tools: build_tool_provider(&config)?,
        config,
        storage,
        events,
    };

    let protected = Router::new()
        .route("/v1/requests", post(routes::submit_request))
        .route("/v1/operations/{operation_id}", get(routes::get_operation))
        .route(
            "/v1/operations/{operation_id}/gate",
            get(routes::get_gate_decision),
        )
        .route(
            "/v1/operations/{operation_id}/compiled-slice",
            get(routes::get_compiled_slice),
        )
        .route(
            "/v1/operations/{operation_id}/reasoning-output",
            get(routes::get_reasoning_output),
        )
        .route("/v1/capabilities", get(routes::get_current_capabilities))
        .route(
            "/v1/capabilities/snapshots/{capability_snapshot_version}",
            get(routes::get_capability_snapshot),
        )
        .route(
            "/v1/capabilities/snapshots",
            post(routes::mint_capability_snapshot),
        )
        .route(
            "/v1/capabilities/current/activate",
            post(routes::activate_current_capability_snapshot),
        )
        .route(
            "/v1/schema-registry/register",
            post(routes::register_schema_entry),
        )
        .route(
            "/v1/schema-registry/{schema_ref}",
            get(routes::get_schema_entry),
        )
        .route("/v1/review-queue", get(routes::list_review_items))
        .route("/v1/review-queue/{item_id}", get(routes::get_review_item))
        .route(
            "/v1/review-queue/{item_id}/decide",
            post(routes::decide_review_item),
        )
        .route(
            "/v1/operations/{operation_id}/syscalls",
            get(routes::list_operation_syscalls),
        )
        .route("/v1/approvals/pending", get(routes::list_pending_approvals))
        .route(
            "/v1/approvals/{approval_id}",
            get(routes::get_approval_item),
        )
        .route("/v1/approvals/{approval_id}", post(routes::decide_approval))
        .route(
            "/v1/audit/{audit_trace_id}/replay",
            post(routes::replay_audit_trace),
        )
        .route("/v1/events", get(ws::events))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_root_owner,
        ));

    Ok(Router::new()
        .route("/", get(ui::index))
        .route("/v1/health", get(routes::health))
        .merge(protected)
        .with_state(state))
}
