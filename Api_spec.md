# Adesh OS Localhost Control Plane API Spec v0.2 (REST + WebSockets)

This spec defines the **Root Owner control plane** API surface for Adesh OS. It wraps the Batch 1–3 contract objects and supports a production-grade UI: approvals, diffs, audit, graph editing, review queues, streaming output, and deterministic governance.

**Key boundary rule**

* The HTTP/WS control plane is **Root Owner only**.
* All non-owner or external-audience access is **403 Forbidden**.
* External agent/audience integrations use the **MCP Host bridge**, not this API.

---

## 0) Conventions

### Base

* Base URL: `http://127.0.0.1:7777`
* Prefix: `/v1`

### Auth

* Header: `Authorization: Bearer <owner_session_token>`
* All endpoints except `/v1/health` require Root Owner session.
* If not Root Owner: return `403 FORBIDDEN`.

### Idempotency

* For side-effecting POST/PUT:

  * `Idempotency-Key: <uuid or stable key>` strongly recommended.
* Server guarantees idempotent behavior for repeated requests with same key.

### Correlation headers (optional)

* `X-Request-Id: <uuid>` if client wants to pin request id.

### Standard response envelope

Success:

```json
{
  "ok": true,
  "data": {},
  "meta": {
    "request_id": "...",
    "ts": "...",
    "audit_trace_id": "..."
  }
}
```

Error:

```json
{
  "ok": false,
  "error": {
    "code": "UNAUTHORIZED|FORBIDDEN|NOT_FOUND|CONFLICT|INVALID_INPUT|TRANSIENT|PERMANENT",
    "message": "Human readable",
    "details": {}
  },
  "meta": { "request_id": "...", "ts": "..." }
}
```

### Object contracts

Payload objects must conform to Batch schemas:

* Batch 1: `OwnerSession`, `RequestEnvelope`, `OperationSpec`
* Batch 2: `GateDecision`, `CompiledSlice`
* Batch 3: `SyscallEnvelope`, `SyscallDeny`, `IPCArtifact`, `AuditTrace`

---

## 1) Health and Diagnostics

### GET `/v1/health`

No auth required.
Response:

```json
{
  "status": "ok|degraded",
  "version": "0.2",
  "storage": "ok|degraded",
  "model_provider": "ok|degraded",
  "tool_provider": "ok|degraded",
  "queue": "ok|degraded"
}
```

### GET `/v1/metrics`

Optional. Prometheus metrics. Root Owner recommended.

---

## 2) Sessions (OwnerSession)

### POST `/v1/sessions`

Create a Root Owner session (local login).

* Body: implementation-specific `auth` payload.
* Response: `OwnerSession`

### GET `/v1/sessions/me`

Return current session details.

* Response: `OwnerSession`

> Security note: OOB verification does **not** elevate the global session. OOB is handled per approval (Section 5).

---

## 3) Requests and Operations

### POST `/v1/requests`

Submit a new request to Adesh OS.

* Headers: `Idempotency-Key` recommended
* Body: `RequestEnvelope` (Batch 1)
* Response:

```json
{
  "request_id": "...",
  "operation_ids": ["..."],
  "primary_operation_id": "...",
  "audit_trace_ids": ["..."]
}
```

### GET `/v1/requests/{request_id}`

Get request status and linked operations.
Response:

```json
{
  "request_id": "...",
  "operation_ids": ["..."],
  "status": "running|blocked|completed|failed|cancelled"
}
```

### GET `/v1/operations/{operation_id}`

Fetch current `OperationSpec` (Batch 1).

* Response: `OperationSpec`

### POST `/v1/operations/{operation_id}/cancel`

Cancel an operation.

* Headers: `Idempotency-Key` recommended
* Response:

```json
{ "operation_id": "...", "state": "cancelled" }
```

### GET `/v1/operations/{operation_id}/gate`

Fetch the `GateDecision` (Batch 2) for the operation.

* Response: `GateDecision`

### GET `/v1/operations/{operation_id}/compiled-slice`

Fetch `CompiledSlice` (Batch 2) used for the operation.

* Response: `CompiledSlice`

---

## 4) Capability Discovery and Management (Capability Self-Model)

### GET `/v1/capabilities`

Return capability snapshot: sensors, actuators, budgets, status.
Response:

