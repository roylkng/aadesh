# Workflow Spec Contract v0.1
Adesh OS

Status: Canonical contract for the post–Wedge v0 composition layer.  
Not required for Wedge v0 and MUST NOT be used to expand the Wedge v0 hot path.

This document defines a durable, versioned workflow representation that Adesh OS can execute deterministically using the existing kernel primitives:
request → operation → gate → compile → model → verify → approval → syscall → audit.

Goal:
- enable personalized, reusable workflows without turning orchestration into promptware
- preserve governance, approvals, taint/sensitivity, and replayability across multi-step processes

Non-goal:
- this is not a general-purpose programming language
- this does not replace SyscallEnvelope; it composes syscalls and transforms

---

## 0) Terms

- **WorkflowSpec**: immutable definition of a workflow (DAG of steps).
- **WorkflowInstance**: a single execution of a WorkflowSpec with concrete inputs.
- **Step**: a node in the workflow graph.
- **Artifact**: immutable input/output object (see artifact specs).
- **IPCArtifact**: explicit cross-step data transfer unit (typed, taint-aware).
- **Gate**: `max(R,S)` computed per step/operation.

---

## 1) WorkflowSpec identity and immutability

### 1.1 Content-addressed identity
A WorkflowSpec MUST be content-addressed:
- `workflow_ref = hash(canonical_json(WorkflowSpec))`

WorkflowSpecs are immutable once registered.

### 1.2 Versioning
WorkflowSpecs may evolve by creating a new workflow_ref.
A workflow may maintain a human-friendly alias that points to a specific workflow_ref via a version pointer (governed mutation).

---

## 2) WorkflowSpec schema (logical)

A WorkflowSpec MUST include:

### 2.1 Metadata
- `workflow_ref`
- `name`
- `description`
- `created_at`
- `author` (Root Owner id)
- `tags[]` (optional)

### 2.2 Inputs and outputs
- `inputs[]`: named inputs with expected schema refs or primitive types
- `outputs[]`: named outputs

Inputs and outputs MUST reference:
- artifact refs, or
- primitive values that are immediately wrapped into artifacts at runtime.

### 2.3 Graph
- `steps[]`: list of Step objects
- `edges[]`: directed edges describing IPC between steps
- `entry_steps[]`
- `exit_steps[]`

The graph MUST be acyclic in v0.1.

---

## 3) Step types (open set, constrained behavior)

Each step has:
- `step_id`
- `step_type`
- `title`
- `inputs[]`
- `outputs[]`
- `constraints` (optional)
- `gate_hints` (non-authoritative)

### 3.1 StepType: transform
Pure function, no side effects.
Examples: format, extract fields, merge artifacts, template fill.

Rules:
- Always R0 (passive)
- Must be deterministic
- Must not call ModelProvider or ToolProvider

### 3.2 StepType: model_call
Calls ModelProvider with a structured request to produce an output artifact.

Rules:
- R1 by default (generative draft)
- May be escalated by governance if it consumes high-sensitivity artifacts or produces high-sensitivity outputs
- Output MUST be persisted as artifacts/events and be replayable (inputs captured)

### 3.3 StepType: syscall
Invokes a ToolProvider action via SyscallEnvelope.

Rules:
- Gate computed per step using existing governance logic
- Requires approvals per gate/approval_mode
- MUST persist syscall pre-image before execution

### 3.4 StepType: subworkflow
Invokes another WorkflowSpec by workflow_ref and maps inputs/outputs.

Rules:
- Inherits constraints from parent workflow
- Must not weaken gates; child steps may be stricter

---

## 4) IPC and data flow

### 4.1 Explicit IPC only
All cross-step data transfer MUST be represented by IPCArtifact records:
- Producer step emits an IPCArtifact
- Consumer step explicitly receives it

No implicit state sharing is allowed.

### 4.2 Sensitivity/taint inheritance
IPCArtifact carries:
- `sensitivity_s`
- `taint_s`

Rules:
- `IPCArtifact.taint_s = MAX(inputs.taint_s)`
- `IPCArtifact.sensitivity_s = MAX(inputs.sensitivity_s)`
- Transform steps must not downgrade taint/sensitivity.
- Sanitization is the only allowed downgrade path and must be explicit as a syscall step.

### 4.3 Operation isolation mapping
By default, each WorkflowInstance runs as:
- one parent operation
- child “step operations” (logical) with isolation boundaries as required

A runtime MAY optimize by grouping consecutive R0 transforms, but must preserve:
- audit anchors
- explicit IPC semantics
- taint tracking

---

## 5) Governance, approvals, and execution semantics

### 5.1 Gate computation
For each step, governance computes:
- Action Risk R (based on step_type, syscall capability metadata, etc.)
- Data Sensitivity S (based on step inputs and intended audience/output)
- `max_gate = max(R,S)`
- approval_mode

### 5.2 Approval binding
Approvals are bound to:
- workflow_instance_id
- step_id
- syscall_id (if syscall step)

If a user edits a diff payload, the step must be re-validated against schema refs and gates.

### 5.3 Idempotency
- WorkflowInstance has an idempotency key per instance creation.
- Each syscall step uses syscall_id as idempotency token.
- Retried workflow execution must not double-execute syscalls.

### 5.4 Failure policy
WorkflowSpec MAY declare a failure policy per step:
- `halt` (default)
- `skip`
- `retry` with bounded attempts
- `compensate` (not supported in v0.1 unless explicitly defined as a syscall step)

---

## 6) Audit and replay

For each WorkflowInstance:
- record WorkflowSpec ref and canonical JSON hash
- record inputs as artifact refs
- record pinned execution context versions:
  - `active_state_version`
  - `capability_snapshot_version`
  - `audience_graph_version`
- record every step transition as audit timeline items
- record step outputs as artifacts
- record syscalls and approvals as in kernel specs

Replay:
- `dry_run` validates gates, schemas, and would-execute plan without side effects
- `full` executes only with equivalent or stricter approvals

---

## 7) Storage and contracts

WorkflowSpec storage can be implemented as:
- registry entries keyed by workflow_ref
- version pointers in current_versions (governed)
- workflow instance records in operations/audit traces

This document does not mandate specific tables; it mandates:
- immutable storage of workflow definitions
- deterministic retrieval by workflow_ref

---

## 8) Minimum acceptance tests (post–Wedge v0)

1) WorkflowSpec is content-addressed and immutable.
2) IPCArtifact is required for any cross-step data transfer.
3) Taint cannot be downgraded without explicit sanitization syscall.
4) Retrying a workflow instance does not double-execute syscalls.
5) Replay dry_run produces identical step plan and validation outcomes.
