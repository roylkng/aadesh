pub mod auth;
pub mod routes;
pub mod ws;

use std::sync::Arc;

use adesh_core::AppConfig;
use adesh_storage_sqlite::SqliteStorage;
use axum::{
    Router, middleware,
    routing::{get, post},
};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub storage: Arc<SqliteStorage>,
}

pub fn app(config: AppConfig, storage: Arc<SqliteStorage>) -> Router {
    let state = AppState { config, storage };

    let protected = Router::new()
        .route("/v1/requests", post(routes::submit_request))
        .route("/v1/events", get(ws::events))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_root_owner,
        ));

    Router::new()
        .route("/v1/health", get(routes::health))
        .merge(protected)
        .with_state(state)
}
