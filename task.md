Task list for next phase (drop into an .md)

0) Baseline alignment
		•	Freeze Agent OS v2.5 as the current north-star spec.
		•	Define Phase 1 target: daemon on Linux/macOS + Raspberry Pi with pluggable backends.

⸻

1) High-level system design (HLD) deliverables
	•	Component map: Gateway, Scheduler, Governance Kernel, JIT Compiler, Verification Core, Reasoning Core Adapter, Experience Log, Active State Store, Reflection Workers, Owner UI, MCP Fabric.
	•	Trust boundaries: what runs trusted vs untrusted, what is sandboxed.
	•	Operation lifecycle state machine: created → compiled → awaiting_approval → running → blocked → completed/failed/cancelled.
	•	Sequence flows (at least):
	•	R1: draft-only response
	•	R2: action requiring confirmation
	•	R3: diff + approval flow
	•	Blocked syscall: policy-aware rejection payload + replanning
	•	IPC pipe: S3 artifact piped to S3 email operation with sensitivity inheritance
	•	Plan-trajectory drift detection flow
	•	Deployment topology:
	•	Linux/macOS dev workstation mode
	•	Raspberry Pi appliance mode
	•	Storage + encryption boundaries
	•	Diagnostics plan (HLD-level): logs, traces, metrics, audit traces, replay.

⸻

2) Interface contracts (kernel API)

2.1 Embodiment Kernel contract (MCP-first)
	•	Sensor registration schema:
	•	modality + observation schema
	•	trust/reliability class
	•	sensitivity ceiling
	•	default audience scoping
	•	cost/rate limits
	•	observability fields
	•	Actuator registration schema:
	•	action schema
	•	risk floor (R-min)
	•	diff capability contract
	•	required approval mode
	•	safety constraints + rate limits
	•	observability fields
	•	Capability self-model export schema (what the OS tells the reasoning core).

2.2 Syscall contract (all tool reads/writes)
	•	Standard syscall request/response envelope:
		•	operation_id, isolation_id, pinned versions (`active_state_version`, `capability_snapshot_version`, `audience_graph_version`)
	•	computed gate: R, S, max
	•	taint labels in/out
	•	Policy-aware rejection payload:
	•	violated constraint ids
	•	triggering fields/data classes
	•	retry allowed flag + conditions
	•	remediation hints (ask user, sanitize, alternate actuator, OOB, refuse)

2.3 JIT compilation contract
	•	compile_entity_slice input/output:
	•	working memory blocks + per-block budgets
	•	deterministic packing/omissions report
	•	audience-scoped slice
	•	taint propagation metadata
	•	intent anchor attachment
	•	audit_trace_id

2.4 IPC / piping contract
	•	Artifact format for IPC:
	•	payload, provenance refs, sensitivity, taint, audience scope tag
	•	Sensitivity inheritance rules:
	•	receiver inherits max S
	•	receiver recompiles under inherited gate
	•	Sanitized IPC syscall definition (explicit redaction/aggregation step)

2.5 Audit + replay contract
	•	Audit trace schema:
	•	gates, scope filters, taint decisions
	•	blocks injected, omissions, provenance refs
	•	approvals, diffs, OOB events
	•	Replay interface:
	•	reproduce decision with pinned versions and inputs

⸻

3) Minimal data model (only what contracts require)
	•	Experience Log canonical event schema (covers chat, tool trace, telemetry).
	•	Active State versioning model (snapshots/transactions).
	•	Audience Graph schema (nodes, edges, scopes, default deny, root owner binding).
	•	Hypothesis ledger schema:
	•	primitive types, predicates, time bounds
	•	confidence/stability tier
	•	evidence refs + evidence quality fields
	•	contradictions + exception links
	•	Working memory block store schema (optional persisted vs ephemeral).

⸻

4) KRIs/KPIs and instrumentation plan

KRIs (risk indicators)
	•	Leakage attempts blocked vs succeeded (by S level).
	•	Policy violation attempts by syscall type.
	•	Cross-operation contamination incidents (should be 0).
	•	Taint laundering attempts detected (should be 0).
	•	Plan-trajectory drift events detected and stopped.
	•	Infinite retry loop occurrences (should trend to 0).

KPIs (product)
	•	Acceptance rate, normalized edit distance, turns-to-accept (per task family).
	•	P95 latency for sync loop (Pi vs workstation).
	•	Reflection backlog age and throughput.

⸻

5) Phase 1 build plan (daemon, reference SQLite backend first)
	•	Build the daemon skeleton:
	•	gateway + scheduler + operation state machine
	•	governance kernel enforcement
	•	JIT compiler with token budgets + block packing
	•	experience log append + active state version pinning
	•	MCP fabric:
	•	MCP client (call tools)
	•	MCP host (expose OS syscalls if needed)
	•	Minimal sensors (pick 2–3):
	•	filesystem reader
	•	notes/todo store
	•	optional email connector (read-only first)
	•	Minimal actuators (pick 2):
	•	draft message (R1)
	•	send message with confirmation (R2)
	•	Owner UI minimal:
	•	approvals (confirm/diff/OOB stub)
	•	audit trace viewer
	•	audience graph editor (basic)
	•	Reflection workers minimal:
	•	append-only ingestion → candidate hypothesis extraction
	•	review queue creation (no auto-promotion beyond low risk)

⸻

6) Test plan (must exist before feature expansion)
	•	Unit tests for gate computation (max(R,S)).
	•	Red-team tests for prompt injection and leakage.
	•	Taint propagation tests across blocks and outputs.
	•	IPC inheritance tests.
	•	Policy-aware rejection behavior tests (no infinite retries).
	•	Replay tests with pinned versions.

⸻
