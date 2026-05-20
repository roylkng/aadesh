# Root Owner Control Plane API Spec v0.1
Adesh OS

This document defines the canonical HTTP/WS control plane contract for Adesh OS.

## 0) Core principles

1. HTTP control plane is Root Owner only.
2. External agents never use these HTTP endpoints; they use `mcp_host_surface_contract_spec.md` only.
3. All state-changing endpoints support idempotency keys.
4. Storage is authoritative; WS is notification-only.
5. Persist-before-emit is mandatory for operation state, approvals, denies, syscalls, and audit updates.

## 1) Base, auth, envelopes

### 1.1 Base
- Base URL: `http://127.0.0.1:<port>`
- Prefix: `/v1`

### 1.2 Auth
- Required for all endpoints except `GET /v1/health`.
- Authenticated principal must be Root Owner.
- Non-Root Owner callers return `403 FORBIDDEN`.
- `WS /v1/events` may authenticate via:
  - `Authorization: Bearer <token>`, or
  - `?access_token=<token>` for browser-local Root Owner UI use on localhost only.

### 1.3 Idempotency
Idempotency is required for state-changing POST endpoints:
- `POST /v1/requests`
- `POST /v1/operations/{operation_id}/cancel`
- `POST /v1/approvals/{approval_id}`
- `POST /v1/approvals/{approval_id}/oob/start`
- `POST /v1/approvals/{approval_id}/oob/verify`
- `POST /v1/review-queue/{item_id}/decide`
- `POST /v1/audit/{audit_trace_id}/replay`
- `POST /v1/artifacts/manual`
- `POST /v1/ingest/jobs`
- `POST /v1/ingest/jobs/{job_id}/cancel`
- `POST /v1/workflow-specs`
- `POST /v1/workflow-instances`
- `POST /v1/workflow-instances/{workflow_instance_id}/cancel`
- `POST /v1/interface-specs`
- `POST /v1/interface-instances`
- `POST /v1/capabilities/{kind}/{name}/enable`
- `POST /v1/capabilities/{kind}/{name}/disable`

If audience graph mutation endpoints are enabled in a deployment, they must also require idempotency keys.

Header:
- `Idempotency-Key: <opaque-key>`

Server behavior:
- same `(endpoint_scope, idempotency_key)` returns the original response without re-execution.

### 1.4 Response envelope
Success:
```json
{
  "ok": true,
  "data": {},
  "meta": {
    "request_id": "string",
    "ts": "rfc3339",
    "audit_trace_id": "string|null"
  }
}
```

Error:
```json
{
  "ok": false,
  "error": {
    "code": "UNAUTHORIZED|FORBIDDEN|NOT_FOUND|CONFLICT|INVALID_INPUT|TRANSIENT|PERMANENT|TIMEOUT|RATE_LIMITED",
    "message": "string",
    "details": {}
  },
  "meta": {
    "request_id": "string",
    "ts": "rfc3339"
  }
}
```

Error code semantics follow `error_remediation.md`.

## 2) Health and events

### 2.1 `GET /v1/health`
No auth required.

Response:
```json
{
  "status": "ok|degraded",
  "version": "string",
  "storage": "ok|degraded",
  "model_provider": "ok|degraded",
  "tool_provider": "ok|degraded",
  "queue": "ok|degraded"
}
```

### 2.2 `WS /v1/events`
- Root Owner only.
- Event envelope and event type semantics are defined in `websocket_events_contract.md`.
- Server emits `hello` on connect.
- At-most-once delivery; client reconciles via REST.

### 2.3 `GET /v1/metrics/wedge`
Returns wedge KPI counters and latency summary for local Root Owner monitoring.

## 3) Requests and operations

### 3.1 `POST /v1/requests`
Submit `RequestEnvelope`.

Request body:
- Batch-1 `RequestEnvelope` schema.

Response data:
```json
{
  "request_id": "string",
  "operation_ids": ["string"],
  "primary_operation_id": "string",
  "audit_trace_ids": ["string"]
}
```

