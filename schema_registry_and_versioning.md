# Schema Registry and Versioning Spec v0.1
Adesh OS

This document defines canonical schema registration, integrity, pinning, and upgrade rules.

## 0) Core principles

1. Schemas are immutable and content-addressed.
2. Every executable decision references pinned schema versions.
3. Hash mismatch is a corruption event and fails closed.
4. Compatibility policy is explicit, never inferred at runtime.
5. Replay uses the original pinned schema refs.

## 1) Schema object model

A registered schema entry must include:
- `schema_ref`: content-addressed id (see section 2)
- `schema_kind`: `tool_action|model_output|api_contract|internal_object`
- `name`: logical schema name
- `semver`: version string
- `content_hash`: `sha256:<hex>`
- `created_at`
- `status`: `active|deprecated|disabled`
- `compatibility`: `backward|forward|none`
- `payload` (JSON schema body)

## 2) `schema_ref` canonical format

### 2.1 Format
- Canonical: `schema:sha256:<hex>`
- Optional locator suffix for debugging (non-authoritative): `schema:sha256:<hex>#<name>@<semver>`

### 2.2 Resolution
`schema_ref` resolution must be deterministic:
1. locate by hash key
2. verify stored payload hash equals hash in ref
3. reject if mismatch

## 3) Integrity checks

On registration:
1. canonicalize schema payload
2. compute hash
3. store immutable row

On read/use:
1. re-hash payload (or verify cached hash)
2. compare with `schema_ref`
3. fail closed with corruption error on mismatch

## 4) Pinning semantics

### 4.1 Capability snapshot pinning
`capability_snapshot_version` must pin:
- tool descriptors
- action-to-schema bindings
- each action `schema_ref`

An operation pinned to snapshot V must resolve tool schemas from V only.

### 4.2 Operation-level pinning
Each operation must pin:
- `active_state_version`
- `capability_snapshot_version`
- `audience_graph_version`

Gate/compile/verify/execution for that operation must use pinned versions only.

### 4.3 Model output schema pinning
ModelProvider requests must include pinned `ReasoningOutput` schema ref for validation.
For v0.1, `model_output_contract.md` schema version `0.1` is authoritative.

## 5) Versioning rules by schema kind

### 5.1 Tool action schemas
- Backward-compatible additive changes may publish a new semver minor.
- Breaking changes require new major semver and a new `schema_ref`.
- Capability snapshot must not silently replace schema refs for in-flight operations.

### 5.2 Model output schema
- `schema_version` in model output must exactly match expected schema.
- Unknown fields are rejected where contract requires strictness.
- Breaking output-shape changes require new schema id and coordinated verifier update.

### 5.3 API contract schemas
- Request/response schema changes must preserve idempotency and error envelope invariants.
- Breaking endpoint payload changes require explicit version bump and migration notes.

## 6) Upgrade and migration workflow

1. Register new schema payload and hash.
2. Run compatibility checks against prior version.
3. If compatible, mark old schema as `deprecated`; otherwise keep active until migration done.
4. Mint new `capability_snapshot_version` (or API/model binding version) referencing new schema refs.
5. New operations may pin new snapshot; in-flight operations remain on old refs.

## 7) Replay requirements

Replay must load the original pinned schema refs.
If pinned schema payload is missing or hash-invalid:
- replay fails with missing/corrupt anchor reason
- no fallback to latest schema is allowed for deterministic replay mode

## 8) Storage contract requirements

Storage layer must support:
- immutable schema rows keyed by `schema_ref`
- lookup by hash and by logical `(name, semver)`
- hash verification and corruption reporting
- retention policy that preserves schema refs referenced by any audit trace

## 9) Minimum test cases

1. Register same payload twice yields same `schema_ref`.
2. Hash mismatch on load returns corruption and fails closed.
3. Operation pinned to old snapshot cannot resolve new schema implicitly.
4. Breaking tool schema change requires new major and new snapshot.
5. Replay fails if referenced schema is missing.
