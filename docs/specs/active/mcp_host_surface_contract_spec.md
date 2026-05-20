# MCP Host Surface Contract Spec v0.1 (External Agent Integration Plane)
Adesh OS

This document defines the **MCP Host** surface exposed by Adesh OS to external clients (OpenClaw, Claude Desktop, other agents). It specifies:
- the exact MCP tools Adesh OS exposes
- request/response schemas for each tool
- scoping and audience graph enforcement
- sensitivity/taint ceilings and redaction rules
- how external agents are mapped to `audience_id` nodes
- strict separation from HTTP control plane (Root Owner only)

This is an integration contract. Not implementation code.

---

## 0) Core principles

1. **MCP Host is not the control plane**
External clients must not be able to:
- mutate governance config
- approve operations
- view full compiled slices
- access raw corpora
- bypass Audience Graph default deny

2. **External agents are audiences**
Every MCP client connection maps to an `agent_client` node in the Audience Graph.

3. **Default deny**
If no edge exists from `root_owner -> agent_client_X` (or an explicit policy for that agent), responses are minimal or refused.

4. **Ceiling enforcement**
All data returned via MCP must be filtered to the agent’s ceiling and allowed scopes.

---

## 1) Identity and mapping

### 1.1 Agent identity
On MCP connection, OS derives:
- `agent_client_id` (stable across reconnects if possible)
- `agent_display_name` (if provided)
- connection metadata (not exposed as secrets)

### 1.2 Audience node mapping
- Map connection to `audience_id = agent:<agent_client_id>`
- Create node if missing with type `agent_client` (but with no scopes by default)
- Do not auto-create permissive edges.

---

## 2) Exposed MCP tools (profiled)

The MCP host surface is profile-based.

### 2.1 Active profile: `cognitive_v0` (current wedge)

This is the active profile for the coding-continuity proof and is backed by the same cognition core used by CLI wrappers.

Exposed tools:
1. `adesh.prepare_task_context`
2. `adesh.store_work_episode`
3. `adesh.recall_relevant_memory`
4. `adesh.connector_event`

Rules:
- no new cognition behavior is introduced by transport
- MCP is a thin adapter over the existing tool contracts
- no approval, audience-graph mutation, capability mutation, or raw corpus export

### 2.2 Deferred profile: `legacy_ops`

The broader operation-control MCP surface remains deferred for this proof path. When reactivated, it includes:
- `adesh.submit_request`
- `adesh.get_operation_status`
- `adesh.get_operation_result`
- `adesh.get_artifact_head`
- `adesh.get_artifact_content`
- `adesh.list_capabilities`
- `adesh.request_owner_approval`

---

## 3) Tool schemas (`cognitive_v0`)

All tools accept and return JSON.

### 3.1 adesh.prepare_task_context

Purpose: return compact, evidence-grounded guidance for the current task using cross-session memory.

Input:
```json
{
  "workspace": {
    "kind": "git|directory|conversation|task_space|unknown",
    "locator": "string|null",
    "cwd": "string|null",
    "branch": "string|null",
    "external_ref": "string|null"
  },
  "task_prompt": "string",
  "files_in_focus": ["string"],
  "task_hint": "string|null"
}
```

Output:
- exact schema is `PrepareTaskContextResponse` from the active contracts
- compact sections with capped items and evidence grounding

### 3.2 adesh.store_work_episode

Purpose: persist one bounded episode and trigger scoped memory promotion.

Input:
```json
{
  "workspace": {
    "kind": "git|directory|conversation|task_space|unknown",
    "locator": "string|null",
    "cwd": "string|null",
    "branch": "string|null",
    "external_ref": "string|null"
  },
  "task_prompt": "string",
  "summary": "string",
  "files_touched": ["string"],
  "tests": [
    {
      "name": "string",
      "status": "pass|fail|skip",
      "summary": "string|null"
    }
  ],
  "decisions": [
    {
      "decision": "string",
      "rationale": "string|null"
    }
  ],
  "unresolved_items": ["string"],
  "observed_preferences": ["string"],
  "risk_signals": ["string"],
  "issue_refs": ["string"],
  "artifact_refs": ["string"],
  "task_hint": "string|null",
  "started_at": "RFC3339|null",
  "ended_at": "RFC3339|null"
}
```

