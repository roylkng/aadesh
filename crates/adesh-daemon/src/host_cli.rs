use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use adesh_contracts::{
    ConnectorEventRequest, PrepareTaskContextRequest, StoreWorkEpisodeRequest, WorkEpisodeDecision,
    WorkEpisodeTestResult, WorkspaceDescriptor,
};
use adesh_core::{AppConfig, ports::storage::StorageProvider};
use adesh_storage_sqlite::SqliteStorage;
use anyhow::{Context, bail};

use crate::{
    cognition, connector_adapter, gemini_wrapper, mcp_stdio, opencode_wrapper, qwen_wrapper,
};

pub async fn run_host_cli(config: AppConfig, args: &[String]) -> anyhow::Result<()> {
    let Some(command) = args.first().map(String::as_str) else {
        bail!("missing host subcommand");
    };

    let cwd = std::env::current_dir().context("failed to determine current working directory")?;
    let storage = Arc::new(SqliteStorage::connect(&config.database_url).await?);
    storage.migrate().await?;

    match command {
        "gemini" | "gemini-cli" => {
            gemini_wrapper::run_gemini_host_cli(storage.as_ref(), &args[1..], Some(&cwd)).await?;
        }
        "opencode" | "opencode-cli" => {
            opencode_wrapper::run_opencode_host_cli(storage.as_ref(), &args[1..], Some(&cwd))
                .await?;
        }
        "qwen" | "qwen-cli" | "qwen-code" => {
            qwen_wrapper::run_qwen_host_cli(storage.as_ref(), &args[1..], Some(&cwd)).await?;
        }
        "mcp-stdio" => {
            if args.len() != 1 {
                bail!("usage: adesh-daemon host mcp-stdio");
            }
            mcp_stdio::run_mcp_stdio(storage.as_ref(), Some(&cwd)).await?;
        }
        "connector" => {
            let json = parse_json_arg(
                &args[1..],
                "usage: adesh-daemon host connector --json '<connector_event_payload>'",
            )?;
            let request: ConnectorEventRequest =
                serde_json::from_str(json).context("invalid connector event payload")?;
            let response = connector_adapter::handle_connector_event(storage.as_ref(), request)
                .await
                .context("connector adapter failed")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&response)
                    .context("failed to serialize connector response")?
            );
        }
        "prepare" => {
            let request = build_prepare_request(&args[1..], Some(&cwd))?;
            let response = cognition::prepare_task_context(storage.as_ref(), request)
                .await
                .context("prepare_task_context failed")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&response)
                    .context("failed to serialize prepare_task_context response")?
            );
        }
        "store" => {
            let request = build_store_request(&args[1..], Some(&cwd))?;
            let response = cognition::store_work_episode(storage.as_ref(), request)
                .await
                .context("store_work_episode failed")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&response)
                    .context("failed to serialize store_work_episode response")?
            );
        }
        other => bail!("unsupported host subcommand: {other}"),
    }

    Ok(())
}

fn parse_json_arg<'a>(args: &'a [String], usage: &str) -> anyhow::Result<&'a str> {
    if args.len() != 2 || args[0] != "--json" {
        bail!("{usage}");
    }
    Ok(args[1].as_str())
}

pub fn build_prepare_request(
    args: &[String],
    current_dir: Option<&Path>,
) -> anyhow::Result<PrepareTaskContextRequest> {
    let common = parse_common_args(args)?;
    let task_prompt = read_required_text_arg(
        common.task.as_deref(),
        common.task_file.as_deref(),
        "--task",
        "--task-file",
    )?;
    let workspace = resolve_workspace_descriptor(&common, current_dir)?;

    Ok(PrepareTaskContextRequest {
        workspace,
        task_prompt,
        files_in_focus: common.files,
        task_hint: common.task_hint,
    })
}

