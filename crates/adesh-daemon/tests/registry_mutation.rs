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

fn app(storage: Arc<SqliteStorage>) -> axum::Router {
    adesh_daemon::http::app(test_config(), storage).unwrap()
}

#[tokio::test]
async fn current_capabilities_returns_bootstrap_snapshot() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = app(storage);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/capabilities")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body["data"]["capability_snapshot_version"], "cap:bootstrap");
    let capabilities = body["data"]["payload"]["capabilities"].as_array().unwrap();
    assert!(capabilities.iter().any(|item| item["tool_name"] == "email"));
    assert!(
        capabilities
            .iter()
            .any(|item| item["tool_name"] == "webhook")
    );
}

#[tokio::test]
async fn schema_register_is_idempotent_and_gettable() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = app(storage);

    let payload = json!({
        "schema_kind": "action_args",
        "name": "demo.echo.args",
        "semver": "0.1.0",
        "schema_payload": {
            "type": "object",
            "required": ["message"],
            "additionalProperties": false,
            "properties": {
                "message": {"type": "string"}
            }
        }
    });

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/schema-registry/register")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "schema-demo-1")
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
                .uri("/v1/schema-registry/register")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "schema-demo-1")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    let second_json: Value =
        serde_json::from_slice(&second.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(first_json["data"], second_json["data"]);

    let schema_ref = first_json["data"]["schema_ref"].as_str().unwrap();
    let get_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/schema-registry/{schema_ref}"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn capability_snapshot_mint_validates_refs_and_is_gettable() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = app(storage);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/capabilities/snapshots")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "cap-snapshot-1")
                .body(Body::from(
                    json!({
                        "base_version": "cap:bootstrap",
                        "snapshot_payload": {
                            "capabilities": [
                                {
                                    "tool_name": "webhook",
                                    "actions": [
                                        {
                                            "action_name": "post_json",
                                            "args_schema_ref": "schema:sha256:adesh-webhook-post-json-args-v0_1",
                                            "result_schema_ref": "schema:sha256:adesh-webhook-post-json-result-v0_1",
                                            "diff_supported": false,
                                            "execution_class": "external_api",
                                            "default_approval_mode": "confirm",
                                            "diff_kind": "webhook_post_json_payload",
                                            "editable_payload_schema": {
                                                "type": "object",
                                                "required": ["url", "payload"],
                                                "additionalProperties": false,
                                                "properties": {
                                                    "url": {"type": "string"},
                                                    "payload": {"type": "object"},
                                                    "headers": {"type": "object"}
                                                }
                                            }
                                        }
                                    ]
                                }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let version = body["data"]["capability_snapshot_version"]
        .as_str()
        .unwrap();
    assert!(version.starts_with("cap:sha256:"));

    let get_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/capabilities/snapshots/{version}"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn capability_activation_creates_review_item_and_approve_updates_current_version() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = app(storage);

    let mint_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/capabilities/snapshots")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "cap-activate-mint")
                .body(Body::from(
                    json!({
                        "base_version": "cap:bootstrap",
                        "snapshot_payload": {
                            "capabilities": [
                                {
                                    "tool_name": "email",
                                    "actions": [
                                        {
                                            "action_name": "send",
                                            "args_schema_ref": "schema:sha256:adesh-email-send-payload-v0_1",
                                            "result_schema_ref": "schema:sha256:adesh-email-send-result-v0_1",
                                            "diff_supported": true,
                                            "execution_class": "external_api",
                                            "default_approval_mode": "diff",
                                            "diff_kind": "email_send_payload",
                                            "editable_payload_schema": {
                                                "type": "object",
                                                "required": ["to", "subject", "body"],
                                                "additionalProperties": false,
                                                "properties": {
                                                    "to": {"type": "array", "items": {"type": "string"}},
                                                    "cc": {"type": "array", "items": {"type": "string"}},
                                                    "bcc": {"type": "array", "items": {"type": "string"}},
                                                    "subject": {"type": "string"},
                                                    "body": {"type": "string"}
                                                }
                                            }
                                        }
                                    ]
                                },
                                {
                                    "tool_name": "webhook",
                                    "actions": [
                                        {
                                            "action_name": "post_json",
                                            "args_schema_ref": "schema:sha256:adesh-webhook-post-json-args-v0_1",
                                            "result_schema_ref": "schema:sha256:adesh-webhook-post-json-result-v0_1",
                                            "diff_supported": false,
                                            "execution_class": "external_api",
                                            "default_approval_mode": "confirm",
                                            "diff_kind": "webhook_post_json_payload",
                                            "editable_payload_schema": {
                                                "type": "object",
                                                "required": ["url", "payload"],
                                                "additionalProperties": false,
                                                "properties": {
                                                    "url": {"type": "string"},
                                                    "payload": {"type": "object"},
                                                    "headers": {"type": "object"}
                                                }
                                            }
                                        }
                                    ]
                                }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mint_response.status(), StatusCode::OK);
    let mint_json: Value = serde_json::from_slice(
        &mint_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let minted_version = mint_json["data"]["capability_snapshot_version"]
        .as_str()
        .unwrap()
        .to_string();

    let create_review = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/capabilities/current/activate")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "cap-activate-review")
                .body(Body::from(
                    json!({"capability_snapshot_version": minted_version}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_review.status(), StatusCode::OK);
    let review_json: Value = serde_json::from_slice(
        &create_review
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let item_id = review_json["data"]["item_id"].as_str().unwrap();
    assert_eq!(review_json["data"]["target_domain"], "capability_registry");

    let decide = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/review-queue/{item_id}/decide"))
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "cap-activate-decision")
                .body(Body::from(json!({"decision": "approve"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(decide.status(), StatusCode::OK);
    let decide_json: Value =
        serde_json::from_slice(&decide.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        decide_json["data"]["applied_version"],
        review_json["data"]["proposal"]["capability_snapshot_version"]
    );

    let current = app
        .oneshot(
            Request::builder()
                .uri("/v1/capabilities")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(current.status(), StatusCode::OK);
    let current_json: Value =
        serde_json::from_slice(&current.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        current_json["data"]["capability_snapshot_version"],
        review_json["data"]["proposal"]["capability_snapshot_version"]
    );
}

#[tokio::test]
async fn capability_activation_reject_leaves_current_version_unchanged() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = app(storage);

    let create_review = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/capabilities/current/activate")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "cap-activate-review-reject")
                .body(Body::from(
                    json!({"capability_snapshot_version": "cap:sha256:missing"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_review.status(), StatusCode::NOT_FOUND);

    let mint_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/capabilities/snapshots")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "base_version": "cap:bootstrap",
                        "snapshot_payload": {
                            "capabilities": [
                                {
                                    "tool_name": "webhook",
                                    "actions": [
                                        {
                                            "action_name": "post_json",
                                            "args_schema_ref": "schema:sha256:adesh-webhook-post-json-args-v0_1",
                                            "result_schema_ref": "schema:sha256:adesh-webhook-post-json-result-v0_1",
                                            "diff_supported": false,
                                            "execution_class": "external_api",
                                            "default_approval_mode": "confirm",
                                            "diff_kind": "webhook_post_json_payload",
                                            "editable_payload_schema": {
                                                "type": "object",
                                                "required": ["url", "payload"],
                                                "additionalProperties": false,
                                                "properties": {
                                                    "url": {"type": "string"},
                                                    "payload": {"type": "object"},
                                                    "headers": {"type": "object"}
                                                }
                                            }
                                        }
                                    ]
                                }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let mint_json: Value = serde_json::from_slice(
        &mint_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let minted_version = mint_json["data"]["capability_snapshot_version"]
        .as_str()
        .unwrap()
        .to_string();

    let review = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/capabilities/current/activate")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"capability_snapshot_version": minted_version}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let review_json: Value =
        serde_json::from_slice(&review.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let item_id = review_json["data"]["item_id"].as_str().unwrap();

    let reject = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/review-queue/{item_id}/decide"))
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .body(Body::from(json!({"decision": "reject"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reject.status(), StatusCode::OK);

    let current = app
        .oneshot(
            Request::builder()
                .uri("/v1/capabilities")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let current_json: Value =
        serde_json::from_slice(&current.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(
        current_json["data"]["capability_snapshot_version"],
        "cap:bootstrap"
    );
}

#[tokio::test]
async fn capability_activation_and_review_decision_are_idempotent() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = app(storage);

    let mint_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/capabilities/snapshots")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "cap-activate-idem-mint")
                .body(Body::from(
                    json!({
                        "base_version": "cap:bootstrap",
                        "snapshot_payload": {
                            "capabilities": [
                                {
                                    "tool_name": "webhook",
                                    "actions": [
                                        {
                                            "action_name": "post_json",
                                            "args_schema_ref": "schema:sha256:adesh-webhook-post-json-args-v0_1",
                                            "result_schema_ref": "schema:sha256:adesh-webhook-post-json-result-v0_1",
                                            "diff_supported": false,
                                            "execution_class": "external_api",
                                            "default_approval_mode": "confirm",
                                            "diff_kind": "webhook_post_json_payload",
                                            "editable_payload_schema": {
                                                "type": "object",
                                                "required": ["url", "payload"],
                                                "additionalProperties": false,
                                                "properties": {
                                                    "url": {"type": "string"},
                                                    "payload": {"type": "object"},
                                                    "headers": {"type": "object"}
                                                }
                                            }
                                        }
                                    ]
                                }
                            ]
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let mint_json: Value = serde_json::from_slice(
        &mint_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let minted_version = mint_json["data"]["capability_snapshot_version"]
        .as_str()
        .unwrap()
        .to_string();

    let activation_body = json!({"capability_snapshot_version": minted_version}).to_string();
    let first_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/capabilities/current/activate")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "cap-activate-idem-create")
                .body(Body::from(activation_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_create_json: Value =
        serde_json::from_slice(&first_create.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    let second_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/capabilities/current/activate")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "cap-activate-idem-create")
                .body(Body::from(activation_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_create_json: Value = serde_json::from_slice(
        &second_create
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(first_create_json["data"], second_create_json["data"]);

    let item_id = first_create_json["data"]["item_id"].as_str().unwrap();
    let decision_body = json!({"decision": "approve"}).to_string();
    let first_decide = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/review-queue/{item_id}/decide"))
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "cap-activate-idem-decide")
                .body(Body::from(decision_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_decide_json: Value =
        serde_json::from_slice(&first_decide.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    let second_decide = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/review-queue/{item_id}/decide"))
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "cap-activate-idem-decide")
                .body(Body::from(decision_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let second_decide_json: Value = serde_json::from_slice(
        &second_decide
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(first_decide_json["data"], second_decide_json["data"]);
}
