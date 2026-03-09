use std::sync::Arc;

use adesh_core::AppConfig;
use adesh_storage_sqlite::SqliteStorage;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

fn test_config() -> AppConfig {
    AppConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        root_owner_token: "test-token".to_string(),
        database_url: "sqlite::memory:".to_string(),
        server_version: "test".to_string(),
        capability_snapshot_version: "cap:bootstrap".to_string(),
        model_provider_backend: "fake".to_string(),
        model_provider_base_url: "http://127.0.0.1:1234".to_string(),
        model_provider_model: "qwen/qwen3.5-35b-a3b".to_string(),
        email_provider_backend: "fake".to_string(),
        email_from_address: "adesh@example.invalid".to_string(),
        email_smtp_host: "127.0.0.1".to_string(),
        email_smtp_port: 1025,
        email_smtp_username: None,
        email_smtp_password: None,
        webhook_provider_backend: "fake".to_string(),
    }
}

#[tokio::test]
async fn health_endpoint_is_public() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon_app(test_config(), storage);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["data"]["status"], "ok");
    assert_eq!(body["data"]["storage"], "ok");
    assert_eq!(body["data"]["model_provider"], "ok");
    assert_eq!(body["data"]["tool_provider"], "ok");
    assert_eq!(body["data"]["queue"], "degraded");
}

#[tokio::test]
async fn root_ui_shell_is_public() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon_app(test_config(), storage);

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("Adesh OS Email Wedge"));
}

#[tokio::test]
async fn root_ui_shell_exposes_adaptive_controls() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon_app(test_config(), storage);

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("id=\"density\""));
    assert!(body.contains("id=\"motion\""));
    assert!(body.contains("id=\"accent\""));
    assert!(body.contains("localStorage.setItem('adesh.ui.density'"));
    assert!(body.contains("localStorage.setItem('adesh.ui.motion'"));
    assert!(body.contains("localStorage.setItem('adesh.ui.accent'"));
}

#[tokio::test]
async fn request_endpoint_requires_root_owner_auth() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon_app(test_config(), storage);

    let body = r#"{
      "request_id": "req-1",
      "source": {"channel": "http", "transport": "rest"},
      "received_at": "2026-03-08T00:00:00Z",
      "requesting_principal": {"principal_type": "root_owner", "principal_id": "owner-1"},
      "requesting_audience_id": "root_owner",
      "input": {"kind": "text", "content": "hello"},
      "constraints": {"policy_mode": "default", "budgets": {"token_budget": 256}}
    }"#;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn request_endpoint_creates_operation_with_auth() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon_app(test_config(), storage);

    let body = r#"{
      "request_id": "req-2",
      "source": {"channel": "http", "transport": "rest"},
      "received_at": "2026-03-08T00:00:00Z",
      "requesting_principal": {"principal_type": "root_owner", "principal_id": "owner-1"},
      "requesting_audience_id": "root_owner",
      "input": {"kind": "text", "content": "hello"},
      "constraints": {"policy_mode": "default", "budgets": {"token_budget": 256}}
    }"#;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body.contains("\"operation_ids\""));
}

fn adesh_daemon_app(config: AppConfig, storage: Arc<SqliteStorage>) -> axum::Router {
    adesh_daemon::http::app(config, storage).unwrap()
}
