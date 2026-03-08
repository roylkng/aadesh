# ToolProvider Port Contract Spec v0.1 (MCP Client Adapter + Syscall Execution)
Adesh OS

This document defines the **ToolProvider** port contract. ToolProvider is responsible for:
- executing permitted syscalls against sensors/actuators/sanitizers via MCP (or internal adapters)
- returning structured outputs with sensitivity/taint metadata
- supporting idempotency where possible
- enforcing safety constraints at execution time (timeouts, rate limits, redaction)

ToolProvider does not decide gates. It executes only syscalls already permitted by Verification/Governance and persisted as pre-images.

This is interface and logic documentation. Not implementation code.

---

## 0) Core principles

1. **No execution without a persisted syscall envelope**
ToolProvider must require a `SyscallEnvelope` id or full envelope that was persisted.

2. **At-most-once effect where possible**
ToolProvider should use `syscall_id` as an idempotency token to avoid duplicate effects.

3. **Structured outputs**
ToolProvider must return outputs in a deterministic structured form:
- `output_ref` (content_ref or event_ref)
- `output_json` (small structured response)
- `sensitivity_s` and `taint_s`
- `execution_metadata` (latency, retries)

4. **No secrets leakage**
ToolProvider must redact sensitive fields before returning logs or error messages.

---

## 1) Syscall categories

ToolProvider executes syscalls with `target.kind` one of:
- `sensor` (read)
- `actuator` (write/side effect)
- `sanitizer` (produce sanitized_view + report)
- `ipc` (emit/consume typed artifacts, if implemented as a tool)
- `memory_read` (optional, internal read operations; usually handled by StorageProvider instead)

The kernel may treat some of these as “internal tools,” but the execution contract stays syscall-shaped.

---

## 2) Method-level contract (conceptual interface)

### 2.1 execute_syscall
Inputs:
- `SyscallEnvelope` (includes syscall_id, tool name, action, args, pinned versions, gate fields)
- `ExecutionHints`:
  - timeout_ms
  - retry_policy (max attempts)
  - idempotency_token (default syscall_id)
  - redaction_policy
Output:
- `SyscallExecutionResult`

Semantics:
- Validate tool/action exists in pinned capability snapshot (or fail fast)
- Enforce timeouts and concurrency limits
- Execute via MCP client call or internal adapter
- Return structured output with classification and references

Errors:
- `ToolError` categorized as:
  - NotFound (tool/action)
  - InvalidArgs (schema mismatch)
  - Timeout
  - RateLimited
  - RemoteError (from tool)
  - Transient
  - Permanent
  - Corruption (unexpected output format)
ToolProvider must map errors to stable codes so Verification can create actionable remediation.

### 2.2 health_check_tool (optional)
Used for capability degraded status and discovery.

### 2.3 discover_tools (MCP client, optional here)
Tool discovery may be separate from execution; capability registry spec governs discovery.

---

## 3) Execution ordering and persistence integration

ToolProvider is invoked only after:
- StorageProvider persisted SyscallEnvelope as `permitted`
- If approval required, approval was consumed

Execution result persistence:
- ToolProvider returns result to kernel
- Kernel stores:
  - tool output as blob/event
  - updates syscall status and audit trace

ToolProvider itself must not write to OS storage.

---

## 4) Output contract: SyscallExecutionResult

Must include:
- `ok` boolean
- `output_kind`: `json|text|artifact_ref`
- `output_json` (small, safe metadata)
- `content_ref` optional (for large text/binary)
- `artifact_id` optional (if tool directly emits IPC artifacts; usually kernel does)
- `sensitivity_s` and `taint_s` (as produced by tool + OS classification)
- `provenance`:
  - syscall_id
  - tool name/action
  - remote request id (if available)
- `timing`:
  - started_at, ended_at, latency_ms
- `retry_info`:
  - attempts_used

Rules:
- Do not return raw large payload in output_json.
- For large payload, return content_ref to BlobStore (kernel writes it).

---

## 5) Classification responsibilities

ToolProvider may provide sensitivity hints, but the OS classification pipeline is authoritative.

Rules:
- ToolProvider should label outputs with best-effort hints:
  - e.g., email bodies likely S2
  - credentials outputs S4
- Kernel must run the deterministic classifiers and compute final labels.

ToolProvider must support:
- providing “output samples” for classification without returning full sensitive content to logs.

---

## 6) Idempotency and dedupe

### 6.1 Actuator idempotency
If the external tool supports idempotency:
- pass `syscall_id` as idempotency token to the tool
Examples:
- email: Message-Id or custom header
- HTTP APIs: idempotency key header

If the tool does not support idempotency:
- ToolProvider must warn via metadata `idempotency_supported=false`
- Kernel must enforce strict retry rules (likely require re-approval)

### 6.2 Sensor reads
Sensor reads are naturally idempotent but may change over time. ToolProvider must include timestamps and paging tokens when relevant.

---

## 7) Timeouts, retries, and backoff

### 7.1 Timeouts
Each syscall has:
- hard timeout_ms
- ToolProvider must abort and return Timeout error

### 7.2 Retries
Retries must be bounded:
- default max attempts 1 for actuators
- sensors may retry more if transient
- any retry must be recorded in execution metadata

Backoff policy:
- deterministic, bounded

---

## 8) Security constraints

1. **No tool calls based on tool output instructions**
ToolProvider must never interpret response text as instructions to perform more actions.

2. **Output sanitization**
For error messages returned from tools:
- redact secrets
- cap message lengths

3. **Trusted boundary**
Tools can be untrusted. ToolProvider must treat tool outputs as tainted per trust_class and classification rules.

4. **Schema conformance**
ToolProvider must validate that tool responses match expected response shapes when defined, otherwise mark as Corruption.

---

## 9) MCP-specific behavior (if tool uses MCP)

### 9.1 Tool naming
MCP tool identifiers must be mapped to `CapabilityDescriptor.name`.

### 9.2 MCP call envelope
ToolProvider must:
- call MCP method with JSON args
- handle errors and timeouts
- normalize response into SyscallExecutionResult

### 9.3 MCP server trust classes
ToolProvider must annotate outputs with trust_class so the OS can taint accordingly:
- untrusted MCP server => taint at least T2/T3 depending on content

---

## 10) Minimum acceptance tests (must pass)

1. Pre-image requirement:
- calling execute_syscall without persisted syscall envelope id must be rejected by kernel; ToolProvider should assume it is provided.

2. Timeout:
- tool stalls -> ToolProvider returns Timeout error deterministically.

3. Idempotency:
- actuator supports idempotency -> repeated execution with same syscall_id results in no duplicate side effects (in test stub).

4. Output classification:
- tool returns token-like string -> sensitivity hints to S4; kernel classification enforces S4.

5. Response shape:
- tool returns malformed JSON -> ToolProvider returns Corruption error code.
