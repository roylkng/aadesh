# Capability Registry and MCP Integration Spec v0.1
Adesh OS

This document specifies the **Capability Self-Model** and how Adesh OS integrates tools via **MCP Client** and exposes itself via **MCP Host**. It defines:
- the canonical capability descriptor schema (sensors and actuators)
- discovery, refresh, and pinning semantics (`capability_snapshot_version`)
- enable/disable flows and their governance gates
- how tool schemas are stored and referenced (`schema_ref`)
- how syscalls map to tool capabilities deterministically
- how external agents connect through MCP Host and become `audience_id` nodes

Action-level execution schema behavior is governed jointly with `schema_based_tools_and_actions.md`.

This is algorithmic logic. Not implementation code.

---

## 0) Core principles

1. **Capabilities are part of the OS physics**
- If a tool is not in the registry, it does not exist to the Reasoning Core.
- Capability registry is injected into the CompiledSlice capability block.

2. **Capability snapshot is pinned per operation**
- Each operation must pin `capability_snapshot_version`.
- Tool changes after pinning must not affect the in-flight operation unless the user explicitly requests refresh and recompilation.

3. **MCP is the integration plane, not the control plane**
- The HTTP API is Root Owner control plane.
- External agent audiences interact via MCP Host.

4. **Tools define risk floors and approval requirements**
- Tool metadata is authoritative over model intent.
- Syscall risk cannot fall below tool risk floor.

---

## 1) Capability Descriptor (canonical schema)

The registry must store two lists:
- `sensors[]`
- `actuators[]`

Each descriptor must be immutable within a snapshot version.

### 1.1 Common fields (Sensor and Actuator)
- `name`: stable unique name (string)
- `provider`: `mcp|adapter|internal`
- `endpoint_ref`: stable reference to the MCP server instance or adapter
- `status`: `enabled|disabled|degraded`
- `trust_class`: `trusted|semi_trusted|untrusted`
- `schema_ref`: pointer to the capability-level tool schema bundle or descriptor document (see Section 2)
- `rate_limits`:
  - `max_calls_per_minute`
  - `max_concurrent`
  - optional cost budgets

### 1.2 Sensor-specific fields
- `sensitivity_ceiling_s` (0..4): maximum sensitivity the sensor may return or access by default
- `data_domains[]`: conceptual labels (email, calendar, filesystem, web, db, etc.)
- `taint_policy`:
  - whether results are considered tainted by default
- `default_risk_r`: usually 0 or 1 unless sensor triggers side effects (rare)

### 1.3 Actuator-specific fields
- `risk_floor_r` (0..4): minimum risk classification
- `diff_supported` (bool): whether a safe diff payload can be produced
- `execution_class`: `external_api|host_local|sandboxed`
- `default_approval_mode`:
  - `none|confirm|diff|oob_required|refuse`
  - note: final approval_mode uses max(R,S) logic and may be stricter
- `side_effect_scope`:
  - `local|internal|external|public`
- `audience_required` (bool): whether the syscall must declare an audience (email recipient etc.)

If `execution_class=sandboxed`, the descriptor must also include:
- `sandbox_profile_id`
- `filesystem_policy`
- `network_policy`
- `resource_budgets`
- `artifact_capture_policy`

Behavioral details are governed by `sandboxed_actuator_capability.md`.

### 1.4 Tool Action descriptors
Within each tool schema, actions must declare or resolve to:
- `action_name`
- `args_schema_ref`
- `result_schema_ref` (optional but recommended)
- `risk_floor_override_r` (optional, action-level)
- `diff_template_ref` (optional)
- `forbidden_fields[]` (action-specific negative memory augment)

`schema_based_tools_and_actions.md` defines the action-level generic execution contract. Capability snapshots pin these action-level schema refs so the kernel does not need built-in per-tool logic.

---

## 2) Tool schema storage and referencing

### 2.1 SchemaRef types
At the capability level, `schema_ref` may point to:
- a blob content_ref
- a StorageProvider object ref
- an embedded schema id within a schema registry

The schema content should be JSON Schema compatible or a deterministic equivalent.

For syscall execution and approval edits, the authoritative schemas are the action-level `args_schema_ref` and optional `result_schema_ref` pinned through the capability snapshot.

### 2.2 Schema requirements
Each tool action schema must specify:
- required fields
- types
- constraints (format, enum)
- which fields are safe to edit in diff mode (editable fields)

This supports:
- Verification schema validation
- Approve-with-modifications
- generic externalized tool/action support without kernel-specific code

---

## 3) Capability snapshot and pinning

### 3.1 capability_snapshot_version
A capability snapshot is an immutable set of:
- sensors descriptors
- actuators descriptors
- schema_refs and their content hashes
- a timestamp and optional notes

Generate a new snapshot when:
- any tool is enabled/disabled
- any MCP server set changes
- any tool schema changes
- any tool health status changes beyond configured threshold (optional)

### 3.2 Pinning to operations
On operation creation:
- pin the current `capability_snapshot_version` into OperationSpec

During compilation:
- only the pinned snapshot is injected into the capability block.

