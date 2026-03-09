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
async fn operation_persists_gate_and_compiled_slice_before_completion() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon::http::app(test_config(), storage.clone()).unwrap();

    let created = create_request(
        &app,
        r#"{
          "request_id": "req-m2-1",
          "source": {"channel": "http", "transport": "rest"},
          "received_at": "2026-03-08T00:00:00Z",
          "requesting_principal": {"principal_type": "root_owner", "principal_id": "owner-1"},
          "requesting_audience_id": "root_owner",
          "input": {"kind": "text", "content": "draft a reply email"},
          "constraints": {"policy_mode": "default", "budgets": {"token_budget": 256}}
        }"#,
    )
    .await;

    let operation_id = created["data"]["primary_operation_id"].as_str().unwrap();

    let gate = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/operations/{operation_id}/gate"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(gate.status(), StatusCode::OK);

    let compiled = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/operations/{operation_id}/compiled-slice"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(compiled.status(), StatusCode::OK);

    let reasoning = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/operations/{operation_id}/reasoning-output"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reasoning.status(), StatusCode::OK);
    let reasoning_json: Value =
        serde_json::from_slice(&reasoning.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(reasoning_json["data"]["operation_id"], operation_id);
    assert_eq!(reasoning_json["data"]["model_id"], "fake-model-v1");
    assert!(reasoning_json["data"]["reasoning_output"]["drafts"].is_array());

    let operation = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/operations/{operation_id}"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let operation_json: Value =
        serde_json::from_slice(&operation.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(operation_json["data"]["state"], "completed");

    let reasoning_events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM experience_events WHERE kind = 'reasoning_output'",
    )
    .fetch_one(storage.pool())
    .await
    .unwrap();
    assert_eq!(reasoning_events, 1);
}

#[tokio::test]
async fn awaiting_approval_state_and_pending_approval_are_persisted_for_send_requests() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon::http::app(test_config(), storage.clone()).unwrap();

    let created = create_request(
        &app,
        r#"{
          "request_id": "req-m2-2",
          "source": {"channel": "http", "transport": "rest"},
          "received_at": "2026-03-08T00:00:00Z",
          "requesting_principal": {"principal_type": "root_owner", "principal_id": "owner-1"},
          "requesting_audience_id": "root_owner",
          "input": {"kind": "text", "content": "draft and send this email"},
          "constraints": {"policy_mode": "default", "budgets": {"token_budget": 256}}
        }"#,
    )
    .await;

    let operation_id = created["data"]["primary_operation_id"].as_str().unwrap();
    let operation = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/operations/{operation_id}"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let operation_json: Value =
        serde_json::from_slice(&operation.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(operation_json["data"]["state"], "awaiting_approval");

    let approvals = app
        .oneshot(
            Request::builder()
                .uri("/v1/approvals/pending")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(approvals.status(), StatusCode::OK);
    let approvals_json: Value =
        serde_json::from_slice(&approvals.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(approvals_json["data"][0]["operation_id"], operation_id);
    assert_eq!(approvals_json["data"][0]["approval_mode"], "diff");

    let reasoning_events: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM experience_events WHERE kind = 'reasoning_output'",
    )
    .fetch_one(storage.pool())
    .await
    .unwrap();
    assert_eq!(reasoning_events, 1);
}

#[tokio::test]
async fn verification_unknown_audience_default_deny() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon::http::app(test_config(), storage).unwrap();

    let created = create_request(
        &app,
        r#"{
          "request_id": "req-m2-3",
          "source": {"channel": "http", "transport": "rest"},
          "received_at": "2026-03-08T00:00:00Z",
          "requesting_principal": {"principal_type": "root_owner", "principal_id": "owner-1"},
          "requesting_audience_id": "root_owner",
          "input": {"kind": "text", "content": "send this email [[unknown_audience]]"},
          "constraints": {"policy_mode": "default", "budgets": {"token_budget": 256}}
        }"#,
    )
    .await;

    let operation_id = created["data"]["primary_operation_id"].as_str().unwrap();
    let operation = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/operations/{operation_id}"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let operation_json: Value =
        serde_json::from_slice(&operation.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(operation_json["data"]["state"], "blocked");
    assert_eq!(
        operation_json["data"]["state_reason"],
        "audience_scope_denied"
    );
}

#[tokio::test]
async fn verification_taint_laundering_deny_requires_sanitize() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon::http::app(test_config(), storage).unwrap();

    let created = create_request(
        &app,
        r#"{
          "request_id": "req-m2-4",
          "source": {"channel": "http", "transport": "rest"},
          "received_at": "2026-03-08T00:00:00Z",
          "requesting_principal": {"principal_type": "root_owner", "principal_id": "owner-1"},
          "requesting_audience_id": "root_owner",
          "input": {
            "kind": "text",
            "content": "draft a public summary [[taint_launder]]",
            "attachments": [{"ref_id": "artifact-1", "ref_type": "artifact", "sensitivity_hint": 4}]
          },
          "constraints": {"policy_mode": "default", "budgets": {"token_budget": 256}}
        }"#,
    )
    .await;

    let operation_id = created["data"]["primary_operation_id"].as_str().unwrap();
    let operation = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/operations/{operation_id}"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let operation_json: Value =
        serde_json::from_slice(&operation.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(operation_json["data"]["state"], "blocked");
    assert_eq!(
        operation_json["data"]["state_reason"],
        "taint_laundering_denied"
    );
}
