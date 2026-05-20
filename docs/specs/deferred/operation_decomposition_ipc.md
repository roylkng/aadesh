# Operation Decomposition and Explicit IPC Spec v0.1
Adesh OS

This document specifies how Adesh OS decomposes a single `RequestEnvelope` into one or more `OperationSpec` units and how operations exchange data via explicit IPC. It defines:
- deterministic decomposition rules (no finite task taxonomy)
- how to avoid cross-sensitivity contamination
- strict isolation guarantees (`isolation_id`)
- explicit IPC artifact model and sensitivity inheritance
- how “piping” works without leaking full sensitive inputs
- required audit and storage semantics for all IPC flows

This is algorithmic logic. Not implementation code.

---

## 0) Core principles

1. **Operations are processes**
- Each operation is an isolated process with its own working memory slice and lifecycle.
- Isolation is enforced by `isolation_id`.

2. **No implicit data sharing**
- No operation may access another operation’s compiled slice, scratch, or intermediate outputs unless explicitly piped via IPCArtifact.

3. **Mixed sensitivity requires isolation**
- If a request contains sub-tasks with different sensitivity levels, those must not share a context window by default.

4. **IPC is explicit, typed, and inherits sensitivity**
- An IPCArtifact carries `sensitivity_s` and `taint_s`.
- Any receiver operation inherits sensitivity constraints and must recompile.

5. **Decomposition is for governance correctness first**
- Performance and UX are secondary to preventing taint laundering and scope leaks.

---

## 1) Inputs and outputs

### Inputs
- `RequestEnvelope`
- Owner context (Root Owner)
- current `active_state_version`
- current `capability_snapshot_version`
- current `audience_graph_version`
- Active State snapshot at `active_state_version`
- Capability snapshot at `capability_snapshot_version`
- Audience Graph snapshot at `audience_graph_version`
- default budgets

### Outputs
- A list of `OperationSpec[]` such that:
  - each has unique `operation_id`, unique `isolation_id`
  - each pins `active_state_version`, `capability_snapshot_version`, and `audience_graph_version` at creation time
  - each has a clear `operation_goal.summary`
  - each declares explicit IPC dependencies if needed (consumes_artifacts, inherits_sensitivity)

---

## 2) Terminology

### Operation types (conceptual, not a fixed taxonomy)
These are descriptive categories used only to reason about splitting:
- **Read/Inspect**: acquire information (sensor reads, document inspection)
- **Transform**: summarize, extract, rewrite, compute
- **Actuate**: external side effects (send, modify, publish)
- **Sanitize**: reduce sensitivity of derived outputs (explicit step)

The system must NOT hardcode a finite list of tasks. It uses **risk/sensitivity predicates**.

### Operation boundary
A boundary is required when two sets of steps:
- have different sensitivity or risk classes, OR
- target different audiences, OR
- require separate approvals, OR
- require strict contamination prevention.

---

## 3) Decomposition algorithm (deterministic)

### Step 3.1: Extract candidate “intents”
From `RequestEnvelope.input.content` and attachments, identify candidate intents:
- intent phrases (verbs and objects)
- referenced resources (files, emails, URLs)
- mentioned audiences (people, orgs, public)
- implied actions (send, delete, publish)

Extraction must be deterministic and parser/heuristic driven in v0.1. Model assistance may be used only for non-authoritative hints and must not create, remove, or suppress a required boundary.

### Step 3.2: Build the candidate action graph
Represent the request as an ordered set of logical steps:
- `Step_i` has:
  - required inputs (attachments, refs)
  - potential outputs (draft, summary, artifact)
  - possible syscalls (sensor/actuator)
  - implied audience targets

This step graph can be coarse.

### Step 3.3: Compute preliminary (R,S) per step
For each step:
- Estimate `S_step` from referenced inputs:
  - attachments sensitivity hints
  - “confidential/financial/PII” signals
  - default conservative inference for unknown docs (S2)
- Estimate `R_step` from implied effects:
  - read/transform tends to R0–R1
  - external side effect tends to R2+
  - publish/money/identity/self-mod tends to R3–R4

These are preliminary, not final. Final gating happens later.

### Step 3.4: Partition into operations using boundary rules
Initialize an empty current operation bucket `O`.
Iterate steps in order, applying boundary rules.

#### Boundary Rule A: Risk discontinuity
Start a new operation if:
- `R_step` differs from `R_current` by >= 2 levels, OR
- step introduces any R3/R4 predicate when current operation is below R3

Rationale: approvals and audit clarity.

#### Boundary Rule B: Sensitivity discontinuity
Start a new operation if:
- `S_step` > `S_current` AND the step is likely to produce outputs that later go to a lower-sensitivity audience, OR
- the step involves S3/S4 materials and any other step in the bucket is intended for outbound communication

Rationale: prevent contamination and laundering.

#### Boundary Rule C: Audience discontinuity
Start a new operation if:
- step targets a different outbound audience than prior steps, OR
- one step is internal-only and another step is external/public

Rationale: Audience Graph ceilings differ.

#### Boundary Rule D: Tool boundary (actuator isolation)
Start a new operation when transitioning from:
- read/transform to actuate, unless the actuation is strictly local (R0/R1)

Rationale: isolates the context window used to send messages or mutate systems.

#### Boundary Rule E: Sanitization boundary
If a high-sensitivity step must feed a lower-sensitivity outbound step:
- create a dedicated **Sanitize operation** between them, unless already present.

Rationale: explicit reduction and audit.

### Step 3.5: Produce OperationSpec for each partition
For each operation bucket:
- `operation_goal.summary` is a deterministic summary of included steps:
  - use the original request phrases when possible
