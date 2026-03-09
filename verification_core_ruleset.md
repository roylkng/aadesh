# Verification Core Ruleset (Plan Trajectory, Taint Laundering, Anti-Retry) Spec v0.1
Adesh OS

This document specifies the deterministic logic of the **Verification Core**. It defines:
- how to validate model outputs and tool-call proposals
- how to enforce **Plan Trajectory Alignment** against the Intent Anchor
- how to detect and prevent **Taint Laundering**
- how to generate actionable failures and **Anti-Retry Trap** denial payloads
- how to integrate with approvals (confirm/diff/OOB) without ambiguity
- how high-stakes factual assertions must be evidenced via the Fact Ledger

This is algorithmic logic. Not implementation code.

---

## 0) Inputs and outputs

### Inputs
For a single operation:
- `OperationSpec` (Batch 1)
- `GateDecision` (Batch 2)
- `CompiledSlice` (Batch 2)
- Operation pinned versions:
  - `active_state_version`
  - `capability_snapshot_version`
  - `audience_graph_version`
- Reasoning output (structured JSON) from ModelProvider:
  - `draft_outputs[]`
  - `proposed_syscalls[]`
  - `ipc_requests[]`
  - optional `plan_steps[]`
- Capability snapshot at `capability_snapshot_version`:
  - tool schemas, risk floors, diff support, default approval requirements
- Audience Graph at `audience_graph_version`
- Any `IPCArtifact` objects referenced
- Any prior `SyscallDeny` objects for replanning context (if in retry path)

### Outputs
One of:
1. **VerificationPass**:
   - validated drafts
   - validated syscall proposals with computed syscall-level gates
   - possibly a list of required approvals to request
2. **VerificationFail** (blocking):
   - a structured failure reason
   - remediation options (ask user, reduce scope, sanitize, require approval/OOB, refuse)
   - if failure relates to a syscall, produce `SyscallDeny`

Verification output must be persisted in Experience Log and referenced from AuditTrace.

---

## 1) Preconditions and hard constraints

1. **Structured output required**
- If the reasoning output is not valid JSON or cannot be validated against the expected schema:
  - allow at most 1 constrained retry at the model layer
  - if still invalid: fail operation with verification failure

2. **No implicit tool access**
- Model may only propose syscalls; it cannot execute them.
- Verification must deny any attempt to smuggle tool calls inside draft text.

3. **No external audience scoping via HTTP**
- HTTP callers are Root Owner only; outbound audiences exist only in syscalls.

4. **Approval and OOB are enforced by verification**
- Verification must classify syscalls as:
  - permitted now
  - awaiting approval (confirm/diff/oob_required)
  - denied

5. **High-stakes facts require evidence**
- If a draft output or proposed syscall justification asserts a high-stakes fact, verification must be able to cite accepted-claim evidence or downgrade/refuse the assertion per `fact_ledger_and_reflection_claims.md`.

---

## 2) Verification pipeline order (must be deterministic)

For each operation, Verification Core executes these stages in order:

1. **Parse and normalize reasoning output**
2. **Intent Anchor derivation check**
3. **Plan Trajectory Alignment**
4. **Syscall proposal validation (schema + capability)**
5. **Syscall gate computation (R/S/max)**
6. **Audience Graph disclosure checks**
7. **Taint Laundering checks**
8. **Approval determination and diff requirements**
9. **Anti-Retry trap packaging for denials**
10. **Emit pass/fail result**

If any stage produces a hard deny, do not proceed to later stages for that syscall (but continue validating other syscalls if safe and independent).

---

## 3) Parse and normalize reasoning output

### 3.1 Normalization
Convert model output to canonical form:
- `draft_outputs`: array of objects:
  - `{ "channel": "draft|plan|explanation|other", "text": "..." }`
- `proposed_syscalls`: array of syscall-intents:
  - `{ "target": {kind,name}, "action": "...", "args": {...}, "declared_audience": "...", "data_handles": [...] }`
- `ipc_requests`: array:
  - `{ "type": "pipe", "from": "...", "to": "...", "artifact_kind": "...", "artifact_ref": "..." }`
- `plan_steps`: array:
  - `{ "step": n, "intent": "...", "expected_outputs": [...], "expected_syscalls": [...] }`

### 3.2 Reject tool-call injection in text
If any draft text contains something that matches tool-call protocol formats (JSON arrays of tool calls, explicit function call payloads, etc.) **and** the structured `proposed_syscalls` is empty or inconsistent:
- classify as `verification_failed`
- remediation: instruct model to output proper structured proposals
- do not execute anything

This prevents “tool calls in content” class attacks.

---

## 4) Intent Anchor derivation and validation

