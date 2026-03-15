use std::time::Duration;

use adesh_core::AppConfig;
use adesh_storage_sqlite::SqliteStorage;
use futures_util::StreamExt;
use http::header::AUTHORIZATION;
use reqwest::Client;
use serde_json::Value;
use tokio::{net::TcpListener, task::JoinHandle, time::timeout};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

fn test_config(bind_addr: std::net::SocketAddr) -> AppConfig {
    AppConfig {
        bind_addr,
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

async fn spawn_server() -> (String, String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let storage = std::sync::Arc::new(SqliteStorage::connect("sqlite::memory:").await.unwrap());
    adesh_core::ports::storage::StorageProvider::migrate(storage.as_ref())
        .await
        .unwrap();

    let app = adesh_daemon::http::app(test_config(addr), storage).unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (
        format!("http://{addr}"),
        format!("ws://{addr}/v1/events"),
        handle,
    )
}

async fn next_text_message(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Value {
    loop {
        let message = timeout(Duration::from_secs(3), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if let Message::Text(text) = message {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

#[tokio::test]
async fn websocket_emits_hello_then_approval_and_execution_events() {
    let (http_base, ws_url, handle) = spawn_server().await;
    let client = Client::builder().build().unwrap();

    let mut request = ws_url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert(AUTHORIZATION, "Bearer test-token".parse().unwrap());

    let (mut socket, _) = connect_async(request).await.unwrap();

    let hello = next_text_message(&mut socket).await;
    assert_eq!(hello["type"], "hello");

    let created: Value = client
        .post(format!("{http_base}/v1/requests"))
        .bearer_auth("test-token")
        .json(&serde_json::json!({
            "request_id": "req-ws-1",
            "source": {"channel": "http", "transport": "rest"},
            "received_at": "2026-03-08T00:00:00Z",
            "requesting_principal": {"principal_type": "root_owner", "principal_id": "owner-1"},
            "requesting_audience_id": "root_owner",
            "input": {"kind": "text", "content": "draft and send this email"},
            "constraints": {"policy_mode": "default", "budgets": {"token_budget": 256}}
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let approvals: Value = client
        .get(format!("{http_base}/v1/approvals/pending"))
        .bearer_auth("test-token")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let approval_id = approvals["data"][0]["approval_id"]
        .as_str()
        .unwrap()
        .to_string();

    let mut saw_approval_required = false;
    for _ in 0..20 {
        let event = next_text_message(&mut socket).await;
        if event["type"] == "approval_required" {
            saw_approval_required = true;
            break;
        }
    }
    assert!(saw_approval_required);

    let _: Value = client
        .post(format!("{http_base}/v1/approvals/{approval_id}"))
        .bearer_auth("test-token")
        .json(&serde_json::json!({
            "decision": "approve",
            "modified_payload": null,
            "oob": null
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let mut saw_approval_granted = false;
    let mut saw_syscall_executed = false;
    let mut saw_operation_completed = false;
    for _ in 0..16 {
        let event = next_text_message(&mut socket).await;
        match event["type"].as_str().unwrap() {
            "approval_granted" => {
                saw_approval_granted = true;
                assert_eq!(event["data"]["next_state"], "running");
            }
            "syscall_executed" => {
                saw_syscall_executed = true;
                assert_eq!(event["data"]["status"], "executed");
            }
            "operation_state" => {
                if event["data"]["state"] == "completed" {
                    saw_operation_completed = true;
                    assert_eq!(event["data"]["reason"], "syscalls_executed");
                }
            }
            _ => {}
        }
        if saw_approval_granted && saw_syscall_executed && saw_operation_completed {
            break;
        }
    }

    assert!(saw_approval_granted);
    assert!(saw_syscall_executed);
    assert!(saw_operation_completed);
    assert!(created["data"]["primary_operation_id"].is_string());

    handle.abort();
}

#[tokio::test]
async fn websocket_accepts_query_token_for_browser_ui() {
    let (_http_base, ws_url, handle) = spawn_server().await;
    let (mut socket, _) = connect_async(format!("{ws_url}?access_token=test-token"))
        .await
        .unwrap();

    let hello = next_text_message(&mut socket).await;
    assert_eq!(hello["type"], "hello");

    handle.abort();
}

#[tokio::test]
async fn websocket_emits_ingest_job_created_and_cancelled_events() {
    let (http_base, ws_url, handle) = spawn_server().await;
    let client = Client::builder().build().unwrap();

    let mut request = ws_url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert(AUTHORIZATION, "Bearer test-token".parse().unwrap());
    let (mut socket, _) = connect_async(request).await.unwrap();

    let hello = next_text_message(&mut socket).await;
    assert_eq!(hello["type"], "hello");

    let created: Value = client
        .post(format!("{http_base}/v1/ingest/jobs"))
        .bearer_auth("test-token")
        .json(&serde_json::json!({
            "sources": [
                {
                    "type": "text",
                    "payload": {"text": "Ingest this note"},
                    "metadata": {"label": "demo"}
                }
            ],
            "options": {
                "dedupe": true,
                "max_artifacts": 10,
                "chunking": "none",
                "classification_mode": "conservative"
            }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let job_id = created["data"]["job_id"].as_str().unwrap();
    let mut saw_created = false;
    for _ in 0..8 {
        let event = next_text_message(&mut socket).await;
        if event["type"] == "ingest_job_created" {
            saw_created = true;
            assert_eq!(event["data"]["job_id"], job_id);
            assert_eq!(event["data"]["status"], "pending");
            break;
        }
    }

    assert!(saw_created);

    let _: Value = client
        .post(format!("{http_base}/v1/ingest/jobs/{job_id}/cancel"))
        .bearer_auth("test-token")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let mut saw_cancelled = false;
    for _ in 0..8 {
        let event = next_text_message(&mut socket).await;
        if event["type"] == "ingest_job_cancelled" {
            saw_cancelled = true;
            assert_eq!(event["data"]["job_id"], job_id);
            assert_eq!(event["data"]["status"], "cancelled");
            break;
        }
    }

    assert!(saw_cancelled);
    handle.abort();
}

#[tokio::test]
async fn websocket_emits_review_queue_and_capability_update_events() {
    let (http_base, ws_url, handle) = spawn_server().await;
    let client = Client::builder().build().unwrap();

    let mut request = ws_url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert(AUTHORIZATION, "Bearer test-token".parse().unwrap());
    let (mut socket, _) = connect_async(request).await.unwrap();

    let hello = next_text_message(&mut socket).await;
    assert_eq!(hello["type"], "hello");

    let minted: Value = client
        .post(format!("{http_base}/v1/capabilities/snapshots"))
        .bearer_auth("test-token")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
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
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let capability_snapshot_version = minted["data"]["capability_snapshot_version"]
        .as_str()
        .unwrap()
        .to_string();

    let review: Value = client
        .post(format!("{http_base}/v1/capabilities/current/activate"))
        .bearer_auth("test-token")
        .header("content-type", "application/json")
        .json(&serde_json::json!({
            "capability_snapshot_version": capability_snapshot_version
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let item_id = review["data"]["item_id"].as_str().unwrap().to_string();

    let mut saw_review_created = false;
    for _ in 0..6 {
        let event = next_text_message(&mut socket).await;
        if event["type"] == "review_queue_update" {
            saw_review_created = true;
            assert_eq!(event["data"]["item_id"], item_id);
            assert_eq!(event["data"]["action"], "created");
            break;
        }
    }
    assert!(saw_review_created);

    let _: Value = client
        .post(format!("{http_base}/v1/review-queue/{item_id}/decide"))
        .bearer_auth("test-token")
        .header("content-type", "application/json")
        .json(&serde_json::json!({ "decision": "approve", "edited_payload": null, "oob": null }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let mut saw_review_resolved = false;
    let mut saw_capability_update = false;
    for _ in 0..8 {
        let event = next_text_message(&mut socket).await;
        match event["type"].as_str().unwrap() {
            "review_queue_update" => {
                saw_review_resolved = true;
                assert_eq!(event["data"]["item_id"], item_id);
                assert_eq!(event["data"]["action"], "resolved");
                assert_eq!(event["data"]["decision"], "approve");
            }
            "capability_update" => {
                saw_capability_update = true;
                assert_eq!(
                    event["data"]["capability_snapshot_version"],
                    review["data"]["proposal"]["capability_snapshot_version"]
                );
            }
            _ => {}
        }
        if saw_review_resolved && saw_capability_update {
            break;
        }
    }

    assert!(saw_review_resolved);
    assert!(saw_capability_update);

    handle.abort();
}

#[tokio::test]
async fn websocket_emits_workflow_and_interface_lifecycle_events() {
    let (http_base, ws_url, handle) = spawn_server().await;
    let client = Client::builder().build().unwrap();

    let mut request = ws_url.into_client_request().unwrap();
    request
        .headers_mut()
        .insert(AUTHORIZATION, "Bearer test-token".parse().unwrap());
    let (mut socket, _) = connect_async(request).await.unwrap();

    let hello = next_text_message(&mut socket).await;
    assert_eq!(hello["type"], "hello");

    let workflow_spec: Value = client
        .post(format!("{http_base}/v1/workflow-specs"))
        .bearer_auth("test-token")
        .json(&serde_json::json!({
            "name": "WS Workflow",
            "description": "Workflow for WS lifecycle test",
            "tags": ["ws"],
            "spec": {
                "inputs": [{"name": "topic", "type": "string"}],
                "outputs": [{"name": "summary", "type": "string"}],
                "steps": [
                    {
                        "step_id": "step-transform",
                        "step_type": "transform",
                        "title": "Prepare",
                        "inputs": ["topic"],
                        "outputs": ["summary"]
                    }
                ],
                "edges": [],
                "entry_steps": ["step-transform"],
                "exit_steps": ["step-transform"]
            }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let workflow_ref = workflow_spec["data"]["workflow_ref"]
        .as_str()
        .unwrap()
        .to_string();

    let _: Value = client
        .post(format!("{http_base}/v1/workflow-instances"))
        .bearer_auth("test-token")
        .json(&serde_json::json!({
            "workflow_ref": workflow_ref,
            "inputs": {"topic": "Status"},
            "request_context": {"parent_request_id": null, "operation_id": null}
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let created: Value = client
        .post(format!("{http_base}/v1/requests"))
        .bearer_auth("test-token")
        .json(&serde_json::json!({
            "request_id": "req-ws-interface-1",
            "source": {"channel": "http", "transport": "rest"},
            "received_at": "2026-03-08T00:00:00Z",
            "requesting_principal": {"principal_type": "root_owner", "principal_id": "owner-1"},
            "requesting_audience_id": "root_owner",
            "input": {"kind": "text", "content": "draft an email update"},
            "constraints": {"policy_mode": "default", "budgets": {"token_budget": 256}}
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let operation_id = created["data"]["primary_operation_id"]
        .as_str()
        .unwrap()
        .to_string();

    let interface_spec: Value = client
        .post(format!("{http_base}/v1/interface-specs"))
        .bearer_auth("test-token")
        .json(&serde_json::json!({
            "name": "WS Interface",
            "description": "Interface for WS lifecycle test",
            "tags": ["ws"],
            "spec": {
                "blocks": [
                    {"block_id": "draft", "type": "draft_view", "title": "Latest draft"}
                ],
                "bindings": [
                    {"block_id": "draft", "source": {"kind": "reasoning_output"}}
                ]
            }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let interface_ref = interface_spec["data"]["interface_ref"]
        .as_str()
        .unwrap()
        .to_string();

    let _: Value = client
        .post(format!("{http_base}/v1/interface-instances"))
        .bearer_auth("test-token")
        .json(&serde_json::json!({
            "interface_ref": interface_ref,
            "operation_id": operation_id,
            "workflow_instance_id": null,
            "viewer": {"audience_id": "root_owner"}
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let mut saw_workflow_instance_state = false;
    let mut saw_workflow_step_state = false;
    let mut saw_interface_ready = false;
    for _ in 0..24 {
        let event = next_text_message(&mut socket).await;
        match event["type"].as_str().unwrap() {
            "workflow_instance_state" => {
                saw_workflow_instance_state = true;
                assert_eq!(event["data"]["state"], "created");
            }
            "workflow_step_state" => {
                saw_workflow_step_state = true;
                assert_eq!(event["data"]["step_type"], "transform");
            }
            "interface_instance_ready" => {
                saw_interface_ready = true;
                assert_eq!(event["data"]["state"], "ready");
            }
            _ => {}
        }
        if saw_workflow_instance_state && saw_workflow_step_state && saw_interface_ready {
            break;
        }
    }

    assert!(saw_workflow_instance_state);
    assert!(saw_workflow_step_state);
    assert!(saw_interface_ready);

    handle.abort();
}
