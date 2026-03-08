# JIT Compiler Algorithm (Context Packing, Provenance, Taint) Spec v0.1
Adesh OS

This document specifies the deterministic algorithm for the **JIT Compiler** that produces a `CompiledSlice` (Batch 2). It defines:
- the exact inputs required
- the retrieval and filtering of primitives and evidence
- the deterministic **packing** of memory blocks within token budgets
- **taint** computation per block and for the whole operation
- **omissions** and auditability rules
- conflict resolution when primitives contradict
- sanitization requirements and how to represent them in the slice

This is algorithmic logic. Not implementation code.

---

## 0) Inputs and outputs

### Inputs
For a single operation:
- `OperationSpec` (Batch 1)
- `GateDecision` (Batch 2)
- pinned versions:
  - `active_state_version`
  - `capability_snapshot_version`
  - `audience_graph_version`
- Active State snapshot at `active_state_version`, containing:
  - policy primitives (negative memory, boundaries)
  - identity primitives (commitments, stable preferences)
  - operational profile primitives (format preferences)
  - hypothesis ledger (probabilistic traits/preferences)
  - provenance pointers to Experience Log refs
- Audience Graph snapshot at `audience_graph_version`
- Capability snapshot at `capability_snapshot_version`
- Operation context sources:
  - `operation_goal.input_refs[]` (experience events)
  - attachments refs (from RequestEnvelope)
  - consumed IPC artifacts (from OperationSpec.ipc)
- Token budgets:
  - `token_budget_total`
  - per-block budgets (policy, capability, operation_context, evidence, scratch)

### Output
- `CompiledSlice` (Batch 2)
  - blocks:
    - policy
    - capability
    - operation_context
    - evidence
    - scratch
  - `gate` summary
  - `intent_anchor`
  - `taint` summary
  - `omissions`
  - `provenance_summary`
  - `audit_trace_id`

---

## 1) Core invariants the compiler must enforce

1. **Non-truncatable Policy Block**  
   Policy block must always be present and non-empty. If policy block cannot fit within its token budget, compilation fails.

2. **Deterministic packing order**  
   Always pack blocks in this order:
   1) policy
   2) capability
   3) operation_context
   4) evidence
   5) scratch

3. **No raw Experience Log injection**  
   The compiler must never inject the full raw Experience Log. Only minimal snippets with provenance refs.

4. **Gate-sensitive filtering**  
   As `max_gate` increases, the slice must become **more conservative**:
   - include fewer low-confidence hypotheses
   - require stronger evidence
   - reduce speculative personalization

5. **Operation taint is the maximum of block taints**  
   `operation_max_taint_s = max(block.taint_s)` exactly.

6. **Omissions must be explicit**  
   If anything is omitted due to budgets or policy, record it in `omissions.omitted_items[]` with a stable reason.

7. **Audience-safe by construction**  
   The compiler must enforce audience constraints by excluding forbidden scopes from the slice.

---

## 2) Token accounting model

The compiler must implement a deterministic token accounting model. Implementation can use approximate token counts, but the method must be consistent.

### TokenCount function
Define `TokenCount(text) -> int`:
- Deterministic approximation for budgeting.
- Must be used consistently in all packing decisions.

### Budget enforcement rule
For each block `B`:
- Maintain `remaining_tokens[B]`.
- Never allow `TokenCount(content[B]) > budget[B]`.
- For total:
- Ensure `sum(TokenCount(block_content)) <= token_budget_total`.

If total exceeds even after truncation of lower priority blocks:
- reduce lower priority blocks further
- if still cannot fit without truncating policy block: fail compilation

---

## 3) Intent Anchor derivation (if missing)

If `RequestEnvelope.intent_anchor` exists, use it.

Else derive deterministically:
- goal: summary = `OperationSpec.operation_goal.summary`
- success_criteria: empty unless explicit in request text (optional in v0)
- forbidden_outcomes: include default safety constraints:
  - “do not leak sensitive info”
  - “do not execute irreversible actions without approval”
- scope_limits: derived from Audience Graph (Root Owner global read, outbound constrained)

The derived intent anchor must be stored in `CompiledSlice.intent_anchor`.

---

## 4) Primitive retrieval and filtering (Active State)

The compiler retrieves **primitives** (structured persona state) from Active State. Primitives are always accompanied by:
- `primitive_id`
- `kind` (policy/identity/operational/hypothesis)
- `context_predicates`
- `time_bounds`
- `evidence_refs[]`
- `evidence_quality` (confirmed/repeated/single/untrusted, recency, conflicts)
- `confidence` (0..1)

