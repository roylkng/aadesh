# WebSocket Events and Streaming Contract Spec v0.1
Adesh OS

This document specifies the **WebSocket event contract** for the Root Owner control plane (`WS /v1/events`). It defines:
- event envelope schema
- event ordering and delivery semantics
- idempotency and dedupe rules
- streaming behavior for reasoning output (`reasoning_stream_chunk`)
- required events for approvals, syscalls, audits, and capabilities
- required events for workflow/interface composition lifecycles (post-wedge)
- persistence rules: what must be stored vs what may be ephemeral

This is algorithmic logic. Not implementation code.

---

## 0) Core principles

1. **WS is the realtime plane for Root Owner UI**
- WS is not used for external audiences. External audiences use MCP Host.

2. **Events are hints, storage is truth**
- WS events may be dropped or delayed.
- UI must be able to reconstruct state by calling REST endpoints.

3. **Events must be correlatable**
- Every event must include:
  - `event_id`
  - `ts`
  - `type`
  - `request_id` (when available)
  - `operation_id` (when relevant)
  - `workflow_instance_id` (when relevant)
  - `step_id` (when relevant)
  - `interface_instance_id` (when relevant)
  - `isolation_id` (when relevant)
  - `audit_trace_id` (when relevant)

4. **Streaming is UX, final artifacts are persisted**
- Token streaming chunks improve UX but are not authoritative records.
- Final reasoning output must be stored in Experience Log / blob and referenced in AuditTrace.

---

## 1) Connection semantics

### 1.1 Endpoint
- `WS /v1/events`

### 1.2 Auth
- Root Owner only.
- Auth via:
  - `Authorization: Bearer <token>`
  - `?access_token=<token>` for browser-local Root Owner UI use on localhost only
- If unauthorized: refuse WS upgrade.

### 1.3 Heartbeats
- Optional server heartbeat: `type=heartbeat` every N seconds.
- UI may use ping/pong; not required.

---

## 2) Event envelope (canonical)

All events must conform to:

```json
{
  "event_id": "uuid",
  "ts": "rfc3339",
  "type": "string",
  "request_id": "string|null",
  "operation_id": "string|null",
  "workflow_instance_id": "string|null",
  "step_id": "string|null",
  "interface_instance_id": "string|null",
  "isolation_id": "string|null",
  "audit_trace_id": "string|null",
  "data": {}
}
```

### 2.1 event_id uniqueness

* `event_id` must be globally unique.
* UI may dedupe events by `event_id`.

### 2.2 Timestamp semantics

* `ts` is server time when event emitted.
* Do not rely on `ts` for strict ordering across different operations.

---

## 3) Delivery and ordering semantics

### 3.1 At-most-once delivery

* WS is at-most-once. Events may be lost on disconnect.

### 3.2 Best-effort ordering per operation

* Within a single operation, events SHOULD be emitted in causal order.
* UI must not assume perfect ordering; it should reconcile via REST.

### 3.3 Reconciliation rule

On reconnect:

* UI must query:

  * `/v1/approvals/pending`
  * `/v1/operations/{id}` for active operations
  * `/v1/audit/{audit_trace_id}` for latest traces
  * `/v1/workflow-instances/{workflow_instance_id}` when workflow composition is enabled
  * `/v1/interface-instances/{interface_instance_id}` when interface composition is enabled
* WS stream is treated as incremental updates.

---

## 4) Required event types

### 4.1 hello

Emitted immediately after connection.
Data:

```json
{ "message": "connected", "server_version": "0.2", "capability_snapshot_version": "..." }
```

### 4.2 operation_state

Emitted on every operation state transition.
Data:

```json
{ "state": "created|compiled|awaiting_approval|running|blocked|completed|failed|cancelled", "reason": "string|null" }
```

Rules:

* Must be emitted after the state transition is persisted (storage-first).
* If persistence fails, no event should be emitted.

### 4.3 audit_update

Emitted after storing any audit-relevant object or updating the AuditTrace.
Data:

