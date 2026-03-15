use std::sync::Arc;

use adesh_core::AppConfig;
use adesh_storage_sqlite::SqliteStorage;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
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
async fn request_txn_rolls_back_when_audit_write_fails() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    storage.inject_fail_audit_trace_writes_for_tests(true);
    let app = adesh_daemon::http::app(test_config(), storage.clone()).unwrap();

    let body = r#"{
      "request_id": "req-audit-fail-1",
      "source": {"channel": "http", "transport": "rest"},
      "received_at": "2026-03-08T00:00:00Z",
      "requesting_principal": {"principal_type": "root_owner", "principal_id": "owner-1"},
      "requesting_audience_id": "root_owner",
      "input": {"kind": "text", "content": "draft email"},
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

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let operations_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM operations")
        .fetch_one(storage.pool())
        .await
        .unwrap();
    let audit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_traces")
        .fetch_one(storage.pool())
        .await
        .unwrap();

    assert_eq!(operations_count, 0);
    assert_eq!(audit_count, 0);
}
