# Storage Semantics and Transaction Boundaries Spec v0.1
Adesh OS

This document defines the **storage-level correctness rules** for Adesh OS. It is the authoritative specification for:
- versioning and pinning semantics
- idempotency behavior
- atomic transaction boundaries
- write ordering constraints for auditability
- replay determinism requirements
- concurrency and isolation requirements for operations and approvals

This is not DDL and not implementation code. It is the logic contract the StorageProvider and all backends must satisfy.

---

## 0) Definitions

### Storage domains
Adesh OS persists the following logical domains:
1. **Experience Log** (append-only)
2. **Operations** (mutable current row) + **Operation Transitions** (append-only)
3. **GateDecision** (immutable record)
4. **CompiledSlice** (immutable record)
5. **SyscallEnvelope** (immutable record per attempt; status transitions allowed by append or controlled update)
6. **SyscallDeny** (immutable record)
7. **IPCArtifact** (immutable record)
8. **AuditTrace** (append-only timeline with a mutable “latest snapshot” option)
9. **Active State** (versioned)
10. **Audience Graph** (versioned)
11. **Review Queue** (mutable state with append-only decisions)
12. **Idempotency keys** (dedupe table)
13. **Jobs queue** (lease-based)

### Strong correctness requirement
Adesh OS is a governed OS. It must be possible to answer:
- what happened
- why it happened
- what it touched
- under what policy and state versions

Therefore:
- audit-critical writes never fail open
- side effects never happen without durable pre-image records

---

## 1) Identity and reference invariants

### 1.1 Stable identifiers
The following IDs must be globally unique:
- `request_id`
- `operation_id`
- `isolation_id`
- `audit_trace_id`
- `syscall_id`
- `artifact_id`
- `event_ref`
- `state_version`
- `graph_version`
- `capability_snapshot_version`
- `idempotency_key` (per endpoint scope)

### 1.2 References must be resolvable
If any stored object contains:
- `event_ref`, `content_ref`, `syscall_id`, `artifact_id`, `audit_trace_id`
then dereferencing it must succeed or the system must treat it as storage corruption.

---

## 2) Versioning semantics (Active State, Audience Graph, Capability Snapshot)

### 2.1 Version immutability
Once created, versions are immutable:
- `active_state_version` refers to an immutable snapshot or a transaction-identified state.
- `audience_graph_version` refers to an immutable graph state.
- `capability_snapshot_version` refers to an immutable set of enabled tools and descriptors.

### 2.2 Pinning point
An operation pins versions at the earliest point where it becomes a first-class unit:
- during operation creation (or immediately after)
- before GateDecision and compilation

Pinned versions must be persisted into the `operations` record and must never change for that operation.

### 2.3 State updates and conflicts
Active State updates are committed using:
- `base_version` -> `new_version`
Rules:
- If `base_version` is not current and the mutation is non-mergeable, return `Conflict`.
- Merge policies must be explicit. Default is strict conflict.

### 2.4 Replay requirement
Replay must be able to load:
- the pinned versions for the operation
- the GateDecision, CompiledSlice, and reasoning output used
Therefore, versions referenced in any AuditTrace must be retained at least for the retention period.

---

## 3) Experience Log semantics (append-only)

### 3.1 Append-only guarantee
Experience Log is strictly append-only:
- no update
- no delete (except under explicit R4 deletion with out-of-band auth, which must itself be logged)

### 3.2 Event shape
Every event must include:
- `event_ref`
- `created_at`
- `kind`
- `source_class`
- `audience_id` (Root Owner for control plane events)
- `sensitivity_s` and `taint_s`
- `json_payload` or `content_ref` (or both)

### 3.3 Audit never fails open
If an event is required for auditability (request accepted, approval, syscall executed), failure to append it must abort the operation and prevent side effects.

---

## 4) Idempotency semantics

### 4.1 Scope of idempotency
Idempotency must be supported for:
- `POST /v1/requests`
- `POST /v1/approvals/{approval_id}`
- `POST /v1/approvals/{approval_id}/oob/start`
- `POST /v1/approvals/{approval_id}/oob/verify`
- any endpoint that triggers execution or state mutation