```json
{ "ref_type": "gate_decision|compiled_slice|audit_trace|syscall|syscall_deny|ipc_artifact|approval|review_item", "ref_id": "string" }
```

Rules:

* Emitted after the referenced object is persisted.
* UI uses this to refresh audit view.

### 4.4 approval_required

Emitted when operation enters awaiting_approval.
Data:

```json
{
  "approval_id": "string",
  "approval_mode": "confirm|diff|oob_required",
  "prompt": "string",
  "diff": {},
  "expires_at": "rfc3339|null"
}
```

Rules:

* Emitted only after ApprovalItem is persisted and operation state is persisted to awaiting_approval.

### 4.5 oob_challenge_requested

Emitted when an OOB challenge is started.
Data:

```json
{ "approval_id": "string", "challenge_id": "string", "expires_at": "rfc3339" }
```

### 4.6 oob_challenge_verified

Emitted when an OOB challenge is verified.
Data:

```json
{ "approval_id": "string", "challenge_id": "string" }
```

### 4.7 approval_granted / approval_denied

Emitted after approval transaction commits.
Data:

```json
{ "approval_id": "string", "decision": "approve|deny", "next_state": "running|blocked|cancelled" }
```

### 4.8 reasoning_stream_start (optional but recommended)

Emitted when streaming begins for an operation.
Data:

```json
{ "stream_id": "string", "channels": ["draft","plan","explanation"], "model_id": "string|null" }
```

### 4.9 reasoning_stream_chunk (token streaming)

Emitted during model generation.
Data:

```json
{
  "stream_id": "string",
  "channel": "draft|plan|explanation|other",
  "seq": 0,
  "delta": "string",
  "is_final": false
}
```

Rules:

* `seq` must be monotonically increasing per `(stream_id, channel)`.
* `delta` is a text fragment appended by the UI.
* Chunks are ephemeral and need not be persisted.
* If WS disconnects mid-stream, UI will still be able to fetch final result from storage.

### 4.10 reasoning_stream_end (optional)

Emitted when streaming completes.
Data:

```json
{ "stream_id": "string", "is_final": true, "final_output_ref": "event_ref|content_ref|null" }
```

### 4.11 syscall_proposed (optional)

Emitted when syscalls are proposed (post reasoning, pre verification).
Data:

```json
{ "syscall_ids": ["..."] }
```

### 4.12 syscall_denied

Emitted when a syscall is denied (pre-execution or execution-time).
Data:

```json
{ "syscall_id": "string", "deny_class": "string", "violations": [], "remediation": {} }
```

Rules:

* The referenced `SyscallDeny` must already be persisted.

### 4.13 syscall_executed

Emitted when syscall completes successfully.
Data:

```json
{ "syscall_id": "string", "ok": true, "output_ref": "string", "output_sensitivity_s": 0, "output_taint_s": 0 }
```

Rules:

* Output must already be persisted and referenced.

### 4.14 ipc_emit / ipc_receive (optional but recommended)

* `ipc_emit` when an IPCArtifact is produced.
* `ipc_receive` when an operation consumes an IPCArtifact.
  Data:

```json
{ "artifact_id": "string", "producer_operation_id": "string", "consumer_operation_id": "string|null" }
```

### 4.15 capability_update

Emitted when capability snapshot changes.
Data:

```json
{ "capability_snapshot_version": "string", "changed": ["sensor:x", "actuator:y"] }
```

### 4.16 review_queue_update

Emitted when review items created/updated/resolved.
Data:

```json
{ "item_id": "string", "action": "created|updated|resolved", "decision": "approve|reject|edit|null", "applied_version": "string|null" }
```

### 4.17 ingest_job_created

Emitted when an ingestion job is created.
Data:

```json
{ "job_id": "string", "status": "pending|running" }
```

### 4.18 ingest_progress

Emitted at bounded frequency while ingestion is running.
Data:

```json
{
  "job_id": "string",
  "artifacts_total": 0,
  "artifacts_succeeded": 0,
  "artifacts_failed": 0,
  "bytes_ingested": 0
}
```

### 4.19 ingest_artifact_added

Emitted after an artifact row and its corresponding event are persisted.
Data:

