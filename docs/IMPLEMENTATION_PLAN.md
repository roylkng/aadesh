# Aadesh Implementation Plan (Next Phase)

Status: sequencing document only.
Authority: implementation order and acceptance gates for the active Aadesh scope.
Framing note: this plan defines the next implementation phase only, not Aadesh's final product identity. The deferred long-horizon direction still includes a broader capability substrate for personalized agentic systems (memory, trace, policy-state, capability/runtime boundaries, and later adaptive workflow surfaces). This phase intentionally focuses on continuity, supervisory observability, evaluation persistence, and advisory learning because they are the highest-confidence substrate work now.

## 1) Active direction summary

Aadesh in this phase is the current operating cut: continuity-first cognitive substrate with supervisory observability.

Active build target:
- continuity memory and context preparation quality
- intervention suggestion/outcome observability
- evaluation result persistence for learning and later external analysis

Deferred:
- governed-OS expansion
- controller/veto/orchestration behavior
- workflow/runtime expansion

See `docs/ARCHITECTURE_STATUS.md` for definitive active vs deferred classification.

Why this phase exists:
This phase delivers concrete user/system value now: more reliable continuity traces, durable evaluation evidence, better advisory ranking from real outcomes, and a cleaner substrate for later policy evolution without prematurely expanding into controller-style behavior.

## 2) Baseline to preserve

Do not regress:
- `store_work_episode`, `prepare_task_context`, `recall_relevant_memory`
- current retrieval/ranking behavior and compact output contract
- host wrapper ergonomics and connector event ingestion
- backward-compatible tool output shape
- additive migrations only

## 3) Phase sequencing

## Phase A: Documentation truth lock

Objective:
- eliminate doc/spec ambiguity so deferred architecture stops driving active milestones.

Deliverables:
- active/deferred/archived truth source maintained
- root entry docs and maps aligned to active scope
- explicit out-of-scope list published

Touched files/modules:
- `README.md`
- `index.md`
- `docs/ARCHITECTURE_STATUS.md`
- `docs/DOCS_MAP.md`
- `docs/CODEBASE_MAP.md`
- `docs/README.md`

Schema/storage impact:
- none

Migration strategy:
- none

Test plan:
- docs consistency review
- run `check_spec_drift.sh` for contract hygiene

Entry criteria:
- repo contains conflicting scope language

Exit criteria:
- one unambiguous status source exists and is referenced by entry docs

Non-goals:
- code behavior changes
- new API/tools

## Phase B: Supervisory trace hardening

Objective:
- make intervention traces reliable enough for learning and evaluation.

Deliverables:
- strict `accepted|ignored|modified` storage semantics
- durable linkage from surfaced direction to later outcome/correction evidence
- deterministic idempotency for outcome events
- internal correlation read path for linked outcomes

Locked storage direction:
- add a dedicated append-only `intervention_outcomes` table
- do not use nullable-column-only expansion on existing memory/claim tables for Phase B outcome truth
- claims/memory tables remain unchanged in this phase

Locked idempotency rule:
- no timestamp-window dedup heuristics
- use deterministic idempotency via one of:
  - host-provided `host_event_id`/`trace_event_id`, or
  - deterministic event hash over stable payload fields
- duplicate detection and replay handling must rely on deterministic keys only

Trace learnability invariant:
- weakly linked or unlinked outcome traces are persisted for observability
- `learnability = false` unless trace is fully linked or later reconciled with sufficient confidence
- unlearnable traces are excluded from downstream ranking-learning inputs and evaluation joins

Out-of-order rule (Phase B):
- persist unresolved outcome as append-only unlinked record
- allow later explicit reconciliation path if needed
- do not add async queue/background reconciler machinery in this phase

Correlation read path (internal only):
- add explicit internal storage-port methods for linked intervention-outcome reads
- implement query logic inside sqlite storage layer, not ad-hoc SQL in call sites
- no public tool/API surface expansion

Touched files/modules:
- `crates/adesh-daemon/src/connector_adapter.rs`
- `crates/adesh-daemon/src/cognition.rs`
- `crates/adesh-core/src/ports/storage.rs`
- `crates/adesh-storage-sqlite/src/storage.rs`
- `crates/adesh-storage-sqlite/migrations/*`
- `crates/adesh-daemon/tests/*` (trace-focused)

Schema/storage impact:
- additive migration introducing append-only `intervention_outcomes`
- additive indexes/constraints for deterministic idempotency and linkage queries

Migration strategy:
- additive migration only
- default-null for optional linkage fields
- backfill not required for existing rows

Test plan:
- unit:
  - outcome label validation (`accepted|ignored|modified`)
  - deterministic idempotency key generation/validation
  - learnability eligibility classification
- integration:
  - suggested direction -> selected outcome -> linked correlation query
  - degraded/unlinked writes persist but remain `learnability=false`
  - deterministic duplicate replay handling
- regression:
  - old host payloads still accepted
  - existing host wrapper flows and continuity tests unchanged

Entry criteria:
- phase A complete
- baseline continuity tests passing

Exit criteria:
- append-only `intervention_outcomes` table in place and populated by host traces
- deterministic idempotent write behavior verified
- linked correlation query path available through internal storage interface
- weak/unlinked traces persisted but excluded from learning/eval joins

Non-goals:
- ranking redesign beyond trace correctness needs
- policy enforcement
- queue/reconciler subsystem
- new public APIs/tools

## Phase C: Evaluation result persistence

Objective:
- persist benchmark/evaluation outcomes as first-class substrate data.

Deliverables:
- structured eval run records (metadata, baseline/treatment summary, judge summary, failure tags, promotion decision)
- artifact references for raw transcripts and bulky outputs
- minimal retrieval path for local analysis scripts

