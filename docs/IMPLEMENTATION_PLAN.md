# Aadesh Implementation Plan

Status: sequencing document only. Canonical behavior remains in root-level specs.

## Active implementation target

The active v1 proof is:

- cross-session cognitive continuity and personalization for host coding agents
- delivered as a callable cognitive sidecar
- local-first, storage-backed, and transport-agnostic at the core

This plan does not discard the broader governed-execution architecture already documented in the repo. It narrows the first shipped proof to the minimum slice required to show that Aadesh is a real missing layer.

## Product proof to build

The first proof succeeds only if a strong host coding agent can ask Aadesh for current-task guidance and receive compact, evidence-grounded, cross-session output that materially improves:

- recall of relevant prior decisions
- recall of unresolved loops and risks
- alignment with user and workspace preferences
- speed to a useful next action
- reduction in repeated user restatement

The proof must work even when the host is not explicitly resuming a prior session.

## What is reused now

Reuse directly:
- local SQLite storage and migrations
- `experience_events`
- `artifacts`
- `claims`, `claim_evidence`, `claim_conflicts`
- `jobs`
- queue foundation
- model/provider abstractions
- typed Rust contracts and response envelopes

Reuse with light refactoring:
- daemon entry points and app wiring
- existing storage provider boundaries
- existing background job flow for extraction/promotion work

## What is not the active product path

Keep but de-center:
- the legacy email draft-and-send slice
- approval/OOB-heavy governed execution as the primary product API
- local UI shell as the main user surface
- workflow/interface runtime as a hot path
- generalized actuator expansion

These remain in the repo as broader architecture or deferred execution material. They must not drive milestone scope for the current proof.

## Generic architecture stance

Do not bake the coding wedge into the base architecture.

Core identity and memory modeling must remain generic:
- `workspace` rather than mandatory `repo_id`
- scoped memory rather than coding-only memory tables
- generic callable cognition surface rather than assistant-specific integration logic

Coding is the first wedge, not the permanent shape of the core.

## Minimum v1 callable surface

Phase-1 tools only:
- `store_work_episode`
- `prepare_task_context`
- `recall_relevant_memory`

Front door order:
1. CLI
2. MCP

HTTP/UI changes are not on the hot path unless required for shared runtime plumbing.

## Minimum v1 data model

Add now:
- `episodes`
- `episode_artifacts`
- lexical search projection or SQLite FTS table
- scoped memory fields on claims:
  - `scope_type`
  - `scope_key`
  - `subject_key`

Reuse now:
- `experience_events` for raw observations
- `claims` for candidate and confirmed memory
- `claim_evidence` for provenance links
- `claim_conflicts` for contradiction tracking

Defer:
- vector retrieval
- graph database
- remote sync
- passive capture adapters for every tool

## v0 workspace identity rules

Identity must be generic and degradation-friendly.

Resolution order:
1. explicit `workspace.locator`
2. git metadata when present
3. current working directory
4. transient workspace mode

The architecture must work for coding and non-repo tasks. Coding-specific heuristics belong in the wedge adapter layer, not in the base storage model.

## Memory promotion rules for the first proof

Use explicit and conservative thresholds.

- candidate preference: one signal
- confirmed workspace-scoped preference: two aligned signals across two episodes in the same scope
- confirmed user-global preference: three aligned signals across at least two workspaces
- confirmed explicit decision: explicit decision plus at least one evidence ref
- confirmed open loop: one explicit unresolved item until resolved or superseded
- confirmed inferred risk: two aligned signals in the same scope, or one signal plus one deterministic artifact

Model-only inference stays candidate memory until corroborated.

## Output discipline for `prepare_task_context`

Hard limits:
- max 3 relevant decisions
- max 3 applicable preferences
- max 3 open loops
- max 3 risks
- max 3 likely next directions
- max 3 uncertainties

Every returned item must include:
- `confidence`
- `evidence_refs`
- `basis`

This is mandatory to avoid broad plausible dumps.

## Phase order

### Phase 0: Repository realignment
Acceptance criteria:
- top-level docs reflect the cognitive-sidecar direction
- active wedge doc is the coding continuity proof
- email wedge is explicitly marked deferred
- navigation docs point to the new proof path first

### Phase 1: Storage and schema slice
Acceptance criteria:
- migrations exist for episodes and scoped memory lookup
- lexical retrieval surface exists
- workspace identity resolution is implemented generically
- existing claims/evidence tables are reused rather than replaced

### Phase 2: Episode ingestion
Acceptance criteria:
- `store_work_episode` exists behind a stable CLI command
- raw observations and episode summaries persist durably
- artifact links and unresolved items persist
- extraction/promotion shortcuts for the proof scenario exist

### Phase 3: Current-task guidance
Acceptance criteria:
- `prepare_task_context` exists behind a stable CLI command
- retrieval uses metadata plus lexical evidence
- output obeys the compact contract and item caps
- output includes evidence refs, confidence, and basis for every item

### Phase 4: Focused recall and proof harness
Acceptance criteria:
- `recall_relevant_memory` exists
- payments-style seeded proof scenario passes
- baseline vs treatment evaluation harness exists
- false-memory rate and relevance scoring are measurable

### Phase 5: Host integration hardening
Acceptance criteria:
- CLI is stable
- MCP surface reuses the same internal cognition core
- no wedge logic is duplicated between transports

## Out of scope for the current proof

- email/send execution expansion
- new approval/OOB flows
- UI redesign as a product requirement
- workflow execution engine
- interface composition runtime
- broad tool/action ecosystem work
- distributed memory sync

## Validation discipline

Before behavior changes:
1. read `index.md`
2. confirm relevant canonical specs still match the intended change
3. run the spec drift guard
4. keep changes vertical, compileable, and testable

Validation commands:

```bash
./.codex/skills/adesh-spec-guard/scripts/check_spec_drift.sh .
cargo fmt --all
cargo test --workspace
```

## Immediate next implementation slice

Build in this order:
1. schema migration plan
2. `store_work_episode`
3. `prepare_task_context`
4. proof test using the seeded payments-style scenario

Only after that should the repo broaden into richer retrieval, MCP, or passive observation capture.
