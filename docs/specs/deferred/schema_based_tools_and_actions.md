# Schema-Based Tools and Actions Spec v0.1
Adesh OS

This document defines the generic mechanism for supporting thousands of tools/actions without baking per-tool contracts into the Adesh OS kernel.

Core idea:
- The kernel defines a small, stable syscall execution model.
- Tools/actions are described by schemas registered in the Schema Registry.
- Capability Snapshots pin tool/action schema refs so execution is deterministic and replayable.

This is a canonical spec. Not implementation code.

---

## 0) Goals and non-goals

### Goals
- Support unbounded tool/action growth without kernel changes.
- Ensure every syscall is:
  - schema-validated
  - diffable when required
  - idempotent
  - auditable and replayable
- Allow tools to provide optional UX helpers (preview renderers) without changing enforcement logic.

### Non-goals (v0.1)
- No automatic UI generation guarantees (possible later).
- No guarantee that every tool can produce a meaningful semantic diff (some will be denied when diff is required but unavailable).
- No attempt to infer schemas from code automatically (manual registration is sufficient for v0.1).

---

## 1) Canonical objects

### 1.1 Schema Registry Entry
Defined by `schema_registry_and_versioning.md`.

Each schema entry must be:
- content-addressed (schema_ref is derived from canonical hash)
- immutable once registered
- retrievable by schema_ref for validation and replay

### 1.2 Capability Descriptor
Defined by `capability_mcp.md` and stored in a pinned capability snapshot.

A capability descriptor binds:
- `tool_id`
- `action`
- `args_schema_ref`
- `result_schema_ref` (optional but recommended)
- `execution_class`: `external_api|host_local|sandboxed`
- `diff_supported` (bool)
- `default_approval_mode`: `none|confirm|diff|oob`
- optional: `preview_renderer_ref` (non-authoritative)
- optional: `risk_hints` (non-authoritative hints; governance is authoritative)

### 1.3 SyscallEnvelope
Defined by Batch contracts and execution specs.

Each syscall envelope must include:
- `syscall_id` (idempotency token)
- `operation_id`, `isolation_id`
- `tool_id`, `action`
- `args_schema_ref`
- `args_payload` (JSON)
- `pinned_versions` (active/capability/audience)
- `gate_decision` fields as required
- `taint` and `sensitivity` as required
- `approval_mode` as computed by governance

---

## 2) Tool/action registration flow

### 2.1 Register schema entries
An operator registers:
- args schema
- result schema (recommended)

Schema refs become durable identifiers.

### 2.2 Mint capability snapshot
Capability snapshots list tools and actions and pin schema refs.

Rules:
- A syscall must always reference the schema_ref pinned in the operation’s capability snapshot.
- Capability changes require minting a new snapshot version.

### 2.3 Tool runtime availability
ToolProvider may expose tool availability (health) separately, but availability must not modify schema contracts.

---

## 3) Generic validation rules

### 3.1 Args validation
Before a syscall can be:
- approved (in diff mode), or
- executed

the kernel must validate:
- `args_payload` against `args_schema_ref`

Unknown fields are rejected when the schema declares `additionalProperties=false`.

### 3.2 Edited approval payload validation
If approval mode is `diff` and the user supplies an edited payload:
- validate edited payload against the same `args_schema_ref`
- re-run governance checks if edited payload changes risk predicates
- if edited payload violates policy, deny with remediation payload (see `error_remediation.md`)

### 3.3 Result validation (recommended)
When ToolProvider returns a result:
- validate result payload against `result_schema_ref` if present
- if validation fails:
  - mark syscall failed
  - persist deny/failure reason
  - do not silently coerce

---

## 4) Generic diff normalization

Diff approval requires a deterministic, enforceable diff that is independent of UI rendering.

### 4.1 Canonicalization function
To compute a diff, the kernel must canonicalize JSON into a stable form:

Canonical JSON rules:
- object keys sorted lexicographically
- numbers normalized (no trailing zeros representation differences)
- booleans and null preserved
- strings preserved byte-for-byte (unless a schema-specific normalizer is defined)
- arrays:
  - preserve order by default
  - if schema marks an array as unordered (via `x-ordering: "set"` extension), sort elements deterministically by canonicalized value hash

Schema extensions allowed (optional):
- `x-ordering: "set"` on arrays that represent unordered sets (e.g., recipients)
- `x-normalize: "trim"` on strings where whitespace is non-semantic
- `x-redact: true` for fields that must not appear in logs/diffs (diff must show placeholder hashes)

### 4.2 Diff definition
Diff is computed as:
- `diff = json_diff(canonical(proposed_args), canonical(approved_args))`

Where:
- proposed_args is the syscall args produced by the model/plan
- approved_args is either identical (approve as-is) or user-edited validated payload

### 4.3 Enforcement
If `approval_mode=diff` and `diff_supported=false` for that capability:
- deny execution
- return remediation: “manual path required”

If canonicalization cannot be computed deterministically:
- deny execution
- return remediation

### 4.4 Optional preview renderer (non-authoritative)
Tools may provide `preview_renderer_ref` to show a human preview.
Preview is never used for enforcement, only UX.

---

## 5) Generic idempotency rules

### 5.1 Syscall idempotency token
`syscall_id` is the idempotency token for the OS.

Rules:
- A syscall with the same syscall_id must not execute side effects twice.
- ToolProvider must accept syscall_id as idempotency input when possible.
- When provider cannot guarantee idempotency:
  - OS must enforce at-most-once by caching the first execution result and refusing re-exec.

### 5.2 Approval idempotency
Approvals must be idempotent by approval_id.
Consuming an approval must be atomic and single-use.

---

## 6) Governance interaction (generic)

Governance computes:
- Action Risk R (from execution_class, side effects, etc.)
- Data Sensitivity S (from taint and data classification)
- approval_mode and max_gate

Tools may provide non-authoritative `risk_hints`, but governance is authoritative.

Sandbox-specific:
- sandbox policies (network, mounts) are risk predicates for R computation per `governance_kernel_logic.md`.

---

## 7) Audit and replay requirements (generic)

For every syscall:
- persist syscall pre-image before side effects
- persist approval artifacts (diff, edited payload) when applicable
- persist tool result payload and validation status
- record schema_refs and pinned versions for deterministic replay

Replay:
- dry_run validates schemas and gates but does not execute actuators
- full replay executes only under the same or stricter approvals

---

## 8) Minimum acceptance tests

1) Schema registration produces immutable schema_ref.
2) Syscall args validation rejects unknown fields when schema forbids them.
3) Diff mode computes stable diff independent of JSON key order.
4) If diff required but diff_supported=false, syscall is denied with remediation.
5) Edited approval payload invalid => denied with remediation.
6) Same syscall_id cannot execute twice (OS-level at-most-once).
7) Replay uses pinned schema_refs and produces identical validation outcomes.