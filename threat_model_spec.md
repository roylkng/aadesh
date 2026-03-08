# Threat Model Spec v0.1
Adesh OS

This document defines the formal threat model for Adesh OS.

## 0) Security objectives

1. Prevent unauthorized side effects.
2. Prevent unauthorized disclosure across audiences/scopes.
3. Preserve auditability and deterministic accountability.
4. Preserve operation isolation and explicit IPC boundaries.
5. Prevent retry loops and governance bypass.

## 1) Assets to protect

### 1.1 Critical assets
- Root Owner authority and session integrity
- governance rules and negative memory constraints
- approval and OOB lifecycle integrity
- syscall pre-image and execution ordering integrity
- audit traces and replay anchors

### 1.2 Sensitive data assets
- S2/S3/S4 user and system data
- credentials/tokens/identity data
- audience graph policies and disclosure ceilings

### 1.3 Integrity assets
- pinned versions (`active_state_version`, `capability_snapshot_version`, `audience_graph_version`)
- schema refs and hashes
- immutable experience events and version history

## 2) Trust boundaries

1. Root Owner HTTP/WS control plane boundary.
2. External MCP server/tool boundary (semi-trusted or untrusted by default).
3. Model provider boundary (untrusted proposer).
4. Storage/blob/queue boundary (durability and integrity critical).
5. MCP Host external-agent boundary (audience-scoped, default deny).

## 3) Adversaries and capabilities

### 3.1 Prompt-injection adversary
Capabilities:
- inject instructions via web/docs/attachments/tool outputs
Goals:
- induce policy bypass, scope drift, unauthorized execution/disclosure

### 3.2 Malicious or compromised tool backend
Capabilities:
- return malformed payloads
- attempt secret exfiltration or idempotency bypass
Goals:
- trigger unsafe side effects or leak sensitive data

### 3.3 External agent overreach
Capabilities:
- query MCP Host with crafted requests
Goals:
- access data beyond granted scopes/ceilings

### 3.4 Storage integrity adversary
Capabilities:
- corrupt anchors, delete required records, tamper schema/blob hashes
Goals:
- break accountability/replay or hide side effects

### 3.5 Race/replay adversary
Capabilities:
- replay approval/OOB tokens, exploit concurrent runners, duplicate requests
Goals:
- double execution, approval bypass, TOCTOU exploits

## 4) Threats and required mitigations

### 4.1 Unauthorized execution
Threat:
- actuator executed without proper permit/approval
Mitigations:
- syscall pre-image persistence before execution
- `max(R,S)` gate and approval mode enforcement
- atomic approval consumption transaction
- idempotency keys for mutation endpoints

### 4.2 Unauthorized disclosure
Threat:
- data sent to unknown or over-ceiling audience
Mitigations:
- audience graph default deny
- scope policy and ceiling checks
- taint laundering prevention
- explicit sanitization syscall + verification before downgrade

### 4.3 OOB replay/elevation
Threat:
- verified challenge reused or treated as global privilege elevation
Mitigations:
- OOB bound to approval_id
- single-use consume in atomic approval transaction
- explicit prohibition on global session elevation

### 4.4 Drift and instruction hijack
Threat:
- model expands scope/objective or injects tool calls in draft text
Mitigations:
- trajectory alignment checks
- strict model output schema
- deny tool-call injection patterns
- anti-retry trap with bounded attempts

### 4.5 Cross-operation contamination
Threat:
- operation accesses another operation’s memory implicitly
Mitigations:
- strict isolation_id boundaries
- no implicit IPC
- explicit IPCArtifact-only transfer with inherited sensitivity/taint

### 4.6 Audit/replay tampering
Threat:
- missing/corrupt anchors hide activity
Mitigations:
- audit fail-closed invariant
- integrity checks for blob/schema refs
- replay fails deterministically on missing anchors

## 5) Residual risks

1. Provider-level nondeterminism in model text (mitigated by structured output validation and replay Strategy A).
2. Untrusted tool ecosystems may produce adversarial outputs (mitigated by tainting, schema checks, and trust classes).
3. Human approval mistakes remain possible (mitigated by diff/OOB UX and explicit risk labeling).

Residual risk must be monitored through KRIs in `test_and_kri.md` and metrics in `observability_audit.md`.

## 6) Threat-to-test mapping

- Prompt injection and drift: `test_and_kri.md` Suite A/B
- Audience leakage and taint laundering: Suite C/D
- OOB/approval bypass: Suite E
- Idempotency/retry abuse: Suite F/G
- Isolation/IPC violations: Suite H

## 7) Monitoring and response requirements

1. Alert on any non-zero:
- unjustified execution failures
- taint laundering failures
- audit coverage failures
- idempotency violations

2. Log correlation IDs for forensic traceability:
- request_id, operation_id, isolation_id, audit_trace_id, syscall_id, approval_id, challenge_id

3. Incident response requires replayability unless anchors are tombstoned with explicit governance record.

## 8) Minimum test cases

1. Replay OOB challenge reuse fails.
2. Unknown audience disclosure denied.
3. Same denied syscall cannot loop beyond retry policy.
4. Corrupted or missing anchor causes fail-closed replay.
5. Cross-operation data access without IPCArtifact is denied.
