# Kernel Execution Loop (Sync Path) Spec v0.1
Adesh OS

This document specifies the **deterministic synchronous execution loop** for Adesh OS. It is the single authoritative source for how a request becomes one or more governed operations, how memory is compiled, how model output is verified, how approvals and OOB are handled, how syscalls execute, and how auditability is preserved.

This is not implementation code. It is precise logic and sequencing that the coding agent must implement.

---

## 0. Definitions and invariants

### Key objects (by contract)
- **Batch 1**: `RequestEnvelope`, `OperationSpec`, `OwnerSession`
- **Batch 2**: `GateDecision`, `CompiledSlice`
- **Batch 3**: `SyscallEnvelope`, `SyscallDeny`, `IPCArtifact`, `AuditTrace`

### Core invariants (must always hold)
1. **Control Plane Root Owner Only**  
   All HTTP/WS control plane actions require Root Owner session. Non-owner access is `403`. Audience scoping for external agents is handled via MCP Host, not HTTP.

2. **All side effects are syscalls**  
   Any sensor read, actuator action, IPC transfer, sanitization, or memory read is represented as a `SyscallEnvelope` and must pass governance + verification.

3. **max(R,S) gating**  
   Every operation and every syscall has computed `R` (Action Risk) and `S` (Data Sensitivity). Enforcement gate is `max_gate = max(R,S)`.

4. **Operation isolation**  
   Each operation has an `isolation_id`. No operation may implicitly access another operation’s working memory or artifacts. Data transfer requires explicit IPC via `IPCArtifact`.

5. **Taint-aware memory**  
   Working memory blocks carry taint `Sx`. Derived artifacts inherit max taint unless sanitized via explicit sanitization syscall.

6. **Audit never fails open**  
   Any failure to serialize or store audit-critical artifacts is a hard error. No silent `{}` or partial logs.

7. **Pinned versions for determinism**  
   Each operation pins:
   - `active_state_version`
   - `capability_snapshot_version`
   - `audience_graph_version`
   All compilation/verification decisions must reference pinned versions.

8. **Approval/OOB is operation-bound and single-use**  
   OOB challenges must be bound to an operation approval and consumed exactly once.

9. **Structured output required**  
   Model output must be machine-parseable to the required schema. Otherwise it is treated as invalid output.

---

## 1. Inputs, outputs, and actors

### Input to sync loop
- A validated `RequestEnvelope` received via `POST /v1/requests`.

### Output of sync loop
- A response containing:
  - `request_id`
  - `operation_ids`
  - `primary_operation_id`
  - `audit_trace_ids`

### Actors
- **Gateway**: validates request schema, enforces Root Owner, idempotency.
- **Scheduler**: decomposes request into operations, manages state machine.
- **Governance Kernel**: computes gates, scopes, policies; produces permit/deny decisions.
- **JIT Compiler**: compiles `CompiledSlice`.
- **ModelProvider**: performs reasoning call; supports token streaming.
- **Verification Core**: validates plan trajectory, tool calls, taint constraints.
- **ToolProvider**: executes permitted syscalls.
- **StorageProvider**: persists events, operations, state, audit.
- **EventBus (WS)**: streams operation state, approvals, reasoning chunks, denials.

---

## 2. Request acceptance and idempotency

### Step 2.1: Authenticate and authorize
- Require a valid Root Owner `OwnerSession`.
- If not Root Owner: return `403` and terminate.

### Step 2.2: Validate `RequestEnvelope`
- Validate JSON schema and contract semantics (`Validate`).
- If invalid: return `400 INVALID_INPUT` and terminate.

### Step 2.3: Idempotency (control plane)
- If `Idempotency-Key` header is present:
  1. Query `StorageProvider.get_idempotent_response(key)`.
  2. If found: return that exact prior response immediately. Do not re-execute.
  3. If not found: proceed and store final response at the end.
- If no key: proceed without idempotency cache.

### Step 2.4: Append request event to Experience Log
- Create `event_ref = event:<uuid>`.
- Serialize full `RequestEnvelope` to JSON. If serialization fails: return `500`.
- `StorageProvider.append_event(...)` with kind=`request`, source_class=`http`, audience_id = Root Owner node, sensitivity/taint = baseline S1.
- If append fails: return `500`. Terminate.

---

## 3. Operation decomposition and initialization

### Step 3.1: Decomposition contract
Scheduler must produce `OperationSpec[]` with:
- unique `operation_id` per operation
- unique `isolation_id` per operation
- `parent_request_id = RequestEnvelope.request_id`
- `requesting_audience_id = Root Owner`
- pinned versions (resolved at Step 3.2)
- budgets set (total + block budgets)

