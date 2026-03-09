use adesh_contracts::RequestEnvelope;
use adesh_core::ports::storage::StorageProvider;
use adesh_storage_sqlite::SqliteStorage;

fn sample_request() -> RequestEnvelope {
    serde_json::from_str(
        r#"{
          "request_id": "req-lease-1",
          "source": {"channel": "http", "transport": "rest"},
          "received_at": "2026-03-08T00:00:00Z",
          "requesting_principal": {"principal_type": "root_owner", "principal_id": "owner-1"},
          "requesting_audience_id": "root_owner",
          "input": {"kind": "text", "content": "draft email"},
          "constraints": {"policy_mode": "default", "budgets": {"token_budget": 256}}
        }"#,
    )
    .unwrap()
}

#[tokio::test]
async fn operation_lease_compare_and_set_exclusive() {
    let storage = SqliteStorage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let accepted = storage
        .create_operation_bundle(&sample_request(), None)
        .await
        .unwrap();
    let operation_id = accepted.primary_operation_id;

    let first = storage
        .try_acquire_operation_lease(&operation_id, "runner-a", 30_000)
        .await
        .unwrap();
    assert!(first.acquired);

    let second = storage
        .try_acquire_operation_lease(&operation_id, "runner-b", 30_000)
        .await
        .unwrap();
    assert!(!second.acquired);
}
