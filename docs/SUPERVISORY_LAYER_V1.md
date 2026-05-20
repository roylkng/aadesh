# Aadesh Supervisory Layer v1 (Design Note)

Status: forward-looking design note.
Authority: non-canonical; v0 behavior remains defined by active root contracts and wedge docs.

## 1) Purpose

Define the next layer above the current cognition core:

- `v0 now`: cross-session cognitive continuity + personalization (advisory context shaping)
- `v1 later`: supervisory cognitive control across multiple coding agents

This note prevents strategic drift while keeping current implementation scope tight.

## 2) What supervisory control means

Supervisory control is not just better memory retrieval. It adds policy and intervention over agent behavior:

- track agent-by-agent task outcomes over time
- compare guidance surfaced vs actions actually taken
- detect repeated quality failures or policy violations
- choose advisory intervention first, with optional future gating

## 3) Required observability to preserve now

The v0 connector path should retain enough trace data for future supervision:

- host and agent identity (`host_agent_id`, `host_agent_kind`, `host_model`)
- execution context linkage (`context_id`)
- guidance adoption signal (`selected_next_direction`)
- outcome summary (`outcome`)
- post-output correction marker (`correction_summary`)

In v0 these are optional and non-blocking. They are persisted as trace artifact references.

## 4) Intervention levels (future)

Planned escalation levels for v1+:

1. Advisory: suggest corrections or alternative plans.
2. Critique: require self-check pass before final response.
3. Policy gate (deferred): block or require approval for high-risk actions.

Only advisory behavior is in scope for v0.

## 5) What remains out of scope for current wedge

- multi-agent orchestration runtime
- approval/veto enforcement logic for code generation
- automatic gating of host agent outputs
- cross-host policy DSL

These are deferred until v0 cognitive usefulness is repeatedly validated in real workflows.

## 6) Compatibility rule

All new host integrations should keep:

- cognition core transport-agnostic
- connector events as Aadesh-owned abstractions
- optional trace fields accepted without making them mandatory

This preserves the path from continuity layer to supervisory layer without a redesign.
