use std::sync::Arc;

use adesh_core::{
    AppConfig,
    action_schemas::{EMAIL_SEND_ARGS_SCHEMA_REF, EMAIL_SEND_RESULT_SCHEMA_REF},
};
use adesh_storage_sqlite::SqliteStorage;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tokio::time::{Duration, sleep};
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

async fn create_send_request(app: &axum::Router) -> (String, String) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(
                    r#"{
                      "request_id": "req-m3-1",
                      "source": {"channel": "http", "transport": "rest"},
                      "received_at": "2026-03-08T00:00:00Z",
                      "requesting_principal": {"principal_type": "root_owner", "principal_id": "owner-1"},
                      "requesting_audience_id": "root_owner",
                      "input": {"kind": "text", "content": "draft and send this email"},
                      "constraints": {"policy_mode": "default", "budgets": {"token_budget": 256}}
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let created: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();

    let operation_id = created["data"]["primary_operation_id"]
        .as_str()
        .unwrap()
        .to_string();

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
    assert_eq!(approvals.status(), StatusCode::OK);
    let pending: Value =
        serde_json::from_slice(&approvals.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let approval_id = pending["data"][0]["approval_id"]
        .as_str()
        .unwrap()
        .to_string();

    (operation_id, approval_id)
}

async fn fetch_operation_state(app: &axum::Router, operation_id: &str) -> Value {
    let response = app
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
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

async fn wait_for_operation_state(
    app: &axum::Router,
    operation_id: &str,
    expected_state: &str,
) -> Value {
    for _ in 0..20 {
        let body = fetch_operation_state(app, operation_id).await;
        if body["data"]["state"] == expected_state {
            return body;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("operation {operation_id} did not reach state {expected_state}");
}

#[tokio::test]
async fn approval_consumption_persists_preimage_then_execution_result() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon::http::app(test_config(), storage.clone()).unwrap();

    let (operation_id, approval_id) = create_send_request(&app).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}"))
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .header("idempotency-key", "approve-1")
                .body(Body::from(
                    r#"{"decision":"approve","modified_payload":null,"oob":null}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let approved: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(approved["data"]["status"], "consumed");
    assert_eq!(approved["data"]["operation_state"], "running");
    assert_eq!(approved["data"]["syscall_ids"].as_array().unwrap().len(), 1);

    let operation = wait_for_operation_state(&app, &operation_id, "completed").await;
    assert_eq!(operation["data"]["state_reason"], "syscalls_executed");

    let syscalls = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/operations/{operation_id}/syscalls"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(syscalls.status(), StatusCode::OK);
    let syscalls_json: Value =
        serde_json::from_slice(&syscalls.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(syscalls_json["data"][0]["status"], "executed");
    assert_eq!(syscalls_json["data"][0]["tool_name"], "email");
    assert_eq!(syscalls_json["data"][0]["action_name"], "send");
    assert_eq!(
        syscalls_json["data"][0]["args_schema_ref"],
        EMAIL_SEND_ARGS_SCHEMA_REF
    );
    assert_eq!(
        syscalls_json["data"][0]["result_schema_ref"],
        EMAIL_SEND_RESULT_SCHEMA_REF
    );
    assert!(syscalls_json["data"][0]["result_ref"].is_string());

    let db_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM syscalls WHERE operation_id = ?1")
        .bind(&operation_id)
        .fetch_one(storage.pool())
        .await
        .unwrap();
    assert_eq!(db_count, 1);

    let timestamps = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT created_at, updated_at, result_ref FROM syscalls WHERE operation_id = ?1",
    )
    .bind(&operation_id)
    .fetch_one(storage.pool())
    .await
    .unwrap();
    assert!(timestamps.2.is_some());
    assert!(timestamps.1 >= timestamps.0);
}

#[tokio::test]
async fn approval_post_is_idempotent_and_does_not_duplicate_syscalls() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon::http::app(test_config(), storage.clone()).unwrap();

    let (operation_id, approval_id) = create_send_request(&app).await;

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}"))
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .header("idempotency-key", "approve-same")
                .body(Body::from(
                    r#"{"decision":"approve","modified_payload":null,"oob":null}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first_json: Value =
        serde_json::from_slice(&first.into_body().collect().await.unwrap().to_bytes()).unwrap();

    let second = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}"))
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .header("idempotency-key", "approve-same")
                .body(Body::from(
                    r#"{"decision":"approve","modified_payload":null,"oob":null}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    let second_json: Value =
        serde_json::from_slice(&second.into_body().collect().await.unwrap().to_bytes()).unwrap();

    assert_eq!(first_json["data"], second_json["data"]);

    let db_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM syscalls WHERE operation_id = ?1")
        .bind(&operation_id)
        .fetch_one(storage.pool())
        .await
        .unwrap();
    assert_eq!(db_count, 1);

    let operation = wait_for_operation_state(&app, &operation_id, "completed").await;
    assert_eq!(operation["data"]["state_reason"], "syscalls_executed");
}

#[tokio::test]
async fn stale_or_conflicting_approval_returns_conflict() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon::http::app(test_config(), storage).unwrap();

    let (_operation_id, approval_id) = create_send_request(&app).await;

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}"))
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .header("idempotency-key", "approve-initial")
                .body(Body::from(
                    r#"{"decision":"approve","modified_payload":null,"oob":null}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let conflict = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}"))
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(
                    r#"{"decision":"deny","modified_payload":null,"oob":null}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn modified_payload_is_normalized_before_execution() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon::http::app(test_config(), storage).unwrap();

    let (operation_id, approval_id) = create_send_request(&app).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}"))
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(
                    r#"{
                      "decision":"approve",
                      "modified_payload":{
                        "to":[" User@Example.com ","user@example.com"],
                        "cc":[" Team@Example.com "],
                        "bcc":[],
                        "subject":"  Hello  ",
                        "body":"  Body text  "
                      },
                      "oob":null
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let syscalls = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/operations/{operation_id}/syscalls"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let syscalls_json: Value =
        serde_json::from_slice(&syscalls.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        syscalls_json["data"][0]["args"]["to"][0],
        "user@example.com"
    );
    assert_eq!(
        syscalls_json["data"][0]["args"]["to"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        syscalls_json["data"][0]["args"]["cc"][0],
        "team@example.com"
    );
    assert_eq!(syscalls_json["data"][0]["args"]["subject"], "Hello");
    assert_eq!(syscalls_json["data"][0]["args"]["body"], "Body text");
}

#[tokio::test]
async fn invalid_modified_payload_returns_invalid_input_without_syscall() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon::http::app(test_config(), storage.clone()).unwrap();

    let (operation_id, approval_id) = create_send_request(&app).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}"))
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(
                    r#"{
                      "decision":"approve",
                      "modified_payload":{
                        "to":[],
                        "cc":[],
                        "bcc":[],
                        "subject":"",
                        "body":""
                      },
                      "oob":null
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error_json: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(error_json["error"]["code"], "INVALID_INPUT");
    assert!(error_json["error"]["details"]["violations"].is_array());

    let db_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM syscalls WHERE operation_id = ?1")
        .bind(&operation_id)
        .fetch_one(storage.pool())
        .await
        .unwrap();
    assert_eq!(db_count, 0);
}

#[tokio::test]
async fn recovery_worker_executes_persisted_permitted_syscall_after_restart_gap() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = adesh_daemon::http::app(test_config(), storage.clone()).unwrap();

    let (operation_id, approval_id) = create_send_request(&app).await;

    let decision = adesh_core::ports::storage::StorageProvider::consume_approval_atomic(
        storage.as_ref(),
        adesh_core::ports::storage::ApprovalConsumeInput {
            approval_id,
            decision: "approve".to_string(),
            modified_payload: None,
            oob_challenge_id: None,
            idempotency_key: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(decision.operation_state, "running");

    let initial_syscalls = adesh_core::ports::storage::StorageProvider::list_syscalls_by_operation(
        storage.as_ref(),
        &operation_id,
    )
    .await
    .unwrap();
    assert_eq!(initial_syscalls.len(), 1);
    assert_eq!(initial_syscalls[0].status, "permitted");

    let operation = wait_for_operation_state(&app, &operation_id, "completed").await;
    assert_eq!(operation["data"]["state_reason"], "syscalls_executed");

    let final_syscalls = adesh_core::ports::storage::StorageProvider::list_syscalls_by_operation(
        storage.as_ref(),
        &operation_id,
    )
    .await
    .unwrap();
    assert_eq!(final_syscalls[0].status, "executed");
    assert!(final_syscalls[0].result_ref.is_some());
}
