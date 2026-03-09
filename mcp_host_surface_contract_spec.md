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

## 2) Exposed MCP tools (minimal, safe)

Adesh OS exposes only these tools:

1. `adesh.submit_request`
2. `adesh.get_operation_status`
3. `adesh.get_operation_result`
4. `adesh.get_artifact_head`
5. `adesh.get_artifact_content` (restricted)
6. `adesh.list_capabilities` (safe subset)
7. `adesh.request_owner_approval` (notification only, no approval)

No tools for:
- approving
- modifying audience graph
- modifying capabilities
- accessing compiled slice
- accessing raw Experience Log

---

## 3) Tool schemas

All tools accept and return JSON. All responses include:
- `ok`
- `error` if any
- `data`
- `meta` with timestamps

### 3.1 adesh.submit_request
Purpose: delegate a task to Adesh OS.

Input:
```json
{
  "client_request_id": "string",
  "input": { "kind": "text", "content": "string" },
  "constraints": { "budgets": { "token_budget": 1024 } },
  "requested_scopes": ["scope:string", "..."],
  "target_audience_hint": "string|null"
}
```

Rules:

* OS maps this to a RequestEnvelope internally with `requesting_audience_id = agent:<id>`
* Governance applies Audience Graph for what the agent is allowed to ask and receive.

Output:

```json
{
  "request_id": "string",
  "operation_ids": ["string"],
  "primary_operation_id": "string"
}
```

If agent lacks scopes:

* refuse with `FORBIDDEN` style error and remediation: “owner must grant scopes.”

### 3.2 adesh.get_operation_status

Input:

```json
{ "operation_id": "string" }
```

Output:

```json
{
  "operation_id": "string",
  "state": "created|compiled|awaiting_approval|running|blocked|completed|failed|cancelled",
  "summary": "string",
  "audit_trace_id": "string|null"
}
```

Scoping:

* agent can only query operations it created OR operations explicitly shared to it via audience policy.

### 3.3 adesh.get_operation_result

Returns a scoped view of results.

Input:

```json
{ "operation_id": "string" }
```

Output:

```json
{
  "operation_id": "string",
  "result": {
    "drafts": [{ "title": "string", "content": "string" }],
    "artifacts": [{ "artifact_id": "string", "kind": "string", "sensitivity_s": 1, "taint_s": 1 }]
  }
}
```

Rules:

* Draft content must be redacted/summarized to agent ceiling.
* If operation output is above ceiling:

  * return a refusal or a sanitized summary only.

### 3.4 adesh.get_artifact_head

Input:

```json
{ "artifact_id": "string" }
```

Output:

```json
{
  "artifact_id": "string",
  "kind": "string",
  "size_bytes": 123,
  "sensitivity_s": 2,
  "taint_s": 2,
  "provenance_refs": ["..."]
}
```

Rules:

* If artifact sensitivity exceeds agent ceiling, return only minimal metadata or refuse.

### 3.5 adesh.get_artifact_content

Input:

```json
{ "artifact_id": "string", "mode": "snippet|full" }
```

Rules:

* `full` is allowed only if artifact sensitivity <= agent ceiling AND scopes allow.
* Otherwise:

  * return snippet that is sanitized to ceiling or refuse.

Output:

```json
{ "artifact_id": "string", "content": "string", "content_type": "text/plain" }
```

### 3.6 adesh.list_capabilities

Input: empty.
Output:

* safe subset of capabilities:

  * tool names and general descriptions
  * no internal endpoints, no secrets
  * risk floors (optional)

### 3.7 adesh.request_owner_approval

Purpose: external agent can request that Root Owner approve a specific action. This does not grant approval. It creates a notification/review item.

Input:

```json
{
  "operation_id": "string",
  "message": "string",
  "requested_action_summary": "string"
}
```

Output:

```json
{ "ok": true, "queued": true }
```

Rules:

* This creates a review queue item or notification for Root Owner.
* Root Owner must approve via HTTP control plane.

---

## 4) Scoping and ceilings for MCP responses

### 4.1 Determine agent ceiling

From Audience Graph edge:

* `root_owner -> agent:<id>` or a specific `system -> agent` policy
  If no edge:
* default deny (no data)

### 4.2 Apply scope checks

For any returned content:

* determine scopes of the content:

  * from IPCArtifact scope tags
  * from operation classification
* ensure scopes are allowed for that agent edge

### 4.3 Apply ceiling and taint checks

* if `S_content > S_ceiling` or `T_content > S_ceiling`:

  * return refusal or sanitized snippet
  * do not leak content above ceiling

---

## 5) Logging and audit

All MCP tool calls must be logged with:

* agent_client_id
* mapped audience_id
* request_id/operation_id when relevant

Experience Log must store:

* `kind=mcp_call`
* tool name
* agent id
* outcome (ok/denied)

No secrets in logs.

---

## 6) Minimum test cases (must pass)

1. Default deny:

* agent connects with no edge -> submit_request refused.

2. Scoped allow:

* owner grants scope work:status_updates ceiling S1 -> agent can retrieve only S1 content.

3. Artifact content:

* agent requests full content of S3 artifact -> refused.

4. Operation result scoping:

* agent can only see operations it created, unless shared.

5. request_owner_approval:

* creates notification but does not approve or execute anything.

```
