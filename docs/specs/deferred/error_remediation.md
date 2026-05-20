# Error Taxonomy and Remediation Catalog Spec v0.1
Adesh OS

This document defines the canonical **error taxonomy**, **constraint id conventions**, and **remediation payload shapes** used across Adesh OS. Its goal is to eliminate ambiguity for the coding agent and UI:
- every denial is structured
- every failure is classifiable
- every retry is bounded
- every remediation is actionable

This spec governs:
- REST error responses
- `SyscallDeny` construction (Batch 3)
- Verification failures
- Approval failures
- Storage/transaction failures

This is algorithmic logic. Not implementation code.

---

## 0) Design goals

1. **No generic “blocked”**
- Any denial must specify what constraint was violated and how to proceed.

2. **Deterministic classification**
- Same situation produces same error code and constraint ids.

3. **Retry is explicitly controlled**
- “Try again” is never implicit. Retry must be allowed with conditions and a max attempts.

4. **UI-first payloads**
- Remediation options include structured payloads so UI can render the next action.

---

## 1) Top-level REST error codes

REST endpoints return the standard envelope:
- `ok=false`
- `error.code` from the list below
- `error.message` for humans
- `error.details` structured

### 1.1 REST error.code enums
- `UNAUTHORIZED` (no session)
- `FORBIDDEN` (not Root Owner or lacks scope)
- `NOT_FOUND` (object missing)
- `CONFLICT` (stale approval, consumed OOB, version mismatch)
- `INVALID_INPUT` (schema/validation failure)
- `TRANSIENT` (retryable infra failure)
- `PERMANENT` (non-retryable server error)
- `RATE_LIMITED` (optional)
- `TIMEOUT` (optional)

### 1.2 Mapping rules
- Validation failures (contract/schema): `INVALID_INPUT`
- Stale approval / already consumed: `CONFLICT`
- Storage down: `TRANSIENT`
- Audit-critical persistence failure: `PERMANENT` (fail closed)
- Non-root callers: `FORBIDDEN`

---

## 2) Internal error categories (kernel-level)

These categories are used in logs, audit notes, and verification outputs.

### 2.1 KernelErrorCategory
- `storage_failure`
- `schema_invalid`
- `policy_denied`
- `approval_required`
- `oob_required`
- `verification_failed`
- `model_output_invalid`
- `tool_execution_failed`
- `timeout`
- `budget_exceeded`
- `rate_limited`
- `conflict_stale`

Kernel must map these to REST codes and/or SyscallDeny deny_class deterministically.

---

## 3) SyscallDeny: deny_class enums and canonical triggers

`SyscallDeny.deny_class` must be one of:

- `audience_scope_denied`
- `sensitivity_ceiling_exceeded`
- `negative_memory_violation`
- `gate_requires_approval`
- `taint_laundering_risk`
- `self_modification_forbidden`
- `schema_requires_forbidden_field`
- `budget_exceeded`
- `verification_failed`

### 3.1 Trigger mapping
- Unknown audience edge or scope not allowed -> `audience_scope_denied`
- S or taint above ceiling -> `sensitivity_ceiling_exceeded` or `taint_laundering_risk` (prefer taint when derived influence is the issue)
- Forbidden data/action -> `negative_memory_violation`
- Approval needed -> `gate_requires_approval`
- Self-modification without OOB or forbidden -> `self_modification_forbidden`
- Tool schema requires forbidden field -> `schema_requires_forbidden_field`
- Token/time/cost budget constraints violated -> `budget_exceeded`
- Drift, invalid output structure, tool call injection -> `verification_failed`

---

## 4) Constraint IDs: stable naming scheme

Constraint ids must be stable and machine-parseable.

### 4.1 Canonical formats
- Policy rules:
  - `policy::<rule_name>`
  - Example: `policy::never_act.send_passwords`
- Gate and approval:
  - `gate::r{R}_s{S}::mode::<mode>`
  - Example: `gate::r3_s2::mode::diff`
- Audience graph:
  - `audience::<src_id>::<dst_id>::scope::<scope_id>`
  - Example: `audience::root_owner::board::scope::board_email`
- Sensitivity ceiling:
  - `ceiling::<audience_id>::s{S_ceiling}`
  - Example: `ceiling::vendor_x::s1`
- Taint laundering:
  - `taint::in_s{S_in}::ceiling_s{S_ceiling}::requires_sanitizer`
- Budget:
  - `budget::token_total::<N>`
  - `budget::block::<block_name>::<N>`
  - `budget::latency_ms::<N>`
- Schema:
  - `schema::<tool_name>::<action>::missing::<field>`
  - `schema::<tool_name>::<action>::forbidden::<field>`
- Capability:
  - `capability::disabled::<tool_name>`
  - `capability::unknown::<tool_name>`

### 4.2 ConstraintType mapping
`SyscallDeny.violations[].constraint_type` must be:
- `policy`
- `audience_scope`
- `gate`
- `taint`
- `budget`
- `schema`
- `verification`

---

## 5) Retry policy semantics (anti-retry)

`SyscallDeny.retry_policy` controls whether the OS should allow reattempt.