Constraints:
- Decomposition must not rely on a finite task taxonomy.
- Decomposition must consider mixed sensitivities and mixed action risks:
  - if sub-parts clearly differ in sensitivity or require distinct approvals, split into separate operations.
- If decomposition fails: create one fallback operation with the entire request.

### Step 3.2: Pin versions
For each operation:
- `active_state_version = StorageProvider.get_active_state_version()`
- `capability_snapshot_version = current capability registry snapshot id`
- `audience_graph_version = current audience graph version id`
Pin these into `OperationSpec.pinned_state` and store in the operation record.

If any version cannot be resolved: return `500` (kernel cannot operate without pinned versions).

### Step 3.3: Persist operations
For each `OperationSpec`:
- Validate `OperationSpec`.
- `StorageProvider.create_operation(op, idempotency_key?)`.
- Transition record to `operation_transitions`:
  - created state recorded with timestamp.
- Emit WS `operation_state` event: `created`.

If persistence fails for any operation:
- Mark failed for those operations; emit failure state.
- Return `500` (atomicity policy: request submission must not partially succeed silently).
Implementation may choose “all-or-nothing” creation by using a DB transaction.

### Step 3.4: Create initial AuditTrace(s)
For each operation:
- Create `audit_trace_id`.
- Initialize minimal `AuditTrace` containing:
  - pinned versions
  - initial timeline entries: request accepted, operation created
  - summary gate unknown yet (placeholder allowed) or set to conservative defaults (R1/S1) until computed.
- Persist via `StorageProvider.store_audit_trace`.
- Emit WS `audit_update`.

Failure to store audit trace is fatal: return `500`.

---

## 4. Per-operation synchronous execution loop

The scheduler processes operations in an ordered queue. Concurrency policy is implementation-specific, but correctness requires:
- each operation maintains isolation boundaries
- any parallel operations must not share working memory
- shared resources (DB, model provider) must enforce backpressure

For each operation `op`:

### Step 4.1: Transition to `compiled` stage intent
- Update operation lifecycle state to `compiled` only after successful compilation.
- Do not emit `compiled` before compilation succeeds.

### Step 4.2: Compute `GateDecision` (operation-level governance)
Governance Kernel inputs:
- `OperationSpec`
- pinned versions
- Audience Graph snapshot at `audience_graph_version`
- Capability snapshot at `capability_snapshot_version`
- Request attachments and referenced artifacts
- IPC consumed artifacts (if any) and their sensitivity labels

Governance Kernel must compute:
- `risk.level`:
  - derived from intent predicates, actuator risk floors if any action is required, and requested effects
- `sensitivity.level`:
  - derived from referenced attachments, evidence refs, IPC artifacts, tool results
- `max_gate = max(risk, sensitivity)`
- audience scope filtering:
  - Root Owner has global access but still bounded by internal negative memory and safety gates
- constraints:
  - negative memory lists
  - token budgets and block budgets
  - taint policy always enabled
  - intent_anchor_required true for gate >= 1 (recommended) and mandatory for gate >= 2

Persist:
- Store `GateDecision` as an object (DB table `gate_decisions`) and link to audit trace timeline.

Emit WS:
- `audit_update` referencing gate decision
- optional `operation_state` reason update (still not “compiled”)

If gate decision fails: mark operation failed, persist audit entry, emit `failed`.

### Step 4.3: Compile `CompiledSlice` (JIT compiler)
Inputs:
- `GateDecision`
- pinned versions
- Active State snapshot at `active_state_version`
- Capability snapshot
- Intent Anchor (from RequestEnvelope or derived)

Compiler output must include:
- 5 blocks: policy, capability, operation_context, evidence, scratch
- deterministic packing order
- per-block token budgets
- omissions list if truncated
- taint labels per block and operation max taint

Compiler rules:
1. Policy block is non-truncatable.
2. Block packing order is fixed:
   - policy -> capability -> operation_context -> evidence -> scratch
3. Evidence inclusion is gate-dependent:
   - higher gate excludes low-confidence hypotheses and untrusted sources.
4. Conflict resolution among primitives uses the deterministic algorithm:
   - applicability -> constraint severity -> explicit exception -> evidence tier -> specificity -> recency
5. Operation max taint equals max block taint.
6. If token budget overflow:
   - omit lowest priority content first, record omissions
   - never omit policy block
7. Sanitization requirement:
   - if operation taint exceeds audience ceiling for intended output class, set `sanitization_required_for_output = true`

Persist:
- Store `CompiledSlice` and link to audit trace.

Emit WS:
- `audit_update` referencing compiled slice
- `operation_state` to `compiled`

If compilation fails: mark operation failed.

### Step 4.4: Transition to `running`
- Update operation state to `running` and emit WS `operation_state`.
- Start reasoning phase.

---

