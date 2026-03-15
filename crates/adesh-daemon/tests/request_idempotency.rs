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
        model_provider_model: "qwen3.5-27b".to_string(),
        model_provider_timeout_seconds: 45,
        email_provider_backend: "fake".to_string(),
        email_from_address: "adesh@example.invalid".to_string(),
        email_smtp_host: "127.0.0.1".to_string(),
        email_smtp_port: 1025,
        email_smtp_username: None,
        email_smtp_password: None,
        webhook_provider_backend: "fake".to_string(),
        rate_limit_window_seconds: 30,
        rate_limit_max_requests: 120,
        syscall_retry_attempts: 2,
    }
}

#[tokio::test]
async fn post_requests_idempotent_no_duplicate_operation() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon::http::app(test_config(), storage.clone()).unwrap();

    let body = r#"{
      "request_id": "req-idem-1",
      "source": {"channel": "http", "transport": "rest"},
      "received_at": "2026-03-08T00:00:00Z",
      "requesting_principal": {"principal_type": "root_owner", "principal_id": "owner-1"},
      "requesting_audience_id": "root_owner",
      "input": {"kind": "text", "content": "draft a reply"},
      "constraints": {"policy_mode": "default", "budgets": {"token_budget": 256}}
    }"#;

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .header("idempotency-key", "idem-req-1")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);
    let first_json: Value =
        serde_json::from_slice(&first.into_body().collect().await.unwrap().to_bytes()).unwrap();

    let second = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .header("idempotency-key", "idem-req-1")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CREATED);
    let second_json: Value =
        serde_json::from_slice(&second.into_body().collect().await.unwrap().to_bytes()).unwrap();

    assert_eq!(first_json, second_json);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM operations")
        .fetch_one(storage.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
}