pub fn build_store_request(
    args: &[String],
    current_dir: Option<&Path>,
) -> anyhow::Result<StoreWorkEpisodeRequest> {
    let parsed = parse_store_args(args)?;
    let task_prompt = read_required_text_arg(
        parsed.common.task.as_deref(),
        parsed.common.task_file.as_deref(),
        "--task",
        "--task-file",
    )?;
    let summary = read_required_text_arg(
        parsed.summary.as_deref(),
        parsed.summary_file.as_deref(),
        "--summary",
        "--summary-file",
    )?;
    let workspace = resolve_workspace_descriptor(&parsed.common, current_dir)?;

    Ok(StoreWorkEpisodeRequest {
        workspace,
        task_prompt,
        summary,
        files_touched: parsed.common.files,
        tests: parsed.tests,
        decisions: parsed.decisions,
        unresolved_items: parsed.unresolved_items,
        observed_preferences: parsed.observed_preferences,
        risk_signals: parsed.risk_signals,
        issue_refs: parsed.issue_refs,
        artifact_refs: parsed.artifact_refs,
        task_hint: parsed.common.task_hint,
        started_at: None,
        ended_at: None,
    })
}

#[derive(Debug, Default, Clone)]
struct CommonHostArgs {
    workspace_kind: Option<String>,
    workspace_locator: Option<String>,
    cwd: Option<String>,
    branch: Option<String>,
    external_ref: Option<String>,
    task: Option<String>,
    task_file: Option<String>,
    task_hint: Option<String>,
    files: Vec<String>,
}

#[derive(Debug, Default)]
struct StoreHostArgs {
    common: CommonHostArgs,
    summary: Option<String>,
    summary_file: Option<String>,
    decisions: Vec<WorkEpisodeDecision>,
    unresolved_items: Vec<String>,
    observed_preferences: Vec<String>,
    risk_signals: Vec<String>,
    issue_refs: Vec<String>,
    artifact_refs: Vec<String>,
    tests: Vec<WorkEpisodeTestResult>,
}

fn parse_common_args(args: &[String]) -> anyhow::Result<CommonHostArgs> {
    let mut common = CommonHostArgs::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        match flag {
            "--workspace-kind" => common.workspace_kind = Some(next_value(args, &mut index, flag)?),
            "--workspace-locator" => {
                common.workspace_locator = Some(next_value(args, &mut index, flag)?)
            }
            "--cwd" => common.cwd = Some(next_value(args, &mut index, flag)?),
            "--branch" => common.branch = Some(next_value(args, &mut index, flag)?),
            "--external-ref" => common.external_ref = Some(next_value(args, &mut index, flag)?),
            "--task" => common.task = Some(next_value(args, &mut index, flag)?),
            "--task-file" => common.task_file = Some(next_value(args, &mut index, flag)?),
            "--task-hint" => common.task_hint = Some(next_value(args, &mut index, flag)?),
            "--file" => common.files.push(next_value(args, &mut index, flag)?),
            other => bail!("unsupported host flag for prepare: {other}"),
        }
        index += 1;
    }
    Ok(common)
}

fn parse_store_args(args: &[String]) -> anyhow::Result<StoreHostArgs> {
    let mut parsed = StoreHostArgs::default();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        match flag {
            "--workspace-kind" => {
                parsed.common.workspace_kind = Some(next_value(args, &mut index, flag)?)
            }
            "--workspace-locator" => {
                parsed.common.workspace_locator = Some(next_value(args, &mut index, flag)?)
            }
            "--cwd" => parsed.common.cwd = Some(next_value(args, &mut index, flag)?),
            "--branch" => parsed.common.branch = Some(next_value(args, &mut index, flag)?),
            "--external-ref" => {
                parsed.common.external_ref = Some(next_value(args, &mut index, flag)?)
            }
            "--task" => parsed.common.task = Some(next_value(args, &mut index, flag)?),
            "--task-file" => parsed.common.task_file = Some(next_value(args, &mut index, flag)?),
            "--task-hint" => parsed.common.task_hint = Some(next_value(args, &mut index, flag)?),
            "--file" => parsed
                .common
                .files
                .push(next_value(args, &mut index, flag)?),
            "--summary" => parsed.summary = Some(next_value(args, &mut index, flag)?),
            "--summary-file" => parsed.summary_file = Some(next_value(args, &mut index, flag)?),
            "--decision" => parsed
                .decisions
                .push(parse_decision(next_value(args, &mut index, flag)?)?),
            "--unresolved" => parsed
                .unresolved_items
                .push(next_value(args, &mut index, flag)?),
            "--preference" => parsed
                .observed_preferences
                .push(next_value(args, &mut index, flag)?),
            "--risk" => parsed
                .risk_signals
                .push(next_value(args, &mut index, flag)?),
            "--issue" => parsed.issue_refs.push(next_value(args, &mut index, flag)?),
            "--artifact" => parsed
                .artifact_refs
                .push(next_value(args, &mut index, flag)?),
            "--test" => parsed
                .tests
                .push(parse_test(next_value(args, &mut index, flag)?)?),
            other => bail!("unsupported host flag for store: {other}"),
        }
        index += 1;
    }
    Ok(parsed)
}