Behavior:
- root-owner auth check
- schema validation
- idempotency lookup
- T1 request acceptance transaction per `storage_semantics_txn.md`

### 3.2 `GET /v1/requests/{request_id}`
Response data:
```json
{
  "request_id": "string",
  "operation_ids": ["string"],
  "status": "running|blocked|completed|failed|cancelled"
}
```

### 3.3 `GET /v1/operations/{operation_id}`
Returns current operation snapshot (`OperationSpec` + runtime state metadata).

### 3.4 `POST /v1/operations/{operation_id}/cancel`
Cancels operation if policy allows.

### 3.5 `POST /v1/artifacts/manual`
Creates a manual artifact from explicit user-provided content for request grounding.

## 4) Gate, compile, syscall, and audit reads

### 4.1 `GET /v1/operations/{operation_id}/gate`
Returns Batch-2 `GateDecision`.

### 4.2 `GET /v1/operations/{operation_id}/compiled-slice`
Returns Batch-2 `CompiledSlice`.

### 4.3 `GET /v1/operations/{operation_id}/reasoning-output`
Returns persisted `ReasoningOutput`.

### 4.4 `GET /v1/operations/{operation_id}/syscalls`
Returns syscall list for operation (statuses and refs).

### 4.5 `GET /v1/syscalls/{syscall_id}/deny`
Returns persisted `SyscallDeny` when present.

### 4.6 `GET /v1/audit/{audit_trace_id}`
Returns `AuditTrace`.

### 4.7 `POST /v1/audit/{audit_trace_id}/replay`
Request:
```json
{
  "mode": "dry_run|full",
  "strategy": "stored_output|rerun_model"
}
```
Response data:
```json
{
  "replay_id": "string",
  "operation_id": "string",
  "audit_trace_id": "string",
  "status": "running|completed|failed|awaiting_approval"
}
```

## 5) Approvals and OOB

Approvals are approval-item scoped, not operation scoped.

### 5.1 `GET /v1/approvals/pending`
Returns pending `ApprovalItem` summaries.

Response data (array items):
```json
{
  "approval_id": "string",
  "operation_id": "string",
  "approval_mode": "confirm|diff|oob_required",
  "prompt": "string",
  "diff": {},
  "expires_at": "rfc3339|null",
  "audit_trace_id": "string"
}
```

### 5.2 `GET /v1/approvals/{approval_id}`
Returns full approval payload including proposal bundle and editable fields.

### 5.3 `POST /v1/approvals/{approval_id}/oob/start`
Starts OOB challenge bound to `approval_id`.

Response data:
```json
{
  "approval_id": "string",
  "challenge_id": "string",
  "expires_at": "rfc3339"
}
```

### 5.4 `POST /v1/approvals/{approval_id}/oob/verify`
Verifies challenge response.

Request body:
```json
{
  "challenge_id": "string",
  "response": {}
}
```

Response data:
```json
{
  "approval_id": "string",
  "challenge_id": "string",
  "status": "verified"
}
```

### 5.5 `POST /v1/approvals/{approval_id}`
Consumes approval decision.

Request body:
```json
{
  "decision": "approve|deny",
  "modified_payload": {},
  "oob": {
    "challenge_id": "string|null"
  }
}
```

Behavior:
- executes `consume_approval_atomic`
- if approved and valid, operation transitions out of `awaiting_approval`
- no syscall execution inside approval transaction
- email `modified_payload` validation and normalization must follow `email_send_payload_contract.md`

## 6) Review queue

### 6.1 `GET /v1/review-queue?status=pending`
List review items.

### 6.2 `GET /v1/review-queue/{item_id}`
Get item details.

### 6.3 `POST /v1/review-queue/{item_id}/decide`
Request body:
```json
{
  "decision": "approve|reject|edit",
  "edited_payload": {},
  "oob": {
    "challenge_id": "string|null"
  }
}
```
Behavior follows `review_queue_and_control_plane.md` and atomic decision semantics.