### 4.1 Gate tiers and evidence thresholds
Define tiers based on `max_gate`:

- Gate 0–1 (low):
  - allow hypothesis primitives if confidence >= 0.4
  - allow single-artifact evidence if source reliability is not “untrusted”
- Gate 2 (medium):
  - allow hypothesis primitives if confidence >= 0.7 OR evidence_quality >= repeated_observation
  - prefer user-confirmed or repeated observation
- Gate 3–4 (high):
  - only user-confirmed or repeated observation primitives
  - discard low-confidence personalization
  - discard speculative style inference unless strongly supported

### 4.2 Applicability filter (context/time)
A primitive is applicable if:
- current operation context satisfies its `context_predicates` (if any)
- current time is within `time_bounds` (if any)
- audience constraints allow it (scoped disclosure)

If predicate evaluation is uncertain:
- treat as non-applicable at gate >= 2
- treat as applicable at gate <= 1 only if it does not affect safety-critical decisions

### 4.3 Conflict set construction
Applicable primitives may contradict.
Build conflict sets by:
- explicit contradiction links from hypothesis ledger
- or same attribute key with mutually exclusive values

Conflict sets are resolved by the deterministic algorithm in Section 7.

---

## 5) Evidence snippet selection (Experience Log)

Evidence is used to ground responses and reduce hallucination.

### 5.1 Evidence candidates
Candidates come from:
- `operation_goal.input_refs[]`
- attachments refs
- IPC artifact provenance refs
- primitive evidence_refs for any included primitives

### 5.2 Evidence inclusion policy by gate
- Gate 0–1:
  - include minimal snippets only when needed for grounding
- Gate 2:
  - include snippets for any claim likely to influence an external side effect
- Gate 3–4:
  - include only high-trust snippets and only the minimum necessary
  - avoid including raw sensitive details if they are not essential for action

### 5.3 Snippet extraction
For each evidence ref:
- Extract at most `N` characters or `K` tokens (implementation-defined) from the most relevant portion.
- Must store:
  - `ref_id`
  - `text`
  - `sensitivity_s` of that source
  - provenance metadata (source_class, artifact_ids)

### 5.4 Evidence taint
Evidence block taint is:
- `taint_s(evidence_block) = max(sensitivity_s of included snippets)`

---

## 6) Block-by-block compilation and packing

### 6.1 Policy Block (non-truncatable)
Policy Block must include:
- Negative Memory:
  - never_store
  - never_act
  - do_not_assume
  - forget/expire
- Core invariants summary:
  - “syscalls only”
  - “max(R,S) gating”
  - “no implicit IPC”
  - “no self-mod without OOB”
- Approval rules summary:
  - confirm/diff/OOB/refuse mapping
- Audience Graph constraints relevant to the operation:
  - default deny for unknown targets
  - sensitivity ceiling for any known intended audience if already specified

Policy Block taint:
- Usually S0–S1 (policy text is system-owned)
- If policy block includes sensitive user-specific boundaries (e.g., “never disclose project X”), taint = S2 or higher as appropriate.

Packing:
- Must fit in `block_budget.policy`.
- If it does not fit: compilation fails.

### 6.2 Capability Block
Capability Block includes:
- list of enabled sensors and actuators (names only)
- per tool metadata:
  - risk floors
  - diff support
  - required approvals
- budget summary (token/time/cost)
- explicit “cannot do” self-model statements (limits)

Capability Block taint:
- S0–S1 (system metadata)
- If revealing tool names itself is sensitive in a deployment, raise accordingly (rare).

Packing:
- Include as much as fits in `block_budget.capability`, prioritize actuators over sensors.
- If overflow: omit least relevant tools and record omission reason `token_budget_exceeded`.

### 6.3 Operation Context Block
Operation Context includes:
- the operation goal summary
- minimal relevant user preferences/identity constraints (as primitives)
- relevant commitments (if the request touches commitments)
- minimal necessary context from recent events (not raw chat history)

Selection priority:
1. Constraints that affect safety, disclosure, or approvals
2. Commitments and deadlines relevant to the goal
3. Preferences relevant to output format
4. Style/voice rules (only if gate <= 2 or strongly evidenced)

Taint:
- `taint_s(operation_context) = max(sensitivity of primitives included, sensitivity of any event snippets included here)`

Packing:
- Start with an empty context string.
- Add items in priority order until budget reached.
- If budget reached:
  - stop adding new items
  - record omitted_items with reason `token_budget_exceeded`

### 6.4 Evidence Block
Evidence block includes snippets per Section 5.

Taint:
- computed as max snippet sensitivity.

