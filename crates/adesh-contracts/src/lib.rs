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
pub struct RequestStatusResponse {
    pub request_id: String,
    pub operation_ids: Vec<String>,
    pub status: String,
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
pub struct OobStartResponse {
    pub approval_id: String,
    pub challenge_id: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OobVerifyRequest {
    pub challenge_id: String,
    pub response: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OobVerifyResponse {
    pub approval_id: String,
    pub challenge_id: String,
    pub status: String,
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
pub struct AuditTraceResponse {
    pub audit_trace_id: String,
    pub request_id: String,
    pub operation_id: String,
    pub isolation_id: String,
    pub pinned: Value,
    pub summary: Value,
    pub timeline: Value,
    pub attachments: Option<Value>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualArtifactCreateRequest {
    pub filename: String,
    pub media_type: String,
    pub content_base64: String,
    pub sensitivity_hint: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManualArtifactResponse {
    pub artifact_id: String,
    pub ref_id: String,
    pub ref_type: String,
    pub filename: String,
    pub media_type: String,
    pub byte_size: i64,
    pub sensitivity_hint: Option<u8>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngestSourceRequest {
    pub r#type: String,
    pub payload: Value,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngestOptionsRequest {
    pub dedupe: bool,
    pub max_artifacts: i64,
    pub chunking: String,
    pub classification_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngestJobCreateRequest {
    pub sources: Vec<IngestSourceRequest>,
    pub options: IngestOptionsRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngestJobCountersResponse {
    pub artifacts_total: i64,
    pub artifacts_succeeded: i64,
    pub artifacts_failed: i64,
    pub bytes_ingested: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IngestJobResponse {
    pub job_id: String,
    pub status: String,
    pub source_count: i64,
    pub counters: IngestJobCountersResponse,
    pub options: Value,
    pub error_summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactResponse {
    pub artifact_id: String,
    pub created_at: DateTime<Utc>,
    pub ingest_job_id: Option<String>,
    pub kind: String,
    pub content_ref: String,
    pub parent_artifact_id: Option<String>,
    pub dedupe_key: Option<String>,
    pub meta: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceDescriptor {
    pub kind: String,
    pub locator: Option<String>,
    pub cwd: Option<String>,
    pub branch: Option<String>,
    pub external_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceResolutionResponse {
    pub resolved_scope_key: String,
    pub scope_type: String,
    pub resolution_basis: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkEpisodeDecision {
    pub decision: String,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkEpisodeTestResult {
    pub name: String,
    pub status: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreWorkEpisodeRequest {
    pub workspace: WorkspaceDescriptor,
    pub task_prompt: String,
    pub summary: String,
    #[serde(default)]
    pub files_touched: Vec<String>,
    #[serde(default)]
    pub tests: Vec<WorkEpisodeTestResult>,
    #[serde(default)]
    pub decisions: Vec<WorkEpisodeDecision>,
    #[serde(default)]
    pub unresolved_items: Vec<String>,
    #[serde(default)]
    pub observed_preferences: Vec<String>,
    #[serde(default)]
    pub risk_signals: Vec<String>,
    #[serde(default)]
    pub issue_refs: Vec<String>,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
    pub task_hint: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkEpisodeResponse {
    pub episode_id: String,
    pub event_ref: String,
    pub workspace: WorkspaceDescriptor,
    pub workspace_resolution: WorkspaceResolutionResponse,
    pub task_scope_key: Option<String>,
    pub task_prompt: String,
    pub summary: String,
    pub files_touched: Vec<String>,
    pub tests: Vec<WorkEpisodeTestResult>,
    pub decisions: Vec<WorkEpisodeDecision>,
    pub unresolved_items: Vec<String>,
    pub observed_preferences: Vec<String>,
    pub risk_signals: Vec<String>,
    pub issue_refs: Vec<String>,
    pub artifact_refs: Vec<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareTaskContextRequest {
    pub workspace: WorkspaceDescriptor,
    pub task_prompt: String,
    #[serde(default)]
    pub files_in_focus: Vec<String>,
    pub task_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopedGuidanceItem {
    pub statement: String,
    pub scope: String,
    pub confidence: f64,
    pub evidence_refs: Vec<String>,
    pub basis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiskFlagItem {
    pub statement: String,
    pub severity: String,
    pub confidence: f64,
    pub evidence_refs: Vec<String>,
    pub basis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NextDirectionItem {
    pub statement: String,
    pub confidence: f64,
    pub evidence_refs: Vec<String>,
    pub basis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareTaskContextResponse {
    pub context_status: String,
    pub workspace_resolution: WorkspaceResolutionResponse,
    pub task_focus: String,
    pub relevant_decisions: Vec<ScopedGuidanceItem>,
    pub applicable_preferences: Vec<ScopedGuidanceItem>,
    pub open_loops: Vec<ScopedGuidanceItem>,
    pub risk_flags: Vec<RiskFlagItem>,
    pub likely_next_directions: Vec<NextDirectionItem>,
    pub uncertainties: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecallRelevantMemoryRequest {
    pub workspace: WorkspaceDescriptor,
    pub query: String,
    pub task_hint: Option<String>,
    #[serde(default)]
    pub memory_types: Vec<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecallRelevantMemoryItem {
    pub memory_type: String,
    pub statement: String,
    pub scope: String,
    pub confidence: f64,
    pub evidence_refs: Vec<String>,
    pub basis: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecallRelevantMemoryResponse {
    pub workspace_resolution: WorkspaceResolutionResponse,
    pub memories: Vec<RecallRelevantMemoryItem>,
    pub uncertainties: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorEventKind {
    TaskStart,
    TaskCheckpoint,
    TaskEnd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorEventRequest {
    pub connector_id: String,
    pub connector_kind: String,
    pub connector_version: Option<String>,
    pub session_id: Option<String>,
    pub host_agent_id: Option<String>,
    pub host_agent_kind: Option<String>,
    pub host_model: Option<String>,
    pub context_id: Option<String>,
    pub selected_next_direction: Option<String>,
    pub outcome: Option<String>,
    pub correction_summary: Option<String>,
    pub event_kind: ConnectorEventKind,
    pub workspace: WorkspaceDescriptor,
    pub task_prompt: String,
    #[serde(default)]
    pub files_in_focus: Vec<String>,
    pub task_hint: Option<String>,
    pub summary: Option<String>,
    #[serde(default)]
    pub files_touched: Vec<String>,
    #[serde(default)]
    pub tests: Vec<WorkEpisodeTestResult>,
    #[serde(default)]
    pub decisions: Vec<WorkEpisodeDecision>,
    #[serde(default)]
    pub unresolved_items: Vec<String>,
    #[serde(default)]
    pub observed_preferences: Vec<String>,
    #[serde(default)]
    pub risk_signals: Vec<String>,
    #[serde(default)]
    pub issue_refs: Vec<String>,
    #[serde(default)]
    pub artifact_refs: Vec<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorEventResponse {
    pub connector_id: String,
    pub connector_kind: String,
    pub connector_version: Option<String>,
    pub session_id: Option<String>,
    pub event_kind: ConnectorEventKind,
    pub handled_as: String,
    pub context_id: Option<String>,
    pub prepare_context: Option<PrepareTaskContextResponse>,
    pub stored_episode: Option<WorkEpisodeResponse>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryClaimEvidence {
    pub evidence_ref: String,
    pub evidence_kind: String,
    pub locator: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryClaimRecord {
    pub claim_id: String,
    pub claim_type: String,
    pub claim_key: String,
    pub scope_type: String,
    pub scope_key: String,
    pub subject_key: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
    pub confidence: f64,
    pub value: Value,
    pub context_predicates: Value,
    pub time_start: Option<DateTime<Utc>>,
    pub time_end: Option<DateTime<Utc>>,
    pub evidence_quality: Value,
    pub promotion_ref: Option<String>,
    pub evidence: Vec<MemoryClaimEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchDocumentHit {
    pub doc_id: String,
    pub scope_type: String,
    pub scope_key: String,
    pub source_type: String,
    pub source_ref: String,
    pub title: Option<String>,
    pub body_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSpecRegisterRequest {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub spec: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSpecSummary {
    pub workflow_ref: String,
    pub name: String,
    pub description: Option<String>,
    pub author: String,
    pub tags: Vec<String>,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowSpecResponse {
    pub workflow_ref: String,
    pub name: String,
    pub description: Option<String>,
    pub author: String,
    pub tags: Vec<String>,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowRequestContext {
    pub parent_request_id: Option<String>,
    pub operation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowInstanceCreateRequest {
    pub workflow_ref: String,
    pub inputs: Value,
    pub request_context: WorkflowRequestContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowStepStateResponse {
    pub step_id: String,
    pub step_type: String,
    pub state: String,
    pub attempt: i64,
    pub operation_id: Option<String>,
    pub approval_id: Option<String>,
    pub syscall_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowInstanceResponse {
    pub workflow_instance_id: String,
    pub workflow_ref: String,
    pub parent_request_id: Option<String>,
    pub parent_operation_id: Option<String>,
    pub state: String,
    pub state_reason: Option<String>,
    pub pinned_active_state_version: String,
    pub pinned_capability_snapshot_version: String,
    pub pinned_audience_graph_version: String,
    pub inputs: Value,
    pub outputs: Option<Value>,
    pub step_states: Vec<WorkflowStepStateResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterfaceSpecRegisterRequest {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub spec: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterfaceSpecSummary {
    pub interface_ref: String,
    pub name: String,
    pub description: Option<String>,
    pub author: String,
    pub tags: Vec<String>,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterfaceSpecResponse {
    pub interface_ref: String,
    pub name: String,
    pub description: Option<String>,
    pub author: String,
    pub tags: Vec<String>,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterfaceViewer {
    pub audience_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterfaceInstanceCreateRequest {
    pub interface_ref: String,
    pub operation_id: Option<String>,
    pub workflow_instance_id: Option<String>,
    pub viewer: InterfaceViewer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterfaceInstanceResponse {
    pub interface_instance_id: String,
    pub interface_ref: String,
    pub operation_id: Option<String>,
    pub workflow_instance_id: Option<String>,
    pub viewer_audience_id: String,
    pub pinned_active_state_version: String,
    pub pinned_capability_snapshot_version: String,
    pub pinned_audience_graph_version: String,
    pub gate_summary: Value,
    pub blocks: Value,
    pub bindings: Value,
    pub taint_summary: Value,
    pub state: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WedgeMetricsResponse {
    pub generated_at: DateTime<Utc>,
    pub requests_total: i64,
    pub completed_drafts_total: i64,
    pub send_requests_total: i64,
    pub approvals_total: i64,
    pub sends_executed_total: i64,
    pub duplicate_send_violations: i64,
    pub audit_fail_open_events: i64,
    pub draft_latency_p50_ms: Option<i64>,
}
