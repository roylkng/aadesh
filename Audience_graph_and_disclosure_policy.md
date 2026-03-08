# Audience Graph and Disclosure Policy Spec v0.1
Adesh OS

This document specifies the **Audience Graph** and how Adesh OS enforces **audience-conditional disclosure**. It defines:
- the audience graph data model (nodes, edges, scopes)
- default-deny behavior and bootstrap Root Owner node
- how audiences are resolved for outbound syscalls
- how scopes, ceilings, and topic partitions are applied
- how disclosure interacts with taint, sanitization, and IPC artifacts
- how updates are gated and audited

This is algorithmic logic. Not implementation code.

---

## 0) Core principles

1. **Default deny**
- Any unknown node, unknown edge, or unspecified scope implies deny for disclosure.

2. **Root Owner bootstrap**
- The system initializes with a single irremovable `root_owner` node.
- Control plane requests map to `root_owner`.
- `root_owner` has maximum privileges over their own data, but outbound disclosures remain scoped by edges.

3. **Disclosure is enforced at syscall time**
- The model can propose recipients. The OS decides whether disclosure is allowed.

4. **Scopes are explicit**
- Disclosure is not “all data.” It is explicitly partitioned into scopes (topics/domains).

5. **Ceilings are enforced**
- Each edge has a sensitivity ceiling `S_ceiling` and allowed scopes.
- Taint laundering rules apply in addition.

---

## 1) Audience Graph data model

### 1.1 Node
A node represents an entity or a channel:
- `node_id` (stable)
- `node_type`:
  - `root_owner`
  - `person`
  - `group`
  - `role`
  - `channel`
  - `public`
  - `agent_client`
  - `system`
- `label` (human-readable)
- `props` (structured metadata, minimal)
- `graph_version`

### 1.2 Edge
An edge represents a relationship from a source to a target:
- `edge_id`
- `src_id`
- `dst_id`
- `edge_type` (e.g., `self`, `family`, `coworker`, `vendor`, `public_audience`, `agent_delegate`)
- `props` (optional)
- `graph_version`

Edges are directional. Disclosure is evaluated from `src_id -> dst_id`.

### 1.3 Scope Policy (per edge)
A scope policy is a binding of:
- `scope_id`
- `src_id`
- `dst_id`
- `allowed_scopes[]` (strings)
- `sensitivity_ceiling_s` (0..4)
- optional `constraints`:
  - time bounds
  - context predicates
  - “never disclose” overrides for certain subtopics
- `graph_version`

---

## 2) Scope taxonomy (open-ended but structured)

Scopes are strings, namespaced to avoid collisions:
- `work:project_x`
- `work:status_updates`
- `personal:family`
- `personal:health` (typically restricted)
- `finance:accounts` (restricted)
- `os:capabilities` (usually safe)
- `public:bio` (safe)

Rules:
- Scopes are additive sets.
- Absence of a scope means deny for that topic category.

The scope system is intentionally open-ended. The kernel does not need a fixed list, but it must treat scope strings as first-class policy identifiers.

---

## 3) Bootstrap and default policies

### 3.1 Root Owner node
On first boot:
- create `node_id = root_owner`
- node_type = `root_owner`
- edge `root_owner -> root_owner` exists with:
  - allowed_scopes = `["*"]` (or a defined “all owner scopes”)
  - sensitivity_ceiling_s = 4

### 3.2 Unknown node behavior
If a syscall resolves to a recipient that cannot be mapped to an existing node:
- deny with `audience_scope_denied`
- remediation: ask_user to add node and define scope policy

### 3.3 Unknown edge behavior
If node exists but no edge `root_owner -> target` exists:
- deny by default
- remediation: ask_user to create edge and scope policy

---

## 4) Audience resolution for outbound syscalls

Outbound syscalls are those with predicate:
- `sends_information_to_third_party` OR `publishes_publicly`

### 4.1 How to resolve target audience_id
Resolution priority:
1. If `proposed_syscall.declared_audience.audience_id` is provided and exists: use it.
2. Else infer from syscall args:
   - email recipients -> match known person/group nodes by email
   - slack channel id -> match channel node
   - “public post” -> use `public` node
