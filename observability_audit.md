# Observability, Telemetry, and Audit Correlation Spec v0.1
Adesh OS

This document specifies the production-grade observability contract for Adesh OS. It defines:
- what must be logged, metered, and traced
- required correlation identifiers and propagation rules
- how telemetry relates to persisted AuditTrace and Experience Log
- security-sensitive logging rules (no secret leakage)
- required metrics (including KRIs) and recommended dashboards
- event emission requirements for the sync loop, approvals, syscalls, and reflection

This is algorithmic logic. Not implementation code.

---

## 0) Core principles

1. **Correlation-first**
Every significant action must be correlated to:
- `request_id`
- `operation_id`
- `isolation_id`
- `audit_trace_id`
- `syscall_id` (when applicable)
- `approval_id` / `challenge_id` (when applicable)

2. **Storage is ground truth**
Logs and traces are diagnostic. The authoritative record remains:
- Experience Log
- persisted objects referenced by AuditTrace

3. **No secrets in logs**
Never log:
- passwords/tokens/SSNs
- full sensitive document content
- raw attachments
- OOB secrets (nonce material, WebAuthn raw blobs)
Log only references and hashes.

4. **Deterministic event taxonomy**
All telemetry must use stable names and schemas.

---

## 1) Telemetry layers

### 1.1 Structured logs
- JSON structured logs with fixed fields
- used for debugging and incident response

### 1.2 Metrics
- counters, histograms, gauges
- used for health monitoring and risk indicators (KRIs)

### 1.3 Traces (spans)
- distributed traces across:
  - gateway
  - scheduler
  - governance
  - compiler
  - model provider
  - verification
  - tool execution
  - storage
- spans must include correlation ids

### 1.4 Persisted audit anchors
- AuditTrace timeline entries must reference relevant ids and persisted artifacts
- Telemetry should include those ids so a log line can be traced to a concrete audit object

---

## 2) Correlation identifiers and propagation rules

### 2.1 Required identifiers
For any request-handling log line:
- `request_id` (from RequestEnvelope)
- `operation_id` (once operations exist)
- `isolation_id`
- `audit_trace_id`

For syscall logs:
- `syscall_id` mandatory
- tool name and action (safe fields only)

For approval/OOB logs:
- `approval_id`
- `challenge_id` (safe)
- never log nonce or response payloads

### 2.2 Propagation
- Gateway creates/accepts `request_id`
- Scheduler creates operation ids and isolation ids
- Audit system creates `audit_trace_id`
- All downstream spans/logs must attach these fields

### 2.3 Field names (canonical)
Use exact field keys:
- `request_id`
- `operation_id`
- `isolation_id`
- `audit_trace_id`
- `syscall_id`
- `approval_id`
- `challenge_id`
- `capability_snapshot_version`
- `active_state_version`
- `audience_graph_version`
- `gate_r`
- `gate_s`
- `gate_max`
- `approval_mode`
- `audience_target_id` (outbound)
- `sensitivity_ceiling_s`
- `taint_s`

---

## 3) Logging policy (what to log and what not to)

### 3.1 Mandatory log events (sync loop)
At minimum, log these transitions with correlation ids:

- `request.accepted`
- `operation.created`
- `operation.gate_computed`
- `operation.compiled`
- `operation.model_call.start`
- `operation.model_call.end`
- `operation.verified`
- `operation.awaiting_approval`
- `operation.syscall.permitted`
- `operation.syscall.executed`
- `operation.syscall.denied`
- `operation.completed`
- `operation.failed`

Each event log must include:
- ids listed in Section 2
- elapsed time since request start (ms)
- summary-safe fields only

### 3.2 Mandatory log events (approvals/OOB)
- `approval.requested`
- `approval.granted`
- `approval.denied`
- `oob.challenge.issued`
- `oob.challenge.verified`
- `oob.challenge.consumed`
- `oob.challenge.expired`

### 3.3 Mandatory log events (reflection)
- `reflection.job.enqueued`
- `reflection.job.started`
- `reflection.candidates.extracted`
- `reflection.review_item.created`
- `reflection.state_version.minted`
- `reflection.job.completed`
- `reflection.job.failed`

### 3.4 Forbidden logging
Never log:
- raw `CompiledSlice.evidence.snippets[].text` above a small capped length unless gate <= 1 and sensitivity <= S1
- full drafts that contain sensitive content
- syscall args fields that may contain secrets (log a redacted summary instead)
- raw attachment content

