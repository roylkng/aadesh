use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use adesh_contracts::PrepareTaskContextResponse;
use adesh_core::ports::storage::StorageProvider;
use anyhow::{Context, bail};

use crate::{cognition, host_cli, host_wrapper_common};

pub async fn run_qwen_host_cli<S: StorageProvider + ?Sized>(
    storage: &S,
    args: &[String],
    current_dir: Option<&Path>,
) -> anyhow::Result<()> {
    let Some(command) = args.first().map(String::as_str) else {
        bail!("missing host qwen subcommand");
    };

    match command {
        "prompt" => {
            let prompt = prepare_qwen_prompt(storage, &args[1..], current_dir).await?;
            println!("{prompt}");
        }
        "run" => {
            run_qwen_command_with_binary(
                storage,
                &args[1..],
                current_dir,
                resolve_qwen_binary_path()?.as_path(),
            )
            .await?;
        }
        "store" => {
            let request = host_cli::build_store_request(&args[1..], current_dir)?;
            let response = cognition::store_work_episode(storage, request)
                .await
                .context("store_work_episode failed")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&response)
                    .context("failed to serialize store_work_episode response")?
            );
        }
        other => bail!("unsupported host qwen subcommand: {other}"),
    }

    Ok(())
}

pub async fn prepare_qwen_prompt<S: StorageProvider + ?Sized>(
    storage: &S,
    args: &[String],
    current_dir: Option<&Path>,
) -> anyhow::Result<String> {
    let request = host_cli::build_prepare_request(args, current_dir)?;
    let response = cognition::prepare_task_context(storage, request.clone())
        .await
        .context("prepare_task_context failed")?;
    Ok(render_qwen_prompt(&request.task_prompt, &response))
}

pub async fn run_qwen_command_with_binary<S: StorageProvider + ?Sized>(
    storage: &S,
    args: &[String],
    current_dir: Option<&Path>,
    qwen_binary: &Path,
) -> anyhow::Result<()> {
    let (host_args, qwen_args) = host_wrapper_common::split_passthrough_args(args);
    let prompt = prepare_qwen_prompt(storage, &host_args, current_dir).await?;
    invoke_qwen_cli(
        qwen_binary,
        &prompt,
        &qwen_args,
        host_wrapper_common::resolved_command_cwd(&host_args, current_dir).as_deref(),
    )
}

pub fn render_qwen_prompt(task_prompt: &str, response: &PrepareTaskContextResponse) -> String {
    let mut sections = vec![
        "Use the Aadesh context below to improve continuity on the current coding task."
            .to_string(),
        "Prioritize the live task. Use historical context only where it is materially relevant."
            .to_string(),
        String::new(),
        "Current task:".to_string(),
        task_prompt.trim().to_string(),
        String::new(),
    ];
    host_wrapper_common::append_standard_context_sections(&mut sections, response);
    sections.push(String::new());
    sections.push("Respond to the current task directly. Keep the answer compact, practical, and grounded in the workspace context above.".to_string());
    sections.join("\n")
}

pub fn invoke_qwen_cli(
    qwen_binary: &Path,
    prompt: &str,
    qwen_args: &[String],
    cwd: Option<&Path>,
) -> anyhow::Result<()> {
    host_wrapper_common::invoke_prompt_cli(qwen_binary, prompt, qwen_args, cwd, "Qwen")
}