3. If ambiguous or no mapping:
   - deny and ask_user to select/create target audience node

### 4.2 Audience hint field
`audience_hint` is non-authoritative and used only to help mapping.

---

## 5) Disclosure decision algorithm

Inputs:
- `src_id` (usually `root_owner`)
- `dst_id` (resolved target)
- `requested_scopes[]` for the syscall (from artifact tags or inferred)
- computed data sensitivity `S_data` and taint `T_data`

### Step 5.1: Locate scope policy
Find `scope_policy` for `(src_id, dst_id)` in current graph version.
If none: deny.

### Step 5.2: Evaluate allowed scopes
Determine `scopes_in_play`:
- If the syscall references IPCArtifacts:
  - use `IPCArtifact.audience_scope_tag.allowed_scopes`
- Else infer scopes from content category:
  - e.g., if sending a work summary, scope might be `work:status_updates`
- If scopes cannot be inferred deterministically:
  - require explicit user confirmation or explicit scope selection (ask_user remediation)

Decision:
- Allowed if all scopes_in_play are included in policy allowed_scopes
- Special handling:
  - wildcard `*` allowed only for root_owner->root_owner and explicitly configured “trusted” edges

If scope mismatch: deny with `audience_scope_denied` and remediation `reduce_scope` / `ask_user`.

### Step 5.3: Enforce sensitivity ceiling
Let `S_ceiling` = policy sensitivity_ceiling_s.
If `S_data > S_ceiling`: deny `sensitivity_ceiling_exceeded` with remediation `sanitize` or `reduce_scope`.

### Step 5.4: Enforce taint laundering
Let `T_data` be max taint influence.
If `T_data > S_ceiling`:
- deny `taint_laundering_risk`
- remediation requires explicit sanitization to create a `sanitized_view` artifact whose validated taint/sensitivity is <= ceiling

### Step 5.5: Output
Return:
- allowed (permit) with `scopes.allowed`, `ceiling`
OR
- deny with `SyscallDeny` containing constraint ids:
  - `audience::<src>::<dst>::scope::<scope_id>`
  - `ceiling::<dst>::sX`
  - `taint::in_sX::ceiling_sY::requires_sanitizer`

---

## 6) IPCArtifacts and audience tagging

IPCArtifacts must carry `audience_scope_tag`:
- `allowed_scopes[]`
- `max_disclosure_s`

Rules:
- When an IPCArtifact is created, it must be tagged conservatively based on its sources.
- If derived from S3 sources, its tag must not allow broad public scopes unless explicitly sanitized and reviewed.

Outbound syscalls must:
- reference IPCArtifacts explicitly
- verification checks artifact scope tags against edge policy

---

## 7) Governance gates for Audience Graph updates

Audience Graph updates are themselves governed.

### 7.1 Operations that mutate the graph
- add node
- add edge
- add/edit scope policy
- raise ceiling
- add wildcard scopes
- connect external agent_client

These are at least R3 when they increase disclosure surface, and R4 if:
- creating wildcard edges
- raising ceiling to S3/S4 for external audiences
- connecting agent_client to privileged scopes

### 7.2 Update workflow
- change proposals are created as review queue items unless explicitly owner-driven via UI with approvals
- applying a patch creates a new `audience_graph_version`
- update is audited:
  - Experience Log `audience_graph_patch`
  - AuditTrace timeline

---

## 8) External agents (MCP Host) as audiences

MCP Host clients map to `agent_client` nodes.

Default:
- agent_client has no scopes and ceiling S0 unless configured.

Any attempt by agent_client to request higher sensitivity:
- denied by default
- remediation: root_owner must explicitly configure edge and scopes

---

## 9) Minimum test cases (must pass)

1. Unknown recipient:
- deny with audience_scope_denied and ask_user to add node.

2. Ceiling enforcement:
- S3 data to edge with ceiling S1 -> deny.

3. Taint laundering:
- derived artifact taint S3 even if text short -> deny without sanitizer.

4. IPC scope tagging:
- artifact tagged work:project_x cannot be sent to personal:family edge.

5. Graph update gating:
- attempt to raise ceiling to S4 for vendor -> requires OOB (R4) or refused by policy.
