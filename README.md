# Aadesh

Aadesh is a local-first cognitive runtime that sits between raw model intelligence and host applications. Its job is to supply continuity, scoped memory, user and workspace preferences, prior decisions, unresolved loops, and evidence-grounded guidance so a host agent can act with context instead of starting cold every session.

This repository contains:
- canonical behavior and architecture specs in root-level `.md` files
- implementation sequencing and navigation docs in [`docs/`](./docs)
- non-authoritative support material in [`reference/`](./reference)
- legacy material in [`archive/`](./archive)

## Current direction

The active v1 proof is not a standalone shell and not the legacy email wedge.

The active proof is:
- cross-session cognitive continuity and personalization for coding agents
- callable via lightweight front doors first
- grounded in durable observations, episodes, memory promotion, retrieval, and compact reasoning

The broader governed-execution and control-plane specs remain in the repo because they are still part of the long-horizon architecture, but they are not the active product center for the first proof.

## Status

- The repo already contains useful substrate: local storage, queueing, artifacts, claims/evidence, model/provider boundaries, and a legacy governed-execution slice.
- The repository is now being realigned around the cognitive-sidecar proof.
- The active implementation target is a small callable surface for storing work episodes and returning compact current-task guidance from cross-session memory.

## Start here

1. Read [`index.md`](./index.md).
2. Read [`docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md`](./docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md).
3. Read [`docs/IMPLEMENTATION_PLAN.md`](./docs/IMPLEMENTATION_PLAN.md).
4. Use [`docs/DOCS_MAP.md`](./docs/DOCS_MAP.md) and [`docs/CODEBASE_MAP.md`](./docs/CODEBASE_MAP.md) for traversal.

## What is canonical

- Root-level `.md` files are canonical specifications unless explicitly marked otherwise.
- [`docs/IMPLEMENTATION_PLAN.md`](./docs/IMPLEMENTATION_PLAN.md) is the sequencing document, not a behavior override.
- [`reference/`](./reference) is non-authoritative.
- [`archive/`](./archive) is legacy only.

## Core invariants that still matter

These remain non-negotiable across the broader architecture, even though the first wedge does not exercise all of them:

- `max_gate = max(R, S)`
- audit never fails open
- no side effects without persisted syscall pre-image
- persist before emit
- default deny audience graph
- explicit IPC only
- no taint laundering without explicit sanitization and verification
- OOB is approval-bound and single-use
- in-flight operations use pinned versions only

## Active wedge

The active proof is documented in [`docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md`](./docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md).

That proof is deliberately narrow:
- store work episodes
- accumulate scoped memory across episodes
- retrieve and rank what matters for a new task
- return compact, evidence-grounded guidance to a host coding agent

Initial callable tools:
- `store_work_episode`
- `prepare_task_context`
- `recall_relevant_memory`

Initial transport stance:
- CLI first
- MCP after CLI is stable
- shared internal core for both

### Host-friendly CLI wrapper

The thin host-facing wrapper keeps the cognition core unchanged while reducing payload friction for real coding-agent calls.

Before-task context:

```bash
cargo run -p adesh-daemon -- host prepare \
  --task "Can you help finish the upload retry work safely?" \
  --file src/upload/upload_worker.rs \
  --task-hint upload-retry
```

After-task writeback:

```bash
cargo run -p adesh-daemon -- host store \
  --task "Refactor retry fix so duplicate guard stays in service layer" \
  --summary "Moved dedupe check into UploadService and kept retry logic explicit. Timeout-path coverage is still missing." \
  --file src/upload/upload_worker.rs \
  --file src/upload/upload_service.rs \
  --decision "Use explicit retry state handling rather than macro abstraction in this subsystem::Failure paths are easier to audit in explicit code" \
  --test "fail::upload_worker_timeout_path::Timeout path still fails in the retry worker" \
  --task-hint upload-retry
```

The wrapper auto-detects git workspaces from the current directory when possible, but the underlying workspace model remains generic.

### Gemini CLI wrapper

Gemini is the first reference host integration. The wrapper stays thin:
- `host gemini prompt` formats `prepare_task_context` output into a compact Gemini-ready prompt
- `host gemini run` formats that prompt and executes `gemini --prompt ...`
- `host gemini store` records the follow-up work episode through the same `store_work_episode` path

Build an Aadesh component with Gemini before-task context:

