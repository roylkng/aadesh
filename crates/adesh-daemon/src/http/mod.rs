pub mod auth;
pub mod routes;
pub mod ui;
pub mod ws;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::Utc;

use adesh_core::{
    AppConfig,
    ports::{job_queue::JobQueueProvider, model::ModelProvider, tool::ToolProvider},
};
use adesh_storage_sqlite::{SqliteJobQueue, SqliteStorage};
use axum::{
    Router, middleware,
    routing::{get, post},
};
use tokio::sync::broadcast;
use tracing::warn;

use crate::{modeling::build_model_provider, tooling::build_tool_provider};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub storage: Arc<SqliteStorage>,
    pub queue: Arc<dyn JobQueueProvider>,
    pub models: Arc<dyn ModelProvider>,
    pub tools: Arc<dyn ToolProvider>,
    pub events: broadcast::Sender<String>,
    pub rate_limiter: Arc<RateLimiter>,
}

#[derive(Default)]
pub struct RateLimiter {
    buckets: Mutex<HashMap<String, Vec<i64>>>,
}

impl RateLimiter {
    pub fn allow(&self, key: &str, max_requests: u32, window_seconds: u64) -> bool {
        let now = Utc::now().timestamp();
        let cutoff = now - i64::try_from(window_seconds).unwrap_or(0);
        let mut guard = self.buckets.lock().expect("rate limiter mutex poisoned");
        let entries = guard.entry(key.to_string()).or_default();
        entries.retain(|ts| *ts >= cutoff);
        if entries.len() >= usize::try_from(max_requests).unwrap_or(usize::MAX) {
            return false;
        }
        entries.push(now);
        true
    }
}

pub fn app(
    config: AppConfig,
    storage: Arc<SqliteStorage>,
) -> Result<Router, adesh_core::StorageError> {
    let (events, _) = broadcast::channel(128);
    let state = AppState {
        queue: Arc::new(SqliteJobQueue::new(storage.pool().clone())),
        models: build_model_provider(&config)?,
        tools: build_tool_provider(&config)?,
        config,
        storage,
        events,
        rate_limiter: Arc::new(RateLimiter::default()),
    };

    let recovery_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(error) = routes::recover_pending_operation_executions(&recovery_state).await
            {
                warn!(error = %error, "operation execution recovery tick failed");
            }
        }
    });

    let protected = Router::new()
        .route("/v1/artifacts/manual", post(routes::create_manual_artifact))
        .route("/v1/ingest/jobs", post(routes::create_ingest_job))
        .route("/v1/ingest/jobs/{job_id}", get(routes::get_ingest_job))
        .route(
            "/v1/ingest/jobs/{job_id}/cancel",
            post(routes::cancel_ingest_job),
        )
        .route("/v1/requests", post(routes::submit_request))
        .route("/v1/requests/{request_id}", get(routes::get_request_status))
        .route("/v1/operations/{operation_id}", get(routes::get_operation))
        .route(
            "/v1/operations/{operation_id}/cancel",
            post(routes::cancel_operation),
        )
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
        .route("/v1/workflow-specs", post(routes::register_workflow_spec))
        .route("/v1/workflow-specs", get(routes::list_workflow_specs))
        .route(
            "/v1/workflow-specs/{workflow_ref}",
            get(routes::get_workflow_spec),
        )
        .route(
            "/v1/workflow-instances",
            post(routes::create_workflow_instance),
        )
        .route(
            "/v1/workflow-instances/{workflow_instance_id}",
            get(routes::get_workflow_instance),
        )
        .route(
            "/v1/workflow-instances/{workflow_instance_id}/cancel",
            post(routes::cancel_workflow_instance),
        )
        .route("/v1/interface-specs", post(routes::register_interface_spec))
        .route("/v1/interface-specs", get(routes::list_interface_specs))
        .route(
            "/v1/interface-specs/{interface_ref}",
            get(routes::get_interface_spec),
        )
        .route(
            "/v1/interface-instances",
            post(routes::create_interface_instance),
        )
        .route(
            "/v1/interface-instances/{interface_instance_id}",
            get(routes::get_interface_instance),
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
            "/v1/approvals/{approval_id}/oob/start",
            post(routes::start_approval_oob),
        )
        .route(
            "/v1/approvals/{approval_id}/oob/verify",
            post(routes::verify_approval_oob),
        )
        .route("/v1/audit/{audit_trace_id}", get(routes::get_audit_trace))
        .route(
            "/v1/audit/{audit_trace_id}/replay",
            post(routes::replay_audit_trace),
        )
        .route("/v1/metrics/wedge", get(routes::get_wedge_metrics))
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