pub fn resolve_qwen_binary_path() -> anyhow::Result<PathBuf> {
    let configured = env::var_os("ADESH_QWEN_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("qwen"));
    let resolved = if configured.components().count() > 1 || configured.is_absolute() {
        configured
    } else {
        host_wrapper_common::find_binary_in_path(&configured).unwrap_or(configured)
    };

    if !resolved.exists() {
        bail!(
            "Qwen CLI binary was not found at {}. Install Qwen Code or set ADESH_QWEN_BIN to the correct binary path.",
            resolved.display()
        );
    }
    if !resolved.is_file() {
        bail!(
            "Qwen CLI path {} is not a file. Set ADESH_QWEN_BIN to the Qwen executable.",
            resolved.display()
        );
    }

    validate_qwen_binary(&resolved)?;
    Ok(resolved)
}

fn validate_qwen_binary(binary: &Path) -> anyhow::Result<()> {
    let output = Command::new(binary)
        .arg("--help")
        .output()
        .with_context(|| format!("failed to invoke Qwen CLI help at {}", binary.display()))?;
    if !output.status.success() {
        bail!(
            "Qwen CLI at {} did not respond to --help successfully. Verify that the binary is a working Qwen Code installation.",
            binary.display()
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    if !combined.contains("--prompt") {
        bail!(
            "Binary at {} does not look like a compatible Qwen CLI. Expected --help output to mention the --prompt flag.",
            binary.display()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use adesh_contracts::{PrepareTaskContextResponse, WorkspaceResolutionResponse};
    use std::{
        ffi::OsStr,
        fs,
        sync::{Mutex, OnceLock},
    };

    use super::{render_qwen_prompt, resolve_qwen_binary_path};

    fn qwen_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_qwen_bin<T>(value: impl AsRef<OsStr>, f: impl FnOnce() -> T) -> T {
        let _guard = qwen_env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let old = std::env::var_os("ADESH_QWEN_BIN");
        unsafe {
            std::env::set_var("ADESH_QWEN_BIN", value);
        }
        let result = f();
        if let Some(value) = old {
            unsafe {
                std::env::set_var("ADESH_QWEN_BIN", value);
            }
        } else {
            unsafe {
                std::env::remove_var("ADESH_QWEN_BIN");
            }
        }
        result
    }

    #[test]
    fn render_qwen_prompt_includes_current_task_and_ranked_sections() {
        let response = PrepareTaskContextResponse {
            context_status: "full".to_string(),
            workspace_resolution: WorkspaceResolutionResponse {
                resolved_scope_key: "workspace:test".to_string(),
                scope_type: "workspace".to_string(),
                resolution_basis: vec!["cwd".to_string()],
                confidence: 0.92,
            },
            task_focus: "Validate the Qwen wrapper in a real coding workflow.".to_string(),
            relevant_decisions: vec![adesh_contracts::ScopedGuidanceItem {
                statement: "Keep the host wrapper thin.".to_string(),
                scope: "workspace".to_string(),
                confidence: 0.9,
                evidence_refs: vec!["ep:1".to_string()],
                basis: "Explicit decision".to_string(),
            }],
            applicable_preferences: vec![],
            open_loops: vec![adesh_contracts::ScopedGuidanceItem {
                statement: "Run a real benchmark before expanding the surface.".to_string(),
                scope: "workspace".to_string(),
                confidence: 0.85,
                evidence_refs: vec!["ep:2".to_string()],
                basis: "Explicit unresolved item".to_string(),
            }],
            risk_flags: vec![],
            likely_next_directions: vec![adesh_contracts::NextDirectionItem {
                statement: "Start with the benchmark run.".to_string(),
                confidence: 0.88,
                evidence_refs: vec!["ep:2".to_string()],
                basis: "Most actionable open loop".to_string(),
            }],
            uncertainties: vec!["Memory is still sparse.".to_string()],
        };

        let prompt =
            render_qwen_prompt("What should I do next to validate the wrapper?", &response);
        assert!(prompt.contains("Current task:\nWhat should I do next to validate the wrapper?"));
        assert!(prompt.contains("Relevant decisions:"));
        assert!(prompt.contains("Likely next directions:"));
        assert!(prompt.contains("Run a real benchmark before expanding the surface."));
    }

    #[test]
    fn resolve_qwen_binary_path_rejects_missing_binary_with_actionable_error() {
        let err = with_qwen_bin("/tmp/adesh-missing-qwen-binary", || {
            resolve_qwen_binary_path().unwrap_err().to_string()
        });
        assert!(err.contains("ADESH_QWEN_BIN"));
        assert!(err.contains("Install Qwen Code"));
    }

    #[test]
    fn resolve_qwen_binary_path_rejects_incompatible_binary() {
        let dir = std::env::temp_dir().join("adesh-qwen-incompatible-binary");
        let _ = fs::create_dir_all(&dir);
        let fake = dir.join("qwen");
        fs::write(
            &fake,
            "#!/usr/bin/env bash\nset -euo pipefail\necho 'not qwen help'\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&fake).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&fake, perms).unwrap();
        }

        let err = with_qwen_bin(&fake, || {
            resolve_qwen_binary_path().unwrap_err().to_string()
        });
        assert!(err.contains("does not look like a compatible Qwen CLI"));
        assert!(err.contains("--prompt"));
    }

    #[test]
    fn resolve_qwen_binary_path_accepts_help_without_branding_when_prompt_flag_exists() {
        let dir = std::env::temp_dir().join("adesh-qwen-compatible-binary");
        let _ = fs::create_dir_all(&dir);
        let fake = dir.join("qwen");
        fs::write(
            &fake,
            "#!/usr/bin/env bash\nset -euo pipefail\nif [ \"${1:-}\" = \"--help\" ]; then\n  echo 'Usage: qwen [options]'\n  echo '  -p, --prompt  Prompt text'\n  exit 0\nfi\nexit 1\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&fake).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&fake, perms).unwrap();
        }

        let resolved = with_qwen_bin(&fake, || resolve_qwen_binary_path().unwrap());
        assert_eq!(resolved, fake);
    }
}