### 3.5 Redaction rules
For loggable payloads:
- replace sensitive strings with:
  - `"[REDACTED]"` plus a stable hash of the original to correlate repeated values
- for structured args, redact by key patterns:
  - `password`, `token`, `secret`, `ssn`, `auth`, `cookie`, `key`

---

## 4) Metrics specification (core)

### 4.1 System health metrics
- `agentos_uptime_seconds` (gauge)
- `agentos_build_info` (labels: version, git_sha)
- `storage_up` (gauge)
- `model_provider_up` (gauge)
- `tool_provider_up` (gauge)
- `job_queue_up` (gauge)

### 4.2 Throughput and latency
- `requests_total` (counter)
- `operations_total` (counter, labels: outcome)
- `operation_duration_ms` (histogram, labels: gate_max, outcome)
- `model_call_duration_ms` (histogram, labels: model_id)
- `verification_duration_ms` (histogram)
- `syscall_duration_ms` (histogram, labels: tool, action, outcome)

### 4.3 Governance metrics
- `gate_decisions_total` (counter, labels: gate_r, gate_s, gate_max, approval_mode)
- `approvals_pending` (gauge)
- `approvals_total` (counter, labels: mode, decision)
- `oob_challenges_total` (counter, labels: type, status)

### 4.4 Safety/KRI metrics (must match test_and_kri.md)
- `privacy_leakage_attempts_total` (counter)
- `privacy_leakage_failures_total` (counter)
- `unjustified_execution_failures_total` (counter)  // target 0
- `taint_laundering_failures_total` (counter)       // target 0
- `plan_drift_detected_total` (counter, labels: drift_type)
- `retry_loop_events_total` (counter, labels: constraint_id)
- `audit_coverage_failures_total` (counter)         // target 0
- `idempotency_violations_total` (counter)          // target 0

### 4.5 Memory and budgets
- `token_budget_total` (histogram, labels: gate_max)
- `token_budget_block_used` (histogram, labels: block_name)
- `compiled_slice_omissions_total` (counter, labels: reason, block)
- `operation_taint_level` (histogram, labels: gate_max)

---

## 5) Trace spans (recommended)

Spans must be nested and include correlation ids.

### 5.1 Span names
- `gateway.request`
- `scheduler.decompose`
- `governance.compute_gate`
- `compiler.compile_slice`
- `model.generate`
- `verification.check`
- `approval.consume`
- `tool.execute_syscall`
- `storage.txn`
- `reflection.process_job`

### 5.2 Span attributes (minimum)
- ids from Section 2
- `gate_max`, `approval_mode`
- `model_id` (for model span)
- `tool_name`, `action` (for syscall span, args redacted)
- `state_version` pins

---

## 6) Persisted audit linkage

Every major telemetry event should be linkable to a persisted object.

Rules:
- When GateDecision persisted:
  - log `gate_decision_id` (or ref)
  - emit `audit_update` referencing it
- When CompiledSlice persisted:
  - log `compiled_slice_id`
- When SyscallEnvelope persisted:
  - log `syscall_id`
- When SyscallDeny persisted:
  - log `syscall_id` + `deny_class`
- When ApprovalItem persisted:
  - log `approval_id`
- When OOB challenge persisted:
  - log `challenge_id`

This ensures a log line can be verified against immutable records.

---

## 7) Alerting and dashboards (recommended)

### 7.1 Alerts (hard)
- `unjustified_execution_failures_total > 0`
- `taint_laundering_failures_total > 0`
- `audit_coverage_failures_total > 0`
- `idempotency_violations_total > 0`

### 7.2 Alerts (soft)
- `operation_duration_ms p95` above threshold
- `model_call_duration_ms p95` above threshold
- `approvals_pending` above threshold (approval backlog)
- `storage_up == 0`

### 7.3 Dashboard panels
- Operations by gate level and outcome
- Approval funnel (pending -> granted/denied)
- Syscalls by tool/action, denied reasons
- Drift detections over time
- OOB usage rate
- CompiledSlice omissions by block/reason
- Reflection: candidates extracted, review items created, acceptance rate

---

## 8) Minimum test cases (must pass)

1. Every operation emits logs with request_id/op_id/isolation_id/audit_trace_id.
2. Any syscall execution emits syscall_id and tool/action.
3. No sensitive fields appear in logs under redaction tests.
4. KRIs counters remain correct under adversarial test suite.
5. A log line can be traced to an AuditTrace reference for gate/compiled/syscall.