## 5. Reasoning phase (model call) and token streaming

### Step 5.1: Model invocation contract
Input to ModelProvider:
- `CompiledSlice`
- runtime hints:
  - timeouts
  - max tokens
  - structured output schema requirement
  - optional prior `SyscallDeny` payloads if replanning is needed

Output required (structured):
- `draft_outputs[]` (text)
- `proposed_syscalls[]` (intent objects)
- `ipc_requests[]` (if needs explicit piping)
- optional `plan_steps[]` (for drift verification)

If ModelProvider returns invalid JSON or schema mismatch:
- attempt one constrained retry with stricter instruction
- if still invalid: fail operation with `verification_failed` and record audit

### Step 5.2: Token streaming
If streaming enabled:
- emit WS event `reasoning_stream_chunk` with:
  - `stream_id`
  - `channel` (draft|plan|explanation)
  - `seq`
  - `delta`
  - `is_final`
Streaming rules:
- Streaming is UI convenience.
- Final assembled text must be persisted as an Experience Log event and referenced in audit.

Failure handling:
- streaming failure must not crash operation. It degrades UI only.
- persistence failure is fatal.

### Step 5.3: Persist reasoning output (final)
- Store a final reasoning output artifact as a blob or experience event:
  - `kind = reasoning_output`
  - includes structured output JSON
- Link to audit trace timeline.

---

## 6. Verification phase (pre-execution)

Verification Core inputs:
- `GateDecision`
- `CompiledSlice`
- model structured output
- Intent Anchor
- Operation pinned versions
- Any IPC artifacts requested/produced so far

Verification must perform:

### 6.1: Plan trajectory alignment
- Validate that `plan_steps` and implied subgoals do not drift from Intent Anchor:
  - no new objectives introduced without confirmation
  - no scope expansion beyond scope_limits
  - no R/S escalation without required approval path
- If drift detected:
  - generate a structured verification failure
  - park operation as `blocked` requiring user clarification or approval
  - emit WS event with reason (do not proceed)

### 6.2: Syscall schema validation
For each proposed syscall intent:
- Validate against capability registry schema for target tool/action.
- Reject missing required fields or invalid types.
- If invalid:
  - generate `SyscallDeny` with `deny_class=verification_failed` and remediation `ask_user` or `alternate_actuator`
  - do not execute

### 6.3: Taint laundering prevention
For any proposed output or syscall that sends data to an audience:
- Determine output sensitivity and taint:
  - taint from inputs and compiled slice blocks
- If output would violate:
  - audience ceiling
  - taint policy without sanitization syscall
- Deny with `deny_class=taint_laundering_risk` and remediation:
  - `sanitize`
  - `reduce_scope`
  - `ask_user`

### 6.4: Gate enforcement and approval requirements
For each proposed syscall:
- Compute syscall-level gate using:
  - tool risk floor
  - data sensitivity used
  - universal predicates
- If approval required (confirm/diff/oob_required):
  - do not execute
  - park operation into `awaiting_approval`
  - produce diff payload if mode=diff
  - emit WS `approval_required`
  - persist pending approval state in storage

If verification passes for all required actions or yields only drafts:
- proceed to execution phase for permitted syscalls
- or complete operation if no syscalls required

---

## 7. Approval and OOB handling (synchronous integration)

When an operation is in `awaiting_approval`:

### Step 7.1: UI requests pending approvals
- UI calls `GET /v1/approvals/pending`.
- Server returns pending approval items with:
  - `approval_id`
  - `operation_id`
  - `approval_mode`
  - `prompt`
  - `diff` (if required)
  - `audit_trace_id`

### Step 7.2: OOB challenge (if required)
- UI calls `POST /v1/approvals/{approval_id}/oob/start`.
- Server issues challenge bound to `approval_id`, with expiry.
- UI verifies via `POST /v1/approvals/{approval_id}/oob/verify`.
- Verification creates a record: `(approval_id, challenge_id, status=verified, expires_at)`.

Rules:
- OOB verification does not elevate OwnerSession.
- OOB is single-use.

### Step 7.3: Approve with optional modification
- UI calls `POST /v1/approvals/{approval_id}` with:
  - `decision=approve|deny`
  - optional `modified_payload` (only allowed when mode=diff)
  - optional `oob.challenge_id` when required

On approve:
- Verification Core must re-validate:
  - modified payload schema
  - modified payload does not introduce new forbidden data handles
  - modified payload does not increase R/S beyond what was approved
- If OOB is required:
  - atomically consume verified challenge id
  - reject if expired or already consumed

On deny:
- Operation transitions to `blocked` or `cancelled` depending on policy.

All approval actions must:
- append an Experience Log event `kind=approval`
- add AuditTrace timeline entry

---

## 8. Syscall execution phase