```json
{
  "capability_snapshot_version": "...",
  "sensors": [
    {
      "name": "...",
      "status": "enabled|disabled|degraded",
      "trust_class": "trusted|semi_trusted|untrusted",
      "sensitivity_ceiling_s": 0,
      "rate_limits": {},
      "schema_ref": "..."
    }
  ],
  "actuators": [
    {
      "name": "...",
      "status": "enabled|disabled|degraded",
      "risk_floor_r": 2,
      "diff_supported": true,
      "approval_mode": "none|confirm|diff|oob_required|refuse",
      "schema_ref": "..."
    }
  ],
  "budgets": {
    "default_token_budget": 4096,
    "default_block_budgets": {
      "policy": 512,
      "capability": 512,
      "operation_context": 1024,
      "evidence": 1536,
      "scratch": 512
    }
  }
}
```

Optional splits:

* GET `/v1/capabilities/sensors`
* GET `/v1/capabilities/actuators`

### POST `/v1/capabilities/{kind}/{name}/enable`

Enable a sensor/actuator.

* `{kind}`: `sensors|actuators`
* Headers: `Idempotency-Key` recommended
* Response:

```json
{ "kind": "...", "name": "...", "status": "enabled", "audit_trace_id": "..." }
```

### POST `/v1/capabilities/{kind}/{name}/disable`

Disable a sensor/actuator.

* `{kind}`: `sensors|actuators`
* Headers: `Idempotency-Key` recommended
* Response:

```json
{ "kind": "...", "name": "...", "status": "disabled", "audit_trace_id": "..." }
```

Rules:

* Enabling/disabling may itself be gated (R3/R4) if it affects governance or safety boundaries.
* Must be audited.

---

## 5) Approvals (Confirm/Diff/OOB), Including OOB Single-Use Binding

### GET `/v1/approvals/pending`

List pending approvals.
Response:

```json
{
  "items": [
    {
      "operation_id": "...",
      "approval_mode": "confirm|diff|oob_required",
      "prompt": "...",
      "diff": {},
      "audit_trace_id": "..."
    }
  ]
}
```

### POST `/v1/approvals/{approval_id}/oob/start`

Start OOB challenge for this operation approval.

* Headers: `Idempotency-Key` recommended
* Body:

```json
{ "challenge_type": "webauthn|totp|device_signature|hardware_key|other" }
```

* Response:

```json
{
  "operation_id": "...",
  "challenge_id": "...",
  "nonce": "...",
  "status": "pending",
  "expires_at": "..."
}
```

### POST `/v1/approvals/{approval_id}/oob/verify`

Verify OOB challenge for this operation approval.

* Headers: `Idempotency-Key` recommended
* Body:

```json
{ "challenge_id": "...", "response": {} }
```

* Response:

```json
{
  "operation_id": "...",
  "challenge_id": "...",
  "status": "verified",
  "bound_to_operation_id": "..."
}
```

### POST `/v1/approvals/{approval_id}`

Approve/deny the next gated step. Supports “approve with edits”.

* Headers: `Idempotency-Key` recommended
* Body:

```json
{
  "decision": "approve|deny",
  "mode": "confirm|diff|oob_required",
  "note": "optional",
  "modified_payload": {},
  "oob": { "challenge_id": "..." }
}
```

Rules:

* `modified_payload` is allowed for `mode=diff` (and optionally `confirm` if you choose).
* If `mode=oob_required`, a verified `challenge_id` must be supplied.
* OOB challenges are **single-use** and **operation-bound**. Consumption is atomic with approval.
* Verification Core must re-validate modified payload:

  * schema correctness
  * no gate downgrade
  * no new forbidden data handles
  * no increased risk/sensitivity without re-approval

Response:

```json
{ "operation_id": "...", "new_state": "running|blocked|cancelled|failed|completed" }
```

---

## 6) Syscalls (debuggable execution objects)

### GET `/v1/syscalls?operation_id=...`

List syscall IDs for an operation.
Response:

```json
{ "syscall_ids": ["..."] }
```

### GET `/v1/syscalls/{syscall_id}`

Fetch `SyscallEnvelope` (Batch 3).

* Response: `SyscallEnvelope`

### GET `/v1/syscalls/{syscall_id}/deny`

If denied, fetch `SyscallDeny` (Batch 3).

* Response: `SyscallDeny`

---

## 7) IPC Artifacts (Explicit Piping)

