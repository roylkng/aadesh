# Provider Interfaces Summary v0.1

Non-authoritative reference summary. Canonical provider behavior is defined by the root-level port contract specs.

### Common conventions

* All methods accept and return **contract objects** (Batch 1–3) or stable references (`ref_id`, `content_ref`).
* All methods must support:

  * `request_id`, `operation_id`, `isolation_id`, `audit_trace_id` correlation where relevant
  * idempotency for writes via `idempotency_key` when applicable
* Error model:

  * `NotFound`
  * `Conflict` (version mismatch, pinned mismatch)
  * `Unauthorized`
  * `InvalidInput` (schema validation)
  * `PolicyDenied` (only for governance/verification, not storage)
  * `Transient` (retryable)
  * `Permanent`

---

# 1) StorageProvider

Owns: Experience Log, Active State (versioned), Audience Graph, hypotheses/review queue, Operations, AuditTrace persistence.

### Methods

**Experience Log**

* `append_event(event, idempotency_key) -> event_ref`
* `get_event(event_ref) -> event`
* `query_events(filter) -> [event_ref]`

**Active State (versioned)**

* `get_current_versions() -> {active_state_version, audience_graph_version, capability_snapshot_version}`
* `get_active_state_snapshot(state_version) -> active_state_snapshot`
* `mint_active_state_version(base_version, mutations, provenance_refs, idempotency_key) -> new_state_version`

  * must fail with `Conflict` if `base_version` is not current (unless mutation is explicitly mergeable)

**Audience Graph**

* `get_audience_graph_snapshot(graph_version) -> graph_snapshot`
* `apply_audience_graph_patch(base_version, patch, idempotency_key) -> new_graph_version`

**Capability snapshots + schema registry**

* `get_capability_snapshot(version) -> capability_snapshot`
* `mint_capability_snapshot_version(base_version, snapshot_payload, idempotency_key) -> new_snapshot_version`
* `register_schema_entry(schema_payload, schema_kind, name, semver) -> schema_ref`
* `get_schema_entry(schema_ref) -> schema_entry`
* `find_schema_entries(filter) -> [schema_entry]`

**Hypotheses + Review Queue**

* `list_review_items(filter) -> [review_item]`
* `get_review_item(id) -> review_item`
* `apply_review_decision(item_id, decision, idempotency_key) -> change_id`

**Operations**

* `create_operation(op_spec, idempotency_key) -> ok`
* `update_operation_state(operation_id, transition, idempotency_key) -> ok`
* `get_operation(operation_id) -> op_spec`

**Audit**

* `store_audit_trace(audit_trace, idempotency_key) -> audit_trace_id`
* `get_audit_trace(audit_trace_id) -> audit_trace`

### Invariants

* Append-only semantics for Experience Log.
* Versioned state commits are atomic.
* Graph edits and hypothesis promotions are versioned and auditable.

---

# 2) JobQueue

Owns: durable async reflection jobs.

### Methods

* `enqueue(job_type, payload, run_after, idempotency_key) -> job_id`
* `lease(worker_id, lease_ms, limit) -> [job]`
* `ack(job_id) -> ok`
* `fail(job_id, error, retry_at) -> ok`
* `heartbeat(job_id, lease_ms) -> ok` (optional)

### Invariants

* At-least-once delivery.
* Leases prevent two workers from processing simultaneously.

---

# 3) BlobStore

Owns: attachments, tool outputs, IPC artifacts payloads, diff payloads.

### Methods

* `put_bytes(bytes, metadata, idempotency_key) -> content_ref`
* `get_bytes(content_ref) -> bytes`
* `head(content_ref) -> metadata`
* `delete(content_ref)` (optional, usually gated)

### Metadata (required)

* `sensitivity_s` (0–4)
* `taint_s` (0–4)
* `provenance_refs[]`
* `content_type`
* `size_bytes`
* checksum

### Invariants

* content_ref is stable.
* metadata is immutable or versioned.

---

# 4) ModelProvider

Owns: model inference (Reasoning Core).

### Methods