Touched files/modules:
- `crates/adesh-storage-sqlite/migrations/*`
- `crates/adesh-core/src/ports/storage.rs`
- `crates/adesh-storage-sqlite/src/storage.rs`
- `scripts/cognitive_eval_harness.sh` (write/read integration)

Schema/storage impact:
- additive eval tables and artifact ref linkage

Migration strategy:
- additive migration
- no schema rewrite of continuity tables

Test plan:
- integration test: persist and fetch eval runs
- contract tests for required fields and artifact refs

Entry criteria:
- phase B trace model stable

Exit criteria:
- eval runs are durably stored and queryable without parsing ad-hoc logs

Non-goals:
- Design Lab logic in this repo
- new ranking policy beyond read-only use

## Phase D: Advisory learning from intervention outcomes

Objective:
- improve ranking using observed intervention outcomes, without control behavior.

Deliverables:
- ranking feature inputs from prior accepted/ignored/modified traces
- bounded weighting using evidence-backed outcome history
- explainable `basis`/`evidence_refs` preserved

Touched files/modules:
- `crates/adesh-daemon/src/cognition.rs`
- `crates/adesh-daemon/tests/cognitive_proof.rs`
- trace/eval read surfaces in storage interfaces if needed

Schema/storage impact:
- no required schema expansion beyond phases B/C unless gaps found

Migration strategy:
- none expected; additive only if required

Test plan:
- discriminating tests where validated intervention outcomes should outrank stale generic debt
- regression against existing seeded examples

Entry criteria:
- phases B and C complete and stable

Exit criteria:
- measurable improvement in next-direction relevance without false-memory increase

Non-goals:
- autonomous policy gating
- orchestration/controller behavior

## Phase E: Gated policy-state substrate (only if needed)

Objective:
- add minimal explicit policy-state storage only when earlier phases show a concrete representational gap.

Entry criteria (all required):
- phases B-D are complete and stable
- and at least one of the following trigger signals appears repeatedly in production-like runs:
  - traces/eval cannot represent policy evolution cleanly
  - ranking/explanations require stable policy lineage objects that current memory classes cannot express cleanly
  - rollback/supersession of advisory policy revisions becomes common and hard to query via traces/claims
  - policy comparison across revisions/scopes becomes a repeated operational need

Deliverables (if triggered):
- minimal policy-state lifecycle schema
- active vs candidate policy pointers
- mutation rationale/evidence and rollback references
- read-only advisory integration (no gating)

Touched files/modules:
- `crates/adesh-storage-sqlite/migrations/*`
- `crates/adesh-core/src/ports/storage.rs`
- `crates/adesh-storage-sqlite/src/storage.rs`
- `crates/adesh-daemon/src/cognition.rs` (read-only influence first)

Schema/storage impact:
- default direction: dedicated policy-state tables
- claims/evidence remains linked provenance, not primary policy-state container

Migration strategy:
- additive, backward-compatible

Test plan:
- policy-state persistence and lifecycle transition tests
- policy lineage query tests
- read-only advisory influence tests

Exit criteria:
- policy-state can be persisted, versioned, compared, and rolled back at metadata level without introducing controller behavior

Non-goals:
- policy enforcement engine
- veto or approval routing
- orchestration/controller expansion

## 4) Global constraints

- no codepath rewrites of working continuity core without defect evidence
- no large new public tool surface
- no vector retrieval rollout
- no remote sync/distributed infra
- no workflow runtime expansion
- Design Lab remains separate

## 5) Core validation commands

```bash
cargo test --workspace
./.codex/skills/adesh-spec-guard/scripts/check_spec_drift.sh .
./scripts/connector_event_smoke.sh
./scripts/supervisory_trace_simulation.sh --sessions 20
./scripts/supervisory_trace_complex_simulation.sh
./scripts/cognitive_eval_harness.sh
```

## 6) Current completion checkpoint

Current repo state (as of this plan revision):
- Phase A complete (docs truth lock in place)
- Phase B complete (intervention traces hardened and durable)
- Phase C complete (eval runs/artifacts persisted)
- Phase D complete (outcome-informed advisory ranking active)
- Phase E not started by default (still gated)

This checkpoint is implementation status only. It does not change phase sequencing or scope.

## 7) Post-Phase-D operational gate (before considering Phase E)

Run an observation period with real host usage and keep scope fixed.

Minimum observation window:
- at least 2 weeks of production-like local usage
- at least 50 linked intervention outcomes (`learnability=true`)
- at least 20 completed sessions across at least 2 distinct workspaces

Required stability/quality checks:
- deterministic standard simulation passes with linked learnable outcomes across two workspaces
- complex simulation passes with sparse payloads, duplicate replay, and controlled unlearnable stale-context handling
- no regression in compact output contract behavior
- no false-memory regression versus current harness threshold (`<10%`)
- advisory usefulness remains above current acceptance baseline (`>=50%`, target stays `>=75%`)

Only if repeated evidence shows representational gaps should Phase E be opened.

Concrete Phase E trigger thresholds (all are operational signals, not one-off incidents):
- policy lineage gap: at least 3 independent cases where traces/eval cannot express policy evolution without manual reconstruction
- rollback pressure: at least 5 advisory-policy supersession/rollback events in a 14-day window that are hard to query from current trace model
- explanation pressure: at least 3 cases where ranking explanations require stable policy version comparison not representable with current memory classes
- repeated policy comparison need: at least 10 explicit policy-comparison queries in a 14-day window requiring ad-hoc joins/manual interpretation

If these thresholds are not met, keep Phase E deferred.