### 4.1 Intent Anchor source
Use `CompiledSlice.intent_anchor` as canonical.
If missing (should not happen): derive from `OperationSpec.operation_goal.summary` and mark gate >=2 operations as failed (compiler should have produced it).

### 4.2 Intent anchor checks
Verify that anchor has:
- `goal` non-empty
- optional success criteria, forbidden outcomes, scope limits

If malformed: fail operation (this indicates a kernel bug).

---

## 5) Plan Trajectory Alignment (Intent Drift Defense)

Plan drift is the primary defense against “slow-burn” prompt injection.

### 5.1 Inputs to drift check
- `intent_anchor.goal`
- `intent_anchor.success_criteria`
- `intent_anchor.forbidden_outcomes`
- `intent_anchor.scope_limits`
- `plan_steps` if provided
- otherwise infer plan from:
  - syscall intents
  - draft outputs

### 5.2 Drift categories (deterministic)
A drift event is any of:

#### Drift A: Objective expansion
- introduces a new goal not entailed by the anchor goal
- examples:
  - anchor: “draft email”
  - plan: “draft + send + publish + update profile”

Policy:
- gate <=1: allow if no external side effects and clearly a suggestion
- gate >=2: block and require user confirmation

#### Drift B: Scope expansion
- violates explicit `scope_limits` or introduces new audiences/tools not mentioned or allowed
- examples:
  - sending to a new recipient group
  - using an actuator unrelated to the request

Policy:
- block if any external side effect is involved

#### Drift C: Risk/Sensitivity escalation
- proposed syscalls increase max_gate beyond operation-level expectations without explicit user request
- examples:
  - user asks for summary; agent proposes to delete emails

Policy:
- require approval escalation, or refuse if violates negative memory

#### Drift D: Forbidden outcome proximity
- proposed content or syscall moves toward forbidden outcomes list:
  - leaking secrets
  - self-modification
  - irreversible actions

Policy:
- immediate deny or require OOB if explicitly requested by Root Owner and policy allows

### 5.3 Drift resolution
If drift detected:
- produce a structured `VerificationFail` with:
  - drift type
  - evidence: which plan step or syscall triggered
  - remediation:
    - ask_user to confirm expanded scope
    - reduce_scope (remove steps)
    - refuse (if forbidden)

Operation state:
- set to `blocked` if user can clarify
- set to `failed` if forbidden

Audit:
- add timeline event `verification_fail` with drift details

---

## 6) Syscall proposal validation (schema + capability)

For each proposed syscall:

### 6.1 Capability existence
- If target tool is not enabled or not registered:
  - deny syscall with `deny_class=verification_failed`
  - remediation: alternate_actuator or enable tool (which itself is gated)
  - do not execute

### 6.2 Action existence and schema
- Validate `action` exists for that tool.
- Validate `args` against tool action schema.

If schema invalid:
- deny with `deny_class=verification_failed`
- include `triggering_fields` and missing required fields
- retry_policy allowed only if missing fields can be supplied safely

If capability `execution_class=sandboxed` and the sandbox profile allows network access or persistent mounts:
- require at least `diff` approval at R3
- if a deterministic diff cannot be produced, deny and require a manual path

### 6.3 Forbidden fields check (negative memory)
If args include forbidden fields (SSN, passwords, tokens, etc.) or require them:
- deny with `deny_class=schema_requires_forbidden_field` or `negative_memory_violation`
- remediation: alternate_actuator, manual workflow, refuse

---

## 7) Syscall gate computation (R/S/max)

Verification computes syscall gates using the Governance Kernel logic:
- R from predicates + tool risk floor
- S from data handles + operation taint assumptions
- max_gate = max(R,S)
- approval mode mapping:
  - 0: none
  - 1: none (draft)
  - 2: confirm
  - 3: diff
  - 4: oob_required or refuse

This computation must be deterministic and recorded for audit.

---

## 8) Audience Graph disclosure checks

For outbound syscalls (sends_information_to_third_party):
- resolve target audience node from syscall:
  - explicit declared_audience_id
  - or recipient mapping from args
- Evaluate Audience Graph edge:
  - if unknown: deny (`audience_scope_denied`)
  - if known: obtain allowed scopes and sensitivity ceiling

If syscall attempts to transmit content outside allowed scopes:
- deny with `audience_scope_denied`
- remediation: reduce_scope, ask_user, alternate_actuator

If syscall sensitivity exceeds ceiling:
- deny with `sensitivity_ceiling_exceeded`
- remediation: sanitize, reduce_scope

---

## 9) Taint Laundering detection

Taint laundering is when derived reasoning influenced by sensitive inputs is emitted into lower-sensitivity outputs or sent to audiences with lower ceilings.