fn next_value(args: &[String], index: &mut usize, flag: &str) -> anyhow::Result<String> {
    *index += 1;
    let Some(value) = args.get(*index) else {
        bail!("missing value for {flag}");
    };
    Ok(value.clone())
}

fn read_required_text_arg(
    inline_value: Option<&str>,
    file_value: Option<&str>,
    inline_flag: &str,
    file_flag: &str,
) -> anyhow::Result<String> {
    match (inline_value, file_value) {
        (Some(_), Some(_)) => bail!("provide only one of {inline_flag} or {file_flag}"),
        (Some(value), None) => Ok(value.to_string()),
        (None, Some(path)) => {
            fs::read_to_string(path).with_context(|| format!("failed to read {file_flag} {path}"))
        }
        (None, None) => bail!("missing required input: {inline_flag} or {file_flag}"),
    }
}

fn parse_decision(value: String) -> anyhow::Result<WorkEpisodeDecision> {
    let mut parts = value.splitn(2, "::");
    let decision = parts
        .next()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .ok_or_else(|| anyhow::anyhow!("decision text may not be empty"))?;
    let rationale = parts
        .next()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string);
    Ok(WorkEpisodeDecision {
        decision: decision.to_string(),
        rationale,
    })
}

fn parse_test(value: String) -> anyhow::Result<WorkEpisodeTestResult> {
    let mut parts = value.splitn(3, "::");
    let status = parts
        .next()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .ok_or_else(|| anyhow::anyhow!("test status may not be empty"))?;
    let name = parts
        .next()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .ok_or_else(|| anyhow::anyhow!("test name may not be empty"))?;
    let summary = parts
        .next()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string);

    if !matches!(status, "pass" | "fail" | "skip") {
        bail!("unsupported test status: {status}");
    }

    Ok(WorkEpisodeTestResult {
        name: name.to_string(),
        status: status.to_string(),
        summary,
    })
}

fn resolve_workspace_descriptor(
    common: &CommonHostArgs,
    current_dir: Option<&Path>,
) -> anyhow::Result<WorkspaceDescriptor> {
    let explicit_cwd = common.cwd.as_ref().map(PathBuf::from);
    let detected_cwd = current_dir.map(Path::to_path_buf);
    let effective_cwd = explicit_cwd.or(detected_cwd);

    let detected_git = if matches!(
        common.workspace_kind.as_deref(),
        Some("conversation" | "task_space" | "unknown")
    ) {
        None
    } else {
        effective_cwd.as_deref().and_then(detect_git_workspace)
    };

    let kind = if let Some(kind) = common.workspace_kind.clone() {
        kind
    } else if common.external_ref.is_some() || common.branch.is_some() || detected_git.is_some() {
        "git".to_string()
    } else if common.workspace_locator.is_some() {
        "task_space".to_string()
    } else if effective_cwd.is_some() {
        "directory".to_string()
    } else {
        "unknown".to_string()
    };

    let cwd = match kind.as_str() {
        "git" | "directory" => effective_cwd
            .as_deref()
            .map(|path| path.display().to_string()),
        _ => common.cwd.clone(),
    };

    let locator = common
        .workspace_locator
        .clone()
        .or_else(|| match kind.as_str() {
            "git" => detected_git
                .as_ref()
                .and_then(|git| git.root.as_ref())
                .map(|path| path.display().to_string())
                .or_else(|| cwd.clone()),
            "directory" => cwd.clone(),
            _ => None,
        });

    let branch = common
        .branch
        .clone()
        .or_else(|| detected_git.as_ref().and_then(|git| git.branch.clone()));
    let external_ref = common.external_ref.clone().or_else(|| {
        detected_git
            .as_ref()
            .and_then(|git| git.external_ref.clone())
    });

    Ok(WorkspaceDescriptor {
        kind,
        locator,
        cwd,
        branch,
        external_ref,
    })
}

