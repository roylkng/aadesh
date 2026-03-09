use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use adesh_contracts::{
    ApprovalDecisionResponse, ApprovalItemDetail, ApprovalItemSummary,
    CapabilitySnapshotMintResponse, CapabilitySnapshotResponse, CompiledSliceResponse,
    CurrentVersionsResponse, GateDecisionResponse, OperationResponse, ReasoningOutputResponse,
    ReplayResponse, RequestAcceptedResponse, RequestEnvelope, ReviewDecisionResponse,
    ReviewItemDetail, ReviewItemSummary, SchemaEntryResponse, SyscallResponse,
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
