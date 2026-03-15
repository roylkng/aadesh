use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use adesh_contracts::{
    ApprovalDecisionResponse, ApprovalItemDetail, ApprovalItemSummary, ArtifactResponse,
    CapabilitySnapshotMintResponse, CapabilitySnapshotResponse, CompiledSliceResponse,
    CurrentVersionsResponse, GateDecisionResponse, IngestJobResponse, InterfaceInstanceResponse,
    InterfaceSpecResponse, InterfaceSpecSummary, ManualArtifactResponse, MemoryClaimRecord,
    OobStartResponse, OobVerifyResponse, OperationResponse, ReasoningOutputResponse,
    ReplayResponse, RequestAcceptedResponse, RequestEnvelope, RequestStatusResponse,
    ReviewDecisionResponse, ReviewItemDetail, ReviewItemSummary, SchemaEntryResponse,
    SearchDocumentHit, SyscallResponse, WedgeMetricsResponse, WorkEpisodeResponse,
    WorkflowInstanceResponse, WorkflowSpecResponse, WorkflowSpecSummary, WorkspaceDescriptor,
    WorkspaceResolutionResponse,
};

use crate::{StorageError, action_schemas::ActionDescriptor};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseAcquisition {
    pub acquired: bool,
    pub lease_epoch: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationLease {
    pub operation_id: String,
    pub lease_owner: String,
    pub leased_until: DateTime<Utc>,
    pub lease_epoch: i64,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDecisionInput {
    pub operation_id: String,
    pub isolation_id: String,
    pub active_state_version: String,
    pub capability_snapshot_version: String,
    pub audience_graph_version: String,
    pub risk_r: i64,
    pub sensitivity_s: i64,
    pub max_gate: i64,
    pub approval_mode: String,
    pub requesting_audience_id: String,
    pub scopes_allowed: serde_json::Value,
    pub scopes_denied: serde_json::Value,
    pub sensitivity_ceiling_s: i64,
    pub predicates: serde_json::Value,
    pub constraints: serde_json::Value,
    pub audit_trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledSliceInput {
    pub operation_id: String,
    pub isolation_id: String,
    pub active_state_version: String,
    pub capability_snapshot_version: String,
    pub audience_graph_version: String,
    pub risk_r: i64,
    pub sensitivity_s: i64,
    pub max_gate: i64,
    pub approval_mode: String,
    pub operation_max_taint_s: i64,
    pub did_omit: bool,
    pub omissions: serde_json::Value,
    pub provenance_summary: serde_json::Value,
    pub intent_anchor: serde_json::Value,
    pub blocks: serde_json::Value,
    pub audit_trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalItemInput {
    pub operation_id: String,
    pub approval_mode: String,
    pub prompt: String,
    pub proposal_bundle: serde_json::Value,
    pub diff_payload: serde_json::Value,
    pub expires_at: Option<DateTime<Utc>>,
    pub audit_trace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalConsumeInput {
    pub approval_id: String,
    pub decision: String,
    pub modified_payload: Option<serde_json::Value>,
    pub oob_challenge_id: Option<String>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallStatusUpdateInput {
    pub syscall_id: String,
    pub new_status: String,
    pub result_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditTraceRecord {
    pub audit_trace_id: String,
    pub request_id: String,
    pub operation_id: String,
    pub isolation_id: String,
    pub pinned: serde_json::Value,
    pub summary: serde_json::Value,
    pub timeline: serde_json::Value,
    pub attachments: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayCreateInput {
    pub source_audit_trace_id: String,
    pub mode: String,
    pub strategy: String,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventAppendInput {
    pub event_ref: String,
    pub created_at: DateTime<Utc>,
    pub source_class: String,
    pub author: String,
    pub audience_id: String,
    pub sensitivity_s: i64,
    pub taint_s: i64,
    pub kind: String,
    pub content_ref: Option<String>,
    pub json_payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningOutputInput {
    pub operation_id: String,
    pub isolation_id: String,
    pub audit_trace_id: String,
    pub model_id: String,
    pub provider_trace_id: Option<String>,
    pub reasoning_output: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitySnapshotMintInput {
    pub base_version: Option<String>,
    pub snapshot_payload: serde_json::Value,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaRegisterInput {
    pub schema_kind: String,
    pub name: String,
    pub semver: String,
    pub schema_payload: serde_json::Value,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityActivationReviewInput {
    pub capability_snapshot_version: String,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDecisionInput {
    pub item_id: String,
    pub decision: String,
    pub edited_payload: Option<serde_json::Value>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OobStartInput {
    pub approval_id: String,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OobVerifyInput {
    pub approval_id: String,
    pub challenge_id: String,
    pub response_payload: serde_json::Value,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualArtifactCreateInput {
    pub filename: String,
    pub media_type: String,
    pub content_base64: String,
    pub sensitivity_hint: Option<u8>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestSourceInput {
    pub source_type: String,
    pub payload: serde_json::Value,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestOptionsInput {
    pub dedupe: bool,
    pub max_artifacts: i64,
    pub chunking: String,
    pub classification_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestJobCreateInput {
    pub sources: Vec<IngestSourceInput>,
    pub options: IngestOptionsInput,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestJobStatusUpdateInput {
    pub job_id: String,
    pub status: String,
    pub artifacts_total: i64,
    pub artifacts_succeeded: i64,
    pub artifacts_failed: i64,
    pub bytes_ingested: i64,
    pub error_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestJobItemUpsertInput {
    pub job_id: String,
    pub item_key: String,
    pub status: String,
    pub artifact_id: Option<String>,
    pub error_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactPutInput {
    pub artifact_id: String,
    pub ingest_job_id: Option<String>,
    pub kind: String,
    pub content_ref: String,
    pub parent_artifact_id: Option<String>,
    pub dedupe_key: Option<String>,
    pub meta: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkEpisodeDecisionInput {
    pub decision: String,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkEpisodeTestResultInput {
    pub name: String,
    pub status: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkEpisodeStoreInput {
    pub workspace: WorkspaceDescriptor,
    pub workspace_resolution: WorkspaceResolutionResponse,
    pub task_scope_key: Option<String>,
    pub task_prompt: String,
    pub summary: String,
    pub files_touched: Vec<String>,
    pub tests: Vec<WorkEpisodeTestResultInput>,
    pub decisions: Vec<WorkEpisodeDecisionInput>,
    pub unresolved_items: Vec<String>,
    pub observed_preferences: Vec<String>,
    pub risk_signals: Vec<String>,
    pub issue_refs: Vec<String>,
    pub artifact_refs: Vec<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkEpisodeListQuery {
    pub scope_type: Option<String>,
    pub scope_key: Option<String>,
    pub task_scope_key: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryClaimEvidenceInput {
    pub evidence_ref: String,
    pub evidence_kind: String,
    pub locator_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryClaimUpsertInput {
    pub claim_id: Option<String>,
    pub claim_type: String,
    pub claim_key: String,
    pub scope_type: String,
    pub scope_key: String,
    pub subject_key: String,
    pub status: String,
    pub created_by: String,
    pub confidence: f64,
    pub value_json: serde_json::Value,
    pub context_predicates_json: serde_json::Value,
    pub time_start: Option<DateTime<Utc>>,
    pub time_end: Option<DateTime<Utc>>,
    pub evidence_quality_json: serde_json::Value,
    pub promotion_ref: Option<String>,
    pub evidence: Vec<MemoryClaimEvidenceInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryClaimQuery {
    pub scope_type: Option<String>,
    pub scope_key: Option<String>,
    pub statuses: Vec<String>,
    pub claim_types: Vec<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchDocumentUpsertInput {
    pub doc_id: String,
    pub scope_type: String,
    pub scope_key: String,
    pub source_type: String,
    pub source_ref: String,
    pub title: Option<String>,
    pub body_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchDocumentQuery {
    pub scope_type: String,
    pub scope_key: String,
    pub query_text: String,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverableOperationExecution {
    pub operation_id: String,
    pub audit_trace_id: String,
    pub syscall_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowSpecRegisterInput {
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub spec_payload: serde_json::Value,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowSpecQuery {
    pub name: Option<String>,
    pub tag: Option<String>,
    pub author: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInstanceCreateInput {
    pub workflow_ref: String,
    pub parent_request_id: Option<String>,
    pub parent_operation_id: Option<String>,
    pub inputs: serde_json::Value,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInstanceStateUpdateInput {
    pub workflow_instance_id: String,
    pub expected_state: Option<String>,
    pub new_state: String,
    pub reason: Option<String>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceSpecRegisterInput {
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub spec_payload: serde_json::Value,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InterfaceSpecQuery {
    pub name: Option<String>,
    pub tag: Option<String>,
    pub author: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceInstanceCreateInput {
    pub interface_ref: String,
    pub operation_id: Option<String>,
    pub workflow_instance_id: Option<String>,
    pub viewer_audience_id: String,
    pub pinned_active_state_version: String,
    pub pinned_capability_snapshot_version: String,
    pub pinned_audience_graph_version: String,
    pub gate_summary: serde_json::Value,
    pub blocks: serde_json::Value,
    pub bindings: serde_json::Value,
    pub taint_summary: serde_json::Value,
    pub idempotency_key: Option<String>,
}

#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn migrate(&self) -> Result<(), StorageError>;
    async fn health(&self) -> Result<(), StorageError>;
    async fn append_event(&self, input: EventAppendInput) -> Result<(), StorageError>;
    async fn create_operation_bundle(
        &self,
        request: &RequestEnvelope,
        idempotency_key: Option<&str>,
    ) -> Result<RequestAcceptedResponse, StorageError>;
    async fn get_request_status(
        &self,
        request_id: &str,
    ) -> Result<RequestStatusResponse, StorageError>;
    async fn cancel_operation(
        &self,
        operation_id: &str,
        reason: Option<&str>,
        idempotency_key: Option<String>,
    ) -> Result<OperationResponse, StorageError>;
    async fn get_operation(&self, operation_id: &str) -> Result<OperationResponse, StorageError>;
    async fn update_operation_state(
        &self,
        operation_id: &str,
        new_state: &str,
        reason: Option<&str>,
        audit_trace_id: &str,
    ) -> Result<(), StorageError>;
    async fn put_gate_decision(
        &self,
        input: GateDecisionInput,
    ) -> Result<GateDecisionResponse, StorageError>;
    async fn get_gate_decision(
        &self,
        operation_id: &str,
    ) -> Result<GateDecisionResponse, StorageError>;
    async fn put_compiled_slice(
        &self,
        input: CompiledSliceInput,
    ) -> Result<CompiledSliceResponse, StorageError>;
    async fn get_compiled_slice(
        &self,
        operation_id: &str,
    ) -> Result<CompiledSliceResponse, StorageError>;
    async fn put_reasoning_output(
        &self,
        input: ReasoningOutputInput,
    ) -> Result<ReasoningOutputResponse, StorageError>;
    async fn get_reasoning_output(
        &self,
        operation_id: &str,
    ) -> Result<ReasoningOutputResponse, StorageError>;
    async fn get_current_versions(&self) -> Result<CurrentVersionsResponse, StorageError>;
    async fn get_capability_snapshot(
        &self,
        capability_snapshot_version: &str,
    ) -> Result<CapabilitySnapshotResponse, StorageError>;
    async fn mint_capability_snapshot(
        &self,
        input: CapabilitySnapshotMintInput,
    ) -> Result<CapabilitySnapshotMintResponse, StorageError>;
    async fn get_schema_entry(&self, schema_ref: &str)
    -> Result<SchemaEntryResponse, StorageError>;
    async fn register_schema_entry(
        &self,
        input: SchemaRegisterInput,
    ) -> Result<SchemaEntryResponse, StorageError>;
    async fn create_capability_activation_review_item(
        &self,
        input: CapabilityActivationReviewInput,
    ) -> Result<ReviewItemDetail, StorageError>;
    async fn list_review_items(&self) -> Result<Vec<ReviewItemSummary>, StorageError>;
    async fn get_review_item(&self, item_id: &str) -> Result<ReviewItemDetail, StorageError>;
    async fn decide_review_item(
        &self,
        input: ReviewDecisionInput,
    ) -> Result<ReviewDecisionResponse, StorageError>;
    async fn resolve_action_descriptor(
        &self,
        capability_snapshot_version: &str,
        tool_name: &str,
        action_name: &str,
    ) -> Result<ActionDescriptor, StorageError>;
    async fn create_approval_item(
        &self,
        input: ApprovalItemInput,
    ) -> Result<ApprovalItemSummary, StorageError>;
    async fn get_approval_item(
        &self,
        approval_id: &str,
    ) -> Result<ApprovalItemDetail, StorageError>;
    async fn list_pending_approvals(&self) -> Result<Vec<ApprovalItemSummary>, StorageError>;
    async fn start_oob_challenge(
        &self,
        input: OobStartInput,
    ) -> Result<OobStartResponse, StorageError>;
    async fn verify_oob_challenge(
        &self,
        input: OobVerifyInput,
    ) -> Result<OobVerifyResponse, StorageError>;
    async fn consume_approval_atomic(
        &self,
        input: ApprovalConsumeInput,
    ) -> Result<ApprovalDecisionResponse, StorageError>;
    async fn get_syscall(&self, syscall_id: &str) -> Result<SyscallResponse, StorageError>;
    async fn update_syscall_status(
        &self,
        input: SyscallStatusUpdateInput,
    ) -> Result<SyscallResponse, StorageError>;
    async fn list_syscalls_by_operation(
        &self,
        operation_id: &str,
    ) -> Result<Vec<SyscallResponse>, StorageError>;
    async fn get_audit_trace(&self, audit_trace_id: &str)
    -> Result<AuditTraceRecord, StorageError>;
    async fn append_audit_timeline_item(
        &self,
        audit_trace_id: &str,
        item: serde_json::Value,
    ) -> Result<(), StorageError>;
    async fn create_replay_dry_run(
        &self,
        input: ReplayCreateInput,
    ) -> Result<ReplayResponse, StorageError>;
    async fn create_manual_artifact(
        &self,
        input: ManualArtifactCreateInput,
    ) -> Result<ManualArtifactResponse, StorageError>;
    async fn get_manual_artifact_context(
        &self,
        ref_id: &str,
        max_chars: usize,
    ) -> Result<Option<String>, StorageError>;
    async fn create_ingest_job(
        &self,
        input: IngestJobCreateInput,
    ) -> Result<IngestJobResponse, StorageError>;
    async fn get_ingest_job(&self, job_id: &str) -> Result<IngestJobResponse, StorageError>;
    async fn update_ingest_job_status(
        &self,
        input: IngestJobStatusUpdateInput,
    ) -> Result<IngestJobResponse, StorageError>;
    async fn upsert_ingest_job_item(
        &self,
        input: IngestJobItemUpsertInput,
    ) -> Result<(), StorageError>;
    async fn put_artifact(&self, input: ArtifactPutInput)
    -> Result<ArtifactResponse, StorageError>;
    async fn get_artifact(&self, artifact_id: &str) -> Result<ArtifactResponse, StorageError>;
    async fn list_artifacts_by_job(
        &self,
        job_id: &str,
    ) -> Result<Vec<ArtifactResponse>, StorageError>;
    async fn store_work_episode(
        &self,
        input: WorkEpisodeStoreInput,
    ) -> Result<WorkEpisodeResponse, StorageError>;
    async fn list_work_episodes(
        &self,
        query: WorkEpisodeListQuery,
    ) -> Result<Vec<WorkEpisodeResponse>, StorageError>;
    async fn upsert_memory_claim(
        &self,
        input: MemoryClaimUpsertInput,
    ) -> Result<MemoryClaimRecord, StorageError>;
    async fn list_memory_claims(
        &self,
        query: MemoryClaimQuery,
    ) -> Result<Vec<MemoryClaimRecord>, StorageError>;
    async fn upsert_search_document(
        &self,
        input: SearchDocumentUpsertInput,
    ) -> Result<(), StorageError>;
    async fn search_documents(
        &self,
        query: SearchDocumentQuery,
    ) -> Result<Vec<SearchDocumentHit>, StorageError>;
    async fn register_workflow_spec(
        &self,
        input: WorkflowSpecRegisterInput,
    ) -> Result<WorkflowSpecResponse, StorageError>;
    async fn get_workflow_spec(
        &self,
        workflow_ref: &str,
    ) -> Result<WorkflowSpecResponse, StorageError>;
    async fn find_workflow_specs(
        &self,
        query: WorkflowSpecQuery,
    ) -> Result<Vec<WorkflowSpecSummary>, StorageError>;
    async fn create_workflow_instance(
        &self,
        input: WorkflowInstanceCreateInput,
    ) -> Result<WorkflowInstanceResponse, StorageError>;
    async fn get_workflow_instance(
        &self,
        workflow_instance_id: &str,
    ) -> Result<WorkflowInstanceResponse, StorageError>;
    async fn update_workflow_instance_state(
        &self,
        input: WorkflowInstanceStateUpdateInput,
    ) -> Result<WorkflowInstanceResponse, StorageError>;
    async fn register_interface_spec(
        &self,
        input: InterfaceSpecRegisterInput,
    ) -> Result<InterfaceSpecResponse, StorageError>;
    async fn get_interface_spec(
        &self,
        interface_ref: &str,
    ) -> Result<InterfaceSpecResponse, StorageError>;
    async fn find_interface_specs(
        &self,
        query: InterfaceSpecQuery,
    ) -> Result<Vec<InterfaceSpecSummary>, StorageError>;
    async fn create_interface_instance(
        &self,
        input: InterfaceInstanceCreateInput,
    ) -> Result<InterfaceInstanceResponse, StorageError>;
    async fn get_interface_instance(
        &self,
        interface_instance_id: &str,
    ) -> Result<InterfaceInstanceResponse, StorageError>;
    async fn get_wedge_metrics(&self) -> Result<WedgeMetricsResponse, StorageError>;
    async fn list_recoverable_operation_executions(
        &self,
    ) -> Result<Vec<RecoverableOperationExecution>, StorageError>;
    async fn try_acquire_operation_lease(
        &self,
        operation_id: &str,
        runner_id: &str,
        lease_duration_ms: i64,
    ) -> Result<LeaseAcquisition, StorageError>;
    async fn renew_operation_lease(
        &self,
        operation_id: &str,
        runner_id: &str,
        lease_epoch: i64,
        lease_duration_ms: i64,
    ) -> Result<OperationLease, StorageError>;
    async fn release_operation_lease(
        &self,
        operation_id: &str,
        runner_id: &str,
        lease_epoch: i64,
    ) -> Result<(), StorageError>;
}
