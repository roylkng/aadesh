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

async fn create_request(app: &axum::Router, body: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

#[tokio::test]
async fn replay_dry_run_never_executes_actuator() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon::http::app(test_config(), storage.clone()).unwrap();

    let created = create_request(
        &app,
        r#"{
          "request_id": "req-replay-1",
          "source": {"channel": "http", "transport": "rest"},
          "received_at": "2026-03-08T00:00:00Z",
          "requesting_principal": {"principal_type": "root_owner", "principal_id": "owner-1"},
          "requesting_audience_id": "root_owner",
          "input": {"kind": "text", "content": "draft and send this email"},
          "constraints": {"policy_mode": "default", "budgets": {"token_budget": 256}}
        }"#,
    )
    .await;

    let audit_trace_id = created["meta"]["audit_trace_id"]
        .as_str()
        .unwrap()
        .to_string();
    let approval_id = {
        let approvals = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/approvals/pending")
                    .header("authorization", "Bearer test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let pending: Value =
            serde_json::from_slice(&approvals.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        pending["data"][0]["approval_id"]
            .as_str()
            .unwrap()
            .to_string()
    };

    let approved = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}"))
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(
                    r#"{"decision":"approve","modified_payload":null,"oob":null}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(approved.status(), StatusCode::OK);

    let replay = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/audit/{audit_trace_id}/replay"))
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .header("idempotency-key", "replay-1")
                .body(Body::from(
                    r#"{"mode":"dry_run","strategy":"stored_output"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::OK);
    let replay_json: Value =
        serde_json::from_slice(&replay.into_body().collect().await.unwrap().to_bytes()).unwrap();

    let replay_operation_id = replay_json["data"]["operation_id"].as_str().unwrap();
    let replay_syscall_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM syscalls WHERE operation_id = ?1")
            .bind(replay_operation_id)
            .fetch_one(storage.pool())
            .await
            .unwrap();
    assert_eq!(replay_syscall_count, 0);
}

#[tokio::test]
async fn replay_missing_anchor_fail_closed() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon::http::app(test_config(), storage.clone()).unwrap();

    let created = create_request(
        &app,
        r#"{
          "request_id": "req-replay-2",
          "source": {"channel": "http", "transport": "rest"},
          "received_at": "2026-03-08T00:00:00Z",
          "requesting_principal": {"principal_type": "root_owner", "principal_id": "owner-1"},
          "requesting_audience_id": "root_owner",
          "input": {"kind": "text", "content": "draft a reply email"},
          "constraints": {"policy_mode": "default", "budgets": {"token_budget": 256}}
        }"#,
    )
    .await;

    let audit_trace_id = created["meta"]["audit_trace_id"]
        .as_str()
        .unwrap()
        .to_string();
    let operation_id = created["data"]["primary_operation_id"]
        .as_str()
        .unwrap()
        .to_string();

    sqlx::query("DELETE FROM gate_decisions WHERE operation_id = ?1")
        .bind(&operation_id)
        .execute(storage.pool())
        .await
        .unwrap();

    let before_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM operations")
        .fetch_one(storage.pool())
        .await
        .unwrap();

    let replay = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/audit/{audit_trace_id}/replay"))
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(
                    r#"{"mode":"dry_run","strategy":"stored_output"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let after_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM operations")
        .fetch_one(storage.pool())
        .await
        .unwrap();
    assert_eq!(before_count, after_count);
}