## 7) Capability and audience governance endpoints

### 7.1 `GET /v1/capabilities`
Returns current capability snapshot summary.

### 7.2 `GET /v1/capabilities/snapshots/{capability_snapshot_version}`
Returns immutable capability snapshot payload.

### 7.3 `POST /v1/capabilities/snapshots`
Mints an immutable capability snapshot candidate.

Request body:
```json
{
  "base_version": "string|null",
  "snapshot_payload": {}
}
```

Behavior:
- Root Owner only
- idempotent
- validates referenced schema refs exist
- does not change `current_versions`
- returns minted immutable snapshot version

### 7.4 `POST /v1/capabilities/current/activate`
Requests activation of a minted capability snapshot candidate through the review queue.

Request body:
```json
{
  "capability_snapshot_version": "string"
}
```

Behavior:
- Root Owner only
- idempotent
- does not mutate `current_versions` directly
- creates a review item targeting `capability_registry`
- approval of that review item activates the snapshot by updating `current_versions.capability_snapshot`

### 7.5 `POST /v1/capabilities/{kind}/{name}/enable`
### 7.6 `POST /v1/capabilities/{kind}/{name}/disable`
These changes are governed operations and may require approval/OOB.

### 7.7 `POST /v1/schema-registry/register`
Registers an immutable schema entry candidate.

Request body:
```json
{
  "schema_kind": "string",
  "name": "string",
  "semver": "string",
  "schema_payload": {}
}
```

Behavior:
- Root Owner only
- idempotent
- canonicalizes payload before hashing
- returns existing schema_ref if identical payload already exists
- does not mutate any pinned operation state or current capability snapshot

### 7.8 `GET /v1/schema-registry/{schema_ref}`
Returns immutable schema registry entry and payload by `schema_ref`.

### 7.4 Audience graph endpoints
If implemented in v0.1:
- patch/apply endpoints must mint new `audience_graph_version`
- changes are governed (R3/R4 depending on blast radius)

## 8) Ingestion endpoints

### 8.1 `POST /v1/ingest/jobs`
Creates an asynchronous ingestion job per `ingestion_pipeline_spec.md`.

Request body:
- `sources[]`
- `options`

Canonical request schema (conceptual):
```json
{
  "sources": [
    {
      "type": "text|file|folder|conversation|url",
      "payload": {},
      "metadata": {}
    }
  ],
  "options": {
    "dedupe": true,
    "max_artifacts": 1000,
    "chunking": "none|page|fixed_tokens",
    "classification_mode": "conservative|normal"
  }
}
```

Rules:
- `sources` must be non-empty.
- unknown source types or option fields are invalid input.
- `dedupe=true` requires persisted non-null dedupe keys for artifact rows created by the job.

Response data:
```json
{
  "job_id": "string",
  "status": "pending|running",
  "counters": {
    "artifacts_total": 0,
    "artifacts_succeeded": 0,
    "artifacts_failed": 0,
    "bytes_ingested": 0
  }
}
```

### 8.2 `GET /v1/ingest/jobs/{job_id}`
Returns current job status, counters, and bounded redacted errors.

### 8.3 `POST /v1/ingest/jobs/{job_id}/cancel`
Requests cancellation of a pending or running ingest job.

## 9) Post-wedge composition endpoints

Status:
- These routes are canonical for post-wedge composition.
- They remain outside the active cognitive-sidecar proof unless a new wedge brief explicitly brings them into scope.

### 9.1 `POST /v1/workflow-specs`
Registers an immutable `WorkflowSpec` from `workflow_spec_contract.md`.

Request body:
```json
{
  "name": "string",
  "description": "string",
  "spec": {}
}
```

Behavior:
- canonicalizes and hashes `spec` into `workflow_ref`
- inserts immutable spec row
- returns existing `workflow_ref` if identical canonical payload already exists

Response data:
```json
{
  "workflow_ref": "string",
  "content_hash": "string"
}
```

