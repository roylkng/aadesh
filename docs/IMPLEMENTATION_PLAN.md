# Adesh OS Implementation Plan

## Inputs and target stack
This implementation plan is derived from the reading order established in `index.md` and the current canonical specs in this repository.

**Target stack:** Rust, Tokio, Axum, with a pluggable provider architecture. SQLite is implemented first as a reference `StorageProvider` backend, but core contracts and kernel behavior must remain backend-agnostic. Audit-critical paths fail closed.

## A) Spec digestion summary

### System in your words
Adesh OS is a governed execution kernel for Root Owner requests. A request is decomposed into isolated operations, each operation pins the current `active_state_version`, `capability_snapshot_version`, and `audience_graph_version`, and governance computes `R`, `S`, and `max(R,S)` before any model reasoning or tool execution proceeds. The JIT compiler constructs a minimal taint-aware slice, the model emits structured intents only, verification enforces trajectory/schema/audience/taint rules, and high-risk or disclosure-expanding actions are parked for approval and single-use OOB when required.

The system is storage-first and replay-first. Durable records exist before notification or side effects. Cross-operation data movement is explicit via IPC artifacts only. Reflection runs asynchronously, writes only through governed version mints and review flows, and never mutates state pinned by in-flight operations.

### Core invariants
- HTTP and WS control plane are Root Owner only.
- External agents use the MCP Host plane, never the Root Owner HTTP control plane.
- All side effects occur only through persisted `SyscallEnvelope` records.
- `max_gate = max(R,S)` governs operation execution and syscall permissioning.
- Unknown audiences, scopes, or graph relationships are default deny.
- Operation isolation is strict; cross-operation transfer requires explicit `IPCArtifact`.
- No taint laundering into lower-sensitivity output without explicit sanitization and verification.
- Approval and OOB are bound to `approval_id`, not `operation_id`.
- OOB is single-use and approval-bound; it never elevates the session globally.
- Persist-before-emit is mandatory for state, approvals, denies, syscalls, and audit updates.
- Missing anchors, hash mismatches, or audit-critical persistence failures fail closed.
- Operations pin `active_state_version`, `capability_snapshot_version`, and `audience_graph_version` for deterministic replay.
- Capability snapshots and schemas are immutable and content-addressed.
- Reflection mints new versions for future work only; pinned in-flight state is immutable.

### Dependency graph of components
```text
Root Owner HTTP Gateway
  -> StorageProvider
  -> Scheduler
  -> WS emitter (after storage commit)

Scheduler
  -> Governance Kernel
  -> JIT Compiler
  -> ModelProvider
  -> Verification Core
  -> StorageProvider

Verification Core
  -> StorageProvider (denies, approvals, audit anchors)
  -> ToolProvider (only after permitted syscall persistence)

Approvals / OOB API
  -> StorageProvider.consume_approval_atomic

Replay API
  -> StorageProvider (anchors, pinned versions, schemas, capability snapshots)
  -> Verification Core
  -> ToolProvider (full replay only)

Reflection Worker
  -> JobQueue
  -> StorageProvider (new active state versions, review queue items)

ToolProvider
  -> BlobStore

MCP Host
  -> separate integration plane
  -> capability discovery and external-agent surface
```

## B) Spec alignment status

### Current status
The documentation set is now aligned for implementation. The Phase 0.5 repairs are incorporated:
- `control_plane_api_spec.md` is canonical and Root Owner only.
- `operation_decomposition_ipc.md` is now the actual decomposition and explicit IPC spec.
- `schema_registry_and_versioning.md` is canonical and content-addressed.
- `retention_and_data_lifecycle.md` and `threat_model_spec.md` are restored to their intended roles.
- Approval and OOB routes are canonicalized to `approval_id`.
- Batch contracts pin all three required versions.
- Cross-references to `test_and_kri.md` are repaired.

### Remaining blocking spec gaps
- None.

### Alignment notes closed by this pass
- `storage_schema.md` now includes durable backing for operation leases across both reference backends.
- `storage_schema.md` now includes `current_versions`, `capability_snapshots`, and `schema_registry_entries`, matching schema-registry and capability snapshot requirements.
- `storage_provider_port_contract.md` now includes read methods for pinned version snapshots and schema registry access, not only mint operations.
- Reference and summary docs (`reference/provider_interfaces_summary.md`, `reference/contract_summaries.md`, `reference/rust_contract_summaries.md`, `reference/implementation_backlog.md`) now reflect approval-bound OOB and three-version pinning.

