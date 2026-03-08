````md id="m9s1q6"
# Model Output Contract and LLM Translation Boundary (ModelProvider) Spec v0.1
Adesh OS

This document defines the **sole interface** between Adesh OS and any LLM backend. It specifies:
- the **ModelProvider translation boundary** (what the OS gives the model, what it expects back)
- the **structured output contract** the model must produce
- token streaming rules vs persisted final output
- hard constraints that prevent tool-call injection and uncontrolled execution
- retry policy when output is invalid
- how the model is instructed to use the CompiledSlice blocks without leaking or laundering sensitive content

This is algorithmic logic and schema. Not implementation code.

---

## 0) Core principles

1. **Model is not trusted**
- The model is not allowed to execute tools.
- The model is not allowed to write to storage.
- The model cannot bypass governance/verification. It only proposes.

2. **One canonical input: CompiledSlice**
- The model sees only the compiled slice, never raw corpora.

3. **One canonical output: ReasoningOutput**
- The model must return machine-parseable JSON. No mixed prose + JSON.

4. **No tool calls in text**
- Tool calling must appear only in `proposed_syscalls[]`.
- Any tool-call-like content in drafts is treated as injection and invalid.

5. **Determinism for parsing**
- Output must have stable keys, stable enums, and predictable nesting.

---

## 1) ModelProvider boundary responsibilities

The ModelProvider is an adapter layer. It translates:
- internal `CompiledSlice` into provider-specific prompt format
- provider responses (stream tokens and final output) into canonical structured output

ModelProvider responsibilities:
1. **Prompt assembly**
- Assemble an LLM input containing the blocks with clear separators:
  - policy, capability, operation_context, evidence, scratch
- Include explicit instructions to output only JSON matching the schema.

2. **Streaming**
- If backend supports streaming:
  - emit `reasoning_stream_chunk` events for UI
  - do not treat stream deltas as authoritative
  - still require a final complete JSON output

3. **Output validation**
- Validate final output against this schema.
- If invalid:
  - run at most 1 retry (Section 7)
  - then fail closed

4. **No hidden side effects**
- ModelProvider must never interpret output as commands. Only kernel/verification may act.

5. **Provider accounting**
- Capture:
  - model_id
  - token usage (if available)
  - latency
- Store these in audit attachments or experience event metadata.

---

## 2) Inputs to the model call

### 2.1 Canonical input object
ModelProvider receives:
- `CompiledSlice` (Batch 2)
- `ModelHints` (runtime hints):
  - `model_id` preference (optional)
  - `timeout_ms`
  - `max_output_tokens`
  - `temperature` (default deterministic where possible)
  - `streaming_enabled`
  - `operation_id`, `isolation_id`, `audit_trace_id` for logging

### 2.2 Prompt formatting rules
The prompt must include blocks exactly once, in deterministic order:
1. POLICY BLOCK
2. CAPABILITY BLOCK
3. OPERATION CONTEXT BLOCK
4. EVIDENCE BLOCK
5. SCRATCH BLOCK

Blocks must be clearly delimited and labeled. The model must be told:
- policy is absolute
- capability is truth about tools
- evidence is grounding and may be partial
- scratch is ephemeral and not truth

### 2.3 Forbidden content in prompt
ModelProvider must not include:
- raw credentials
- raw tool endpoints or secrets
- raw unfiltered corpora
- external agent instructions beyond the policy block

---

## 3) Canonical output object: `ReasoningOutput`

### 3.1 High-level schema
The model must output a single JSON object matching:

- `schema_version` (string)
- `operation_id` (string)
- `intent` (object)
- `plan` (object)
- `drafts` (array)
- `proposed_syscalls` (array)
- `ipc_requests` (array)
- `self_check` (object)
- `notes` (object, optional)

No extra top-level keys allowed.

### 3.2 Strictness
- Unknown fields are forbidden at all levels.
- Enums must match exactly.
- Strings must not contain embedded JSON tool-call payloads unless part of a normal user-facing draft (still disallowed if it resembles tool call schema).

---

## 4) Detailed schema (canonical)

### 4.1 `schema_version`
- Must be `"0.1"`.