### 9.2 `GET /v1/workflow-specs/{workflow_ref}`
Returns immutable `WorkflowSpec` payload by content-addressed ref.

### 9.3 `GET /v1/workflow-specs`
Optional filters:
- `name`
- `tag`
- `limit`

Returns immutable spec summaries.

### 9.4 `POST /v1/workflow-instances`
Creates a `WorkflowInstance` execution record.

Request body:
```json
{
  "workflow_ref": "string",
  "inputs": {},
  "request_context": {
    "parent_request_id": "string|null",
    "operation_id": "string|null"
  }
}
```

Behavior:
- validates `workflow_ref` exists
- creates instance state and initial step state records
- pins current versions at creation:
  - `active_state_version`
  - `capability_snapshot_version`
  - `audience_graph_version`

Response data:
```json
{
  "workflow_instance_id": "string",
  "workflow_ref": "string",
  "state": "created|running|awaiting_approval|blocked|completed|failed|cancelled"
}
```

### 9.5 `GET /v1/workflow-instances/{workflow_instance_id}`
Returns instance snapshot, pinned versions, and per-step state summaries.

### 9.6 `POST /v1/workflow-instances/{workflow_instance_id}/cancel`
Requests cancellation of a running workflow instance.

### 9.7 `POST /v1/interface-specs`
Registers immutable `InterfaceSpec` from `interface_spec_contract.md`.

Request body:
```json
{
  "name": "string",
  "description": "string",
  "spec": {}
}
```

Behavior:
- canonicalizes and hashes `spec` into `interface_ref`
- inserts immutable spec row
- returns existing `interface_ref` if identical payload already exists

Response data:
```json
{
  "interface_ref": "string",
  "content_hash": "string"
}
```

### 9.8 `GET /v1/interface-specs/{interface_ref}`
Returns immutable `InterfaceSpec` payload by content-addressed ref.

### 9.9 `GET /v1/interface-specs`
Optional filters:
- `name`
- `tag`
- `limit`

Returns immutable spec summaries.

### 9.10 `POST /v1/interface-instances`
Compiles and persists an `InterfaceInstance`.

Request body:
```json
{
  "interface_ref": "string",
  "operation_id": "string|null",
  "workflow_instance_id": "string|null",
  "viewer": {
    "audience_id": "root_owner"
  }
}
```

Rules:
- exactly one of `operation_id` or `workflow_instance_id` must be set
- viewer authorization remains Root Owner only in v0.1 control plane
- if `workflow_instance_id` is used, the referenced workflow instance must have a persisted `parent_operation_id`; otherwise interface compilation fails closed in the v0.1 reference runtime

Response data:
```json
{
  "interface_instance_id": "string",
  "interface_ref": "string",
  "state": "ready|stale"
}
```

### 9.11 `GET /v1/interface-instances/{interface_instance_id}`
Returns persisted `InterfaceInstance` including:
- pinned versions
- block list
- binding list
- taint summary

## 10) Ordering and consistency requirements

1. Persist operation state transition before emitting `operation_state` WS event.
2. Persist approvals/OOB state before emitting approval-related WS events.
3. Persist `SyscallDeny` before returning denial response or WS deny event.
4. Persist syscall result refs before emitting `syscall_executed`.
5. On write failure for audit-critical artifacts, fail closed.
6. Persist workflow instance/step transitions before emitting workflow WS events.
7. Persist interface instances before emitting interface WS events.

## 11) Compatibility and source-of-truth alignment

This file must align with:
- `kernel_execution_loop.md`
- `approval_oob_spec.md`
- `ingestion_pipeline_spec.md`
- `workflow_spec_contract.md`
- `interface_spec_contract.md`
- `websocket_events_contract.md`
- `review_queue_and_control_plane.md`
- `replay_and_deterministic_re_execution.md`
- `storage_semantics_txn.md`

If endpoint examples conflict, this file is canonical for HTTP route shapes and path parameters.