- `operation_goal.input_refs` include:
  - request event_ref
  - directly referenced attachment refs relevant to this operation
  - IPC artifacts consumed (if any)
- `requested_outputs` is descriptive only and may be omitted

### Step 3.6: Establish IPC dependencies between operations
If operation B depends on the output of operation A:
- B must list `consumes_artifacts=[artifact_id]` in `OperationSpec.ipc`
- B must set `inherits_sensitivity = max(artifact.sensitivity_s, artifact.taint_s)` (conservative)

---

## 4) IPC model and explicit piping

### 4.1 IPCArtifact creation
An IPCArtifact is created only by an explicit step:
- either a syscall of kind `ipc` or an internal kernel action that is still represented as a syscall-like audited event.

Fields (Batch 3):
- `artifact_id`
- `produced_by_operation_id`
- `kind` (summary, draft, extracted_fields, sanitized_view, table, other)
- `content_ref` (BlobStore reference)
- `sensitivity_s`
- `taint_s`
- `provenance_refs[]`
- `audience_scope_tag` (allowed scopes and max disclosure)
- `ipc_rules.receiver_inherits_s`, `requires_recompile=true`

### 4.2 Explicit piping semantics
Piping is an explicit link:
- “Operation A produces artifact X”
- “Operation B consumes artifact X”

No other state moves across the boundary.

### 4.3 Sensitivity inheritance (mandatory)
When B consumes artifact X:
- Governance must treat X as a sensitivity source.
- Compiler must treat X as a taint source for all derived blocks if referenced.
- The receiver operation’s computed `S_operation` must be at least `X.sensitivity_s`.
- The receiver operation must recompile even if it previously compiled without X.

### 4.4 IPC and audience scopes
Even though the control plane is Root Owner, artifacts may later be used in outbound syscalls.
Therefore:
- IPCArtifacts must carry `audience_scope_tag`:
  - allowed scopes (e.g., "work:board_email", "personal:partner")
  - max disclosure ceiling
- Verification must enforce that outbound syscalls may only include content from artifacts whose scope tag allows that audience.

---

## 5) Anti-contamination rules (non-negotiable)

### 5.1 No shared compiled slices
A compiled slice is per operation and cannot be read by another operation.

### 5.2 No “ambient” carryover
No implicit carryover of:
- scratch block
- model internal chain of thought
- intermediate drafts
- tool results
across operation boundaries.

### 5.3 IPC artifact is the only bridge
All cross-operation transfer must be via IPCArtifact.

### 5.4 Derived-taint rule
If an IPCArtifact is derived from S3 sources, its `taint_s` must be at least S3, even if the text looks innocuous, unless sanitization is performed and verified.

This prevents laundering by “summarize and strip details” without explicit sanitization.

---

## 6) Sanitization operations (explicit reduction step)

### 6.1 When required
Insert a sanitization operation when:
- high-sensitivity sources (S3/S4) must contribute to an outbound message with lower ceiling, OR
- the user asks to publish externally based on internal confidential sources.

### 6.2 Sanitization operation semantics
A sanitization operation:
- consumes an IPCArtifact or attachment
- produces a new IPCArtifact of kind `sanitized_view`
- must include:
  - a sanitization policy summary (what was removed/generalized)
  - provenance refs to the source artifact(s)
- sets:
  - `sensitivity_s` to a reduced value only if verification determines that the output is truly reduced
  - `taint_s` must still reflect the maximum taint influence unless the sanitization process is approved and verified as sufficient.

### 6.3 Verification requirement
Verification must:
- prevent direct outbound use of unsanitized S3-derived artifacts when ceiling is lower
- require sanitized_view artifact handles only

---

## 7) Storage and atomicity requirements for decomposition + IPC

### 7.1 Decomposition persistence
Once operations are created:
- they must be persisted with pinned versions
- operation creation should be atomic per request (all-or-nothing recommended)

### 7.2 IPC artifact persistence ordering
To create IPCArtifact:
1. store content to BlobStore -> `content_ref`
2. persist IPCArtifact referencing content_ref
3. append experience event `ipc_emit`
4. audit timeline update
5. emit WS `ipc_emit` (optional)

Receiver operation must not proceed until IPCArtifact exists durably.

### 7.3 Idempotency
If decomposition is invoked under an idempotent request:
- repeated execution must not create duplicate operations or artifacts
- use deterministic IDs if you choose, or store idempotent response and short-circuit.

---

## 8) UI and WS considerations

The UI should see:
- multiple operations created for one request
- operation states and audit updates per operation
- explicit IPC artifacts listed under each operation

WS events:
- `operation_state` per operation
- `audit_update` per operation
- optional `ipc_emit` and `ipc_receive` events including `artifact_id` and producer/consumer operation ids

---

## 9) Minimum test cases (must pass)

1. Mixed sensitivity:
- “Summarize confidential Q3 financials and email it to vendor.”
Expected:
- operation A reads/summarizes (S3)
- operation B sanitizes (if vendor ceiling < S3) or blocks
- operation C sends email (separate operation) using sanitized_view only

2. Mixed audiences:
- “Write two emails: one to my team and one public LinkedIn post.”
Expected:
- at least two operations with distinct audience ceilings
- no shared context window

3. No implicit piping:
- Operation B must fail if it references Operation A’s output without an IPCArtifact.

4. Sensitivity inheritance:
- B consumes artifact with S3 -> B’s S must be >= S3.

5. Explicit sanitization:
- Attempt to send unsanitized S3 artifact to public -> denied with taint laundering.