* `generate(compiled_slice, runtime_hints) -> reasoning_output`

  * reasoning_output must be strict structured JSON: drafts + syscall proposals + IPC requests + optional plan steps
* `estimate_cost(compiled_slice, model_id) -> cost_estimate` (optional)
* `health()`

### Invariants

* Output must be schema-valid or returned as `InvalidOutput` with raw trace for audit.
* Must honor timeouts and token budgets in runtime_hints.

---

# 5) ToolProvider

Owns: executing permitted syscalls against sensors/actuators.

### Methods

* `execute_syscall(syscall_envelope) -> syscall_result_ref`

  * result content goes to BlobStore + Experience Log, returns `output_ref`
* `discover_tools() -> capability_snapshot` (optional, for MCP tool discovery)
* `health(tool_endpoint_ref?)`

### Invariants

* Must not execute if syscall status is not `permitted`.
* Must emit traces (start/end, errors) to Experience Log.

---

# 6) AuthProvider

Owns: Root Owner session + OOB verification.

### Methods

* `create_owner_session(auth_input) -> OwnerSession`
* `validate_owner_session(session_id) -> OwnerSession|Unauthorized`
* `start_oob_challenge(approval_id, challenge_type) -> {challenge_id, nonce}`
* `verify_oob_challenge(approval_id, challenge_id, response) -> OobVerificationReceipt`

### Invariants

* OOB verification must be bound to `approval_id` + nonce (anti-replay).

---

# 7) ObservabilityProvider

Owns: logs/metrics/traces.

### Methods

* `emit_event(name, fields)`
* `inc_counter(name, labels)`
* `observe_histogram(name, value, labels)`
* `start_span(name, fields) -> span_handle`
* `end_span(span_handle)`

### Invariants

* Always include correlation ids when available.

---

## Rust trait skeletons (v0.1)

Below is a Rust-oriented outline. Types like `RequestEnvelope`, `OperationSpec`, `GateDecision`, etc. correspond to your Batch schemas (you’ll define them as Rust structs and validate with serde + schema validation at API boundaries).

