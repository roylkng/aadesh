use serde_json::{Value, json};

use adesh_contracts::RequestEnvelope;
use adesh_core::action_schemas::{
    ActionDescriptor, approval_diff_payload_for_action, default_email_send_payload,
};
use adesh_core::ports::storage::{ApprovalItemInput, CompiledSliceInput, GateDecisionInput};

#[derive(Debug, Clone)]
pub enum KernelOutcome {
    CompletedDraft,
    AwaitingApproval(ApprovalPlan),
    Blocked { reason: String },
}

#[derive(Debug, Clone)]
pub struct ApprovalPlan {
    pub prompt: String,
    pub proposal_bundle: Value,
    pub diff_payload: Value,
}

#[derive(Debug, Clone)]
pub struct KernelArtifacts {
    pub gate_decision: GateDecisionInput,
    pub compiled_slice: CompiledSliceInput,
    pub outcome: KernelOutcome,
}

pub fn compile_and_verify_stub(
    request: &RequestEnvelope,
    operation_id: &str,
    isolation_id: &str,
    audit_trace_id: &str,
    active_state_version: &str,
    capability_snapshot_version: &str,
    audience_graph_version: &str,
    email_send_descriptor: Option<&ActionDescriptor>,
) -> KernelArtifacts {
    let content_lower = request.input.content.to_lowercase();
    let attachment_sensitivity = request
        .input
        .attachments
        .iter()
        .filter_map(|attachment| attachment.sensitivity_hint.map(i64::from))
        .max()
        .unwrap_or(0);

    let requested_send = content_lower.contains("send");
    let unknown_audience = content_lower.contains("[[unknown_audience]]");
    let taint_launder = content_lower.contains("[[taint_launder]]")
        || (attachment_sensitivity >= 3 && content_lower.contains("public"));

    let risk_r = if requested_send { 3 } else { 1 };
    let sensitivity_s = attachment_sensitivity;
    let max_gate = risk_r.max(sensitivity_s);
    let approval_mode = if requested_send { "diff" } else { "none" }.to_string();

    let outcome = if taint_launder {
        KernelOutcome::Blocked {
            reason: "taint_laundering_denied".to_string(),
        }
    } else if unknown_audience {
        KernelOutcome::Blocked {
            reason: "audience_scope_denied".to_string(),
        }
    } else if requested_send {
        let descriptor = email_send_descriptor
            .expect("email send descriptor required when requested_send is true");
        let proposal_bundle = default_email_send_payload(&request.input.content);
        let diff_payload = approval_diff_payload_for_action(descriptor, &proposal_bundle);
        KernelOutcome::AwaitingApproval(ApprovalPlan {
            prompt: "Review and approve the email send payload.".to_string(),
            proposal_bundle: proposal_bundle.clone(),
            diff_payload,
        })
    } else {
        KernelOutcome::CompletedDraft
    };

    let predicates = json!({
        "requested_send": requested_send,
        "unknown_audience": unknown_audience,
        "taint_launder": taint_launder,
    });
    let constraints = json!({
        "wedge": "email_draft_and_send",
        "manual_artifacts_only": true,
    });

    let gate_decision = GateDecisionInput {
        operation_id: operation_id.to_string(),
        isolation_id: isolation_id.to_string(),
        active_state_version: active_state_version.to_string(),
        capability_snapshot_version: capability_snapshot_version.to_string(),
        audience_graph_version: audience_graph_version.to_string(),
        risk_r,
        sensitivity_s,
        max_gate,
        approval_mode: approval_mode.clone(),
        requesting_audience_id: request.requesting_audience_id.clone(),
        scopes_allowed: json!(["root_owner"]),
        scopes_denied: if unknown_audience {
            json!(["external_unknown"])
        } else {
            json!([])
        },
        sensitivity_ceiling_s: if requested_send { 1 } else { 4 },
        predicates,
        constraints,
        audit_trace_id: audit_trace_id.to_string(),
    };

    let compiled_slice = CompiledSliceInput {
        operation_id: operation_id.to_string(),
        isolation_id: isolation_id.to_string(),
        active_state_version: active_state_version.to_string(),
        capability_snapshot_version: capability_snapshot_version.to_string(),
        audience_graph_version: audience_graph_version.to_string(),
        risk_r,
        sensitivity_s,
        max_gate,
        approval_mode,
        operation_max_taint_s: sensitivity_s,
        did_omit: false,
        omissions: json!([]),
        provenance_summary: json!({
            "request_id": request.request_id,
            "attachment_count": request.input.attachments.len(),
        }),
        intent_anchor: json!({
            "goal": request.intent_anchor.as_ref().and_then(|anchor| anchor.goal.clone()).unwrap_or_else(|| request.input.content.clone()),
            "forbidden_outcomes": request.intent_anchor.as_ref().map(|anchor| anchor.forbidden_outcomes.clone()).unwrap_or_default(),
        }),
        blocks: json!({
            "policy": ["syscalls only", "persist before emit", "email send requires diff approval"],
            "operation_context": [{"kind": request.input.kind, "content": request.input.content}],
            "evidence": request.input.attachments,
        }),
        audit_trace_id: audit_trace_id.to_string(),
    };

    KernelArtifacts {
        gate_decision,
        compiled_slice,
        outcome,
    }
}

pub fn should_allow_retry_for_same_deny(previous_denials: usize) -> bool {
    previous_denials == 0
}

pub fn approval_item_input(
    operation_id: &str,
    audit_trace_id: &str,
    plan: ApprovalPlan,
) -> ApprovalItemInput {
    ApprovalItemInput {
        operation_id: operation_id.to_string(),
        approval_mode: "diff".to_string(),
        prompt: plan.prompt,
        proposal_bundle: plan.proposal_bundle,
        diff_payload: plan.diff_payload,
        expires_at: None,
        audit_trace_id: audit_trace_id.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::should_allow_retry_for_same_deny;

    #[test]
    fn anti_retry_returns_same_deny_then_blocks() {
        assert!(should_allow_retry_for_same_deny(0));
        assert!(!should_allow_retry_for_same_deny(1));
        assert!(!should_allow_retry_for_same_deny(2));
    }
}
