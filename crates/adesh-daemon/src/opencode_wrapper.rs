use std::{
    env,
    path::{Path, PathBuf},
};

use adesh_contracts::PrepareTaskContextResponse;
use adesh_core::ports::storage::StorageProvider;
use anyhow::{Context, bail};

use crate::{cognition, host_cli, host_wrapper_common};

pub async fn run_opencode_host_cli<S: StorageProvider + ?Sized>(
    storage: &S,
    args: &[String],
    current_dir: Option<&Path>,
) -> anyhow::Result<()> {
    let Some(command) = args.first().map(String::as_str) else {
        bail!("missing host opencode subcommand");
    };

    match command {
        "prompt" => {
            let prompt = prepare_opencode_prompt(storage, &args[1..], current_dir).await?;
            println!("{prompt}");
        }
        "run" => {
            run_opencode_command_with_binary(
                storage,
                &args[1..],
                current_dir,
                resolve_opencode_binary_path()?.as_path(),
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
        other => bail!("unsupported host opencode subcommand: {other}"),
    }

    Ok(())
}

pub async fn prepare_opencode_prompt<S: StorageProvider + ?Sized>(
    storage: &S,
    args: &[String],
    current_dir: Option<&Path>,
) -> anyhow::Result<String> {
    let request = host_cli::build_prepare_request(args, current_dir)?;
    let response = cognition::prepare_task_context(storage, request.clone())
        .await
        .context("prepare_task_context failed")?;
    Ok(render_opencode_prompt(&request.task_prompt, &response))
}

pub async fn run_opencode_command_with_binary<S: StorageProvider + ?Sized>(
    storage: &S,
    args: &[String],
    current_dir: Option<&Path>,
    opencode_binary: &Path,
) -> anyhow::Result<()> {
    let (host_args, opencode_args) = host_wrapper_common::split_passthrough_args(args);
    let prompt = prepare_opencode_prompt(storage, &host_args, current_dir).await?;
    invoke_opencode_cli(
        opencode_binary,
        &prompt,
        &opencode_args,
        host_wrapper_common::resolved_command_cwd(&host_args, current_dir).as_deref(),
    )
}

pub fn render_opencode_prompt(task_prompt: &str, response: &PrepareTaskContextResponse) -> String {
    let mut sections = vec![
        "Use the Aadesh context below to improve continuity on the current coding task."
            .to_string(),
        "Prioritize live task intent; only apply historical context when materially relevant."
            .to_string(),
        String::new(),
        "Current task:".to_string(),
        task_prompt.trim().to_string(),
        String::new(),
    ];
    host_wrapper_common::append_standard_context_sections(&mut sections, response);
    sections.push(String::new());
    sections.push("Respond to the current task directly with compact, practical output grounded in the context above.".to_string());
    sections.join("\n")
}

pub fn invoke_opencode_cli(
    opencode_binary: &Path,
    prompt: &str,
    opencode_args: &[String],
    cwd: Option<&Path>,
) -> anyhow::Result<()> {
    host_wrapper_common::invoke_prompt_cli(opencode_binary, prompt, opencode_args, cwd, "OpenCode")
}

pub fn resolve_opencode_binary_path() -> anyhow::Result<PathBuf> {
    let configured = env::var_os("ADESH_OPENCODE_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("opencode"));
    let resolved = if configured.components().count() > 1 || configured.is_absolute() {
        configured
    } else {
        host_wrapper_common::find_binary_in_path(&configured).unwrap_or(configured)
    };

    if !resolved.exists() {
        bail!(
            "OpenCode CLI binary was not found at {}. Install OpenCode CLI or set ADESH_OPENCODE_BIN.",
            resolved.display()
        );
    }
    if !resolved.is_file() {
        bail!(
            "OpenCode CLI path {} is not a file. Set ADESH_OPENCODE_BIN to the executable.",
            resolved.display()
        );
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use adesh_contracts::{PrepareTaskContextResponse, WorkspaceResolutionResponse};

    use super::render_opencode_prompt;

    #[test]
    fn render_opencode_prompt_includes_current_task_and_ranked_sections() {
        let response = PrepareTaskContextResponse {
            context_status: "full".to_string(),
            workspace_resolution: WorkspaceResolutionResponse {
                resolved_scope_key: "workspace:test".to_string(),
                scope_type: "workspace".to_string(),
                resolution_basis: vec!["cwd".to_string()],
                confidence: 0.92,
            },
            task_focus: "Validate the wedge in a real coding workflow.".to_string(),
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
            render_opencode_prompt("What should I do next to validate the wedge?", &response);
        assert!(prompt.contains("Current task:\nWhat should I do next to validate the wedge?"));
        assert!(prompt.contains("Relevant decisions:"));
        assert!(prompt.contains("Likely next directions:"));
        assert!(prompt.contains("Run a real benchmark before expanding the surface."));
    }
}
