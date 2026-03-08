```md id="v4n8p1"
# StorageProvider Port Contract Spec v0.1
Adesh OS

This document defines the **StorageProvider port contract**. It is the authoritative method-level specification for any storage backend (SQLite, Postgres, etc.). It complements:
- `storage_semantics_txn.md` (correctness rules)
- API specs (HTTP v0.3)
- Batch contracts (1–3)

The coding agent must implement StorageProvider exactly to this contract:
- method signatures (conceptually)
- required atomic operations
- error taxonomy mapping
- idempotency behavior
- concurrency and lease semantics
- retention hooks

This is interface and logic documentation. Not implementation code.

---

## 0) Core principles

1. **Fail closed on audit-critical writes**
If any required write fails, the caller must treat it as a hard failure and prevent side effects.

2. **Atomic units are first-class**
StorageProvider must offer operations that allow atomic commit of:
- approval consumption + syscall permit persistence
- OOB consume + approval decision
- operation creation + initial audit anchors
- syscall pre-image + execution status transitions

3. **Deterministic reads**
Reads must return the latest committed data and allow reconstruction of state transitions.

---

## 1) Error model

StorageProvider methods return either:
- success result
- `StorageError` mapped to canonical error classes

### 1.1 StorageError classes
- `NotFound(resource)`
- `Conflict(resource, reason)`
- `Unauthorized(reason)` (rare in storage layer, usually caller-level)
- `InvalidInput(reason)`
- `Io(reason)`
- `Db(reason)`
- `Transient(reason)` (optional)
- `Corruption(reason)` (hash mismatch, missing anchor)

### 1.2 Mapping to REST codes
- NotFound -> `NOT_FOUND`
- Conflict -> `CONFLICT`
- InvalidInput -> `INVALID_INPUT`
- Unauthorized -> `FORBIDDEN` or `UNAUTHORIZED` at caller layer
- Io/Db/Transient -> `TRANSIENT` or `PERMANENT` based on retryability
- Corruption -> `PERMANENT` (fail closed)

---

## 2) Required entity groups (logical tables)

StorageProvider must persist and fetch:

### 2.1 Experience Log
- append-only events
- event metadata includes S/T labels and provenance

### 2.2 Operations + Transitions + Leases
- operations latest state
- transitions append-only
- lease metadata for scheduler

### 2.3 GateDecision, CompiledSlice
- immutable per operation attempt (keyed by operation_id + version or unique id)

### 2.4 Syscalls + Denies
- syscall envelope persisted before execution
- deny persisted before returning

### 2.5 Approvals + OOB Challenges
- approval items pending/resolved
- OOB challenge lifecycle

### 2.6 IPCArtifacts
- immutable artifacts with scope tags and S/T labels

### 2.7 AuditTrace
- audit anchors and timeline entries

### 2.8 Versions
- active_state versions (immutable)
- audience_graph versions (immutable)
- capability_snapshot versions (immutable)

### 2.9 Review Queue
- review items pending/resolved, linked to version mints

### 2.10 Idempotency keys
- stored responses for mutation endpoints

---

## 3) Method-level contract (conceptual interface)

The following methods are mandatory. Names may differ but semantics must match.

---

## 4) Experience Log methods

### 4.1 append_event
**Purpose:** Append immutable event record.

Inputs:
- `event_ref` (unique)
- `created_at`
- `kind`, `source_class`
- `audience_id` (Root Owner for control plane)
- `sensitivity_s`, `taint_s`
- `content_ref` optional
- `json_payload` (structured)
- `idempotency_key` optional for correlation

Semantics:
- append-only, no overwrite
- fail if event_ref already exists (Conflict) unless event_ref is deterministically reused under idempotency

Errors:
- Conflict if duplicate without idempotency semantics
- InvalidInput if payload too large or invalid
- Db/Io for persistence issues

### 4.2 get_event
Fetch JSON payload by event_ref. Must include metadata if requested.

---

## 5) Idempotency methods

### 5.1 get_idempotent_response
Inputs:
- idempotency_key
Output:
- optional stored response JSON

### 5.2 put_idempotent_response
Inputs:
- idempotency_key, request_id, response_json
Semantics:
- must be atomic and unique per key
- if key already exists:
  - either return Conflict
  - or return success only if the stored response matches exactly (recommended)

Retention:
- storage must support eviction policy hooks (Section 13).

---

## 6) Operation lifecycle methods

### 6.1 create_operation_bundle (atomic)
**Purpose:** Create operation and initial anchors in one atomic unit.

Inputs:
- `OperationSpec`
- initial transitions (created)
- initial `AuditTrace` skeleton or audit anchor record
- optional idempotency key

Semantics:
- all-or-nothing recommended
- persists operation row, transitions, audit trace, and links

If not implemented as a single method:
- the caller must be able to run these writes in a transaction via `run_txn()` facility.

### 6.2 update_operation_state
Inputs:
- operation_id
- new_state
- reason optional
Semantics:
- must write transition record and update latest state atomically

### 6.3 get_operation
Returns latest OperationSpec snapshot (including pinned versions and lifecycle)

### 6.4 list_operations
Filter by state, created_at, etc.

---

## 7) Operation leasing (scheduler concurrency)

### 7.1 try_acquire_operation_lease
Inputs:
- operation_id
- runner_id
- lease_duration_ms
Output:
- acquired true/false
- lease_epoch optional

Semantics:
- atomic compare-and-set:
  - acquire only if expired or owned by same runner
- must prevent two runners from acquiring simultaneously

### 7.2 renew_operation_lease
Inputs:
- operation_id, runner_id, lease_epoch, lease_duration_ms
Semantics:
- extend leased_until only if still owned by runner and epoch matches

### 7.3 release_operation_lease
Optional explicit release. Expiry-based release acceptable.

---

## 8) GateDecision and CompiledSlice methods

### 8.1 put_gate_decision
Inputs:
- `GateDecision`
Semantics:
- immutable insert
- may allow replacement only if identical hash and same pinned versions, else Conflict

### 8.2 get_gate_decision
Fetch by operation_id (or gate_decision_id)

### 8.3 put_compiled_slice
Inputs:
- `CompiledSlice`
Semantics:
- immutable insert
- must store taint, omissions, provenance summary
- link to audit trace

### 8.4 get_compiled_slice
Fetch by operation_id (or compiled_slice_id)

---

## 9) Reasoning output persistence

### 9.1 put_reasoning_output
Inputs:
- operation_id, isolation_id
- content_ref or event_ref
- structured ReasoningOutput JSON
- S/T labels

Semantics:
- persisted final output must be retrievable for replay and audit
- may be stored as event payload or blob + metadata

---

## 10) Syscalls and denies

### 10.1 put_syscall_envelope (pre-image)
Inputs:
- `SyscallEnvelope` with status `permitted` or `awaiting_approval` or `proposed`
Semantics:
- must exist before execution begins
- immutable base record is recommended; status updates may be separate rows or controlled updates

### 10.2 update_syscall_status
Inputs:
- syscall_id
- new_status
- result fields optional (output_ref, timings, ok, error_code)
Semantics:
- must be atomic with result refs persistence linkage

### 10.3 put_syscall_deny
Inputs:
- `SyscallDeny`
Semantics:
- must be persisted before returning denial to UI
- links to syscall_id and operation_id

### 10.4 list_syscalls_by_operation
Used for audit and replay.

---

## 11) Approvals and OOB

### 11.1 create_approval_item
Inputs:
- ApprovalItem payload (logical; OS may store in its own schema)
Semantics:
- persistent pending record
- `approval_id` is the primary key for approval workflows
- must link to operation_id and syscall_ids

### 11.2 list_pending_approvals
Returns summaries and ids for UI.

### 11.3 get_approval_item
Returns full approval payload.

### 11.4 consume_approval_atomic (mandatory)
**Purpose:** Apply approval decision with atomic side effects.

Inputs:
- approval_id
- decision approve/deny
- optional modified_payload
- optional challenge_id (for OOB)
- list of syscall envelopes to mark permitted (on approve)
- operation state transition (awaiting_approval -> running or blocked)

Semantics (atomic, per `storage_semantics_txn.md`):
- validate approval pending
- validate OOB challenge verified + unconsumed if required
- consume OOB challenge (mark consumed)
- mark approval resolved/consumed
- append approval event and audit anchor references
- persist syscall envelopes as permitted if approving
- update operation state accordingly

If any part fails, none must commit.

### 11.5 OOB lifecycle methods
- `create_oob_challenge(approval_id, challenge_type, nonce, expires_at)`
- `mark_oob_verified(challenge_id, verified_at)`
- `consume_oob_challenge(challenge_id)` (normally inside consume_approval_atomic)
- `get_oob_challenge(challenge_id)`

All approval/OOB mutations must resolve via `approval_id`, not `operation_id`.

---

## 12) IPC artifacts and blob metadata

### 12.1 put_ipc_artifact
Inputs:
- `IPCArtifact`
Semantics:
- immutable insert
- must store S/T labels and scope tags

### 12.2 get_ipc_artifact
Fetch by artifact_id.

---

## 13) AuditTrace methods

### 13.1 put_audit_trace
Store initial audit trace.

### 13.2 append_audit_timeline_item
Append an event to timeline (append-only model) OR update snapshot with appended item.

Semantics:
- must preserve chronological order
- must include references to stored anchors

### 13.3 get_audit_trace
Fetch full trace.

---

## 14) Versioning methods

### 14.1 get_current_versions
Returns:
- current active_state_version
- current audience_graph_version
- current capability_snapshot_version

### 14.2 mint_active_state_version
Inputs:
- base_version
- change set (primitives added/updated/deprecated)
- provenance refs
Semantics:
- creates immutable new version, returns id

### 14.3 mint_audience_graph_version
Inputs:
- base_version
- patch set (nodes/edges/policies)
Semantics:
- immutable new version

### 14.4 mint_capability_snapshot_version
Inputs:
- base_version
- snapshot payload (tool descriptors + schema refs)
Semantics:
- immutable new version

---

## 15) Review queue methods

### 15.1 create_review_item
Inputs:
- ReviewItem payload
Semantics:
- persistent pending record

### 15.2 list_review_items
Filter by status.

### 15.3 decide_review_item_atomic
Inputs:
- item_id
- decision approve/reject/edit
- edited_payload optional
- optional OOB challenge
- resulting version mint (if approve/edit)
Semantics:
- atomic decision and version minting
- append experience event
- audit linkage

---

## 16) Retention hooks (optional in v0.1, must be spec’d)

StorageProvider should expose:
- `gc_idempotency_keys(older_than)`
- `gc_blobs_unreferenced(older_than)` (careful: reference tracking required)
- `compact_experience_log(policy)`

If not implemented, retention is a future feature but must not break correctness.

---

## 17) Deterministic integrity checks

StorageProvider must support integrity checks:
- verify schema_ref hash matches schema content
- verify blob checksum matches metadata (optional)
- detect missing anchors referenced by AuditTrace

Failure mode:
- return `Corruption` and fail closed.

---

## 18) Minimum acceptance tests (must pass)

1. Approval atomicity:
- simulate failure mid-consume_approval_atomic: no partial OOB consumption, no permitted syscalls.

2. Syscall pre-image:
- ensure syscall envelope exists before execution can be marked started.

3. Lease exclusivity:
- two runners contend; only one acquires.

4. Idempotency:
- same key -> same response, no duplicate operation creation.

5. Audit anchors:
- gate decision, compiled slice, reasoning output, syscalls, denies all retrievable by trace references.

```
