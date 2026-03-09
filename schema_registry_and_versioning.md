# Schema Registry and Versioning Spec v0.1
Adesh OS

This document specifies how Adesh OS stores, versions, validates, and references schemas across the system. It covers:
- Tool action schemas (MCP tools and internal adapters)
- Model output schema (ReasoningOutput)
- Approval diff editable schema fragments
- Internal object schemas (Batch 1–3) if stored as JSON
- Schema hashing, integrity checks, and upgrade policy
- How `schema_ref` is resolved deterministically across backends

The generic execution model that consumes these schemas is defined in `schema_based_tools_and_actions.md`.

This is algorithmic logic. Not implementation code.

---

## 0) Core principles

1. **Schemas are part of OS physics**
If a schema changes, the meaning of syscalls and validation changes. This must be explicit and versioned.

2. **Immutable and content-addressed**
Schemas must be immutable once stored and referenced by hash.

3. **Pinned where it matters**
Operations pin `capability_snapshot_version`, which pins tool schemas. This prevents mid-operation schema drift.

4. **Validation is deterministic**
Same input and same schema must yield the same validation result.

---

## 1) Schema categories

### 1.1 Tool schemas
Defines:
- tool name
- actions and their arg JSON schema
- action-level `args_schema_ref` / `result_schema_ref`
- required/optional fields
- editable fields for diff mode
- risk floor overrides (optional)
- forbidden fields list (optional)

### 1.2 Model output schema
Defines ReasoningOutput contract v0.1.

### 1.3 Internal object schemas
Batch 1–3 structs are already typed, but if persisted as JSON, schema definitions may be stored for integrity checks.

---

## 2) Schema identity and integrity

### 2.1 Content hash
Every schema document must be hashed:
- `sha256(schema_bytes)` => `schema_hash`

### 2.2 SchemaRef format (canonical)
A `schema_ref` must be a stable pointer that includes the hash:

Preferred:
- `schema:sha256:<hex>`

Optional:
- `schema:<namespace>:<name>:sha256:<hex>`

### 2.3 Immutable storage rule
Once a `schema_ref` is stored, its content must never change. Any update produces a new `schema_ref`.

---

## 3) Storage model (logical)

### 3.1 Schema document record
A schema registry record must store:
- `schema_ref`
- `schema_hash`
- `schema_kind`: `tool_action|model_output|internal`
- `schema_version`: semantic version string if applicable (e.g., "0.1")
- `tool_name` and `action_name` (for tool schemas)
- `created_at`
- `content_ref` (blob pointer) OR inline JSON (backend-dependent)
- `compatibility_notes` (optional)

### 3.2 Resolution
Resolving a schema_ref means:
- fetch schema document by schema_ref
- verify hash matches schema_ref
- return parsed schema object

If hash mismatch:
- treat as storage corruption and fail closed.

---

## 4) Tool schema composition

### 4.1 Tool schema document shape
Canonical tool schema format:
```json
{
  "tool": "send_email",
  "tool_schema_version": "0.1",
  "actions": [
    {
      "name": "send",
      "args_schema": { "type": "object", "properties": { ... }, "required": [...] },
      "editable_fields": ["subject", "body", "to"],
      "risk_floor_override_r": 3,
      "forbidden_fields": ["password", "token"]
    }
  ]
}
```

Rules:

* `args_schema` must be JSON Schema draft compatible (choose one draft and freeze it).
* `editable_fields` must be a strict subset of schema properties.
* Forbidden fields are additive with global negative memory.

### 4.2 Action schema_ref

Each `(tool, action)` must have an action-level schema binding:

* `args_schema_ref` required
* `result_schema_ref` optional but recommended
* stored in capability snapshot descriptors
* used by verification and tool-result validation

---

## 5) Model output schema versioning

### 5.1 schema_version field

ReasoningOutput includes `schema_version`. For v0.1 it must be `"0.1"`.

### 5.2 Backward compatibility

ModelProvider must accept only the schema versions explicitly supported by the OS build.

If the model returns an unsupported schema_version:

* treat as invalid output
* retry once with the correct version requirement
* fail closed if repeated

---

## 6) Schema pinning and drift prevention

### 6.1 Pinning through capability snapshot

`capability_snapshot_version` must include:

* tool list
* each tool action schema_ref
* schema hashes

Operations pin the snapshot, so they pin schemas.

### 6.2 Mid-flight schema change

If tool schema changes after operation start:

* it does not affect the operation
* verification uses pinned schema_ref from snapshot

If an operation requires a tool that was updated:

* it must explicitly recompile against latest snapshot (new operation or explicit refresh flow)

---

## 7) Schema upgrades and migration policy

### 7.1 Tool schema upgrades

If a schema changes in a backward-incompatible way (required fields added, meaning changed):

* increase tool_schema_version
* mint new schema_ref
* mint new capability snapshot version

Existing operations pinned to old snapshot continue to validate against old schema.

### 7.2 Internal schema upgrades

If Batch object fields change:

* support versioned parsing
* store migration notes

---

## 8) Validation rules

### 8.1 Deterministic validation

Validation must:

* reject unknown fields by default (unless schema explicitly allows additionalProperties)
* enforce required fields
* enforce type constraints

### 8.2 Validation error structure

Validation errors must be structured and mapped to Error Taxonomy:

* `schema::<tool>::<action>::missing::<field>`
* `schema::<tool>::<action>::type::<field>`
* etc.

This enables actionable SyscallDeny remediation.

---

## 9) Security constraints

1. Schemas are untrusted inputs unless they come from trusted capability sources.
2. Schema registry must not allow runtime replacement of existing schema_ref contents.
3. If schema resolution fails or hash mismatch:

* treat as corruption and fail closed.

4. Schema content must be small enough to avoid resource exhaustion. Enforce size limits.

---

## 10) Minimum test cases (must pass)

1. Hash mismatch:

* corrupt schema content => resolution fails closed.

2. Pinned schema:

* tool schema updated => existing operation still validates using old schema_ref.

3. Editable fields enforcement:

* modified_payload includes non-editable field => validation fails.

4. Unsupported model schema_version:

* retry once, then fail closed.

5. Unknown fields:

* syscall args contain unknown key => validation fails with structured violation.