### 9.1 Taint inputs
Define:
- `T_operation = CompiledSlice.taint.operation_max_taint_s`
- For each syscall, compute `T_syscall_in`:
  - max of:
    - referenced data_handles taint/sensitivity
    - any evidence snippets used
    - if outbound and not explicitly limited to a sanitized artifact: assume it can leak `T_operation`

### 9.2 Output class and expected sensitivity
Classify the syscall:
- outbound message -> output inherits `T_syscall_in` unless sanitized
- DB update -> may be internal but still sensitive
- publishing -> treat as public S0 ceiling, so high sensitivity becomes violation

### 9.3 Laundering rule (deterministic)
If:
- `T_syscall_in > audience_ceiling` OR
- `T_syscall_in > allowed_output_class_ceiling` (e.g., public publish implies ceiling S0)
Then:
- deny with `deny_class=taint_laundering_risk`
- remediation must include:
  - sanitize syscall (explicit)
  - reduce_scope
  - ask_user

### 9.4 Sanitization requirement
If sanitization is suggested:
- verification must require a dedicated sanitizer syscall producing a new artifact:
  - `IPCArtifact.kind = sanitized_view`
  - with reduced sensitivity label (if truly reduced)
- outbound syscall must reference only that sanitized artifact handle.

No implicit sanitization.

---

## 10) Approval determination and diff requirements

### 10.1 Approval modes
If `approval_mode` computed for syscall is:
- `confirm`: park operation in `awaiting_approval` and produce approval prompt.
- `diff`: must produce a diff payload describing:
  - tool name, action, args
  - what will change in external system
  - recipients or targets
  - any sensitive fields redacted
- `oob_required`: require OOB start/verify endpoints and single-use binding.

### 10.2 Approve with modification
If mode is `diff`, approval endpoint may carry `modified_payload`:
- verification must re-run:
  - schema validation
  - negative memory check
  - audience scope check
  - taint laundering check
- If modified payload increases gate:
  - require new approval at higher mode
  - do not execute

### 10.3 Operation state transitions
If any syscall requires approval:
- operation becomes `awaiting_approval`
- emit WS `approval_required` with payload
- persist approval state (implementation-specific)
- return control to UI without executing

---

## 11) Anti-Retry Trap (prevent infinite loops)

The Verification Core must ensure that denials are actionable and reduce repeated attempts.

### 11.1 When to issue SyscallDeny
Issue a `SyscallDeny` when:
- syscall cannot proceed due to policy, scope, taint, or schema constraints
- syscall lacks approval
- syscall is forbidden

### 11.2 Denial payload requirements
`SyscallDeny` must include:
- deny_class
- violations with:
  - stable constraint_id
  - constraint_type
  - triggering_fields and triggering_refs
  - computed gate and taint fields
- retry_policy:
  - allowed boolean
  - max_attempts
  - conditions
- remediation options:
  - ask_user
  - sanitize
  - alternate_actuator
  - require_approval
  - require_oob
  - reduce_scope
  - refuse

### 11.3 Retry permission rules
Set retry_policy.allowed = false when:
- action category is forbidden
- audience is unknown and cannot be resolved
- negative memory hard violation
- taint ceiling violation without possible sanitization
- self-modification forbidden

Set allowed = true only when remediation can change outcome:
- missing allowed fields
- user confirmation needed
- sanitizer can reduce sensitivity
- alternate actuator exists

### 11.4 Deduping repeated denials
If the same syscall is proposed again without changing args or context:
- do not re-run full checks repeatedly
- return the prior `SyscallDeny` and increment an internal attempt counter
- if max_attempts reached: refuse and block operation

This prevents token-burning loops.

---

## 12) Verification outputs

### 12.1 VerificationPass payload
Must include:
- list of safe draft outputs
- list of syscalls categorized:
  - `permitted_now`
  - `awaiting_approval` with approval payload
  - `denied` with SyscallDeny

### 12.2 VerificationFail payload
If the operation must stop:
- include:
  - failure_reason (drift, invalid output, etc.)
  - remediation options (ask_user, reduce_scope, refuse)
- operation becomes blocked or failed
- audit timeline updated

---

## 13) Minimum test cases (must pass)

1. Plan drift: request “summarize doc” but model proposes “email summary to vendor” -> must require approval or block.
2. Unknown audience: model proposes sending to unknown recipient -> deny with audience_scope_denied, remediation ask_user.
3. Taint laundering: S3 input then propose public tweet -> deny taint_laundering_risk, remediation sanitize/reduce_scope.
4. Schema invalid: missing required args -> deny verification_failed, retry allowed only if safe.
5. Negative memory: password field present -> deny negative_memory_violation, retry not allowed.
6. Diff edit: modified_payload changes recipients -> must re-evaluate audience scopes and possibly increase gate.
7. Anti-retry: same denied syscall repeated -> return same SyscallDeny and stop after max_attempts.