For each syscall that is permitted and approved:

### Step 8.1: Construct `SyscallEnvelope`
Populate:
- ids: `syscall_id`, `operation_id`, `isolation_id`
- pinned versions
- caller component (verification_core or scheduler)
- target tool info
- intent and args
- gate fields (R,S,max_gate,approval_mode)
- taint_in sources

Persist syscall with status:
- `proposed` then `permitted` (or `awaiting_approval` earlier), then `executed` or `failed`.

### Step 8.2: Execute via ToolProvider
- ToolProvider executes syscall
- Tool output stored in BlobStore/Experience Log with sensitivity and taint labels
- Update syscall result fields with `output_ref`, `output_sensitivity_s`, `output_taint_s`

### Step 8.3: Handle denial at execution time
If ToolProvider fails or governance re-evaluates and denies:
- Create `SyscallDeny` with:
  - deny_class
  - violations with stable constraint ids
  - retry policy
  - remediation options
- Persist deny and emit WS `syscall_denied`
- Operation transitions to `blocked` unless policy says fail.

### Step 8.4: IPC artifacts and sanitization
If execution produces an artifact intended for IPC:
- Create `IPCArtifact` with:
  - `content_ref`
  - `sensitivity_s`, `taint_s`
  - provenance refs
  - audience scope tag
- Persist and emit WS `ipc_emit`

If sanitization is required:
- Sanitization must be executed as a dedicated syscall:
  - target kind `sanitizer`
  - output is a new artifact with reduced sensitivity, if permitted
- Persist sanitization steps in audit.

---

## 9. Completion criteria and finalization

An operation completes when:
- all required syscalls are executed successfully, OR
- the operation is draft-only and final output is persisted, OR
- the operation is blocked awaiting user input, OR
- the operation fails irrecoverably.

### Step 9.1: Write final Experience Log entries
At minimum:
- reasoning output final artifact
- approvals (if any)
- syscall results and denies (if any)
- final operation outcome event

### Step 9.2: Final AuditTrace update
- Append final timeline entries:
  - completion/failure
  - references to gate decision, compiled slice, syscalls, denies, IPC artifacts
- Update summary:
  - final outcome
  - gate summary
- Persist updated AuditTrace.

### Step 9.3: Emit WS final events
- `operation_state` -> completed/failed/blocked/cancelled
- `audit_update`

### Step 9.4: Persist idempotent response (if idempotency key present)
- Store the full HTTP response JSON under `Idempotency-Key` for future retries.

---

## 10. Failure modes (mandatory behavior)

### 10.1: Storage write failure (audit-critical)
If any of these fail:
- Experience Log append for request
- Operation creation
- AuditTrace creation/update
- Syscall record write
Then:
- fail operation
- return `500`
- do not proceed with execution

### 10.2: Model invalid output
- one constrained retry allowed
- if still invalid:
  - mark operation failed
  - store failure in audit
  - return failure status to UI

### 10.3: Policy denial
- Must return a structured denial object:
  - `SyscallDeny` for syscalls
  - Approval-required parking for gated actions
- Must not loop retries without remediation.

### 10.4: Timeout
- If model call exceeds timeout:
  - mark operation blocked or failed based on policy
  - audit must record timeout
- If tool call exceeds timeout:
  - mark syscall failed
  - provide remediation options and retry policy

---

## 11. Required WS events and when to emit

For each operation, emit at minimum:
- `operation_state` on each state transition
- `audit_update` after persisting GateDecision, CompiledSlice, AuditTrace updates
- `approval_required` when parking in awaiting_approval
- `reasoning_stream_chunk` during model generation (optional but recommended)
- `syscall_denied` and `syscall_executed` as applicable

Events must carry:
- `operation_id`
- `isolation_id`
- `audit_trace_id`

---

## 12. Determinism and replay requirements

To support replay via `/v1/audit/{id}/replay`, the system must store:
- pinned versions
- the exact compiled slice (or enough to recompile deterministically)
- model request parameters (model id, temperature if any, timeouts)
- final model output
- all syscalls, their args, and outcomes
- approval payloads and modified payloads
- OOB challenge id references (not secrets)

Replay modes:
- `dry_run`: does not execute actuators, but runs verification and emits what would happen
- `full`: allowed only under explicit policy and owner approval

---

## 13. Minimal compliance checklist (implementation acceptance)
Implementation is compliant with this spec only if:

- No request is accepted without logging the full RequestEnvelope or failing.
- No syscall executes without a persisted SyscallEnvelope and governance permit.
- Approvals and OOB are operation-bound and single-use.
- Taint is tracked per block and prevents laundering.
- Operation isolation is enforced and IPC is explicit.
- AuditTrace contains enough references to reconstruct “why” for every decision.
