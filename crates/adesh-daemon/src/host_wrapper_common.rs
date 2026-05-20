use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use adesh_contracts::{
    NextDirectionItem, PrepareTaskContextResponse, RiskFlagItem, ScopedGuidanceItem,
};
use anyhow::{Context, bail};

use crate::host_cli;

pub fn split_passthrough_args(args: &[String]) -> (Vec<String>, Vec<String>) {
    if let Some(index) = args.iter().position(|arg| arg == "--") {
        (args[..index].to_vec(), args[index + 1..].to_vec())
    } else {
        (args.to_vec(), Vec::new())
    }
}

pub fn resolved_command_cwd(args: &[String], current_dir: Option<&Path>) -> Option<PathBuf> {
    let request = host_cli::build_prepare_request(args, current_dir).ok()?;
    request
        .workspace
        .cwd
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| current_dir.map(Path::to_path_buf))
}

pub fn find_binary_in_path(name: &Path) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.exists() && candidate.is_file())
}

pub fn invoke_prompt_cli(
    binary: &Path,
    prompt: &str,
    args: &[String],
    cwd: Option<&Path>,
    tool_label: &str,
) -> anyhow::Result<()> {
    let mut command = Command::new(binary);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command
        .arg("--prompt")
        .arg(prompt)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = command
        .status()
        .with_context(|| format!("failed to invoke {tool_label} CLI at {}", binary.display()))?;
    if !status.success() {
        bail!("{tool_label} CLI exited with status {status}");
    }
    Ok(())
}

pub fn append_standard_context_sections(
    sections: &mut Vec<String>,
    response: &PrepareTaskContextResponse,
) {
    sections.push("Aadesh context:".to_string());
    sections.push(format!(
        "- Workspace: {} ({}, confidence {:.2})",
        response.workspace_resolution.resolved_scope_key,
        response.workspace_resolution.scope_type,
        response.workspace_resolution.confidence
    ));
    sections.push(format!("- Task focus: {}", response.task_focus.trim()));

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