## C) Database + storage mapping

### C1. Method-to-schema mapping
- `append_event`, `get_event` -> `experience_events`
- `get_idempotent_response`, `put_idempotent_response` -> `idempotency_keys`
- `create_operation_bundle` -> `operations`, `operation_transitions`, `audit_traces`
- `try_acquire_operation_lease`, `renew_operation_lease`, `release_operation_lease` -> `operation_leases`
- `put_gate_decision`, `get_gate_decision` -> `gate_decisions`
- `put_compiled_slice`, `get_compiled_slice` -> `compiled_slices`
- `put_reasoning_output` -> `experience_events` plus `blob_objects` when payloads are externalized
- `put_syscall_envelope`, `update_syscall_status`, `list_syscalls_by_operation` -> `syscalls`
- `put_syscall_deny` -> `syscall_denies`
- `create_approval_item`, `list_pending_approvals`, `get_approval_item`, `consume_approval_atomic` -> `approval_items`, `approval_item_syscalls`, `oob_challenges`, `syscalls`, `operation_transitions`
- `put_ipc_artifact`, `get_ipc_artifact` -> `ipc_artifacts`, `blob_objects`
- `put_audit_trace`, `append_audit_timeline_item`, `get_audit_trace` -> `audit_traces`
- `get_current_versions` -> `current_versions`
- `get_active_state_snapshot`, `mint_active_state_version` -> `active_state_versions`
- `get_audience_graph_snapshot`, `mint_audience_graph_version` -> `audience_graph_nodes`, `audience_graph_edges`, `audience_graph_scopes`, `current_versions`
- `get_capability_snapshot`, `mint_capability_snapshot_version` -> `capability_snapshots`, `current_versions`
- `register_schema_entry`, `get_schema_entry`, `find_schema_entry` -> `schema_registry_entries`
- `create_review_item`, `list_review_items`, `decide_review_item_atomic` -> `review_queue_items`, `review_queue_decisions`
- reflection work leasing -> `jobs`
- externalized payload metadata -> `blob_objects`

### C2. Required tables and indices by concern
#### Idempotency
- Table: `idempotency_keys`
- Primary key: `(endpoint_scope, idempotency_key)`
- Required fields: request id, stored response JSON, response hash, created/expiry timestamps
- Required index: `expires_at`

#### Operation leases
- Table: `operation_leases`
- Primary key: `operation_id`
- Required fields: `lease_owner`, `leased_until`, `lease_epoch`, `last_heartbeat_at`, `updated_at`
- Required indices: `leased_until`, `(lease_owner, leased_until)`

#### Approvals and OOB
- Tables: `approval_items`, `approval_item_syscalls`, `oob_challenges`
- Primary key: `approval_id` for approvals, `challenge_id` for OOB
- Required indices: `approval_items(operation_id, status)`, `approval_items(expires_at)`, `oob_challenges(approval_id, status)`, `oob_challenges(expires_at)`

#### Pinned versions and schema integrity
- Tables: `current_versions`, `active_state_versions`, `capability_snapshots`, `schema_registry_entries`
- Required indices:
  - `active_state_versions(created_at)`, `active_state_versions(parent_version)`
  - `capability_snapshots(created_at)`, `capability_snapshots(parent_version)`
  - `schema_registry_entries(name, semver)` unique
  - `schema_registry_entries(content_hash)`

#### Replay anchors and audit
- Tables: `audit_traces`, `gate_decisions`, `compiled_slices`, `syscalls`, `syscall_denies`, `ipc_artifacts`, `approval_items`, `oob_challenges`, `experience_events`
- Required indices: `audit_traces(operation_id)`, operation-time indexes for gate decisions, compiled slices, syscalls, and IPC artifacts

### C3. Atomic units from `storage_semantics_txn.md`
#### T1 Request acceptance
- Insert request event
- Insert operation rows
- Insert initial transition rows
- Insert audit trace skeletons
- Insert idempotency response placeholder or final stored response record
- Commit before returning success

#### T2 GateDecision persistence
- Insert immutable `gate_decisions` row
- Append audit timeline anchor
- Commit before downstream processing

#### T3 CompiledSlice persistence
- Insert immutable `compiled_slices` row
- Update operation state and append transition row atomically
- Append audit timeline anchor

#### T4 Approval consumption
- Lock approval record and validate pending state
- Validate challenge state for approval-bound OOB when required
- Consume OOB exactly once
- Mark approval resolved
- Persist permitted syscall envelopes
- Transition operation out of `awaiting_approval`
- Append approval event and audit anchor
- No tool execution occurs inside this transaction

