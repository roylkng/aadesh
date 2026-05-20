# Approvals and OOB (Single-Use, Diff-Edit, Atomic Consumption) Spec v0.1
Adesh OS

This document specifies the **approval subsystem** and **out-of-band (OOB) authorization** model for Adesh OS. It defines:
- approval state model and persistence requirements
- how operations enter and exit `awaiting_approval`
- confirm vs diff vs OOB-required flows
- “approve with modifications” (`modified_payload`) rules and re-gating
- OOB challenge lifecycle: issued, verified, consumed, expired
- required atomic transaction boundaries and audit events
- WebSocket event semantics for approval UX

This is algorithmic logic. Not implementation code.

---

## 0) Core principles

1. **Approvals gate execution, not reasoning**
- The model may propose actions, but no external side effects occur until the approval workflow completes.

2. **Approvals are syscall-scoped but operation-owned**
- Approvals are triggered because one or more syscalls require it.
- Approval state is attached to an operation lifecycle and unblocks specific syscalls.

3. **OOB is never session elevation**
- OOB verification must not elevate the global OwnerSession.
- OOB verification is operation-bound and single-use.

4. **Approval is an atomic commitment**
- Approval consumption must be atomic with:
  - audit recording
  - operation state transition
  - syscall permit persistence

5. **Diff edit is first-class**
- User must be able to approve with corrected syscall arguments without re-running the model.

---

## 1) Objects and state model

### 1.1 ApprovalItem (logical)
The system must represent pending approvals explicitly.

Minimum fields:
- `approval_id` (stable id)
- `operation_id`
- `isolation_id`
- `created_at`
- `approval_mode` = confirm|diff|oob_required
- `status` = pending|approved|denied|expired|consumed
- `syscall_ids[]` impacted (one or more)
- `proposal_bundle`:
  - the syscall intents proposed (normalized)
  - computed syscall gates at time of approval request
  - audience targets (if outbound)
- `diff_payload` (required for diff mode, optional for confirm)
- `prompt` human-readable
- `expires_at` optional
- `audit_trace_id`

Persistence:
- ApprovalItems must be stored durably (StorageProvider).

### 1.2 OOBChallenge (logical)
Minimum fields:
- `challenge_id`
- `approval_id` (required)
- `nonce`
- `challenge_type`
- `status` = pending|verified|consumed|expired|failed
- `issued_at`
- `verified_at` optional
- `consumed_at` optional
- `expires_at`
- optional `attempts`

Persistence:
- OOBChallenges must be stored durably.

---

## 2) When to request approval

### 2.1 Syscall approval requirement
For each proposed syscall, Verification computes:
- `R_syscall`, `S_syscall`, `max_gate`
- `approval_mode` based on max_gate and tool constraints

Approval is required when:
- `approval_mode != none`

### 2.2 Aggregation into ApprovalItem(s)
If multiple syscalls require approval, group them into ApprovalItems deterministically:

Grouping rules:
1. Group syscalls that share the same:
   - approval_mode
   - target audience (for outbound actions)
   - execution phase (if sequential)
2. Do not group syscalls that differ in:
   - max_gate level (requires separate approval at higher gate)
   - sensitivity ceiling constraints (different audiences)
   - tool category where diff payload would become confusing

Result:
- 1..N ApprovalItems per operation.

### 2.3 Operation state transition
If any ApprovalItem is pending:
- operation transitions to `awaiting_approval`
- persist transition record
- emit WS `approval_required` for each ApprovalItem (or one aggregated UI view)

---

## 3) Approval modes and required UI payloads

### 3.1 Confirm mode
Confirm is a binary acknowledgement for medium-stakes actions.

Required payload:
- `prompt`
- list of syscalls (tool, action, key args)
- audience targets if outbound
- computed gate summary

No diff required, but may include a lightweight preview.

### 3.2 Diff mode
Diff is required for high-stakes changes.

Diff payload must contain a deterministic “what will change” view:
- tool name and action
- target identifiers (recipient, resource id, account id)
- before/after where applicable OR a synthetic diff summary:
  - “Will send email to X with subject Y”
  - “Will update setting A from off to on”
- any sensitive fields must be redacted or replaced with placeholders in the UI payload

Diff payload must also include:
- a machine-readable `editable_payload_schema` indicating which fields can be edited safely
- a `current_args` object (the proposed args)

### 3.3 OOB required
OOB is required for critical/irreversible actions or self-modification.

Required payload:
- everything in diff mode
- explicit statement: “OOB authorization required”
- allowed OOB methods

---

## 4) “Approve with modifications” (`modified_payload`)

### 4.1 When allowed
- Allowed only when `approval_mode = diff` (and optionally for confirm if you decide, but default is diff-only).
- The UI supplies `modified_payload` as a patch or full replacement args.

### 4.2 Validation on modified payload
Before consuming approval, Verification must re-run checks as if the syscall were newly proposed:
- schema validation against tool action schema
- negative memory check (forbidden fields)
- audience scope and ceiling check (recipients changed = re-evaluate Audience Graph)
- taint laundering check (new fields may leak sensitive content)
- recompute syscall gate (R/S/max)