```json
{ "job_id": "string", "artifact_id": "string", "kind": "string", "content_ref": "string" }
```

### 4.20 ingest_job_completed / ingest_job_failed / ingest_job_cancelled

Emitted when the ingest job reaches a terminal state.
Data:

```json
{ "job_id": "string", "status": "completed|failed|cancelled", "error_summary": "string|null" }
```

Rules:
- `ingest_job_completed` must carry `status = "completed"`
- `ingest_job_failed` must carry `status = "failed"`
- `ingest_job_cancelled` must carry `status = "cancelled"`

### 4.21 workflow_instance_state (post-wedge)

Emitted on workflow instance transitions.
Data:

```json
{
  "workflow_instance_id": "string",
  "state": "created|running|awaiting_approval|blocked|completed|failed|cancelled",
  "reason": "string|null"
}
```

Rules:

* Must be emitted only after workflow instance state is persisted.

### 4.22 workflow_step_state (post-wedge)

Emitted on workflow step transitions.
Data:

```json
{
  "workflow_instance_id": "string",
  "step_id": "string",
  "step_type": "transform|model_call|syscall|subworkflow",
  "state": "pending|running|awaiting_approval|blocked|completed|failed|skipped|cancelled",
  "linked_operation_id": "string|null",
  "linked_approval_id": "string|null",
  "linked_syscall_id": "string|null"
}
```

Rules:

* Must be emitted only after step state and step transition records are persisted.

### 4.23 interface_instance_ready / interface_instance_stale (post-wedge)

Emitted when an interface instance is compiled or marked stale.
Data:

```json
{
  "interface_instance_id": "string",
  "interface_ref": "string",
  "operation_id": "string|null",
  "workflow_instance_id": "string|null",
  "state": "ready|stale"
}
```

### 4.24 replay_reconstruction_update (post-wedge)

Emitted when replay persists workflow/interface reconstruction reports.
Data:

```json
{
  "replay_id": "string",
  "reconstruction_kind": "workflow|interface",
  "report_ref": "string"
}
```

---

## 5) Persistence rules (what must be stored)

### 5.1 Must be persisted (authoritative)

* all operation state transitions (storage table + transitions log)
* GateDecision, CompiledSlice
* approvals (decision, modified_payload)
* OOB challenge lifecycle state (issued/verified/consumed)
* syscalls + results
* syscall denials
* IPC artifacts
* artifact metadata rows
* ingest job status transitions
* workflow instance state and step transitions
* interface instance payloads and state transitions
* final reasoning output artifact (event_ref or content_ref)
* AuditTrace timeline entries

### 5.2 May be ephemeral (WS only)

* reasoning_stream_chunk
* heartbeat
* intermediate progress hints (percent complete)

---

## 6) UI behavior requirements (so WS stays simple)

1. UI must treat events as notifications and always reconcile via REST for ground truth.
2. UI must dedupe by `event_id`.
3. UI must handle out-of-order delivery by:

   * keeping the latest seen state per operation
   * refreshing details when `audit_update` arrives
4. UI must support reconnect:

   * on reconnect, fetch pending approvals, active operations, and any active workflow/interface instances.

---

## 7) Minimum test cases (must pass)

1. WS disconnect mid-stream:

* UI reconnects, fetches final reasoning output via REST.

2. approval_required emitted:

* approval exists in `/v1/approvals/pending`.

3. syscall_denied emitted:

* `/v1/syscalls/{id}/deny` returns the persisted SyscallDeny.

4. operation_state emitted:

* `/v1/operations/{id}` returns the same state.

5. capability_update emitted:

* `/v1/capabilities` returns the new snapshot version.

6. ingest_job_completed emitted:

* `/v1/ingest/jobs/{job_id}` returns the same terminal status and counters.

7. workflow_step_state emitted (post-wedge):

* `/v1/workflow-instances/{workflow_instance_id}` returns matching persisted step state.

8. interface_instance_ready emitted (post-wedge):

* `/v1/interface-instances/{interface_instance_id}` returns persisted blocks and bindings.
