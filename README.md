# Aadesh

Aadesh is a local-first supervisory continuity substrate for agents.

Framing note: this repository's current plan documents the next implementation phase only, not Aadesh's final product identity. The deferred long-horizon direction still includes a broader capability substrate for personalized agentic systems (memory, trace, policy-state, capability/runtime boundaries, and later adaptive workflow surfaces). The active implementation focus for this phase is continuity, supervisory observability, evaluation persistence, and advisory learning as the highest-confidence substrate work now.

It sits between raw model intelligence and host environments to provide:
- scoped memory across episodes
- context preparation with ranked evidence
- continuity and personalization
- supervisory traces about suggestions, acceptance, outcomes, and corrections
- policy-state primitives for later intervention learning

## Current Product Direction

Active target in this repo:
- cognitive continuity + supervisory substrate for bounded agent workflows
- callable through thin host-facing surfaces (CLI first, MCP on the same core)
- quality measured by real host usage and benchmark evidence

Not the current target:
- full governed execution OS expansion
- broad governance kernel/JIT/control-plane rollout
- workflow/interface runtime expansion
- audience graph and sanitization system expansion

See the authoritative status split in [`docs/ARCHITECTURE_STATUS.md`](./docs/ARCHITECTURE_STATUS.md).

## Repo Status Model

- Active: current implementation-driving docs/specs
- Deferred: long-horizon architecture references, not active milestone gates
- Archived: historical records only

Status classification lives in [`docs/ARCHITECTURE_STATUS.md`](./docs/ARCHITECTURE_STATUS.md).

## Start Here

1. [`index.md`](./index.md)
2. [`docs/ARCHITECTURE_STATUS.md`](./docs/ARCHITECTURE_STATUS.md)
3. [`docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md`](./docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md)
4. [`docs/IMPLEMENTATION_PLAN.md`](./docs/IMPLEMENTATION_PLAN.md)
5. [`docs/DOCS_MAP.md`](./docs/DOCS_MAP.md)
6. [`docs/CODEBASE_MAP.md`](./docs/CODEBASE_MAP.md)
7. [`docs/COMPARISON_BENCHMARK.md`](./docs/COMPARISON_BENCHMARK.md)
8. [`docs/specs/README.md`](./docs/specs/README.md)

## Repo Organization

Root markdown is intentionally minimal:
- `README.md`
- `index.md`
- `AGENTS.md`

Specs are organized under:
- [`docs/specs/active/`](./docs/specs/active/): current implementation inputs
- [`docs/specs/deferred/`](./docs/specs/deferred/): long-horizon references

This keeps the repository entry clean while preserving the older architecture material.

## Active Proof Slice

The continuity wedge remains the shipped core slice:
- `store_work_episode`
- `prepare_task_context`
- `recall_relevant_memory`

This core is then exercised through host wrappers and connector events with optional supervisory trace fields.

## Authority

- Current scope/status: [`docs/ARCHITECTURE_STATUS.md`](./docs/ARCHITECTURE_STATUS.md)
- Sequencing: [`docs/IMPLEMENTATION_PLAN.md`](./docs/IMPLEMENTATION_PLAN.md)
- Comparison benchmark: [`docs/COMPARISON_BENCHMARK.md`](./docs/COMPARISON_BENCHMARK.md)
- Spec inventory: [`docs/specs/README.md`](./docs/specs/README.md)
- Process contract: [`docs/REPO_ORGANIZATION.md`](./docs/REPO_ORGANIZATION.md)
- `reference/` is non-authoritative support material.
- `archive/` is historical only.

## Local Commands

Before-task context:

```bash
cargo run -p adesh-daemon -- host prepare \
  --task "What should I focus on next?" \
  --task-hint active-session
```

After-task writeback:

```bash
cargo run -p adesh-daemon -- host store \
  --task "What I just worked on" \
  --summary "Summary of decisions, outcomes, and open loops" \
  --task-hint active-session
```

Connector event path:

```bash
cargo run -p adesh-daemon -- host connector --json '<connector_event_payload>'
```

## Legacy Breadth Note

The repo still contains broad architecture specs from an earlier governed-OS framing. They are retained under `docs/specs/deferred/` because parts may be reusable later, but they are not active build targets in the current phase.