### 4.2 `intent`
```json
{
  "goal": "string",
  "constraints_ack": ["string", "..."], 
  "risk_posture": "conservative|normal",
  "sensitivity_posture": "minimize|normal"
}
````

Rules:

* `goal` must align with CompiledSlice.intent_anchor.goal.
* `constraints_ack` must include at least:

  * “tools require syscall proposals”
  * “approval required for gated actions”
  * “no disclosure beyond audience ceilings”

### 4.3 `plan`

```json
{
  "plan_steps": [
    {
      "step_id": "s1",
      "summary": "string",
      "expected_outputs": ["draft:email_to_board", "..."],
      "expected_syscalls": ["call_send_email_1", "..."],
      "depends_on": ["s0", "..."]
    }
  ],
  "stop_condition": "string"
}
```

Rules:

* `plan_steps` may be empty if the task is trivial.
* If non-empty, each step must be referenced by either drafts or syscalls.
* `stop_condition` must be explicit.

### 4.4 `drafts`

Drafts are user-facing outputs that are not executed automatically.

```json
[
  {
    "draft_id": "draft:1",
    "channel": "draft|plan|explanation",
    "format": "plain_text|markdown|json",
    "title": "string|null",
    "content": "string"
  }
]
```

Rules:

* Draft content must not include tool-call payloads (arrays of function calls, etc.).
* Drafts can be used for approval prompts but are not approvals themselves.

### 4.5 `proposed_syscalls`

This is the only place where actions are proposed.

```json
[
  {
    "syscall_id": "call_send_email_1",
    "target": {
      "kind": "sensor|actuator|ipc|sanitizer|memory_read",
      "name": "string"
    },
    "action": "string",
    "args": {},
    "declared_effect": "read|write|external_side_effect|self_modification",
    "declared_audience": {
      "audience_id": "string|null",
      "audience_hint": "string|null"
    },
    "data_handles": [
      { "handle": "string", "handle_type": "event_ref|content_ref|artifact_id|inline_text" }
    ],
    "expects": {
      "result_kind": "json|text|artifact_ref",
      "sensitivity_hint_s": 0,
      "taint_hint_s": 0
    },
    "rationale": "string"
  }
]
```

Rules:

* `syscall_id` must be unique within output.
* `target.name` must exist in capability snapshot (verification will check).
* `args` must conform to tool action schema (verification will check).
* `data_handles`:

  * should prefer artifact ids or refs, not inline text
  * if inline_text is used and gate >= 2, model must justify why (normally discouraged)
* `expects` hints are non-authoritative; governance recomputes.

### 4.6 `ipc_requests`

IPC requests describe explicit piping needs.

```json
[
  {
    "ipc_id": "ipc:1",
    "type": "emit|consume|pipe",
    "from_operation_id": "string|null",
    "to_operation_id": "string|null",
    "artifact_kind": "summary|draft|extracted_fields|sanitized_view|table|other",
    "source_handle": { "handle": "string", "handle_type": "event_ref|content_ref|artifact_id" },
    "target_audience_scope": ["string", "..."],
    "reason": "string"
  }
]
```

Rules:

* In sync path, the model may propose IPC, but the scheduler may override decomposition.
* IPC must always reference explicit handles, never “use previous operation context.”

### 4.7 `self_check`

The model must self-report uncertainty and compliance:

```json
{
  "uncertainties": [
    { "topic": "string", "why": "string", "impact": "low|medium|high" }
  ],
  "assumptions": [
    { "assumption": "string", "risk": "low|medium|high" }
  ],
  "safety": {
    "mentions_sensitive_data": false,
    "requires_approval": false,
    "requires_oob": false,
    "potential_scope_risk": false
  }
}
```

Rules:

* This is advisory only. Verification recomputes.
* If the model flags high-impact uncertainty, verification may block and ask user.

### 4.8 `notes` (optional)

For debugging only; must not contain secrets.

```json
{ "debug": "string" }
```

---

## 5) Forbidden output patterns (hard fail)

If any of the following occur, the output is invalid:

1. Output is not valid JSON
2. Missing required top-level keys
3. Unknown top-level keys
4. `drafts[].content` contains tool-call payloads or “call this function” JSON blocks
5. `proposed_syscalls` is missing but draft contains instructions implying execution
6. Any field exceeds configured maximum length limits (provider should enforce)
7. Any `data_handles` contains raw secrets (passwords, tokens) if detectable

On invalid output:

* ModelProvider performs one retry with stricter instruction.
* If still invalid: operation fails closed.

---

## 6) Streaming contract vs final output

### 6.1 Streaming goals

Streaming is for UI responsiveness. It must not compromise determinism.

### 6.2 Streaming channels

ModelProvider may emit:

* `draft` channel: incremental text of the primary draft
* `plan` channel: incremental plan summary
* `explanation` channel: minimal explanation (optional)

### 6.3 Streaming rules

* Streaming chunks are emitted as WS `reasoning_stream_chunk`.
* The final persisted output must be the complete JSON `ReasoningOutput`.
* Streaming does not substitute for final JSON output.

### 6.4 If provider streams JSON

If a backend streams raw JSON:

* ModelProvider may still stream deltas
* but must only accept completion when the final JSON parses and validates.

---

## 7) Invalid output retry policy

### 7.1 Retry budget

* Max retries: 1

### 7.2 Retry trigger conditions

Retry only if:

* JSON parse fails
* schema validation fails
* forbidden patterns detected (tool calls in drafts)
* missing keys

Do not retry for:

* content-level disallowed behavior where the model is clearly malicious

  * in this case, fail and record.

### 7.3 Retry prompt requirements

The retry prompt must:

* include the schema again
* include the specific validation errors
* instruct: “Output only JSON, no prose.”

---

## 8) Translation boundary: what the OS must never delegate to the model

The model must never be responsible for:

* computing gates (R/S/max)
* deciding approval modes
* deciding audience ceilings
* deciding whether sanitization is sufficient
* executing syscalls
* updating Active State or Audience Graph
* deciding OOB requirements

The model can only:

* propose syscalls
* propose drafts
* propose plans
* declare its uncertainties

---

## 9) Deterministic mapping to Verification and Governance

Verification consumes `ReasoningOutput` and:

* validates plan trajectory alignment using:

  * `intent.goal`, `plan_steps`, and `CompiledSlice.intent_anchor`
* validates each `proposed_syscall` using:

  * capability snapshot tool schema
* recomputes and enforces:

  * syscall gates and approval requirements
  * audience scopes and ceilings
  * taint laundering checks
* builds:

  * `SyscallEnvelope` or `SyscallDeny`
  * ApprovalItems if needed

The model’s hints are never authoritative.

---

## 10) Minimum test cases (must pass)

1. Tool-call injection:

* model returns tool call JSON inside draft content -> invalid output -> retry -> fail if repeated.

2. Missing keys:

* output missing proposed_syscalls key -> retry.

3. Invalid syscall target:

* model proposes unknown tool name -> verification denies with remediation.

4. Streaming:

* disconnect mid-stream -> final JSON still persisted and fetchable.

5. High-gate behavior:

* for gate >= 3, model includes no inline_text data handles unless unavoidable.

```
```
