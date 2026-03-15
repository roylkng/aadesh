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

fn test_config(rate_limit_max_requests: u32) -> AppConfig {
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
        rate_limit_max_requests,
        syscall_retry_attempts: 2,
    }
}

async fn setup_app(rate_limit_max_requests: u32) -> (axum::Router, Arc<SqliteStorage>) {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    (
        adesh_daemon::http::app(test_config(rate_limit_max_requests), storage.clone()).unwrap(),
        storage,
    )
}

async fn json_body(response: axum::response::Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

#[tokio::test]
async fn request_status_cancel_and_audit_read_are_available() {
    let (app, _storage) = setup_app(120).await;
    let request_id = "req-extensions-1";
    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(
                    r#"{
                      "request_id":"req-extensions-1",
                      "source":{"channel":"http","transport":"rest"},
                      "received_at":"2026-03-08T00:00:00Z",
                      "requesting_principal":{"principal_type":"root_owner","principal_id":"owner-1"},
                      "requesting_audience_id":"root_owner",
                      "input":{"kind":"text","content":"draft and send this email"},
                      "constraints":{"policy_mode":"default","budgets":{"token_budget":256}}
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_json = json_body(created).await;
    let operation_id = created_json["data"]["primary_operation_id"]
        .as_str()
        .unwrap()
        .to_string();
    let audit_trace_id = created_json["data"]["audit_trace_ids"][0]
        .as_str()
        .unwrap()
        .to_string();

    let request_status = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/requests/{request_id}"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(request_status.status(), StatusCode::OK);
    let request_status_json = json_body(request_status).await;
    assert_eq!(request_status_json["data"]["request_id"], request_id);
    assert!(request_status_json["data"]["status"].is_string());

    let cancelled = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/operations/{operation_id}/cancel"))
                .header("authorization", "Bearer test-token")
                .header("idempotency-key", "cancel-op-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancelled.status(), StatusCode::OK);
    let cancelled_json = json_body(cancelled).await;
    assert_eq!(cancelled_json["data"]["state"], "cancelled");

    let audit = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/audit/{audit_trace_id}"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(audit.status(), StatusCode::OK);
    let audit_json = json_body(audit).await;
    assert_eq!(audit_json["data"]["audit_trace_id"], audit_trace_id);
    assert!(audit_json["data"]["timeline"].is_array());
}

#[tokio::test]
async fn oob_start_verify_and_approval_consume_work_for_oob_mode() {
    let (app, storage) = setup_app(120).await;

    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(
                    r#"{
                      "request_id":"req-extensions-2",
                      "source":{"channel":"http","transport":"rest"},
                      "received_at":"2026-03-08T00:00:00Z",
                      "requesting_principal":{"principal_type":"root_owner","principal_id":"owner-1"},
                      "requesting_audience_id":"root_owner",
                      "input":{"kind":"text","content":"draft and send this email"},
                      "constraints":{"policy_mode":"default","budgets":{"token_budget":256}}
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);

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
    let approvals_json = json_body(approvals).await;
    let approval_id = approvals_json["data"][0]["approval_id"]
        .as_str()
        .unwrap()
        .to_string();

    sqlx::query("UPDATE approval_items SET approval_mode = 'oob_required' WHERE approval_id = ?1")
        .bind(&approval_id)
        .execute(storage.pool())
        .await
        .unwrap();

    let start = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}/oob/start"))
                .header("authorization", "Bearer test-token")
                .header("idempotency-key", "oob-start-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(start.status(), StatusCode::OK);
    let start_json = json_body(start).await;
    let challenge_id = start_json["data"]["challenge_id"]
        .as_str()
        .unwrap()
        .to_string();

    let verify = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}/oob/verify"))
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "oob-verify-1")
                .body(Body::from(format!(
                    "{{\"challenge_id\":\"{challenge_id}\",\"response\":{{\"code\":\"123456\"}}}}"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(verify.status(), StatusCode::OK);
    let verify_json = json_body(verify).await;
    assert_eq!(verify_json["data"]["status"], "verified");

    let approved = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/approvals/{approval_id}"))
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .body(Body::from(
                    format!(
                        "{{\"decision\":\"approve\",\"modified_payload\":null,\"oob\":{{\"challenge_id\":\"{challenge_id}\"}}}}"
                    ),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(approved.status(), StatusCode::OK);
    let approved_json = json_body(approved).await;
    assert_eq!(approved_json["data"]["status"], "consumed");
}

#[tokio::test]
async fn manual_artifacts_enable_grounded_requests_and_high_stakes_without_evidence_is_blocked() {
    let (app, _storage) = setup_app(120).await;

    let upload = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/artifacts/manual")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "artifact-1")
                .body(Body::from(
                    r#"{
                      "filename":"contract.txt",
                      "media_type":"text/plain",
                      "content_base64":"VGhpcyBpcyBhIGNvbnRyYWN0IHdpdGgga2V5IHRlcm1zLg==",
                      "sensitivity_hint":2
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upload.status(), StatusCode::OK);
    let upload_json = json_body(upload).await;
    let artifact_ref = upload_json["data"]["ref_id"].as_str().unwrap().to_string();

    let grounded = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(
                    format!(
                        r#"{{
                          "request_id":"req-extensions-3",
                          "source":{{"channel":"http","transport":"rest"}},
                          "received_at":"2026-03-08T00:00:00Z",
                          "requesting_principal":{{"principal_type":"root_owner","principal_id":"owner-1"}},
                          "requesting_audience_id":"root_owner",
                          "input":{{
                            "kind":"text",
                            "content":"draft legal follow-up using attachment context",
                            "attachments":[{{"ref_id":"{artifact_ref}","ref_type":"manual_artifact","sensitivity_hint":2}}]
                          }},
                          "constraints":{{"policy_mode":"default","budgets":{{"token_budget":256}}}}
                        }}"#
                    ),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(grounded.status(), StatusCode::CREATED);
    let grounded_json = json_body(grounded).await;
    let grounded_operation = grounded_json["data"]["primary_operation_id"]
        .as_str()
        .unwrap()
        .to_string();

    let grounded_reasoning = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/operations/{grounded_operation}/reasoning-output"
                ))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(grounded_reasoning.status(), StatusCode::OK);
    let grounded_reasoning_json = json_body(grounded_reasoning).await;
    let draft_content = grounded_reasoning_json["data"]["reasoning_output"]["drafts"][0]["content"]
        .as_str()
        .unwrap();
    assert!(draft_content.contains("Context preview"));

    let blocked = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(
                    r#"{
                      "request_id":"req-extensions-4",
                      "source":{"channel":"http","transport":"rest"},
                      "received_at":"2026-03-08T00:00:00Z",
                      "requesting_principal":{"principal_type":"root_owner","principal_id":"owner-1"},
                      "requesting_audience_id":"root_owner",
                      "input":{"kind":"text","content":"draft legal analysis for external recipients"},
                      "constraints":{"policy_mode":"default","budgets":{"token_budget":256}}
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(blocked.status(), StatusCode::CREATED);
    let blocked_json = json_body(blocked).await;
    let blocked_operation_id = blocked_json["data"]["primary_operation_id"]
        .as_str()
        .unwrap()
        .to_string();
    let blocked_operation = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/operations/{blocked_operation_id}"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let blocked_operation_json = json_body(blocked_operation).await;
    assert_eq!(
        blocked_operation_json["data"]["state_reason"],
        "high_stakes_evidence_required"
    );
}

#[tokio::test]
async fn request_rate_limit_and_wedge_metrics_endpoint_work() {
    let (app, _storage) = setup_app(1).await;

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(
                    r#"{
                      "request_id":"req-rate-1",
                      "source":{"channel":"http","transport":"rest"},
                      "received_at":"2026-03-08T00:00:00Z",
                      "requesting_principal":{"principal_type":"root_owner","principal_id":"owner-1"},
                      "requesting_audience_id":"root_owner",
                      "input":{"kind":"text","content":"draft only"},
                      "constraints":{"policy_mode":"default","budgets":{"token_budget":256}}
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);

    let second = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(
                    r#"{
                      "request_id":"req-rate-2",
                      "source":{"channel":"http","transport":"rest"},
                      "received_at":"2026-03-08T00:00:00Z",
                      "requesting_principal":{"principal_type":"root_owner","principal_id":"owner-1"},
                      "requesting_audience_id":"root_owner",
                      "input":{"kind":"text","content":"draft only"},
                      "constraints":{"policy_mode":"default","budgets":{"token_budget":256}}
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    let second_json = json_body(second).await;
    assert_eq!(second_json["error"]["code"], "RATE_LIMITED");

    let metrics = app
        .oneshot(
            Request::builder()
                .uri("/v1/metrics/wedge")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(metrics.status(), StatusCode::OK);
    let metrics_json = json_body(metrics).await;
    assert!(metrics_json["data"]["requests_total"].as_i64().unwrap() >= 1);
}

#[tokio::test]
async fn send_request_is_blocked_when_send_capability_is_missing() {
    let (app, storage) = setup_app(120).await;

    sqlx::query(
        "UPDATE current_versions SET version_id = 'cap:missing', updated_at = ?1 WHERE version_kind = 'capability_snapshot'",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(storage.pool())
    .await
    .unwrap();

    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(
                    r#"{
                      "request_id":"req-extensions-5",
                      "source":{"channel":"http","transport":"rest"},
                      "received_at":"2026-03-08T00:00:00Z",
                      "requesting_principal":{"principal_type":"root_owner","principal_id":"owner-1"},
                      "requesting_audience_id":"root_owner",
                      "input":{"kind":"text","content":"draft and send this email"},
                      "constraints":{"policy_mode":"default","budgets":{"token_budget":256}}
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_json = json_body(created).await;
    let operation_id = created_json["data"]["primary_operation_id"]
        .as_str()
        .unwrap()
        .to_string();

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
    assert_eq!(operation.status(), StatusCode::OK);
    let operation_json = json_body(operation).await;
    assert_eq!(operation_json["data"]["state"], "blocked");
    assert_eq!(
        operation_json["data"]["state_reason"],
        "send_capability_unavailable"
    );

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
    let approvals_json = json_body(approvals).await;
    assert_eq!(approvals_json["data"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn send_request_is_blocked_when_diff_is_unavailable() {
    let (app, storage) = setup_app(120).await;

    sqlx::query(
        "UPDATE capability_snapshots
         SET json_payload = json_set(json_payload, '$.capabilities[0].actions[0].diff_supported', json('false'))
         WHERE capability_snapshot_version = 'cap:bootstrap'",
    )
    .execute(storage.pool())
    .await
    .unwrap();

    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(
                    r#"{
                      "request_id":"req-extensions-6",
                      "source":{"channel":"http","transport":"rest"},
                      "received_at":"2026-03-08T00:00:00Z",
                      "requesting_principal":{"principal_type":"root_owner","principal_id":"owner-1"},
                      "requesting_audience_id":"root_owner",
                      "input":{"kind":"text","content":"draft and send this email"},
                      "constraints":{"policy_mode":"default","budgets":{"token_budget":256}}
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_json = json_body(created).await;
    let operation_id = created_json["data"]["primary_operation_id"]
        .as_str()
        .unwrap()
        .to_string();

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
    assert_eq!(operation.status(), StatusCode::OK);
    let operation_json = json_body(operation).await;
    assert_eq!(operation_json["data"]["state"], "blocked");
    assert_eq!(
        operation_json["data"]["state_reason"],
        "diff_unavailable_for_send"
    );

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
    let approvals_json = json_body(approvals).await;
    assert_eq!(approvals_json["data"].as_array().unwrap().len(), 0);
}