### 4.2 Idempotency key table semantics
A stored idempotent response must satisfy:
- key uniqueness: `idempotency_key` is unique within endpoint scope
- value: stored full HTTP response JSON
- metadata: `request_id`, `created_at`

### 4.3 Idempotency lookup rule
On receiving a request with `Idempotency-Key`:
1. lookup key in storage
2. if present: return stored response **without executing**
3. if absent: execute normally and store response before returning success

### 4.4 Storage atomicity for idempotency
Storing the idempotent response should occur in the **same transaction** that commits the externally visible effects of the endpoint, when feasible.

At minimum:
- never store a success response if core operation creation failed
- never execute external side effects unless idempotent response storage is guaranteed (or can be reconstructed deterministically)

### 4.5 Retention and eviction
Idempotency keys must have a retention policy:
- default retention: configurable (e.g., days)
- eviction must be safe: only evict keys older than retention and not referenced by any active operation.

---

## 5) Operation state machine persistence semantics

### 5.1 Dual representation
Operations have:
- a mutable current row (`operations.state`)
- an append-only transition log (`operation_transitions`)

Correctness rule:
- every change in `operations.state` must have a corresponding transition record
- transitions are the canonical audit history; `operations` is the latest snapshot

### 5.2 Ordering constraints
The following ordering must be enforced:

1. `operations` row exists before any:
   - GateDecision
   - CompiledSlice
   - syscalls
   - approvals

2. Transition to `awaiting_approval` must be persisted before emitting WS `approval_required`.

3. Transition to `running` must be persisted before emitting WS `running`.

### 5.3 Concurrency control
Only one scheduler worker may advance a given operation at a time.

Implementation must provide one of:
- DB advisory lock keyed by `operation_id`
- row-level lock with lease semantics
- single-threaded operation runner

Violations cause:
- duplicated syscalls
- duplicate approvals
- non-deterministic audit

---

## 6) Syscall persistence and execution ordering (critical)

### 6.1 Pre-image rule (no side effects without a record)
A syscall with external side effects must satisfy:

**Rule A:** `SyscallEnvelope` must be persisted with status `permitted` (or `awaiting_approval` earlier) **before** execution begins.

**Rule B:** `SyscallEnvelope.status` must be updated to `executed|failed` only after execution completes.

**Rule C:** `SyscallDeny` must be persisted before returning a denial/remediation to the user.

### 6.2 Atomicity boundary for execution
The system must preserve a “recoverable state” if it crashes mid-execution:

Minimum requirement:
- The persisted SyscallEnvelope includes enough information to:
  - detect it was in-progress
  - either retry safely or mark as failed with remediation

Preferred requirement:
- Use an execution lease:
  - set status `executing` (optional) with `leased_until`
  - worker heartbeats
  - on crash, another worker can resume or fail deterministically

### 6.3 Idempotency at syscall layer
For actuators that support idempotency (email send with message-id, etc.):
- ToolProvider should pass an idempotency token derived from `syscall_id`.

If the actuator does not support idempotency:
- the OS must treat retries as high-risk and require explicit user approval for reattempt beyond the configured retry count.

---

## 7) Approvals and OOB: transaction boundaries

### 7.1 Approval consumption must be atomic
When approving an operation step that triggers syscalls:

In one transaction (or equivalent atomic unit), the system must:
1. validate the approval request
2. validate `modified_payload` (if any)
3. if OOB required:
   - verify the challenge is `verified`, bound to this operation, not expired
   - **consume** the challenge (mark used)
4. record an approval event in Experience Log
5. record approval entry in AuditTrace timeline
6. update operation state from `awaiting_approval` -> `running` (or next state)
7. persist the SyscallEnvelope(s) that will execute next with status `permitted`

Only after this transaction commits may execution begin.

Rationale:
- prevents TOCTOU and replay of OOB
- ensures auditability before side effects

### 7.2 OOB challenge lifecycle persistence
OOB records must be stored with:
- `operation_id`
- `challenge_id`
- `nonce`
- `status` (pending/verified/consumed/expired)
- `expires_at`
- `verified_at`
- `consumed_at`

