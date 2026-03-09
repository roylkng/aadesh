use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Meta {
    pub request_id: String,
    pub ts: DateTime<Utc>,
    pub audit_trace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiSuccess<T> {
    pub ok: bool,
    pub data: T,
    pub meta: Meta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiErrorResponse {
    pub ok: bool,
    pub error: ApiErrorBody,
    pub meta: Meta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub storage: String,
    pub model_provider: String,
    pub tool_provider: String,
    pub queue: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentVersionsResponse {
    pub active_state_version: String,
    pub audience_graph_version: String,
    pub capability_snapshot_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySnapshotResponse {
    pub capability_snapshot_version: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySnapshotMintRequest {
    pub base_version: Option<String>,
    pub snapshot_payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityActivationRequest {
    pub capability_snapshot_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySnapshotMintResponse {
    pub capability_snapshot_version: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaRegisterRequest {
    pub schema_kind: String,
    pub name: String,
    pub semver: String,
    pub schema_payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaEntryResponse {
    pub schema_ref: String,
    pub schema_kind: String,
    pub name: String,
    pub semver: String,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
    pub status: String,
    pub compatibility: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewItemSummary {
    pub item_id: String,
    pub status: String,
    pub source: String,
    pub target_domain: String,
    pub risk_r_estimate: i64,
    pub sensitivity_s_estimate: i64,
    pub requires_oob: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewItemDetail {
    pub item_id: String,
    pub status: String,
    pub source: String,
    pub target_domain: String,
    pub risk_r_estimate: i64,
    pub sensitivity_s_estimate: i64,
    pub requires_oob: bool,
    pub created_at: DateTime<Utc>,
    pub proposal: Value,
    pub evidence: Value,
    pub impact: Value,
    pub base_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDecisionRequest {
    pub decision: String,
    pub edited_payload: Option<Value>,
    pub oob: Option<ApprovalOobPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewDecisionResponse {
    pub item_id: String,
    pub status: String,
    pub decision: String,
    pub applied_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestEnvelope {
    pub request_id: String,
    pub source: RequestSource,
    pub received_at: DateTime<Utc>,
    pub requesting_principal: RequestingPrincipal,
    pub requesting_audience_id: String,
    pub input: RequestInput,
    pub constraints: RequestConstraints,
    pub conversation: Option<RequestConversation>,
    pub intent_anchor: Option<IntentAnchor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestSource {
    pub channel: String,
    pub transport: String,
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestingPrincipal {
    pub principal_type: String,
    pub principal_id: String,
    pub owner_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestConversation {
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    #[serde(default)]
    pub history_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestInput {
    pub kind: String,
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<AttachmentRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentRef {
    pub ref_id: String,
    pub ref_type: String,
    pub sensitivity_hint: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestConstraints {
    pub policy_mode: String,
    pub budgets: RequestBudgets,
    pub preferred_model: Option<String>,
    pub allow_multi_operation: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestBudgets {
    pub token_budget: i64,
    pub latency_ms: Option<i64>,
    pub cost_cents: Option<i64>,
    pub compute_units: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentAnchor {
    pub goal: Option<String>,
    #[serde(default)]
    pub success_criteria: Vec<String>,
    #[serde(default)]
    pub forbidden_outcomes: Vec<String>,
    #[serde(default)]
    pub scope_limits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestAcceptedResponse {
    pub request_id: String,
    pub operation_ids: Vec<String>,
    pub primary_operation_id: String,
    pub audit_trace_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationResponse {
    pub operation_id: String,
    pub request_id: String,
    pub isolation_id: String,
    pub state: String,
    pub state_reason: Option<String>,
    pub requesting_audience_id: String,
    pub audit_trace_id: String,
    pub pinned_active_state_version: String,
    pub pinned_capability_snapshot_version: String,
    pub pinned_audience_graph_version: String,
    pub budgets: Value,
    pub operation_goal: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateDecisionResponse {
    pub gate_decision_id: String,
    pub operation_id: String,
    pub isolation_id: String,
    pub evaluated_at: DateTime<Utc>,
    pub active_state_version: String,
    pub capability_snapshot_version: String,
    pub audience_graph_version: String,
    pub risk_r: i64,
    pub sensitivity_s: i64,
    pub max_gate: i64,
    pub approval_mode: String,
    pub requesting_audience_id: String,
    pub scopes_allowed: Value,
    pub scopes_denied: Value,
    pub sensitivity_ceiling_s: i64,
    pub predicates: Value,
    pub constraints: Value,
    pub audit_trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledSliceResponse {
    pub compiled_slice_id: String,
    pub operation_id: String,
    pub isolation_id: String,
    pub compiled_at: DateTime<Utc>,
    pub active_state_version: String,
    pub capability_snapshot_version: String,
    pub audience_graph_version: String,
    pub risk_r: i64,
    pub sensitivity_s: i64,
    pub max_gate: i64,
    pub approval_mode: String,
    pub operation_max_taint_s: i64,
    pub did_omit: bool,
    pub omissions: Value,
    pub provenance_summary: Value,
    pub intent_anchor: Value,
    pub blocks: Value,
    pub audit_trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalItemSummary {
    pub approval_id: String,
    pub operation_id: String,
    pub approval_mode: String,
    pub prompt: String,
    pub diff: Value,
    pub expires_at: Option<DateTime<Utc>>,
    pub audit_trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalItemDetail {
    pub approval_id: String,
    pub operation_id: String,
    pub status: String,
    pub approval_mode: String,
    pub prompt: String,
    pub proposal_bundle: Value,
    pub diff: Value,
    pub expires_at: Option<DateTime<Utc>>,
    pub audit_trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalOobPayload {
    pub challenge_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalDecisionRequest {
    pub decision: String,
    pub modified_payload: Option<Value>,
    pub oob: Option<ApprovalOobPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalDecisionResponse {
    pub approval_id: String,
    pub operation_id: String,
    pub decision: String,
    pub status: String,
    pub operation_state: String,
    pub syscall_ids: Vec<String>,
    pub audit_trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyscallResponse {
    pub syscall_id: String,
    pub operation_id: String,
    pub approval_id: Option<String>,
    pub tool_name: String,
    pub action_name: String,
    pub args_schema_ref: String,
    pub result_schema_ref: Option<String>,
    pub status: String,
    pub args: Value,
    pub result_ref: Option<String>,
    pub audit_trace_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayRequest {
    pub mode: String,
    pub strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayResponse {
    pub replay_id: String,
    pub operation_id: String,
    pub audit_trace_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReasoningOutputResponse {
    pub event_ref: String,
    pub operation_id: String,
    pub model_id: String,
    pub provider_trace_id: Option<String>,
    pub reasoning_output: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WsEnvelope<T> {
    pub event_id: String,
    pub ts: DateTime<Utc>,
    pub r#type: String,
    pub request_id: Option<String>,
    pub operation_id: Option<String>,
    pub isolation_id: Option<String>,
    pub audit_trace_id: Option<String>,
    pub data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WsHelloData {
    pub message: String,
    pub server_version: String,
    pub capability_snapshot_version: String,
}
