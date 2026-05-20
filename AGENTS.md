# Aadesh Repository Instructions

## Scope

These instructions apply to the entire repository rooted here.

## Current Product Direction

Aadesh is currently being built as a continuity-first supervisory substrate for agents in bounded environments.

Active implementation focus:
- cross-session cognitive continuity
- scoped memory and ranked context preparation
- host wrappers / connector events / MCP surface on the same core
- supervisory traces for suggested directions, acceptance, modification, ignored outcomes, and later evidence
- evaluation persistence and advisory learning

Deferred unless explicitly reopened:
- full governed execution OS expansion
- broad governance kernel/JIT/control-plane rollout
- workflow/interface runtime expansion
- audience graph and sanitization expansion
- controller/veto/orchestration behavior

## Source Of Truth

Use this order for current work:

1. `docs/ARCHITECTURE_STATUS.md`
2. `docs/IMPLEMENTATION_PLAN.md`
3. `docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md`
4. `docs/WEDGE_V0_RUNBOOK.md`
5. `docs/specs/README.md`
6. active specs under `docs/specs/active/`
7. `docs/DOCS_MAP.md` and `docs/CODEBASE_MAP.md`
8. deferred specs under `docs/specs/deferred/` only when intentionally working on deferred scope
9. `reference/`
10. `archive/`

## Active Spec Locations

Active implementation specs live under `docs/specs/active/`, not the repository root.

Important active specs:
- `docs/specs/active/storage_semantics_txn.md`
- `docs/specs/active/storage_provider_port_contract.md`
- `docs/specs/active/storage_schema.md`
- `docs/specs/active/fact_ledger_and_reflection_claims.md`
- `docs/specs/active/artifact_normalization_contract.md`
- `docs/specs/active/ingestion_pipeline_spec.md`
- `docs/specs/active/model_output_contract.md`
- `docs/specs/active/mcp_host_surface_contract_spec.md`

Deferred architecture specs live under `docs/specs/deferred/` and are not current acceptance gates.

## Non-Negotiable Current Invariants

- Do not re-expand scope into deferred architecture without updating status and plan docs.
- Intervention outcomes must remain deterministic and idempotent.
- Unlinked or weakly linked outcomes are observable but not learnable.
- Advisory learning remains advisory; no controller/veto/orchestration behavior in the current phase.
- Persist before using traces for learning.
- Keep public tool/API surface stable unless the plan explicitly changes it.

Legacy fail-closed governance invariants remain relevant for old control-plane paths, but they do not define the active product wedge.

## Implementation Rules

- Do not invent unspecified behavior.
- If behavior is ambiguous, patch the relevant active doc/spec before changing code.
- Keep working continuity/cognition paths intact unless there is concrete defect evidence.
- Keep changes vertical, compileable, and testable.
- Do not merge partial unsafe behavior behind TODO comments on critical paths.

## Repo Hygiene

- Root markdown is entry-only: `README.md`, `index.md`, and `AGENTS.md`.
- Put active specs in `docs/specs/active/`.
- Put deferred specs in `docs/specs/deferred/`.
- Put summaries, sketches, and migration notes in `reference/` or `docs/` depending on authority.
- Put superseded material in `archive/`.
- Keep `docs/specs/README.md`, `docs/DOCS_MAP.md`, and `docs/ARCHITECTURE_STATUS.md` aligned after moves.

## Validation Before Behavior Changes

Run:

```bash
./.codex/skills/adesh-spec-guard/scripts/check_spec_drift.sh .
CARGO_TARGET_DIR=/tmp/adesh-cargo-target cargo test --workspace
```

For connector/supervisory work, also run:

```bash
./scripts/connector_event_smoke.sh
./scripts/supervisory_trace_simulation.sh --sessions 20
./scripts/supervisory_trace_complex_simulation.sh
./scripts/cognitive_eval_harness.sh
```
