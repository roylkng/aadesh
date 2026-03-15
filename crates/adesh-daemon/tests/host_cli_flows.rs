use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use adesh_core::ports::storage::StorageProvider;
use adesh_daemon::{cognition, host_cli};
use adesh_storage_sqlite::SqliteStorage;

async fn new_storage() -> SqliteStorage {
    let storage = SqliteStorage::connect("sqlite::memory:").await.unwrap();
    StorageProvider::migrate(&storage).await.unwrap();
    storage
}

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("adesh-host-flow-{name}-{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn init_git_workspace(path: &Path, branch: &str, remote: &str) {
    assert!(
        Command::new("git")
            .arg("init")
            .arg(path)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["checkout", "-b", branch])
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["remote", "add", "origin", remote])
            .status()
            .unwrap()
            .success()
    );
}

#[tokio::test]
async fn host_wrapper_supports_before_task_prepare_flow() {
    let storage = new_storage().await;
    let repo = temp_dir("payments");
    init_git_workspace(
        &repo,
        "fix/upload-retry",
        "git@github.com:acme/payments-service.git",
    );

    let episode_one = host_cli::build_store_request(
        &[
            "--task".into(),
            "Reduce duplicate processing when upload worker retries after transient storage errors"
                .into(),
            "--summary".into(),
            "Added retry handling around transient storage failures in upload worker. Duplicate processing is still possible after partial write.".into(),
            "--file".into(),
            "src/upload/upload_worker.rs".into(),
            "--decision".into(),
            "Keep duplicate protection in UploadService, not in the worker retry loop::Retry transport and dedupe boundary should stay separated".into(),
            "--unresolved".into(),
            "Crash and partial-write retry behavior is still not proven safe by test coverage".into(),
            "--test".into(),
            "fail::retries_do_not_duplicate_chunks::Duplicate chunk handling is still broken after retry".into(),
            "--issue".into(),
            "PAY-241".into(),
            "--artifact".into(),
            "diff:ep1".into(),
            "--task-hint".into(),
            "upload-retry".into(),
        ],
        Some(&repo),
    )
    .unwrap();
    cognition::store_work_episode(&storage, episode_one)
        .await
        .unwrap();

    let episode_two = host_cli::build_store_request(
        &[
            "--task".into(),
            "Refactor retry fix so duplicate guard stays in service layer".into(),
            "--summary".into(),
            "Moved dedupe check into UploadService and kept retry logic explicit. Timeout-path coverage is still missing.".into(),
            "--file".into(),
            "src/upload/upload_worker.rs".into(),
            "--file".into(),
            "src/upload/upload_service.rs".into(),
            "--decision".into(),
            "Use explicit retry state handling rather than macro abstraction in this subsystem::Failure paths are easier to audit in explicit code".into(),
            "--test".into(),
            "pass::upload_service_dedupes_replayed_chunks".into(),
            "--task-hint".into(),
            "upload-retry".into(),
        ],
        Some(&repo),
    )
    .unwrap();
    cognition::store_work_episode(&storage, episode_two)
        .await
        .unwrap();

    let episode_three = host_cli::build_store_request(
        &[
            "--task".into(),
            "Review incident learnings around duplicate upload processing".into(),
            "--summary".into(),
            "Reviewed incident and confirmed duplicate processing risk is highest around retry plus partial-write behavior.".into(),
            "--file".into(),
            "tests/integration/upload_retry.rs".into(),
            "--unresolved".into(),
            "Retry metrics not added".into(),
            "--preference".into(),
            "Backend retry-path changes should include integration tests.".into(),
            "--risk".into(),
            "Expanding retries before closing the partial-write test gap risks repeating duplicate-processing behavior.".into(),
            "--test".into(),
            "fail::integration_upload_retry_partial_write::Partial-write recovery still fails the integration path".into(),
            "--artifact".into(),
            "doc:upload-duplication-incident".into(),
            "--task-hint".into(),
            "upload-retry".into(),
        ],
        Some(&repo),
    )
    .unwrap();
    cognition::store_work_episode(&storage, episode_three)
        .await
        .unwrap();

    let prepare_request = host_cli::build_prepare_request(
        &[
            "--task".into(),
            "Can you help finish the upload retry work safely?".into(),
            "--file".into(),
            "src/upload/upload_worker.rs".into(),
            "--task-hint".into(),
            "upload-retry".into(),
        ],
        Some(&repo),
    )
    .unwrap();
    let response = cognition::prepare_task_context(&storage, prepare_request)
        .await
        .unwrap();

    assert_eq!(
        response.task_focus,
        "Can you help finish the upload retry work safely?"
    );
    assert!(
        response
            .relevant_decisions
            .iter()
            .any(|item| item.statement.contains("UploadService"))
    );
    assert!(
        response
            .open_loops
            .iter()
            .any(|item| item.statement.contains("partial-write"))
    );
    assert!(
        response
            .likely_next_directions
            .first()
            .unwrap()
            .statement
            .contains("partial-write")
    );
}

#[tokio::test]
async fn host_wrapper_supports_sparse_after_task_store_flow() {
    let storage = new_storage().await;
    let repo = temp_dir("sparse");
    init_git_workspace(
        &repo,
        "feature/cache-safety",
        "git@github.com:acme/cache-service.git",
    );

    let summary_path = repo.join("episode-summary.txt");
    fs::write(
        &summary_path,
        "Moved cache invalidation into CacheService and kept retry logic explicit. Stale-read regression test is still failing.\n",
    )
    .unwrap();

    let store_request = host_cli::build_store_request(
        &[
            "--task".into(),
            "Harden cache invalidation retry flow".into(),
            "--summary-file".into(),
            summary_path.display().to_string(),
            "--file".into(),
            "src/cache/cache_worker.rs".into(),
            "--file".into(),
            "src/cache/cache_service.rs".into(),
            "--test".into(),
            "fail::cache_retry_stale_read::Stale-read regression still fails after retry".into(),
            "--artifact".into(),
            "doc:cache-stale-read-incident".into(),
            "--task-hint".into(),
            "cache-retry".into(),
        ],
        Some(&repo),
    )
    .unwrap();
    cognition::store_work_episode(&storage, store_request)
        .await
        .unwrap();

    let prepare_request = host_cli::build_prepare_request(
        &[
            "--task".into(),
            "What matters before I continue the cache retry fix?".into(),
            "--file".into(),
            "src/cache/cache_worker.rs".into(),
            "--task-hint".into(),
            "cache-retry".into(),
        ],
        Some(&repo),
    )
    .unwrap();
    let response = cognition::prepare_task_context(&storage, prepare_request)
        .await
        .unwrap();

    assert_eq!(
        response.task_focus,
        "What matters before I continue the cache retry fix?"
    );
    assert!(response
        .relevant_decisions
        .iter()
        .any(|item| item.statement.contains("CacheService") || item.basis.contains("candidate")));
    assert!(
        response
            .open_loops
            .iter()
            .any(|item| item.statement.contains("Stale-read regression"))
    );
    assert_eq!(response.open_loops.len(), 1);
    assert!(
        response.risk_flags.is_empty()
            || response
                .risk_flags
                .iter()
                .all(|item| !item.statement.contains("Stale-read regression"))
    );
    assert!(
        response
            .uncertainties
            .iter()
            .any(|item| item.contains("candidate"))
    );
}
