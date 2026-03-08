# Rust Contract Summaries v0.1

Non-authoritative reference summary. Canonical schema and behavior definitions live in the root-level batch and port contract specs.

### Cargo deps (core)

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["serde", "v4"] }
thiserror = "1"
```

Optional for schema validation at HTTP boundary:

```toml
jsonschema = "0.17"
```

---

### `contracts/mod.rs`

```rust
pub mod common;
pub mod batch1;
pub mod batch2;
pub mod batch3;

pub use common::*;
pub use batch1::*;
pub use batch2::*;
pub use batch3::*;
```

---

### `contracts/common.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub type Id = String;

#[derive(Debug, Error)]
pub enum ContractError {
  #[error("missing field: {0}")]
  Missing(&'static str),
  #[error("invalid field: {field}: {msg}")]
  Invalid { field: &'static str, msg: String },
  #[error("constraint violation: {0}")]
  Constraint(String),
}

pub trait Validate {
  fn validate(&self) -> Result<(), ContractError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetaIds {
  pub request_id: Option<Id>,
  pub operation_id: Option<Id>,
  pub isolation_id: Option<Id>,
  pub syscall_id: Option<Id>,
  pub audit_trace_id: Option<Id>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
  None,
  Confirm,
  Diff,
  OobRequired,
  Refuse,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
  Created,
  Compiled,
  AwaitingApproval,
  Running,
  Blocked,
  Completed,
  Failed,
  Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyscallStatus {
  Proposed,
  Permitted,
  Denied,
  AwaitingApproval,
  Executed,
  Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
  Sensor,
  Actuator,
  Ipc,
  Sanitizer,
  MemoryRead,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceChannel {
  Http,
  Mcp,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
  Rest,
  Websocket,
  Sse,
  McpStdio,
  McpHttp,
}

pub fn now_utc() -> DateTime<Utc> {
  Utc::now()
}

pub fn new_id() -> String {
  Uuid::new_v4().to_string()
}
```

---

## Batch 1 models

### `contracts/batch1.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::contracts::common::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthLevel {
  Local,
  Strong,
  // NOTE: In v0.2 HTTP spec, OOB is per-approval and single-use.
  // Keep this enum for compatibility, but avoid using it as a global elevation.
  OobVerified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientType {
  Ui,
  Cli,
  Api,
  Mcp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OobChallengeType {
  Totp,
  Webauthn,
  DeviceSignature,
  HardwareKey,
  Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OobStatus {
  Pending,
  Verified,
  Expired,
  Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerSessionClient {
  pub client_type: Option<ClientType>,
  pub device_id: Option<String>,
  pub ip: Option<String>,
  pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerSessionOob {
  pub challenge_id: Option<String>,
  pub nonce: Option<String>,
  pub challenge_type: Option<OobChallengeType>,
  pub status: Option<OobStatus>,
  pub requested_at: Option<DateTime<Utc>>,
  pub verified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerSession {
  pub owner_id: String,
  pub session_id: String,
  pub auth_level: AuthLevel,
  pub issued_at: DateTime<Utc>,
  pub expires_at: DateTime<Utc>,
  pub scopes: Vec<String>,
  pub client: Option<OwnerSessionClient>,
  pub oob: Option<OwnerSessionOob>,
}

impl Validate for OwnerSession {
  fn validate(&self) -> Result<(), ContractError> {
    if self.owner_id.is_empty() { return Err(ContractError::Invalid{ field:"owner_id", msg:"empty".into()}); }
    if self.session_id.is_empty() { return Err(ContractError::Invalid{ field:"session_id", msg:"empty".into()}); }
    if self.expires_at <= self.issued_at {
      return Err(ContractError::Invalid{ field:"expires_at", msg:"must be after issued_at".into()});
    }
    Ok(())
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalType {
  RootOwner,
  AgentClient,
  ExternalUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestingPrincipal {
  pub principal_type: PrincipalType,
  pub principal_id: String,
  pub owner_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestSource {
  pub channel: SourceChannel,
  pub transport: Transport,
  pub client_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
  Text,
  Structured,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentType {
  File,
  Doc,
  Email,
  Image,
  Audio,
  Url,
  Artifact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttachmentRef {
  pub ref_id: String,
  pub ref_type: AttachmentType,
  pub sensitivity_hint: Option<u8>, // 0..4
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestInput {
  pub kind: InputKind,
  pub content: String,
  pub attachments: Option<Vec<AttachmentRef>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyMode {
  Default,
  Strict,
  Lenient,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestBudgets {
  pub token_budget: u32,
  pub latency_ms: Option<u32>,
  pub cost_cents: Option<u32>,
  pub compute_units: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestConstraints {
  pub policy_mode: PolicyMode,
  pub budgets: RequestBudgets,
  pub preferred_model: Option<String>,
  pub allow_multi_operation: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentAnchor {
  pub goal: String,
  pub success_criteria: Option<Vec<String>>,
  pub forbidden_outcomes: Option<Vec<String>>,
  pub scope_limits: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversationCtx {
  pub thread_id: Option<String>,
  pub turn_id: Option<String>,
  pub history_refs: Option<Vec<String>>, // event refs, not raw text
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestEnvelope {
  pub request_id: String,
  pub source: RequestSource,
  pub received_at: DateTime<Utc>,
  pub requesting_principal: RequestingPrincipal,
  pub requesting_audience_id: String,
  pub conversation: Option<ConversationCtx>,
  pub input: RequestInput,
  pub constraints: RequestConstraints,
  pub intent_anchor: Option<IntentAnchor>,
}

impl Validate for RequestEnvelope {
  fn validate(&self) -> Result<(), ContractError> {
    if self.request_id.is_empty() { return Err(ContractError::Invalid{ field:"request_id", msg:"empty".into()}); }
    if self.requesting_audience_id.is_empty() { return Err(ContractError::Invalid{ field:"requesting_audience_id", msg:"empty".into()}); }
    if self.input.content.is_empty() { return Err(ContractError::Invalid{ field:"input.content", msg:"empty".into()}); }
    if self.constraints.budgets.token_budget < 256 {
      return Err(ContractError::Invalid{ field:"constraints.budgets.token_budget", msg:"must be >= 256".into()});
    }
    if let Some(att) = &self.input.attachments {
      for a in att {
        if let Some(s) = a.sensitivity_hint {
          if s > 4 { return Err(ContractError::Invalid{ field:"attachments.sensitivity_hint", msg:"must be 0..4".into()}); }
        }
      }
    }
    Ok(())
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationGoal {
  pub summary: String,
  pub input_refs: Option<Vec<String>>,
  pub requested_outputs: Option<Vec<String>>, // descriptive only
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationLifecycle {
  pub state: OperationState,
  pub state_reason: Option<String>,
  pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockBudgets {
  pub policy: u32,
  pub capability: u32,
  pub operation_context: u32,
  pub evidence: u32,
  pub scratch: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationBudgets {
  pub token_budget: u32,
  pub block_budgets: BlockBudgets,
  pub latency_ms: Option<u32>,
  pub cost_cents: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedState {
  pub active_state_version: String,
  pub capability_snapshot_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceHints {
  pub sensitivity_hint: Option<u8>, // 0..4
  pub risk_hint: Option<u8>,        // 0..4
  pub requires_owner_session: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationIpc {
  pub consumes_artifacts: Option<Vec<String>>,
  pub inherits_sensitivity: Option<u8>, // 0..4
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationSpec {
  pub operation_id: String,
  pub parent_request_id: String,
  pub isolation_id: String,
  pub created_at: DateTime<Utc>,
  pub requesting_audience_id: String,
  pub operation_goal: OperationGoal,
  pub lifecycle: OperationLifecycle,
  pub budgets: OperationBudgets,
  pub pinned_state: PinnedState,
  pub governance_hints: Option<GovernanceHints>,
  pub ipc: Option<OperationIpc>,
}

impl Validate for OperationSpec {
  fn validate(&self) -> Result<(), ContractError> {
    if self.operation_id.is_empty() { return Err(ContractError::Invalid{ field:"operation_id", msg:"empty".into()}); }
    if self.isolation_id.is_empty() { return Err(ContractError::Invalid{ field:"isolation_id", msg:"empty".into()}); }
    if self.budgets.token_budget < 256 { return Err(ContractError::Invalid{ field:"budgets.token_budget", msg:"must be >= 256".into()}); }
    // Ensure block budgets sum <= token budget (soft rule, but good hygiene).
    let sum = self.budgets.block_budgets.policy
      + self.budgets.block_budgets.capability
      + self.budgets.block_budgets.operation_context
      + self.budgets.block_budgets.evidence
      + self.budgets.block_budgets.scratch;
    if sum > self.budgets.token_budget {
      return Err(ContractError::Constraint(format!(
        "block_budgets sum {} exceeds token_budget {}", sum, self.budgets.token_budget
      )));
    }
    Ok(())
  }
}
```

---

## Batch 2 models

### `contracts/batch2.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::contracts::common::*;
use crate::contracts::batch1::IntentAnchor;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiskInfo {
  pub level: u8, // 0..4
  pub predicates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitivitySourceKind {
  Attachment,
  EventRef,
  IpcArtifact,
  ToolResult,
  Inferred,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensitivitySource {
  pub kind: SensitivitySourceKind,
  pub ref_id: String,
  pub sensitivity_hint: Option<u8>, // 0..4
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SensitivityInfo {
  pub level: u8, // 0..4
  pub sources: Vec<SensitivitySource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudienceInfo {
  pub requesting_audience_id: String,
  pub is_root_owner: bool,
  pub graph_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeInfo {
  pub allowed: Vec<String>,
  pub denied: Vec<String>,
  pub sensitivity_ceiling: u8, // 0..4
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NegativeMemory {
  pub never_store: Vec<String>,
  pub never_act: Vec<String>,
  pub do_not_assume: Vec<String>,
  pub forget_expire: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenBudgets {
  pub total: u32,
  pub blocks: super::batch1::BlockBudgets,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaintPolicy {
  pub propagate_max_sensitivity: bool,
  pub requires_sanitization_syscall: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Constraints {
  pub negative_memory: NegativeMemory,
  pub token_budgets: TokenBudgets,
  pub taint_policy: TaintPolicy,
  pub intent_anchor_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalInfo {
  pub mode: ApprovalMode,
  pub reason: Option<String>,
  pub confirm_prompt: Option<String>,
  pub diff_template: Option<serde_json::Value>,
  pub oob: Option<OobRequirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OobRequirement {
  pub challenge_id: Option<String>,
  pub required: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateDecision {
  pub operation_id: String,
  pub isolation_id: String,
  pub evaluated_at: DateTime<Utc>,
  pub pinned: PinnedVersions,
  pub risk: RiskInfo,
  pub sensitivity: SensitivityInfo,
  pub max_gate: u8,
  pub audience: AudienceInfo,
  pub scopes: ScopeInfo,
  pub constraints: Constraints,
  pub approval: ApprovalInfo,
  pub audit_trace_id: Option<String>,
}

impl Validate for GateDecision {
  fn validate(&self) -> Result<(), ContractError> {
    if self.risk.level > 4 { return Err(ContractError::Invalid{ field:"risk.level", msg:"0..4".into()}); }
    if self.sensitivity.level > 4 { return Err(ContractError::Invalid{ field:"sensitivity.level", msg:"0..4".into()}); }
    let max_calc = self.risk.level.max(self.sensitivity.level);
    if self.max_gate != max_calc {
      return Err(ContractError::Constraint(format!("max_gate {} != max(risk,sensitivity) {}", self.max_gate, max_calc)));
    }
    if self.scopes.sensitivity_ceiling > 4 {
      return Err(ContractError::Invalid{ field:"scopes.sensitivity_ceiling", msg:"0..4".into()});
    }
    Ok(())
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedVersions {
  pub active_state_version: String,
  pub capability_snapshot_version: String,
  pub audience_graph_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateSummary {
  pub risk_level: u8,
  pub sensitivity_level: u8,
  pub max_gate: u8,
  pub approval_mode: ApprovalMode,
  pub sensitivity_ceiling: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
  pub token_budget: u32,
  pub content: String,
  pub taint_s: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSnippet {
  pub ref_id: String,
  pub text: String,
  pub sensitivity_s: u8,
  pub provenance: Option<SnippetProvenance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnippetProvenance {
  pub source_class: Option<String>,
  pub artifact_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceBlock {
  pub token_budget: u32,
  pub snippets: Vec<EvidenceSnippet>,
  pub taint_s: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScratchBlock {
  pub token_budget: u32,
  pub content: String,
  pub taint_s: u8,
  pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Blocks {
  pub policy: Block,
  pub capability: Block,
  pub operation_context: Block,
  pub evidence: EvidenceBlock,
  pub scratch: ScratchBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaintSummary {
  pub operation_max_taint_s: u8,
  pub sanitization_required_for_output: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OmissionReason {
  TokenBudgetExceeded,
  AudienceScopeDenied,
  GateConfidenceThreshold,
  SensitivityCeiling,
  TaintPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockName {
  Policy,
  Capability,
  OperationContext,
  Evidence,
  Scratch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OmittedItem {
  pub block: BlockName,
  pub reason: OmissionReason,
  pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Omissions {
  pub did_omit: bool,
  pub omitted_items: Vec<OmittedItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceSummary {
  pub primitive_refs: Vec<String>,
  pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledSlice {
  pub operation_id: String,
  pub isolation_id: String,
  pub compiled_at: DateTime<Utc>,
  pub pinned: PinnedVersions,
  pub gate: GateSummary,
  pub intent_anchor: IntentAnchor,
  pub blocks: Blocks,
  pub taint: TaintSummary,
  pub omissions: Omissions,
  pub provenance_summary: ProvenanceSummary,
  pub audit_trace_id: String,
}

impl Validate for CompiledSlice {
  fn validate(&self) -> Result<(), ContractError> {
    for (field, v) in [
      ("gate.risk_level", self.gate.risk_level),
      ("gate.sensitivity_level", self.gate.sensitivity_level),
      ("gate.max_gate", self.gate.max_gate),
      ("gate.sensitivity_ceiling", self.gate.sensitivity_ceiling),
      ("taint.operation_max_taint_s", self.taint.operation_max_taint_s),
      ("blocks.policy.taint_s", self.blocks.policy.taint_s),
      ("blocks.capability.taint_s", self.blocks.capability.taint_s),
      ("blocks.operation_context.taint_s", self.blocks.operation_context.taint_s),
      ("blocks.evidence.taint_s", self.blocks.evidence.taint_s),
      ("blocks.scratch.taint_s", self.blocks.scratch.taint_s),
    ] {
      if v > 4 { return Err(ContractError::Invalid{ field, msg:"0..4".into()}); }
    }

    let calc = self.gate.risk_level.max(self.gate.sensitivity_level);
    if self.gate.max_gate != calc {
      return Err(ContractError::Constraint(format!("gate.max_gate {} != max(r,s) {}", self.gate.max_gate, calc)));
    }

    // Governance block must not be empty.
    if self.blocks.policy.content.trim().is_empty() {
      return Err(ContractError::Invalid{ field:"blocks.policy.content", msg:"empty".into()});
    }

    // Operation taint must be >= each block taint (max taint invariant).
    let max_block_taint = self.blocks.policy.taint_s
      .max(self.blocks.capability.taint_s)
      .max(self.blocks.operation_context.taint_s)
      .max(self.blocks.evidence.taint_s)
      .max(self.blocks.scratch.taint_s);

    if self.taint.operation_max_taint_s != max_block_taint {
      return Err(ContractError::Constraint(format!(
        "operation_max_taint_s {} != max block taint {}",
        self.taint.operation_max_taint_s, max_block_taint
      )));
    }

    Ok(())
  }
}
```

---

## Batch 3 models

### `contracts/batch3.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::contracts::common::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CallerComponent {
  ReasoningCore,
  VerificationCore,
  Scheduler,
  Gateway,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pinned {
  pub active_state_version: String,
  pub capability_snapshot_version: String,
  pub audience_graph_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredEffect {
  Read,
  Write,
  ExternalSideEffect,
  SelfModification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyscallCaller {
  pub component: CallerComponent,
  pub model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyscallTarget {
  pub kind: TargetKind,
  pub name: String,
  pub provider: String,       // mcp|adapter|internal
  pub endpoint_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyscallIntent {
  pub action: String,
  pub args: serde_json::Value,
  pub declared_effect: Option<DeclaredEffect>,
  pub declared_audience_id: Option<String>,
  pub data_handles: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyscallGate {
  pub risk_r: u8,
  pub sensitivity_s: u8,
  pub max_gate: u8,
  pub approval_mode: ApprovalMode,
  pub audience_ceiling_s: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaintSourceKind {
  Block,
  Evidence,
  IpcArtifact,
  ToolResult,
  Inferred,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaintSource {
  pub kind: TaintSourceKind,
  pub ref_id: String,
  pub taint_s: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaintIn {
  pub max_taint_s: u8,
  pub sources: Vec<TaintSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyscallResult {
  pub ok: bool,
  pub started_at: Option<DateTime<Utc>>,
  pub finished_at: Option<DateTime<Utc>>,
  pub output_ref: Option<String>,
  pub output_sensitivity_s: Option<u8>,
  pub output_taint_s: Option<u8>,
  pub error_code: Option<String>,
  pub error_message: Option<String>,
  pub retryable: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyscallEnvelope {
  pub syscall_id: String,
  pub operation_id: String,
  pub isolation_id: String,
  pub issued_at: DateTime<Utc>,
  pub pinned: Pinned,
  pub caller: SyscallCaller,
  pub target: SyscallTarget,
  pub intent: SyscallIntent,
  pub gate: SyscallGate,
  pub taint_in: TaintIn,
  pub status: SyscallStatus,
  pub result: Option<SyscallResult>,
  pub audit_trace_id: String,
}

impl Validate for SyscallEnvelope {
  fn validate(&self) -> Result<(), ContractError> {
    for (field, v) in [
      ("gate.risk_r", self.gate.risk_r),
      ("gate.sensitivity_s", self.gate.sensitivity_s),
      ("gate.max_gate", self.gate.max_gate),
      ("gate.audience_ceiling_s", self.gate.audience_ceiling_s),
      ("taint_in.max_taint_s", self.taint_in.max_taint_s),
    ] {
      if v > 4 { return Err(ContractError::Invalid{ field, msg:"0..4".into()}); }
    }
    let calc = self.gate.risk_r.max(self.gate.sensitivity_s);
    if self.gate.max_gate != calc {
      return Err(ContractError::Constraint(format!("gate.max_gate {} != max {}", self.gate.max_gate, calc)));
    }
    Ok(())
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenyClass {
  AudienceScopeDenied,
  SensitivityCeilingExceeded,
  NegativeMemoryViolation,
  GateRequiresApproval,
  TaintLaunderingRisk,
  SelfModificationForbidden,
  SchemaRequiresForbiddenField,
  BudgetExceeded,
  VerificationFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintType {
  Policy,
  AudienceScope,
  Gate,
  Taint,
  Budget,
  Schema,
  Verification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DenyComputed {
  pub risk_r: Option<u8>,
  pub sensitivity_s: Option<u8>,
  pub max_gate: Option<u8>,
  pub audience_ceiling_s: Option<u8>,
  pub taint_s: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Violation {
  pub constraint_id: String,
  pub constraint_type: ConstraintType,
  pub message: String,
  pub triggering_fields: Option<Vec<String>>,
  pub triggering_refs: Option<Vec<String>>,
  pub computed: Option<DenyComputed>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetryPolicy {
  pub allowed: bool,
  pub conditions: Vec<String>,
  pub cooldown_ms: Option<u32>,
  pub max_attempts: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemediationType {
  AskUser,
  Sanitize,
  AlternateActuator,
  RequireApproval,
  RequireOob,
  Refuse,
  ReduceScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemediationOption {
  pub r#type: RemediationType,
  pub description: String,
  pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remediation {
  pub options: Vec<RemediationOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyscallDeny {
  pub syscall_id: String,
  pub operation_id: String,
  pub isolation_id: String,
  pub denied_at: DateTime<Utc>,
  pub deny_class: DenyClass,
  pub violations: Vec<Violation>,
  pub retry_policy: RetryPolicy,
  pub remediation: Remediation,
  pub audit_trace_id: String,
}

impl Validate for SyscallDeny {
  fn validate(&self) -> Result<(), ContractError> {
    if self.violations.is_empty() {
      return Err(ContractError::Invalid{ field:"violations", msg:"must not be empty".into()});
    }
    Ok(())
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
  Summary,
  Draft,
  Table,
  ExtractedFields,
  SanitizedView,
  Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudienceScopeTag {
  pub allowed_scopes: Vec<String>,
  pub max_disclosure_s: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IpcRules {
  pub receiver_inherits_s: Option<u8>,
  pub requires_recompile: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IPCArtifact {
  pub artifact_id: String,
  pub produced_by_operation_id: String,
  pub produced_at: DateTime<Utc>,
  pub kind: ArtifactKind,
  pub content_ref: String,
  pub sensitivity_s: u8,
  pub taint_s: u8,
  pub provenance_refs: Vec<String>,
  pub audience_scope_tag: AudienceScopeTag,
  pub ipc_rules: Option<IpcRules>,
  pub audit_trace_id: Option<String>,
}

impl Validate for IPCArtifact {
  fn validate(&self) -> Result<(), ContractError> {
    if self.sensitivity_s > 4 { return Err(ContractError::Invalid{ field:"sensitivity_s", msg:"0..4".into()}); }
    if self.taint_s > 4 { return Err(ContractError::Invalid{ field:"taint_s", msg:"0..4".into()}); }
    if self.provenance_refs.is_empty() { return Err(ContractError::Invalid{ field:"provenance_refs", msg:"must not be empty".into()}); }
    Ok(())
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineEventType {
  OperationStateChange,
  GateDecision,
  CompiledSlice,
  ReasoningOutput,
  VerificationPass,
  VerificationFail,
  SyscallProposed,
  SyscallPermitted,
  SyscallDenied,
  SyscallExecuted,
  IpcEmit,
  IpcReceive,
  ApprovalRequested,
  ApprovalGranted,
  ApprovalDenied,
  OobChallengeRequested,
  OobChallengeVerified,
  SanitizationApplied,
  OmissionsRecorded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditSummaryGate {
  pub risk_r: u8,
  pub sensitivity_s: u8,
  pub max_gate: u8,
  pub approval_mode: ApprovalMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditSummaryAudience {
  pub requesting_audience_id: String,
  pub sensitivity_ceiling_s: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationResult {
  Completed,
  Failed,
  Cancelled,
  Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditSummary {
  pub gate: AuditSummaryGate,
  pub audience: AuditSummaryAudience,
  pub result: OperationResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditTimelineItem {
  pub ts: DateTime<Utc>,
  pub event_type: TimelineEventType,
  pub ref_id: Option<String>,
  pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditPinned {
  pub active_state_version: String,
  pub capability_snapshot_version: String,
  pub audience_graph_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditAttachments {
  pub gate_decision_ref: Option<String>,
  pub compiled_slice_ref: Option<String>,
  pub syscall_refs: Option<Vec<String>>,
  pub ipc_artifact_refs: Option<Vec<String>>,
  pub experience_log_refs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditTrace {
  pub audit_trace_id: String,
  pub created_at: DateTime<Utc>,
  pub request_id: String,
  pub operation_id: String,
  pub isolation_id: String,
  pub pinned: AuditPinned,
  pub timeline: Vec<AuditTimelineItem>,
  pub summary: AuditSummary,
  pub attachments: Option<AuditAttachments>,
}
```