### GET `/v1/ipc/artifacts?operation_id=...`

List IPC artifacts produced or consumed by an operation.
Response:

```json
{ "artifact_ids": ["..."] }
```

### GET `/v1/ipc/artifacts/{artifact_id}`

Fetch `IPCArtifact` (Batch 3).

* Response: `IPCArtifact`

---

## 8) Audience Graph Management (Root Owner Only)

### GET `/v1/audience-graph`

Fetch graph snapshot (owner view).
Response:

```json
{
  "graph_version": "...",
  "nodes": [ ... ],
  "edges": [ ... ],
  "scopes": [ ... ]
}
```

### PUT `/v1/audience-graph/patch`

Apply a graph patch.

* Headers: `Idempotency-Key` required
* Body:

```json
{ "base_version": "...", "patch": { ... } }
```

Response:

```json
{ "new_graph_version": "...", "audit_trace_id": "..." }
```

---

## 9) Review Queue (Hypothesis Promotion and Grooming)

### GET `/v1/review-queue`

List review items.
Response:

```json
{
  "items": [
    { "item_id": "...", "summary": "...", "risk": 0, "sensitivity": 2, "created_at": "..." }
  ]
}
```

### GET `/v1/review-queue/{item_id}`

Fetch full review item detail.
Response:

```json
{
  "item_id": "...",
  "proposed_change": {},
  "evidence_refs": ["..."],
  "confidence": 0.8,
  "requires_owner_confirmation": true
}
```

### POST `/v1/review-queue/{item_id}/decide`

Apply decision.

* Headers: `Idempotency-Key` required
* Body:

```json
{ "decision": "approve|reject|edit", "edited_payload": {} }
```

Response:

```json
{ "change_id": "...", "new_state_version": "...", "audit_trace_id": "..." }
```

---

## 10) Audit and Replay

### GET `/v1/audit/{audit_trace_id}`

Fetch `AuditTrace` (Batch 3).

* Response: `AuditTrace`

### POST `/v1/audit/{audit_trace_id}/replay`

Replay an operation decision path using pinned versions.

* Headers: `Idempotency-Key` recommended
* Body:

```json
{ "mode": "dry_run|full", "override_budgets": { "token_budget": 4096 } }
```

Response:

```json
{ "replay_id": "...", "status": "running|completed", "audit_trace_id": "..." }
```

---

## 11) WebSocket Events (Real-time UI Plane)

### WS `/v1/events`

Root Owner only. Auth via header or query token.

Common envelope:

```json
{
  "event_id": "...",
  "ts": "...",
  "type": "operation_state|approval_required|reasoning_stream_chunk|syscall_denied|syscall_executed|audit_update|review_queue_update|capability_update",
  "operation_id": "...",
  "isolation_id": "...",
  "audit_trace_id": "...",
  "data": {}
}
```

#### `operation_state`

```json
{ "state": "created|compiled|awaiting_approval|running|blocked|completed|failed|cancelled", "reason": "..." }
```

#### `approval_required`

```json
{ "approval_mode": "confirm|diff|oob_required", "prompt": "...", "diff": {} }
```

#### `reasoning_stream_chunk` (token streaming)

```json
{
  "stream_id": "...",
  "channel": "draft|plan|explanation",
  "seq": 12,
  "delta": "text chunk",
  "is_final": false
}
```

#### `syscall_denied`

```json
{ "syscall_id": "...", "deny_class": "...", "violations": [ ... ], "remediation": { ... } }
```

#### `syscall_executed`

```json
{ "syscall_id": "...", "ok": true, "output_ref": "...", "output_sensitivity_s": 2 }
```

#### `audit_update`

```json
{ "audit_trace_id": "...", "ref_id": "gate_decision|compiled_slice|syscall|ipc|..." }
```

#### `review_queue_update`

```json
{ "item_id": "...", "action": "created|updated|resolved" }
```

#### `capability_update`

```json
{ "capability_snapshot_version": "...", "changed": ["sensor:fs", "actuator:send_email"] }
```

---

## Notes for implementation

* All endpoints enforce Batch schema validation at boundaries.
* All responses should include correlation IDs and emit audit events.
* Streaming chunks are UI-only convenience; final output must be persisted for audit/replay.
* OOB is operation-bound and single-use to prevent session elevation TOCTOU.
