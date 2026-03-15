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

fn ingest_request_body() -> Value {
    json!({
        "sources": [
            {
                "type": "text",
                "payload": {
                    "text": "Personal notes about a draft email."
                },
                "metadata": {
                    "label": "notes"
                }
            }
        ],
        "options": {
            "dedupe": true,
            "max_artifacts": 10,
            "chunking": "none",
            "classification_mode": "conservative"
        }
    })
}

#[tokio::test]
async fn ingest_job_create_get_and_cancel_are_available() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = app(storage.clone());

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/jobs")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "ingest-job-1")
                .body(Body::from(ingest_request_body().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::ACCEPTED);
    let created: Value =
        serde_json::from_slice(&create.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let job_id = created["data"]["job_id"].as_str().unwrap().to_string();
    assert_eq!(created["data"]["status"], "pending");
    assert_eq!(created["data"]["counters"]["artifacts_total"], 0);
    assert_eq!(created["data"]["counters"]["bytes_ingested"], 0);

    let fetched = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/ingest/jobs/{job_id}"))
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fetched.status(), StatusCode::OK);
    let fetched_json: Value =
        serde_json::from_slice(&fetched.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(fetched_json["data"]["job_id"], job_id);
    assert_eq!(fetched_json["data"]["status"], "pending");

    let queued_status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE job_id = ?1")
        .bind(&job_id)
        .fetch_one(storage.pool())
        .await
        .unwrap();
    assert_eq!(queued_status, "pending");

    let cancel = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/ingest/jobs/{job_id}/cancel"))
                .header("authorization", "Bearer test-token")
                .header("idempotency-key", "ingest-cancel-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancel.status(), StatusCode::OK);
    let cancelled: Value =
        serde_json::from_slice(&cancel.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(cancelled["data"]["job_id"], job_id);
    assert_eq!(cancelled["data"]["status"], "cancelled");

    let cancelled_queue_status: String =
        sqlx::query_scalar("SELECT status FROM jobs WHERE job_id = ?1")
            .bind(&job_id)
            .fetch_one(storage.pool())
            .await
            .unwrap();
    assert_eq!(cancelled_queue_status, "cancelled");
}

#[tokio::test]
async fn ingest_job_post_is_idempotent() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = app(storage);
    let body = ingest_request_body();

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/jobs")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "ingest-idempotent")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    let first_json: Value =
        serde_json::from_slice(&first.into_body().collect().await.unwrap().to_bytes()).unwrap();

    let second = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/jobs")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .header("idempotency-key", "ingest-idempotent")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::ACCEPTED);
    let second_json: Value =
        serde_json::from_slice(&second.into_body().collect().await.unwrap().to_bytes()).unwrap();

    assert_eq!(first_json["data"], second_json["data"]);
}

#[tokio::test]
async fn ingest_job_create_rejects_empty_sources() {
    let storage = Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();
    let app = app(storage);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/ingest/jobs")
                .header("authorization", "Bearer test-token")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "sources": [],
                        "options": {
                            "dedupe": true,
                            "max_artifacts": 1,
                            "chunking": "none",
                            "classification_mode": "conservative"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