#[derive(Debug, Default)]
struct DetectedGitWorkspace {
    root: Option<PathBuf>,
    branch: Option<String>,
    external_ref: Option<String>,
}

fn detect_git_workspace(cwd: &Path) -> Option<DetectedGitWorkspace> {
    let root = run_git(cwd, &["rev-parse", "--show-toplevel"])
        .ok()
        .map(PathBuf::from)?;
    let branch = run_git(cwd, &["branch", "--show-current"]).ok();
    let external_ref = run_git(cwd, &["config", "--get", "remote.origin.url"]).ok();
    Some(DetectedGitWorkspace {
        root: Some(root),
        branch,
        external_ref,
    })
}

fn run_git(cwd: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .with_context(|| format!("failed to invoke git {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8(output.stdout).context("git output was not valid utf-8")?;
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        bail!("git {} returned no output", args.join(" "));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::{build_prepare_request, build_store_request};
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("adesh-host-cli-{name}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn init_git_workspace(path: &Path) {
        let status = Command::new("git").arg("init").arg(path).status().unwrap();
        assert!(status.success());
        let status = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["checkout", "-b", "feature/host-wrapper"])
            .status()
            .unwrap();
        assert!(status.success());
        let status = Command::new("git")
            .arg("-C")
            .arg(path)
            .args([
                "remote",
                "add",
                "origin",
                "git@github.com:acme/payments-service.git",
            ])
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn build_prepare_request_auto_detects_git_workspace() {
        let dir = temp_dir("prepare");
        init_git_workspace(&dir);

        let args = vec![
            "--task".to_string(),
            "Help finish the upload retry work safely.".to_string(),
            "--file".to_string(),
            "src/upload/upload_worker.rs".to_string(),
            "--task-hint".to_string(),
            "upload-retry".to_string(),
        ];
        let request = build_prepare_request(&args, Some(&dir)).unwrap();
        assert_eq!(request.workspace.kind, "git");
        assert_eq!(
            request.workspace.branch.as_deref(),
            Some("feature/host-wrapper")
        );
        assert_eq!(
            request.workspace.external_ref.as_deref(),
            Some("git@github.com:acme/payments-service.git")
        );
        assert_eq!(request.files_in_focus, vec!["src/upload/upload_worker.rs"]);
    }

    #[test]
    fn build_store_request_accepts_sparse_flags_and_file_inputs() {
        let dir = temp_dir("store");
        let summary_file = dir.join("summary.txt");
        fs::write(
            &summary_file,
            "Moved dedupe check into UploadService. Timeout-path coverage is still missing.\n",
        )
        .unwrap();

        let args = vec![
            "--workspace-kind".to_string(),
            "git".to_string(),
            "--cwd".to_string(),
            "/work/payments-service".to_string(),
            "--external-ref".to_string(),
            "git@github.com:acme/payments-service.git".to_string(),
            "--branch".to_string(),
            "fix/upload-retry".to_string(),
            "--task".to_string(),
            "Help finish the upload retry work safely.".to_string(),
            "--summary-file".to_string(),
            summary_file.display().to_string(),
            "--file".to_string(),
            "src/upload/upload_worker.rs".to_string(),
            "--decision".to_string(),
            "Keep duplicate protection in UploadService::Retry transport and dedupe boundary should stay separated".to_string(),
            "--unresolved".to_string(),
            "Timeout-path coverage is still missing".to_string(),
            "--test".to_string(),
            "fail::upload_worker_timeout_path::Timeout path still fails in the retry worker".to_string(),
        ];
        let request = build_store_request(&args, None).unwrap();
        assert_eq!(request.workspace.kind, "git");
        assert_eq!(request.files_touched, vec!["src/upload/upload_worker.rs"]);
        assert_eq!(request.decisions.len(), 1);
        assert_eq!(
            request.decisions[0].rationale.as_deref(),
            Some("Retry transport and dedupe boundary should stay separated")
        );
        assert_eq!(request.tests.len(), 1);
        assert_eq!(request.tests[0].status, "fail");
    }
}