#### T5 Syscall execution result persistence
- Persist result artifact metadata and/or event records
- Update syscall status and result refs atomically
- Append audit timeline anchor

#### T6 Syscall denial persistence
- Persist `syscall_denies`
- Update syscall status to denied
- Append audit timeline anchor

## D) Vertical slice milestones

### Milestone 1: Kernel skeleton (no real tools, stub model)
Acceptance criteria:
- HTTP and WS boot on localhost only.
- Root Owner auth is enforced for HTTP and WS.
- `POST /v1/requests` creates operations, transitions, and audit skeletons atomically.
- Idempotency works for `POST /v1/requests`.
- Operation leases prevent concurrent advancement by multiple runners.
- Audit persistence failure fails closed and blocks request creation.

### Milestone 2: Governance + Compiler + Verification loop (stub model output)
Acceptance criteria:
- `GateDecision` and `CompiledSlice` persist immutably.
- Verification enforces `max(R,S)`, audience default deny, taint laundering denials, and anti-retry behavior.
- Approval parking persists `approval_items` without execution.
- WS emits state and audit updates only after persistence.

### Milestone 3: Approvals + OOB atomic consumption
Acceptance criteria:
- Approval endpoints are approval-item scoped and support diff edits.
- OOB lifecycle is bound to `approval_id` and single-use.
- Approval consumption creates permitted syscall envelopes but does not execute them.
- Stale or superseded approvals fail deterministically with conflict semantics.

### Milestone 4: Tool execution + replay + reflection
Acceptance criteria:
- ToolProvider executes a minimal fake sensor and fake actuator.
- Syscall pre-image exists before execution and result persistence follows execution.
- Replay `dry_run` works from stored anchors and fails closed on missing anchors.
- Reflection enqueues jobs, mints new active state versions, and writes review items without dangerous auto-application.

## E) Test plan mapping

### Milestone 1
- Unit:
  - `idempotency_store_returns_identical_response_for_same_key`
  - `operation_lease_compare_and_set_exclusive`
  - `request_txn_rolls_back_when_audit_write_fails`
- Integration:
  - `post_requests_idempotent_no_duplicate_operation`
  - `two_runners_one_operation_only_one_lease`
  - `ws_events_emitted_only_after_persisted_transition`
- Specs validated:
  - `storage_semantics_txn.md`
  - `scheduler_concurrency.md`
  - `error_remediation.md`
  - `test_and_kri.md`

### Milestone 2
- Unit:
  - `gate_computation_uses_max_r_s`
  - `verification_unknown_audience_default_deny`
  - `verification_taint_laundering_deny_requires_sanitize`
  - `anti_retry_returns_same_deny_then_blocks`
- Integration:
  - `operation_persists_gate_and_compiled_slice_before_running`
  - `awaiting_approval_state_persisted_before_ws_approval_required`
- Specs validated:
  - `governance_kernel_logic.md`
  - `jit_compiler.md`
  - `verification_core_ruleset.md`
  - `storage_semantics_txn.md`
  - `test_and_kri.md`

### Milestone 3
- Unit:
  - `oob_challenge_single_use_atomic_consume`
  - `approval_modified_payload_regates_and_escalates`
  - `approval_idempotency_no_double_consume`
- Integration:
  - `approval_flow_toc_tou_prevented`
  - `stale_or_superseded_approval_returns_conflict`
  - `consume_approval_atomic_persists_permitted_syscalls_without_execution`
- Specs validated:
  - `approval_oob_spec.md`
  - `storage_semantics_txn.md`
  - `error_remediation.md`
  - `test_and_kri.md`

### Milestone 4
- Unit:
  - `tool_execution_rejects_missing_syscall_preimage`
  - `replay_dry_run_never_executes_actuator`
  - `reflection_mints_new_version_not_mutate_pinned`
- Integration:
  - `syscall_preimage_then_execution_result_persist_order`
  - `replay_missing_anchor_fail_closed`
  - `reflection_review_items_created_no_auto_dangerous_write`
- Specs validated:
  - `replay_and_deterministic_re_execution.md`
  - `reflection_and_persona.md`
  - `scheduler_concurrency.md`
  - `storage_semantics_txn.md`
  - `test_and_kri.md`

## Implementation readiness
The spec set is now in a state where implementation can begin deterministically after this documentation PR is reviewed and merged. The first code PR should target Milestone 1 only.
