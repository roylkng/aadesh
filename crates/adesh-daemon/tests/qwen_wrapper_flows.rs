use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use adesh_core::ports::storage::StorageProvider;
use adesh_daemon::{cognition, host_cli, qwen_wrapper};
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
    let dir = std::env::temp_dir().join(format!("adesh-qwen-wrapper-{name}-{nanos}"));
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

fn seed_realistic_aadesh_episode(repo: &Path) -> adesh_contracts::StoreWorkEpisodeRequest {
    host_cli::build_store_request(
        &[
            "--task".into(),
            "Add a thin Qwen CLI wrapper without changing the cognition core.".into(),
            "--summary".into(),
            "Added a host-friendly wrapper plan around prepare-task-context and store-work-episode. The next task should validate the wrapper in real coding-agent use.".into(),
            "--file".into(),
            "crates/adesh-daemon/src/host_cli.rs".into(),
            "--file".into(),
            "README.md".into(),
            "--decision".into(),
            "Keep the cognition core unchanged and add a thin host-specific wrapper::Transport integration should not mutate the cognitive API".into(),
            "--unresolved".into(),
            "Need a realistic Qwen CLI wrapper flow with commands and tests".into(),
            "--preference".into(),
            "Prefer thin integration wrappers over new cognitive tools.".into(),
            "--task-hint".into(),
            "qwen-wrapper".into(),
        ],
        Some(repo),
    )
    .unwrap()
}

#[tokio::test]
async fn qwen_wrapper_formats_prepare_context_for_qwen() {
    let storage = new_storage().await;
    let repo = temp_dir("prompt");
    init_git_workspace(
        &repo,
        "feature/qwen-wrapper",
        "git@github.com:acme/aadesh.git",
    );

    let episode = seed_realistic_aadesh_episode(&repo);
    cognition::store_work_episode(&storage, episode)
        .await
        .unwrap();

    let prompt = qwen_wrapper::prepare_qwen_prompt(
        &storage,
        &[
            "--task".into(),
            "Use Qwen CLI to review the wrapper component for Aadesh itself.".into(),
            "--file".into(),
            "crates/adesh-daemon/src/host_cli.rs".into(),
            "--task-hint".into(),
            "qwen-wrapper".into(),
        ],
        Some(&repo),
    )
    .await
    .unwrap();

    assert!(prompt.contains("Current task:"));
    assert!(prompt.contains("Use Qwen CLI to review the wrapper component for Aadesh itself."));
    assert!(prompt.contains("Relevant decisions:"));
    assert!(prompt.contains("Open loops:"));
    assert!(prompt.contains("Likely next directions:"));
    assert!(prompt.contains("thin host-specific wrapper"));
    assert!(prompt.contains("Need a realistic Qwen CLI wrapper flow with commands and tests"));
}

#[tokio::test]
async fn qwen_wrapper_runs_fake_cli_with_context_prompt_and_passthrough_args() {
    let storage = new_storage().await;
    let repo = temp_dir("run");
    init_git_workspace(
        &repo,
        "feature/qwen-wrapper",
        "git@github.com:acme/aadesh.git",
    );

    let episode = seed_realistic_aadesh_episode(&repo);
    cognition::store_work_episode(&storage, episode)
        .await
        .unwrap();

    let fake_dir = temp_dir("fake-bin");
    let prompt_path = fake_dir.join("prompt.txt");
    let args_path = fake_dir.join("args.txt");
    let cwd_path = fake_dir.join("cwd.txt");
    let fake_qwen = fake_dir.join("qwen");
    fs::write(
        &fake_qwen,
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nprintf '%s\\n' \"$PWD\" > '{}'\nprintf '%s\\n' \"$@\" > '{}'\nprompt=''\nprev=''\nfor arg in \"$@\"; do\n  if [ \"$prev\" = \"--prompt\" ]; then\n    prompt=\"$arg\"\n    break\n  fi\n  prev=\"$arg\"\ndone\nprintf '%s' \"$prompt\" > '{}'\n",
            cwd_path.display(),
            args_path.display(),
            prompt_path.display(),
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&fake_qwen).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake_qwen, perms).unwrap();
    }

    qwen_wrapper::run_qwen_command_with_binary(
        &storage,
        &[
            "--task".into(),
            "Use Qwen CLI to review the wrapper component for Aadesh itself.".into(),
            "--file".into(),
            "crates/adesh-daemon/src/host_cli.rs".into(),
            "--task-hint".into(),
            "qwen-wrapper".into(),
            "--".into(),
            "--model".into(),
            "qwen3-coder-plus".into(),
        ],
        Some(&repo),
        &fake_qwen,
    )
    .await
    .unwrap();

    let prompt = fs::read_to_string(&prompt_path).unwrap();
    let args = fs::read_to_string(&args_path).unwrap();
    let cwd = fs::read_to_string(&cwd_path).unwrap();

    assert!(args.contains("--prompt"));
    assert!(args.contains("--model"));
    assert!(args.contains("qwen3-coder-plus"));
    assert!(prompt.contains("Current task:"));
    assert!(prompt.contains("Relevant decisions:"));
    assert!(prompt.contains("Likely next directions:"));
    assert!(prompt.contains("Need a realistic Qwen CLI wrapper flow with commands and tests"));
    assert_eq!(cwd.trim(), repo.display().to_string());
}