```bash
cargo run -p adesh-daemon -- host gemini run \
  --task "Use Gemini CLI to build the wrapper component for Aadesh itself." \
  --file crates/adesh-daemon/src/host_cli.rs \
  --file README.md \
  --task-hint gemini-wrapper \
  -- --model gemini-2.5-pro
```

Persist what happened after the task:

```bash
cargo run -p adesh-daemon -- host gemini store \
  --task "Use Gemini CLI to build the wrapper component for Aadesh itself." \
  --summary "Added a thin Gemini wrapper on top of the host prepare/store flow and validated it with a fake Gemini binary." \
  --file crates/adesh-daemon/src/gemini_wrapper.rs \
  --file crates/adesh-daemon/tests/gemini_wrapper_flows.rs \
  --decision "Keep the cognition core unchanged and add a thin host-specific wrapper::Transport integration should not mutate the cognitive API" \
  --test "pass::gemini_wrapper_flows::Gemini wrapper passes prompt and passthrough args to the CLI" \
  --task-hint gemini-wrapper
```

Shell entrypoint for the same flow:

```bash
./scripts/gemini_with_aadesh.sh run \
  --task "Use Gemini CLI to build the wrapper component for Aadesh itself." \
  --file crates/adesh-daemon/src/host_cli.rs \
  --task-hint gemini-wrapper \
  -- --model gemini-2.5-pro
```

Qwen uses the same wrapper pattern:

```bash
cargo run -p adesh-daemon -- host qwen run \
  --task "Use Qwen CLI to review the wrapper component for Aadesh itself." \
  --file crates/adesh-daemon/src/host_cli.rs \
  --task-hint qwen-wrapper \
  -- --model qwen3-coder-plus
```

```bash
./scripts/qwen_with_aadesh.sh run \
  --task "Use Qwen CLI to review the wrapper component for Aadesh itself." \
  --file crates/adesh-daemon/src/host_cli.rs \
  --task-hint qwen-wrapper \
  -- --model qwen3-coder-plus
```

## What is being reused

Useful substrate already in the repo:
- SQLite-backed local storage and migrations
- append-oriented event and artifact persistence
- claims/evidence/conflict machinery
- job queue foundation
- model/provider boundaries
- typed contracts and response envelopes

These are being reused for the wedge instead of replaced.

## What is being deferred

Deferred from the first proof:
- the legacy email draft-and-send wedge
- approval/OOB-heavy execution as the primary product path
- workflow/interface runtime as a hot path
- UI-first product experience
- broad actuator and sandbox surfaces
- distributed sync and remote infra

Deferred documents remain in the repo, but they are not the current implementation driver.

## Repository layout

### Canonical specs
Root-level `.md` files define behavior and architecture.

Primary entry points:
- [`index.md`](./index.md)
- [`storage_semantics_txn.md`](./storage_semantics_txn.md)
- [`governance_kernel_logic.md`](./governance_kernel_logic.md)
- [`verification_core_ruleset.md`](./verification_core_ruleset.md)
- port contracts such as [`storage_provider_port_contract.md`](./storage_provider_port_contract.md)

### Implementation and navigation docs
- [`docs/README.md`](./docs/README.md)
- [`docs/IMPLEMENTATION_PLAN.md`](./docs/IMPLEMENTATION_PLAN.md)
- [`docs/DOCS_MAP.md`](./docs/DOCS_MAP.md)
- [`docs/CODEBASE_MAP.md`](./docs/CODEBASE_MAP.md)
- [`docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md`](./docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md)

### Non-authoritative support material
- [`reference/README.md`](./reference/README.md)

### Legacy material
- [`archive/README.md`](./archive/README.md)

## Validation and drift checks

Use the repo-local drift guard before behavior changes:

```bash
./.codex/skills/adesh-spec-guard/scripts/check_spec_drift.sh .
```

Useful grep-based checks that do not require `ripgrep`:

```bash
grep -RInE "/v1/approvals/\\{operation_id\\}|approvals/\\{operation_id\\}" .
grep -RIn --exclude=README.md --exclude-dir=.codex "pinned_state_version" .
grep -RIn "WEDGE_V0_EMAIL_DRAFT_AND_SEND" .
```

## Contribution rule

Specs before behavior:

- patch canonical specs first when behavior is missing or ambiguous
- keep the core architecture generic instead of baking the first wedge into the base model
- preserve reusable substrate unless there is a concrete reason to replace it
- do not broaden scope toward a full shell or OS until the cognitive-sidecar proof is real