Rules:
- verifying does not elevate OwnerSession
- consumption is single-use
- expired challenges cannot be consumed

### 7.3 Denial behavior
If approval fails validation:
- do not consume OOB
- do not transition operation
- persist a denial event and return a structured error

---

## 8) IPC artifacts and sensitivity inheritance

### 8.1 IPC artifact write ordering
When producing an IPC artifact:
1. persist the artifact content to BlobStore
2. persist `IPCArtifact` referencing `content_ref`
3. append an Experience Log event for IPC emission
4. add AuditTrace timeline entry

Only after persistence may the receiver operation be notified.

### 8.2 Sensitivity inheritance rule
When an operation consumes an IPC artifact:
- its computed sensitivity must be at least the artifact sensitivity
- its compilation must treat the artifact as a sensitivity source

This must be enforced by governance and compiler. Storage must preserve the sensitivity labels.

---

## 9) AuditTrace persistence semantics

### 9.1 Two acceptable models
Implementations may choose:

**Model A: append-only timeline entries**  
- store each timeline item as an event
- reconstruct AuditTrace snapshot on read

**Model B: snapshot + append**  
- store a full AuditTrace JSON snapshot
- append timeline items and periodically compact into snapshot

Both must preserve:
- chronological timeline
- references to gate decisions, compiled slices, syscalls, denies, artifacts, approvals

### 9.2 Required audit anchors
For each operation, AuditTrace must reference:
- pinned versions
- GateDecision ref
- CompiledSlice ref
- reasoning output ref
- approvals and OOB refs (if any)
- syscalls and denies
- IPC artifacts

If any required anchor cannot be written, operation must fail closed.

---

## 10) Job queue semantics (reflection loop)

### 10.1 Lease semantics
JobQueue must implement:
- enqueue
- lease with `leased_until` and `lease_owner`
- ack and fail with retry schedule

At-least-once delivery is required.

### 10.2 Interaction with pinned versions
Reflection jobs must never mutate the pinned state of in-flight operations.
They create new versions for future operations only.

---

## 11) Required transactional boundaries (summary table)

The coding agent must implement these atomic units:

### T1: Request acceptance
- append request event
- create operation(s) + initial transition(s)
- create initial audit trace(s)
- optionally store idempotent response placeholder
Must commit before returning success.

### T2: GateDecision persistence
- store gate decision
- update audit trace timeline reference

### T3: CompiledSlice persistence
- store compiled slice
- update operation state to compiled
- update audit trace timeline reference

### T4: Approval consumption (with OOB if required)
- validate approval
- consume OOB challenge (if any)
- append approval event
- update audit trace
- set operation state running
- persist SyscallEnvelope(s) as permitted

### T5: Syscall execution record update
- write execution start (optional)
- on completion:
  - persist tool result event/blob
  - update syscall status and result refs
  - update audit trace timeline

### T6: Syscall denial
- persist SyscallDeny
- update syscall status denied
- update audit trace timeline
Must commit before returning denial to UI.

---

## 12) Replay determinism requirements

To replay an operation deterministically, storage must retain:
- pinned versions
- GateDecision and CompiledSlice
- exact reasoning output JSON
- exact syscall args and outcomes (or denotes that actuator execution is skipped in dry_run)
- approvals including modified_payload
- OOB references (challenge ids, status transitions)
- all timeline references

Dry-run replay must not require any external tool calls.

---

## 13) Minimum test cases (must pass)

1. Crash after persisting SyscallEnvelope but before executing actuator:
   - system must not lose the syscall record
   - must either resume safely or surface as blocked with remediation

2. Approval with OOB:
   - verifying OOB does not elevate session
   - OOB cannot be reused for a different operation
   - OOB consumption is atomic with approval

3. Idempotency:
   - repeated POST /v1/requests with same key returns identical response and creates no extra operations
   - repeated approval POST with same key does not execute actuator twice

4. Audit fail-closed:
   - if audit trace write fails, syscall must not execute

5. Operation isolation:
   - receiver operation cannot access producer artifacts unless IPCArtifact is persisted and referenced