### 5.1 Default rules
Set `allowed=false` when denial is structural:
- negative memory hard deny
- unknown audience edge (unless remediation is “add edge”)
- self-modification forbidden
- taint ceiling violation without sanitizer option
- schema requires forbidden field (passwords, tokens, SSNs)

Set `allowed=true` only when there is a concrete remediation that changes inputs:
- approval required
- sanitize available and permitted
- missing non-sensitive fields
- alternate actuator exists

### 5.2 max_attempts
- default:
  - `max_attempts=1` for most retryable denials
  - `max_attempts=0` for non-retryable
- For repeated identical proposals:
  - after `max_attempts` reached, operation transitions to `blocked` with `ask_user` remediation only

### 5.3 cooldown_ms
- optional; if present, it is advisory for the scheduler to delay retries.

---

## 6) Remediation options: types and payload schemas

Remediation options are UI/actionable next steps.
`SyscallDeny.remediation.options[]` must include one or more of:

### 6.1 RemediationType enums
- `ask_user`
- `sanitize`
- `alternate_actuator`
- `require_approval`
- `require_oob`
- `reduce_scope`
- `refuse`

### 6.2 Payload shapes

#### ask_user
Used when clarification or explicit user decision is needed.
Payload:
```json
{
  "question": "string",
  "choices": ["string", "..."] ,
  "default_choice": "string|null"
}
```

#### sanitize

Used when taint/sensitivity must be reduced via explicit sanitizer syscall.
Payload:

```json
{
  "requires_sanitizer_syscall": true,
  "source_handles": ["artifact_id|event_ref|content_ref"],
  "target_artifact_kind": "sanitized_view",
  "required_ceiling_s": 1
}
```

#### alternate_actuator

Used when the chosen tool is disabled/unsupported or requires forbidden fields.
Payload:

```json
{
  "candidates": [
    { "tool": "string", "action": "string", "reason": "string" }
  ]
}
```

#### require_approval

Used when the denial is purely gating.
Payload:

```json
{
  "approval_mode": "confirm|diff",
  "approval_id": "string",
  "prompt": "string",
  "diff": {}
}
```

#### require_oob

Used when OOB is required.
Payload:

```json
{
  "approval_id": "string",
  "oob_methods": ["webauthn","totp","device_signature"],
  "start_endpoint": "/v1/approvals/{approval_id}/oob/start"
}
```

#### reduce_scope

Used when the action is allowed only with reduced content, fewer recipients, or narrower query.
Payload:

```json
{
  "recommended_changes": [
    { "field": "string", "from": "any", "to": "any", "reason": "string" }
  ]
}
```

#### refuse

Used when no safe path exists.
Payload:

```json
{
  "reason": "string",
  "policy_refs": ["constraint_id", "..."]
}
```

---

## 7) Verification failures: canonical reasons and mapping

Verification failures are not always syscall-specific. They can block the operation.

### 7.1 VerificationFailReason enums (logical)

* `model_output_invalid`
* `tool_call_injection`
* `plan_drift_objective_expansion`
* `plan_drift_scope_expansion`
* `plan_drift_risk_escalation`
* `forbidden_outcome_proximity`
* `taint_laundering_detected`
* `schema_validation_failed`
* `capability_unavailable`
* `budget_violation`
* `internal_invariant_broken`

### 7.2 Mapping to response

* If failure relates to a specific syscall: produce `SyscallDeny` with deny_class `verification_failed`.
* If it blocks operation globally: return REST `ok=true` but operation state becomes `blocked` and WS emits `operation_state` blocked with reason, plus `audit_update`.

Never return generic 500 when the cause is user-fixable.

---

## 8) Approval errors (REST and audit)

Approval endpoints (`POST /v1/approvals/...`) must return deterministic errors.

### 8.1 Approval error cases

* stale approval_id / already consumed -> `CONFLICT` with details:

  * `{ "approval_id": "...", "status": "consumed|expired|superseded" }`
* missing required OOB -> `INVALID_INPUT`:

  * `{ "required": "oob", "approval_id": "..." }`
* OOB expired or consumed -> `CONFLICT`
* modified_payload increases gate -> `CONFLICT` with:

  * `{ "requires_new_approval": true, "new_approval_id": "..." }`
* modified_payload schema invalid -> `INVALID_INPUT` with violations

All approval failures must append an Experience Log event `kind=approval_failed` and update audit timeline.

---

## 9) Storage/audit failures: fail-closed rules

If any of these fail:

* append request event
* persist syscall envelope before execution
* persist audit trace update
* persist approval decision before execution
  then:
* operation fails closed (`failed`)
* return REST `PERMANENT`
* do not execute any actuators

These are invariants. No exceptions.

---

## 10) Minimum test cases (must pass)

1. Deny payload completeness:

* any denial must include at least one violation with constraint_id and remediation options.

2. Anti-retry:

* identical denied syscall proposed twice -> same denial returned, attempt counted, then operation blocked.

3. Approval failure:

* reuse consumed OOB -> `CONFLICT` and no side effects.

4. Schema forbidden:

* tool requires password -> deny with schema_requires_forbidden_field and refuse/alternate actuator remediation.

5. Budget:

* token budget exceeded -> denial uses budget constraint ids and suggests reduce_scope.