Output:
- exact schema is `StoreWorkEpisodeResponse` from the active contracts
- includes persisted episode identifiers and resolved workspace scope

### 3.3 adesh.recall_relevant_memory

Purpose: focused memory recall without full task-context assembly.

Input:
```json
{
  "workspace": {
    "kind": "git|directory|conversation|task_space|unknown",
    "locator": "string|null",
    "cwd": "string|null",
    "branch": "string|null",
    "external_ref": "string|null"
  },
  "query": "string",
  "task_hint": "string|null",
  "memory_types": ["string"],
  "limit": "integer|null"
}
```

Output:
- exact schema is `RecallRelevantMemoryResponse` from the active contracts

### 3.4 adesh.connector_event

Purpose: normalize host lifecycle events into prepare/store cognition calls.

Input:
- exact schema is `ConnectorEventRequest` from the active contracts
- supports event kinds:
  - `task_start`
  - `task_checkpoint`
  - `task_end`
- optional supervisory trace metadata may be included:
  - `host_agent_id`
  - `host_agent_kind`
  - `host_model`
  - `context_id`
  - `selected_next_direction`
  - `outcome`
  - `correction_summary`
- these fields are advisory in `cognitive_v0` and are persisted for future supervisory analytics;
  they do not add approval/gating behavior in this profile

Output:
- exact schema is `ConnectorEventResponse` from the active contracts
- includes:
  - `handled_as = prepare_task_context | store_work_episode`
  - either `prepare_context` or `stored_episode`
  - degraded-mode warnings when connector payload is sparse

### 3.5 Transport envelope

For stdio MCP:
- methods must be fail-closed on invalid schema
- `tools/list` enumerates only the active profile tools
- `tools/call` accepts one of the active tool names above

### 3.6 Local safety posture for `cognitive_v0`

The active profile is currently local stdio adapter usage for trusted host agents.
It does not replace the broader audience-graph policy model required by the deferred `legacy_ops` profile.

---

## 4) Scoping and safety by profile

### 4.1 `cognitive_v0` (active)

- scope resolution and memory ceilings come from the cognition contracts and storage-backed policy used by CLI
- stdio adapter must fail-closed on invalid payloads, unknown tools, and schema mismatches
- no control-plane mutation or approval paths are exposed

### 4.2 `legacy_ops` (deferred)

When the deferred profile is reactivated, it must enforce:
- audience-graph default deny
- scope checks against audience edges
- sensitivity and taint ceilings with refusal/sanitized views

---

## 5) Logging and audit

All MCP tool calls must be logged with:
- tool name
- request outcome (ok/error)
- workspace scope key when available

When `legacy_ops` is enabled, logs must additionally include:
- mapped audience identifiers
- operation/request identifiers as applicable

No secrets in logs.

---

## 6) Minimum test cases (must pass)

### 6.1 Active `cognitive_v0`

1. `initialize` returns MCP server info and `tools` capability.
2. `tools/list` returns only:
   - `adesh.prepare_task_context`
   - `adesh.store_work_episode`
   - `adesh.recall_relevant_memory`
   - `adesh.connector_event`
3. `tools/call` with valid payload returns structured content.
4. `tools/call` with unknown tool fails closed.
5. Invalid payload schema fails closed.

### 6.2 Deferred `legacy_ops`

When re-enabled, existing audience/scope/ceiling tests remain mandatory:
1. default deny without audience edge
2. scoped allow with constrained ceiling
3. sensitive artifact refusal above ceiling
4. operation visibility restricted to created/shared operations
5. owner-approval request path does not auto-approve