Packing:
- Add snippets in ranked order of relevance:
  - direct attachments > direct input_refs > IPC provenance > primitive evidence_refs
- Stop when token budget reached.
- Record omissions if snippets dropped.

### 6.5 Scratch Block (ephemeral)
Scratch starts empty (or minimal guidance) and includes:
- a short instruction that scratch is ephemeral and must not be treated as truth
- expiration timestamp

Taint:
- if empty guidance only, S0
- if pre-populated with sensitive intermediate notes, taint follows content sensitivity (rare at compile time)

Packing:
- Must fit. Usually trivial.

---

## 7) Deterministic conflict resolution among primitives

When two or more applicable primitives contradict, resolve using this strict order:

1. **Applicability**  
   Discard primitives whose context predicates do not match.

2. **Constraint severity**  
   Negative memory, hard boundaries, and safety rules dominate preferences.

3. **Explicit exception**  
   A user-confirmed rule with a more specific matching context predicate may override a broader boundary if explicitly marked as an exception.

4. **Evidence quality tier**
   - user_confirmed > repeated_observation > single_artifact > untrusted_external

5. **Specificity**
   More specific predicates beat less specific ones within the same evidence tier.

6. **Recency**
   Break ties by newest evidence timestamp.

If still unresolved:
- At gate <= 1: include both with uncertainty notes.
- At gate >= 2: exclude both and require user clarification (compiler should set a flag in omissions/details for verifier to trigger clarification).

All conflicts and resolutions must be recorded in `provenance_summary.primitive_refs` and optionally in audit trace notes (implementation choice).

---

## 8) Taint computation and sanitization requirement

### 8.1 Per-block taint
- policy taint: based on included user-specific sensitive constraints
- capability taint: usually low
- operation_context taint: max sensitivity of included primitives/events
- evidence taint: max snippet sensitivity
- scratch taint: as set

### 8.2 Operation max taint
Compute:
- `operation_max_taint_s = max(policy, capability, operation_context, evidence, scratch)`

Set:
- `CompiledSlice.taint.operation_max_taint_s = operation_max_taint_s`

### 8.3 Sanitization-required flag
If the operation is likely to produce an outbound output (any actuator enabled that sends to third parties, or model proposes it typically), compute:

- `audience_ceiling = GateDecision.scopes.sensitivity_ceiling`
- If `operation_max_taint_s > audience_ceiling`:
  - set `sanitization_required_for_output = true`
Else:
  - false

Note:
- This is conservative. Verification can refine later based on actual outputs and specific data handles.

---

## 9) Omissions reporting (must be complete)

If anything is omitted, `CompiledSlice.omissions.did_omit = true` and `omitted_items[]` must include entries with:
- block name
- reason:
  - token_budget_exceeded
  - audience_scope_denied
  - gate_confidence_threshold
  - sensitivity_ceiling
  - taint_policy
- details:
  - what category was omitted (e.g., “style rules”, “low-confidence preferences”, “snippet X”)

If nothing omitted:
- `did_omit = false`, `omitted_items = []`

---

## 10) Provenance summary construction

`CompiledSlice.provenance_summary` must include:
- `primitive_refs`: ids of all primitives included in policy/context blocks
- `evidence_refs`: refs of all evidence snippets included in evidence block

Rules:
- Every non-trivial preference/value inserted must have a primitive_ref.
- Every grounded snippet must have an evidence_ref.
- If a preference/value was excluded due to gating, record in omissions (not provenance_summary).

---

## 11) Output assembly: CompiledSlice fields

Populate:
- `operation_id`, `isolation_id`, `compiled_at`
- `pinned` versions (active_state_version, capability_snapshot_version, audience_graph_version)
- `gate` summary from GateDecision
- `intent_anchor`
- `blocks` contents and per-block budgets
- `taint` summary
- `omissions`
- `provenance_summary`
- `audit_trace_id`

Validation:
- Policy block must be non-empty.
- `operation_max_taint_s` must equal max block taints.
- `gate.max_gate` must equal max(risk,sensitivity) from gate summary.

Failure:
- If any invariant fails, compilation fails and operation must transition to failed with audit entry.

---

## 12) Minimum test cases (must pass)
1. If policy block exceeds budget, compiler fails (no truncation).
2. For gate >= 3, low-confidence hypotheses are excluded and recorded as omissions.
3. If evidence snippets overflow budget, snippets are dropped in deterministic order and recorded.
4. Operation taint equals max block taints exactly.
5. If operation taint > audience ceiling, sanitization_required_for_output is true.
6. Conflicting primitives resolve deterministically with the specified tie-breaker order.