### 4.3 Re-gating rule (critical)
If modified payload increases `max_gate` or changes `approval_mode` to stricter:
- do not execute
- generate a new ApprovalItem requiring the higher mode
- mark previous approval attempt as “superseded” in audit timeline

If modified payload decreases gate:
- do not accept downgrade silently
- keep original gate or require explicit confirmation that user intends the downgrade
Default policy: do not allow gate downgrade via edits.

### 4.4 Audit requirements
All modified payloads must be stored:
- in Experience Log as an approval event
- referenced in AuditTrace timeline

---

## 5) OOB challenge lifecycle (single-use binding)

### 5.1 Start OOB
Endpoint: `POST /v1/approvals/{approval_id}/oob/start`
- server creates `OOBChallenge` bound to `approval_id`
- server returns `{challenge_id, nonce, expires_at}`

Constraints:
- nonce must be cryptographically random
- expires_at must be short (policy-defined)
- multiple concurrent challenges for same approval are allowed only if explicitly configured, otherwise deny to reduce confusion

### 5.2 Verify OOB
Endpoint: `POST /v1/approvals/{approval_id}/oob/verify`
- server checks:
  - challenge exists
  - bound to the correct approval
  - not expired
  - not consumed
- server verifies the response using the chosen method (TOTP/WebAuthn/device signature)
- on success: status -> verified

### 5.3 Consume OOB (atomic)
OOB verification does nothing by itself. It is only consumed during approval.

During `POST /v1/approvals/{approval_id}` with `mode=oob_required`:
- system must atomically:
  1) validate approval
  2) validate modified payload if present
  3) check challenge status is verified and not expired
  4) mark challenge as consumed
  5) mark approval as approved/consumed
  6) persist SyscallEnvelope(s) as permitted
  7) append Experience Log approval event
  8) update AuditTrace timeline
  9) transition operation state to running

If any step fails:
- entire transaction rolls back
- challenge is not consumed

### 5.4 Expiration
A challenge becomes expired when `now > expires_at`.
Expired challenges cannot be verified or consumed.

---

## 6) Atomic transaction boundaries (must be enforced)

### T-APPROVAL: Approval consumption transaction
In one atomic unit:

Input:
- approval_id (implicit or explicit)
- decision approve|deny
- optional modified_payload
- optional challenge_id for OOB

Atomic steps on approve:
1. Lock operation runner for `operation_id` (prevent concurrent advances)
2. Load pending ApprovalItem and ensure status pending
3. If OOB required:
   - validate and load verified challenge bound to approval
4. Validate modified payload (if present) and recompute gates
5. If gate escalation required:
   - create new ApprovalItem and do not proceed
6. Persist approval decision record
7. Append Experience Log approval event
8. Update AuditTrace timeline (approval granted/denied + payload refs)
9. If approving:
   - mark approval consumed
   - consume OOB challenge if present
   - persist SyscallEnvelope(s) with status `permitted`
   - transition operation state to running
10. Commit

On deny:
- persist decision and audit
- operation transitions to `blocked` or `cancelled` depending on policy

No syscall execution occurs inside this transaction. Execution begins after commit.

---

## 7) Approval failure modes and responses

### 7.1 Stale approval
If approval is no longer pending (already consumed/expired):
- return `409 CONFLICT` with details
- do not execute

### 7.2 Missing OOB
If mode requires OOB and challenge_id is missing or not verified:
- return `400 INVALID_INPUT` (or 409) with required action
- emit WS update indicating OOB required

### 7.3 Modified payload invalid
If modified payload fails validation:
- return `400 INVALID_INPUT` with violation details
- do not consume approval
- do not execute

### 7.4 Approval superseded
If underlying syscalls changed due to replanning while awaiting approval:
- return `409 CONFLICT` and provide new approval_id
- UI must refresh pending approvals

---

## 8) WebSocket event semantics for approvals

### 8.1 approval_required
Emit when operation enters awaiting_approval:
- includes approval_id, mode, prompt, diff payload

### 8.2 oob_challenge_requested
Emit after successful OOB start:
- includes approval_id, challenge_id, expires_at

### 8.3 oob_challenge_verified
Emit after successful verify:
- includes approval_id, challenge_id

### 8.4 approval_granted / approval_denied
Emit after consumption transaction commits:
- includes approval_id, operation_id, next_state

### 8.5 capability_update (optional)
If approval toggles tools or affects capabilities, emit capability update events.

---

## 9) Minimum test cases (must pass)

1. **Single-use OOB**
- Verify challenge for Approval A.
- Attempt to reuse it for Approval B.
- Must fail.

2. **TOCTOU prevention**
- Verify OOB.
- Attempt a different operation approval without providing correct challenge.
- Must fail.

3. **Modified payload re-gating**
- Change recipients from internal to external.
- Must recompute audience scope and possibly increase gate.

4. **Approval superseded**
- While awaiting approval, system replans and produces new syscalls.
- Old approval must be rejected as stale.

5. **Atomic audit**
- Simulate crash mid-approval transaction.
- Must not execute syscalls and must not partially consume OOB.

6. **Idempotency**
- Repeat approval POST with same Idempotency-Key.
- Must not execute twice.
