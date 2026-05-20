# Competitor Testing Notes

Status: active benchmark research note.
Authority: explains how Aadesh comparison tests should borrow from, but not copy, adjacent systems.

## Why This Exists

Aadesh should not compete as a generic memory server or a Hermes-style agent runtime. The benchmark should therefore separate:
- memory recall quality
- next-direction quality
- setup friction
- cross-host portability
- outcome-trace learning

The hard benchmark now uses a production data profile so synthetic traces look closer to real host usage: noisy review feedback, blocked local prerequisites, flaky CI evidence, stale or unrelated workspaces, and accepted/ignored/modified outcomes.

## OpenMemory / Mem0 Benchmark Pattern

Reference:
- https://github.com/mem0ai/memory-benchmarks

Observed pattern:
- benchmarks focus on memory-augmented LLM systems
- datasets include LOCOMO, LongMemEval, and BEAM
- pipeline is `Ingest -> Search -> Evaluate`
- results inspect retrieval details and per-question evaluations
- self-hosted OSS mode uses Docker plus Qdrant

What Aadesh should borrow:
- keep benchmark stages explicit
- report per-case outputs, not only aggregate scores
- include temporal and update-heavy cases
- preserve raw artifacts outside the hot path

What Aadesh should not copy directly:
- do not collapse supervision into retrieval accuracy
- do not treat memory recall as sufficient proof
- do not ignore accepted/ignored/modified outcome traces

## Hermes Benchmark Pattern

References:
- https://github.com/NousResearch/hermes-agent
- https://raw.githubusercontent.com/NousResearch/hermes-agent/main/scripts/run_tests.sh
- https://raw.githubusercontent.com/NousResearch/hermes-agent/main/AGENTS.md

Observed pattern:
- Hermes is an agent runtime with CLI, gateway, toolsets, session state, skills, memory plugins, and multiple terminal backends
- the canonical test runner wraps pytest instead of encouraging ad hoc test commands
- tests are run with deterministic environment settings such as UTC locale/hash seed behavior
- credential-shaped environment variables are unset during test runs
- integration and e2e tests are separated from the default fast test path

What Aadesh should borrow:
- provide one canonical benchmark entrypoint instead of scattered manual commands
- keep runs deterministic where possible
- record blocked prerequisites as blocked/not-run, not as competitor failures
- isolate comparator home directories and databases

What Aadesh should not copy directly:
- do not become a full agent runtime
- do not add controller or orchestration behavior just to match Hermes
- treat Hermes as a host/runtime comparator and possible future integration target

## Production Data Profile Requirements

The production profile must test that Aadesh can handle:
- flaky CI or incident evidence outranking cleanup
- PR review corrections becoming concrete next directions
- blocked external comparators staying observational rather than false failures
- unrelated workspace noise not leaking into the current task
- non-repo continuity still working
- linked outcomes influencing future advice only when learnable

This keeps the benchmark aligned with the product wedge: Aadesh is valuable only if it turns cross-host continuity and outcome traces into better current-task guidance.
