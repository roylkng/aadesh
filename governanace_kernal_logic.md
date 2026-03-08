```md id="a3c1n7"
# Governance Kernel Logic (R/S Math) Spec v0.1
Adesh OS

This document specifies the deterministic logic of the **Governance Kernel**. It defines:
- how **Action Risk (R)** and **Data Sensitivity (S)** are computed for operations and syscalls
- how **max_gate = max(R,S)** is enforced
- how **ApprovalMode** is derived
- how **Audience Graph scopes** and **sensitivity ceilings** are applied
- how to produce **policy-aware denials** (`SyscallDeny`) with stable constraint ids and remediation options

This is algorithmic logic. Not implementation code.

---

## 0) Inputs and outputs

### Inputs (operation-level)
- `OperationSpec` (Batch 1)
- `RequestEnvelope` (Batch 1) referenced by `parent_request_id`
- pinned versions:
  - `active_state_version`
  - `capability_snapshot_version`
  - `audience_graph_version`
- Audience Graph snapshot at `audience_graph_version`
- Capability snapshot at `capability_snapshot_version`
- Referenced inputs:
  - attachments (from RequestEnvelope)
  - experience event refs (from operation_goal.input_refs)
  - IPC artifacts consumed (from OperationSpec.ipc.consumes_artifacts)

### Outputs (operation-level)
- `GateDecision` (Batch 2)
  - includes: `risk.level`, `risk.predicates`
  - includes: `sensitivity.level`, `sensitivity.sources`
  - includes: `max_gate`
  - includes: `scopes.allowed/denied`, `scopes.sensitivity_ceiling`
  - includes: `constraints` (negative memory, budgets, taint policy)
  - includes: `approval.mode` (often "none" at operation-level unless the operation is itself a direct high-stakes action request)

### Inputs (syscall-level)
- Proposed syscall intent from Reasoning output
- `GateDecision` and `CompiledSlice`
- Capability descriptor for target tool/action:
  - risk floor
  - diff support
  - approval requirements
  - schema requirements
- Any data handles referenced:
  - evidence refs
  - attachments
  - IPC artifacts
  - tool results

### Outputs (syscall-level)
- `SyscallEnvelope` with computed gate fields
- OR `SyscallDeny` if denied before execution
- OR transition to `awaiting_approval` with approval payload

---

## 1) Vocabulary

### Risk level R (Action Risk)
- R0 passive/transformative (no external side effects)
- R1 generative drafts (user sends manually)
- R2 medium-stakes active (external side effect but reversible/low impact)
- R3 high-stakes (money/accounts/publishing/privileged changes)
- R4 critical/irreversible (legal/health/identity/system self-modification/mass destructive)

### Sensitivity level S (Data Sensitivity)
- S0 public
- S1 internal/routine
- S2 confidential
- S3 restricted (PII, credentials, financials, internal secrets)
- S4 regulated/critical (identity-level secrets, auth tokens, core OS config, irreversible destructive data)

### Universal predicates (examples)
Predicates are boolean facts about an operation or syscall.
They drive R and sometimes S.
- has_external_side_effect
- sends_information_to_third_party
- publishes_publicly
- touches_money_or_accounts
- touches_identity_or_security
- touches_health_or_legal
- is_irreversible_or_mass_impact
- attempts_self_modification
- accesses_sensitive_memory
- requires_oob_auth
- uses_untrusted_source
- audience_out_of_scope

---

## 2) Deterministic computation: Data Sensitivity S

### 2.1 Sensitivity sources collection
For operation-level sensitivity, collect sources:

1. Attachments from `RequestEnvelope.input.attachments[]`
2. Experience Log event refs listed in `operation_goal.input_refs[]`
3. IPC artifacts in `OperationSpec.ipc.consumes_artifacts[]`
4. Any explicitly referenced content refs already ingested (e.g., email ids, file ids)

Each source is represented as a `SensitivitySource`:
- kind = attachment|event_ref|ipc_artifact|tool_result|inferred
- ref_id = stable reference
- sensitivity_hint = optional (0..4)

### 2.2 Sensitivity hint resolution
For each source, resolve a concrete sensitivity value:

Priority order:
1. If the source has an explicit stored sensitivity label in metadata: use it.
2. Else if there is a sensitivity_hint provided: use it.
3. Else infer using heuristics (conservative default):
   - unknown user-provided docs default to S2
   - unknown system telemetry default to S1
   - unknown external web content default to S1 but tainted (handled separately)
4. Apply promotion:
   - if any source is PII/credential-like (by classifier): minimum S3
   - if any source is core OS config or secret: S4

### 2.3 Operation sensitivity aggregation
Compute:
- `S_operation = max(sensitivity(source_i))`

Set `GateDecision.sensitivity.level = S_operation`
Set `GateDecision.sensitivity.sources = collected sources`

### 2.4 Syscall sensitivity
For each syscall proposal, compute `S_syscall` from:
- any data_handles referenced in the syscall intent
- plus the CompiledSlice taint if the syscall uses or could leak compiled content (default yes for outbound communications)

Rule:
- if syscall is outbound (sends_information_to_third_party), assume it *could* leak compiled context unless explicitly constrained to a sanitized artifact handle.

Compute:
- `S_syscall = max( sensitivity(data_handles), S_inherited_from_operation_if_applicable )`

---

## 3) Deterministic computation: Action Risk R

### 3.1 Capability risk floor
Each actuator (and some sensors) has a declared **risk floor**:
- `R_floor(tool, action)`

Rules:
- R of a syscall can never be below its tool’s risk floor.

### 3.2 Predicate-based risk mapping
Compute risk predicates based on syscall/operation properties.

The kernel maintains a deterministic mapping:

#### R4 triggers (any => R4)
- attempts_self_modification
- touches_health_or_legal with action side effects (not just reading)
- is_irreversible_or_mass_impact
- deletes_large_amount_of_data
- modifies_core_identity_or_security (passwords, auth tokens, root owner, policy kernel)
- executes_untrusted_code_with_privilege

#### R3 triggers (any => at least R3)
- touches_money_or_accounts
- publishes_publicly
- modifies_account_settings
- sends_to_large_audience_or_external_org
- privileged infrastructure change (deploy to prod, rotate keys) unless explicitly downgraded by policy

#### R2 triggers (any => at least R2)
- has_external_side_effect AND not covered by R3/R4
- sends_information_to_third_party (email, slack message, API write)
- schedules/creates events or tasks in external systems
- updates non-critical DB fields or internal records

#### R1 triggers
- generative draft only (no side effect)
- brainstorming, rewriting, summarization of user-provided text

#### R0 triggers
- local formatting/transformation with no new content and no side effects

### 3.3 Operation risk aggregation
Operation risk is computed from:
- inferred intended effects from the request text (best-effort)
- any explicit requested outputs or actions
- conservative default: if unknown, assume R1

Compute:
- `R_operation = max( R_from_predicates, R_from_capabilities_if_known )`

### 3.4 Syscall risk aggregation
For a syscall:
- `R_syscall = max( R_floor, R_from_predicates(syscall) )`

Where predicates are derived from:
- target_kind: actuator implies side effect unless declared otherwise
- declared_effect field if present
- presence of outbound audience id
- tool/action metadata

---

## 4) max_gate and ApprovalMode derivation

### 4.1 max_gate
For operation:
- `max_gate_operation = max(R_operation, S_operation)`

For syscall:
- `max_gate_syscall = max(R_syscall, S_syscall)`

### 4.2 ApprovalMode policy
ApprovalMode for syscalls is deterministic:

- If `max_gate_syscall == 0`: `none`
- If `max_gate_syscall == 1`: `none` (draft-only) unless policy says confirm
- If `max_gate_syscall == 2`: `confirm` by default
- If `max_gate_syscall == 3`: `diff` required (must show diff)
- If `max_gate_syscall == 4`: `oob_required` OR `refuse` depending on policy

Additional rules:
- If tool supports diff and gate >= 3: must use `diff`. If tool cannot produce diff: deny with remediation or force manual path.
- If policy explicitly forbids the action category: `refuse` even if gate is lower.

### 4.3 Operation-level approval mode
Generally, approvals are syscall-specific. Operation-level approval.mode is used for:
- operations that are direct self-modification requests
- operations that request batch destructive actions
- operations that are purely an approval wrapper for a single syscall

Otherwise, set operation approval.mode = `none` and enforce at syscall stage.

---

## 5) Audience Graph evaluation and disclosure ceilings

### 5.1 Control plane Root Owner
For HTTP control plane:
- caller is Root Owner
- treat as global view for reads
- still enforce negative memory, self-mod rules, and taint laundering constraints for outbound actions

### 5.2 Outbound audience scoping
For syscalls that send information to an audience:
- determine `audience_id_target`:
  - from syscall intent declared_audience_id
  - or inferred from tool target (e.g., email recipient)
- Evaluate Audience Graph edge (Root Owner -> target):
  - if unknown edge: default deny
  - if edge exists: fetch:
    - allowed scopes
    - sensitivity ceiling

Set:
- `scopes.allowed`, `scopes.denied`, `scopes.sensitivity_ceiling`

If the syscall’s computed S or taint exceeds audience ceiling:
- deny with `sensitivity_ceiling_exceeded` or `taint_laundering_risk`
- remediation: sanitize, reduce_scope, ask_user

---

## 6) Negative memory enforcement

Negative memory defines hard constraints:
- Never store: do not persist certain data classes into Active State or logs
- Never act: disallow certain syscall categories
- Do not assume: force clarification for certain topics
- Forget/expire: session-bound contexts must not persist

Enforcement points:
1. During compilation: omit forbidden data classes from blocks.
2. During verification: deny syscalls requiring forbidden fields.
3. During execution: deny if tool requires forbidden field.

If a syscall schema requires a forbidden field:
- deny with `schema_requires_forbidden_field`
- remediation:
  - alternate_actuator
  - manual workflow
  - ask_user for different approach

---

## 7) Policy-aware denial construction (SyscallDeny)

### 7.1 Deny classes
Use deterministic mapping:
- audience scope issues -> `audience_scope_denied`
- ceiling exceeded -> `sensitivity_ceiling_exceeded`
- negative memory -> `negative_memory_violation`
- requires approval -> `gate_requires_approval`
- taint laundering -> `taint_laundering_risk`
- forbidden self-mod -> `self_modification_forbidden`
- schema requires forbidden field -> `schema_requires_forbidden_field`
- budget -> `budget_exceeded`
- verification failure -> `verification_failed`

### 7.2 Stable constraint ids
Constraint ids must be stable strings, not ad-hoc text. Format:
- `policy::<rule_name>`
- `audience::<src>::<dst>::<scope_id>`
- `gate::<r>::<s>::<mode>`
- `taint::<ceiling>::<taint>`
- `budget::<total>::<block>`

### 7.3 Retry policy
Retry must not loop blindly.
- `allowed = false` if the violation is structural (forbidden action, ceiling, negative memory).
- `allowed = true` only if remediation exists that can change the outcome:
  - sanitize
  - remove forbidden fields
  - supply missing required inputs that are allowed
- `max_attempts` default 1 unless explicitly configured.

### 7.4 Remediation options
Must be a list of concrete options:
- ask_user (clarify or choose alternate)
- sanitize (explicit sanitizer syscall)
- alternate_actuator
- require_approval (if denial is only gating)
- require_oob
- reduce_scope
- refuse

---

## 8) Deterministic outputs

### 8.1 GateDecision object
Populate all required fields:
- risk.level, risk.predicates
- sensitivity.level, sensitivity.sources
- max_gate
- scopes.allowed/denied + ceiling
- constraints including:
  - negative memory lists
  - token budgets from operation budgets
  - taint policy enabled
  - intent_anchor_required based on gate
- approval.mode (operation-level, usually none)

Persist and attach to AuditTrace.

### 8.2 Syscall gate fields
Populate `SyscallEnvelope.gate` deterministically as above.
Persist syscall record before execution.

---

## 9) Minimum test cases (must pass)
1. If any input source is S3, operation S must be >= 3.
2. If an actuator has risk floor R3, syscall R must be >= 3 regardless of model intent.
3. If a syscall requires approval (gate >= 2), kernel must not execute and must park operation in awaiting_approval.
4. If outbound audience edge is unknown, default deny.
5. If tool schema requires forbidden field and negative memory forbids it, deny with remediation.
6. Denials must return structured violations and remediation. No generic “blocked” errors.
```
