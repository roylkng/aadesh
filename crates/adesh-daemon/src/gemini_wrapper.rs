use std::{
    path::Path,
    process::{Command, Stdio},
};

use adesh_contracts::{
    NextDirectionItem, PrepareTaskContextResponse, RiskFlagItem, ScopedGuidanceItem,
};
use adesh_core::ports::storage::StorageProvider;
use anyhow::{Context, bail};

use crate::{cognition, host_cli};

pub async fn run_gemini_host_cli<S: StorageProvider + ?Sized>(
    storage: &S,
    args: &[String],
    current_dir: Option<&Path>,
) -> anyhow::Result<()> {
    let Some(command) = args.first().map(String::as_str) else {
        bail!("missing host gemini subcommand");
    };

    match command {
        "prompt" => {
            let prompt = prepare_gemini_prompt(storage, &args[1..], current_dir).await?;
            println!("{prompt}");
        }
        "run" => {
            run_gemini_command_with_binary(
                storage,
                &args[1..],
                current_dir,
                gemini_binary_path().as_path(),
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
        other => bail!("unsupported host gemini subcommand: {other}"),
    }

    Ok(())
}

pub async fn prepare_gemini_prompt<S: StorageProvider + ?Sized>(
    storage: &S,
    args: &[String],
    current_dir: Option<&Path>,
) -> anyhow::Result<String> {
    let request = host_cli::build_prepare_request(args, current_dir)?;
    let response = cognition::prepare_task_context(storage, request.clone())
        .await
        .context("prepare_task_context failed")?;
    Ok(render_gemini_prompt(&request.task_prompt, &response))
}

pub async fn run_gemini_command_with_binary<S: StorageProvider + ?Sized>(
    storage: &S,
    args: &[String],
    current_dir: Option<&Path>,
    gemini_binary: &Path,
) -> anyhow::Result<()> {
    let (host_args, gemini_args) = split_passthrough_args(args);
    let prompt = prepare_gemini_prompt(storage, &host_args, current_dir).await?;
    invoke_gemini_cli(
        gemini_binary,
        &prompt,
        &gemini_args,
        resolved_command_cwd(&host_args, current_dir).as_deref(),
    )
}

pub fn render_gemini_prompt(task_prompt: &str, response: &PrepareTaskContextResponse) -> String {
    let mut sections = vec![
        "You are working with Aadesh context for cross-session continuity.".to_string(),
        "Use the context below only where it improves the current task. Prefer the live task over stale history when they conflict.".to_string(),
        String::new(),
        "Current task:".to_string(),
        task_prompt.trim().to_string(),
        String::new(),
        "Aadesh context:".to_string(),
        format!(
            "- Workspace: {} ({}, confidence {:.2})",
            response.workspace_resolution.resolved_scope_key,
            response.workspace_resolution.scope_type,
            response.workspace_resolution.confidence
        ),
        format!("- Task focus: {}", response.task_focus.trim()),
    ];

    sections.extend(render_guidance_section(
        "Relevant decisions",
        &response.relevant_decisions,
    ));
    sections.extend(render_guidance_section(
        "Applicable preferences",
        &response.applicable_preferences,
    ));
    sections.extend(render_guidance_section("Open loops", &response.open_loops));
    sections.extend(render_risk_section("Risk flags", &response.risk_flags));
    sections.extend(render_direction_section(
        "Likely next directions",
        &response.likely_next_directions,
    ));

    if !response.uncertainties.is_empty() {
        sections.push("Uncertainties:".to_string());
        for item in response.uncertainties.iter().take(3) {
            sections.push(format!("- {}", item.trim()));
        }
    }

    sections.push(String::new());
    sections.push("Respond to the current task. Apply the Aadesh context where it is materially helpful, but do not treat it as ground truth if the repo or live task contradicts it.".to_string());
    sections.join("\n")
}

pub fn invoke_gemini_cli(
    gemini_binary: &Path,
    prompt: &str,
    gemini_args: &[String],
    cwd: Option<&Path>,
) -> anyhow::Result<()> {
    let mut command = Command::new(gemini_binary);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command
        .arg("--prompt")
        .arg(prompt)
        .args(gemini_args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = command
        .status()
        .with_context(|| format!("failed to invoke Gemini CLI at {}", gemini_binary.display()))?;
    if !status.success() {
        bail!("Gemini CLI exited with status {status}");
    }
    Ok(())
}

pub fn gemini_binary_path() -> std::path::PathBuf {
    std::env::var_os("ADESH_GEMINI_BIN")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("gemini"))
}

fn resolved_command_cwd(args: &[String], current_dir: Option<&Path>) -> Option<std::path::PathBuf> {
    let request = host_cli::build_prepare_request(args, current_dir).ok()?;
    request
        .workspace
        .cwd
        .as_ref()
        .map(std::path::PathBuf::from)
        .or_else(|| current_dir.map(Path::to_path_buf))
}

fn split_passthrough_args(args: &[String]) -> (Vec<String>, Vec<String>) {
    if let Some(index) = args.iter().position(|arg| arg == "--") {
        (args[..index].to_vec(), args[index + 1..].to_vec())
    } else {
        (args.to_vec(), Vec::new())
    }
}

fn render_guidance_section(title: &str, items: &[ScopedGuidanceItem]) -> Vec<String> {
    render_section(
        title,
        items.iter().map(|item| {
            format_guidance_line(
                &item.statement,
                item.confidence,
                &item.basis,
                &item.evidence_refs,
            )
        }),
    )
}

fn render_risk_section(title: &str, items: &[RiskFlagItem]) -> Vec<String> {
    render_section(
        title,
        items.iter().map(|item| {
            format!(
                "{} [severity={}, confidence={:.2}; basis={}; evidence={}]",
                item.statement.trim(),
                item.severity,
                item.confidence,
                compact_text(&item.basis),
                compact_evidence(&item.evidence_refs),
            )
        }),
    )
}

fn render_direction_section(title: &str, items: &[NextDirectionItem]) -> Vec<String> {
    render_section(
        title,
        items.iter().map(|item| {
            format_guidance_line(
                &item.statement,
                item.confidence,
                &item.basis,
                &item.evidence_refs,
            )
        }),
    )
}

fn render_section<I>(title: &str, items: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let collected = items.into_iter().take(3).collect::<Vec<_>>();
    if collected.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::with_capacity(collected.len() + 1);
    lines.push(format!("{title}:"));
    for item in collected {
        lines.push(format!("- {item}"));
    }
    lines
}

fn format_guidance_line(
    statement: &str,
    confidence: f64,
    basis: &str,
    evidence_refs: &[String],
) -> String {
    format!(
        "{} [confidence={:.2}; basis={}; evidence={}]",
        statement.trim(),
        confidence,
        compact_text(basis),
        compact_evidence(evidence_refs),
    )
}

fn compact_text(text: &str) -> String {
    let mut normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.len() > 120 {
        normalized.truncate(117);
        normalized.push_str("...");
    }
    normalized
}

fn compact_evidence(evidence_refs: &[String]) -> String {
    if evidence_refs.is_empty() {
        return "none".to_string();
    }
    let joined = evidence_refs
        .iter()
        .take(3)
        .map(|item| item.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if evidence_refs.len() > 3 {
        format!("{joined}, ...")
    } else {
        joined
    }
}

#[cfg(test)]
mod tests {
    use adesh_contracts::{PrepareTaskContextResponse, WorkspaceResolutionResponse};

    use super::render_gemini_prompt;

    #[test]
    fn render_gemini_prompt_includes_current_task_and_ranked_sections() {
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
            render_gemini_prompt("What should I do next to validate the wedge?", &response);
        assert!(prompt.contains("Current task:\nWhat should I do next to validate the wedge?"));
        assert!(prompt.contains("Relevant decisions:"));
        assert!(prompt.contains("Likely next directions:"));
        assert!(prompt.contains("Run a real benchmark before expanding the surface."));
    }
}