```rust
// Shared error model
#[derive(thiserror::Error, Debug)]
pub enum AdeshError {
  #[error("not found: {0}")]
  NotFound(String),
  #[error("conflict: {0}")]
  Conflict(String),
  #[error("unauthorized: {0}")]
  Unauthorized(String),
  #[error("invalid input: {0}")]
  InvalidInput(String),
  #[error("transient: {0}")]
  Transient(String),
  #[error("permanent: {0}")]
  Permanent(String),
}

// ---------- StorageProvider ----------
#[async_trait::async_trait]
pub trait StorageProvider: Send + Sync {
  async fn append_event(&self, event: ExperienceEvent, idempotency_key: Option<String>)
    -> Result<String, AdeshError>;

  async fn get_event(&self, event_ref: &str) -> Result<ExperienceEvent, AdeshError>;
  async fn query_events(&self, filter: EventQuery) -> Result<Vec<String>, AdeshError>;

  async fn get_current_versions(&self) -> Result<CurrentVersions, AdeshError>;
  async fn get_active_state_snapshot(&self, state_version: &str) -> Result<ActiveStateSnapshot, AdeshError>;

  async fn mint_active_state_version(
    &self,
    base_version: &str,
    mutations: Vec<StateMutation>,
    provenance_refs: Vec<String>,
    idempotency_key: Option<String>,
  ) -> Result<String, AdeshError>;

  async fn get_audience_graph_snapshot(&self, version: &str) -> Result<AudienceGraphSnapshot, AdeshError>;
  async fn apply_audience_graph_patch(
    &self,
    base_version: &str,
    patch: AudienceGraphPatch,
    idempotency_key: Option<String>,
  ) -> Result<String, AdeshError>;

  async fn get_capability_snapshot(&self, version: &str) -> Result<CapabilitySnapshot, AdeshError>;
  async fn mint_capability_snapshot_version(
    &self,
    base_version: &str,
    snapshot: CapabilitySnapshot,
    idempotency_key: Option<String>,
  ) -> Result<String, AdeshError>;
  async fn register_schema_entry(&self, entry: SchemaEntry) -> Result<String, AdeshError>;
  async fn get_schema_entry(&self, schema_ref: &str) -> Result<SchemaEntry, AdeshError>;
  async fn find_schema_entries(&self, filter: SchemaQuery) -> Result<Vec<SchemaEntry>, AdeshError>;

  async fn create_operation(&self, op: OperationSpec, idempotency_key: Option<String>) -> Result<(), AdeshError>;
  async fn update_operation_state(&self, op_id: &str, transition: OperationTransition, idempotency_key: Option<String>)
    -> Result<(), AdeshError>;
  async fn get_operation(&self, op_id: &str) -> Result<OperationSpec, AdeshError>;

  async fn store_audit_trace(&self, trace: AuditTrace, idempotency_key: Option<String>) -> Result<String, AdeshError>;
  async fn get_audit_trace(&self, audit_trace_id: &str) -> Result<AuditTrace, AdeshError>;

  async fn list_review_items(&self, filter: ReviewQuery) -> Result<Vec<ReviewItem>, AdeshError>;
  async fn apply_review_decision(&self, item_id: &str, decision: ReviewDecision, idempotency_key: Option<String>)
    -> Result<String, AdeshError>;
}

// ---------- JobQueue ----------
#[async_trait::async_trait]
pub trait JobQueue: Send + Sync {
  async fn enqueue(&self, job_type: &str, payload: serde_json::Value, run_after: Option<chrono::DateTime<chrono::Utc>>,
    idempotency_key: Option<String>) -> Result<String, AdeshError>;

  async fn lease(&self, worker_id: &str, lease_ms: u64, limit: u32) -> Result<Vec<Job>, AdeshError>;
  async fn ack(&self, job_id: &str) -> Result<(), AdeshError>;
  async fn fail(&self, job_id: &str, error: &str, retry_at: Option<chrono::DateTime<chrono::Utc>>) -> Result<(), AdeshError>;
}

// ---------- BlobStore ----------
#[async_trait::async_trait]
pub trait BlobStore: Send + Sync {
  async fn put_bytes(&self, bytes: bytes::Bytes, meta: BlobMeta, idempotency_key: Option<String>)
    -> Result<String, AdeshError>;
  async fn get_bytes(&self, content_ref: &str) -> Result<bytes::Bytes, AdeshError>;
  async fn head(&self, content_ref: &str) -> Result<BlobMeta, AdeshError>;
}

// ---------- ModelProvider ----------
#[async_trait::async_trait]
pub trait ModelProvider: Send + Sync {
  async fn generate(&self, slice: CompiledSlice, hints: ModelHints) -> Result<ReasoningOutput, AdeshError>;
  async fn health(&self) -> Result<(), AdeshError>;
}

// ---------- ToolProvider ----------
#[async_trait::async_trait]
pub trait ToolProvider: Send + Sync {
  async fn execute_syscall(&self, syscall: SyscallEnvelope) -> Result<SyscallExecutionResult, AdeshError>;
  async fn health(&self, endpoint_ref: Option<&str>) -> Result<(), AdeshError>;
}

// ---------- AuthProvider ----------
#[async_trait::async_trait]
pub trait AuthProvider: Send + Sync {
  async fn create_owner_session(&self, input: AuthInput) -> Result<OwnerSession, AdeshError>;
  async fn validate_owner_session(&self, session_id: &str) -> Result<OwnerSession, AdeshError>;
  async fn start_oob_challenge(&self, approval_id: &str, challenge_type: &str) -> Result<OobChallenge, AdeshError>;
  async fn verify_oob_challenge(&self, approval_id: &str, challenge_id: &str, response: OobResponse)
    -> Result<OobVerificationReceipt, AdeshError>;
}

// ---------- ObservabilityProvider ----------
pub trait ObservabilityProvider: Send + Sync {
  fn emit_event(&self, name: &str, fields: &[(&str, String)]);
  fn inc_counter(&self, name: &str, labels: &[(&str, &str)]);
  fn observe_histogram(&self, name: &str, value: f64, labels: &[(&str, &str)]);
}
```
