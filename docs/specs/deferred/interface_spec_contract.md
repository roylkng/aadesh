# Interface Spec Contract v0.1
Adesh OS

Status: Canonical contract for the post–Wedge v0 interface composition layer.  
Not required for Wedge v0 and MUST NOT be used to expand the Wedge v0 hot path.

This document defines a declarative interface representation that Adesh OS can produce and the local UI can render.
It is designed to:
- keep UI composition deterministic and auditable
- prevent sensitive context from leaking through “generated UI”
- bind UI affordances to operation state, approvals, and artifacts

Non-goal:
- this is not a general UI framework
- this does not allow arbitrary HTML or code execution

---

## 0) Terms

- **InterfaceSpec**: declarative UI schema template (immutable).
- **InterfaceInstance**: compiled UI plan for a specific operation/workflow instance.
- **UIBlock**: a renderable element (text, form, diff viewer, artifact viewer).
- **Binding**: a mapping from UIBlock to OS state (operation state, approvals, artifacts).

---

## 1) Core safety principles

1) No arbitrary code
Interface plans are declarative JSON objects only.

2) Gate-aware rendering
UI must not display data outside the viewer’s allowed scope and ceiling.

3) Taint-aware UI
Any block built from S3/S4 inputs remains tainted at that ceiling for the operation lifespan.
No laundering of derived text into lower sensitivity blocks.

4) Explicit user action for side effects
UI can stage and approve actions, but cannot execute side effects without passing through kernel approvals.

---

## 2) InterfaceSpec identity and immutability

### 2.1 Content-addressed identity
- `interface_ref = hash(canonical_json(InterfaceSpec))`

Immutable once registered.

### 2.2 Version pointers
A human-friendly interface name may map to a pinned interface_ref via a governed pointer.

---

## 3) InterfaceInstance (compiled UI plan)

An InterfaceInstance is generated per operation (or workflow instance) by the OS, and includes:

- `interface_ref` (template ref)
- `instance_id`
- `operation_id` and optional `workflow_instance_id`
- pinned execution context versions:
  - `active_state_version`
  - `capability_snapshot_version`
  - `audience_graph_version`
- `viewer` (Root Owner only in early phases)
- `gate_summary` (R,S,max,approval_mode)
- `blocks[]` (UIBlocks)
- `bindings[]` (how blocks bind to OS data)
- `taint_summary`

InterfaceInstance is persisted as an artifact (or event) for audit/replay.

v0.1 reference runtime note:
- operation-backed interface compilation is fully supported
- workflow-backed interface compilation requires `workflow_instance.parent_operation_id` so `gate_summary` and taint summary can be derived deterministically from persisted operation anchors
- if no linked parent operation exists, compilation must fail closed

---

## 4) UIBlock types (v0.1 set)

UIBlocks are typed; the renderer only supports known block types.

### 4.1 text
- markdown/plain text
- may be streamed

### 4.2 artifact_view
- shows an artifact by artifact_id
- must respect sensitivity/taint

### 4.3 draft_view
- shows the latest reasoning output/draft for an operation
- may support token streaming

### 4.4 diff_view
- shows canonical JSON diff for an approval item
- must render the exact SyscallEnvelope arguments diff

### 4.5 approval_action
- approve/deny/edit actions for a specific approval_id
- edit must be schema-validated before accept

### 4.6 metrics_panel
- renders wedge metrics from a read endpoint
- must not expose sensitive raw data; only aggregates

### 4.7 input_form
- structured form inputs (e.g., recipients, subject)
- values are staged into an approval edit payload, not executed directly

---

## 5) Bindings and data sources

Bindings reference OS objects by stable identifiers:
- operation_id
- approval_id
- artifact_id
- audit_trace_id

Allowed data sources (Root Owner control plane):
- operation status
- approval state + diff payload
- artifacts metadata + content refs (subject to ceiling)
- wedge metrics aggregates
- audit trace timeline summary

Bindings MUST NOT allow arbitrary queries.

---

## 6) Gate and taint constraints for UI composition

### 6.1 Viewer model
In v0.1, viewer is Root Owner only.

Future: viewers may be audiences. When that is enabled:
- UI composition must use Audience Graph scope and ceiling rules
- unknown audience: deny

### 6.2 Taint propagation
For each UIBlock:
- compute `block_taint = MAX(all bound data taint)`
- block_taint is persisted and must not decrease during operation

If a block binds to S3 data:
- that block remains S3 even if it displays a short derived summary

### 6.3 Operation isolation and UI
Blocks must not mix outputs from different operations unless:
- explicit IPC artifacts are referenced
- and the receiving operation inherits the higher taint

---

## 7) Audit and replay

InterfaceInstance creation must be auditable:
- record which blocks were produced
- record which bindings were used
- record taint levels
- record any redactions applied

Replay:
- dry_run re-renders InterfaceInstance from stored artifacts/events without executing side effects
- deterministic: same instance_id should render same blocks for the same pinned versions

---

## 8) Storage and contracts

InterfaceSpec and InterfaceInstance can be stored as artifacts:
- InterfaceSpec in registry keyed by interface_ref
- InterfaceInstance as an artifact linked to operation_id

The contract requires:
- immutable storage of InterfaceSpec
- persisted InterfaceInstance per operation when generated

---

## 9) Minimum acceptance tests (post–Wedge v0)

1) Unknown block types are rejected by renderer.
2) Diff view renders canonical JSON diff exactly as approved.
3) UI blocks cannot display data above viewer ceiling.
4) Blocks derived from S3/S4 inputs remain tainted for operation lifespan.
5) InterfaceInstance is persisted and replayable without side effects.