If the user enables/disables a tool while an operation is running:
- it must not affect the operation unless:
  - the operation is explicitly recompiled (new operation or explicit refresh flow)
  - audit logs record the change and recompile

---

## 4) Capability discovery (MCP Client)

### 4.1 Discovery triggers
Discovery runs:
- on daemon startup
- on explicit “refresh capabilities” command
- on MCP server connection events
- periodically (optional) with a safe cadence

### 4.2 Discovery procedure
For each configured MCP server:
1. Connect and fetch tool list and schemas
2. Normalize tools into the canonical Capability Descriptor format
3. Assign:
   - trust_class (based on server config)
   - risk floors (configured defaults per tool category, overridden by schema metadata if present)
4. Store schemas and compute content hashes
5. Create a new `capability_snapshot_version`

### 4.3 Health and degraded status
Tools may be marked `degraded` if:
- repeated connection failures
- repeated timeouts
- schema mismatch errors

Degraded tools may still exist but:
- Verification should treat them as higher risk or block usage based on policy.

---

## 5) Enabling and disabling capabilities

### 5.1 Why this is gated
Enabling/disabling tools changes OS “physics”. Some of these changes are effectively self-modification.

### 5.2 Gate classification
- Disabling a sensor: typically R2–R3 depending on criticality
- Enabling an actuator with external side effects: at least R3
- Enabling/disabling a tool that affects governance, memory, auth, or kernel config: R4 (self-modification class)

### 5.3 Enable/disable flow
Control plane endpoints:
- `POST /v1/capabilities/{kind}/{name}/enable`
- `POST /v1/capabilities/{kind}/{name}/disable`

Required behavior:
1. Create an approval item if gate requires it (confirm/diff/OOB)
2. On approval commit:
   - apply change to capability config
   - run discovery refresh
   - mint new `capability_snapshot_version`
   - append Experience Log event (capability_change)
   - update AuditTrace
   - emit WS `capability_update`

For immutable snapshot candidates minted via the control plane:
- activation of a candidate snapshot into `current_versions` must go through the review queue
- direct current-version mutation is not allowed on the mint endpoint

---

## 6) Syscall mapping to capabilities

### 6.1 Target resolution
A proposed syscall includes:
- `target.kind`
- `target.name`
- `action`
- `args`

Verification must:
- find the target tool descriptor in pinned snapshot
- confirm status is enabled
- resolve action-level `args_schema_ref` from the pinned capability snapshot and validate args for action

If tool not found or disabled:
- deny with remediation:
  - ask_user to enable tool (gated)
  - alternate actuator

### 6.2 Risk floor enforcement
- `R_syscall >= risk_floor_r(tool, action)`
- even if model claims it is “just a draft”

### 6.3 Diff support requirement
If `approval_mode` requires diff and `diff_supported=false`:
- deny or force alternative:
  - manual workflow
  - require user to perform action outside OS
- must not silently downgrade approval mode.

---

## 7) MCP Host bridge (external agent integration)

### 7.1 External agent as an audience
Any MCP Host client is treated as an `audience_id` node:
- `principal_type = agent_client`
- map connection identity to `audience_id`
- default deny unless configured in Audience Graph

### 7.2 MCP Host tool surface (minimal)
Expose a small set of OS functions to external agents:
- submit_request (delegation)
- get_operation_status (scoped)
- fetch_result_artifact (scoped and ceiling-limited)
- request_approval (only Root Owner can approve, external cannot)

External agents must not:
- access raw control plane endpoints
- access full compiled slices
- bypass Audience Graph scopes

### 7.3 Scoping rules
For MCP Host calls:
- apply Audience Graph default deny and scope ceilings
- any returned data must be filtered/sanitized to ceiling
- any attempt to get higher sensitivity data must yield structured refusal

---

## 8) Capability block injection rules (JIT compiler input)

The capability block in CompiledSlice must include:
- a compact list of enabled tools (names + short constraints)
- per tool:
  - risk floor
  - approval hints (diff/oob)
  - key limitations
- global budgets (token/time/cost)
- a canonical “self-model limitations” section:
  - what the OS cannot do in this deployment profile

It must not include:
- raw secrets
- auth tokens
- internal endpoints unless explicitly safe

---

## 9) Storage and audit requirements

Every capability change must be recorded:
- Experience Log event: `kind=capability_change`
- AuditTrace timeline: capability_update
- New pinned snapshot version is stored and referenced

Capability discovery results should be auditable:
- store schema hashes
- store tool list
- store any changes compared to previous snapshot (diff)

---

## 10) Minimum test cases (must pass)

1. Disabled tool:
- model proposes syscall to disabled actuator
- verification denies with remediation and does not execute

2. Risk floor:
- actuator risk_floor_r=3
- model proposes it as low-risk
- syscall R must be >=3 and require diff approval

3. Snapshot pinning:
- operation pins snapshot V1
- user disables tool producing V2
- operation must still see V1 until recompile is explicitly triggered

4. MCP Host scoping:
- external agent connects with unknown audience edge
- default deny applies and returns refusal

5. Diff support:
- approval requires diff but tool says diff_supported=false
- must deny or route to manual path, never downgrade silently
