use std::sync::Arc;

use adesh_core::AppConfig;
use adesh_storage_sqlite::SqliteStorage;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
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

fn app(storage: Arc<SqliteStorage>) -> axum::Router {
    adesh_daemon::http::app(test_config(), storage).unwrap()
}

fn workflow_spec_payload() -> Value {
    json!({
        "inputs": [{"name": "topic", "type": "string"}],
        "outputs": [{"name": "summary", "type": "string"}],
        "steps": [
            {
                "step_id": "step-transform",
                "step_type": "transform",
                "title": "Prepare prompt",
                "inputs": ["topic"],
                "outputs": ["summary"]
            }
        ],
        "edges": [],
        "entry_steps": ["step-transform"],
        "exit_steps": ["step-transform"]
    })
}

fn interface_spec_payload() -> Value {
    json!({
        "blocks": [
            {
                "block_id": "draft",
                "type": "draft_view",
                "title": "Latest draft"
            }
        ],
        "bindings": [
            {
                "block_id": "draft",
                "source": {
                    "kind": "reasoning_output"
                }
            }
        ]
    })
}

async fn create_operation(app: &axum::Router) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "request_id": "req-interface-op",
                        "source": {"channel": "http", "transport": "rest"},
                        "received_at": "2026-03-08T00:00:00Z",
                        "requesting_principal": {"principal_type": "root_owner", "principal_id": "owner-1"},
                        "requesting_audience_id": "root_owner",
                        "input": {"kind": "text", "content": "draft an email update"},
                        "constraints": {"policy_mode": "default", "budgets": {"token_budget": 256}}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    body["data"]["primary_operation_id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn workflow_spec_register_get_and_list_are_available() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = app(storage);

    let payload = json!({
        "name": "Daily Summary",
        "description": "Simple deterministic workflow",
        "tags": ["personal", "drafting"],
        "spec": workflow_spec_payload()
    });

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflow-specs")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "workflow-spec-1")
                .body(Body::from(payload.to_string()))
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
                .uri("/v1/workflow-specs")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "workflow-spec-1")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    let second_json: Value =
        serde_json::from_slice(&second.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(first_json["data"], second_json["data"]);

    let workflow_ref = first_json["data"]["workflow_ref"].as_str().unwrap();
    let fetched = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/workflow-specs/{workflow_ref}"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
    let fetched_json: Value =
        serde_json::from_slice(&fetched.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        fetched_json["data"]["payload"]["steps"][0]["step_id"],
        "step-transform"
    );

    let listed = app
        .oneshot(
            Request::builder()
                .uri("/v1/workflow-specs?name=Daily%20Summary")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_json: Value =
        serde_json::from_slice(&listed.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(listed_json["data"][0]["workflow_ref"], workflow_ref);
}

#[tokio::test]
async fn workflow_instance_create_get_and_cancel_are_available() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = app(storage);

    let registered = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflow-specs")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Weekly Review",
                        "description": "Simple review workflow",
                        "tags": ["weekly"],
                        "spec": workflow_spec_payload()
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let registered_json: Value =
        serde_json::from_slice(&registered.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    let workflow_ref = registered_json["data"]["workflow_ref"].as_str().unwrap();

    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workflow-instances")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "workflow-instance-1")
                .body(Body::from(
                    json!({
                        "workflow_ref": workflow_ref,
                        "inputs": {"topic": "Inbox review"},
                        "request_context": {
                            "parent_request_id": null,
                            "operation_id": null
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_json: Value =
        serde_json::from_slice(&created.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let workflow_instance_id = created_json["data"]["workflow_instance_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(created_json["data"]["state"], "created");
    assert_eq!(created_json["data"]["step_states"][0]["state"], "pending");

    let fetched = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/workflow-instances/{workflow_instance_id}"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);

    let cancelled = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/v1/workflow-instances/{workflow_instance_id}/cancel"
                ))
                .header("authorization", "Bearer test-token")
                .header("idempotency-key", "workflow-cancel-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancelled.status(), StatusCode::OK);
    let cancelled_json: Value =
        serde_json::from_slice(&cancelled.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(cancelled_json["data"]["state"], "cancelled");
    assert_eq!(
        cancelled_json["data"]["state_reason"],
        "cancelled_by_root_owner"
    );
}

#[tokio::test]
async fn interface_spec_register_get_and_list_are_available() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = app(storage);

    let payload = json!({
        "name": "Draft Surface",
        "description": "Simple operation interface",
        "tags": ["ui"],
        "spec": interface_spec_payload()
    });

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/interface-specs")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "interface-spec-1")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first_json: Value =
        serde_json::from_slice(&first.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let interface_ref = first_json["data"]["interface_ref"].as_str().unwrap();

    let fetched = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/interface-specs/{interface_ref}"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);

    let listed = app
        .oneshot(
            Request::builder()
                .uri("/v1/interface-specs?tag=ui")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_json: Value =
        serde_json::from_slice(&listed.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(listed_json["data"][0]["interface_ref"], interface_ref);
}

#[tokio::test]
async fn interface_instance_create_and_get_for_operation_are_available() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = app(storage);

    let registered = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/interface-specs")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Draft Detail",
                        "description": "Operation draft interface",
                        "tags": ["ui", "draft"],
                        "spec": interface_spec_payload()
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let registered_json: Value =
        serde_json::from_slice(&registered.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    let interface_ref = registered_json["data"]["interface_ref"]
        .as_str()
        .unwrap()
        .to_string();

    let operation_id = create_operation(&app).await;

    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/interface-instances")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "interface-instance-1")
                .body(Body::from(
                    json!({
                        "interface_ref": interface_ref,
                        "operation_id": operation_id,
                        "workflow_instance_id": null,
                        "viewer": {"audience_id": "root_owner"}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_json: Value =
        serde_json::from_slice(&created.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let interface_instance_id = created_json["data"]["interface_instance_id"]
        .as_str()
        .unwrap();
    assert_eq!(created_json["data"]["state"], "ready");
    assert_eq!(created_json["data"]["blocks"][0]["type"], "draft_view");
    assert!(created_json["data"]["gate_summary"]["max_gate"].is_number());

    let fetched = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/interface-instances/{interface_instance_id}"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
}
