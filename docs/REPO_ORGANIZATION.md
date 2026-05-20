# Repository Organization Contract

Status: process contract for repository layout and contribution hygiene.
Scope: humans and coding agents working in this repository.

This document defines where content belongs and what checks must pass before behavior-changing work proceeds.

## 1) Top-Level Layout

Authoritative layout:

- root: entry documents and workspace manifests only
- `docs/`: implementation sequencing, scope/status, runbooks, design notes, and spec folders
- `docs/specs/active/`: canonical specs used by the current implementation cut
- `docs/specs/deferred/`: long-horizon architecture specs retained as references
- `crates/`: Rust implementation, tests, and migrations
- `registry/`: bootstrap schema and capability artifacts
- `reference/`: non-authoritative summaries and planning notes
- `archive/`: legacy material only
- `.codex/skills/`: local agent skills and guard scripts

Anything outside this structure must be justified in the PR description.

## 2) Canonicality Rules

- Current implementation authority starts with `docs/ARCHITECTURE_STATUS.md` and `docs/IMPLEMENTATION_PLAN.md`.
- Active behavior specs live in `docs/specs/active/`.
- Deferred specs live in `docs/specs/deferred/` and are not active acceptance gates.
- `reference/` and `archive/` must never override active specs or status docs.
- New implementation-driving specs must be:
  - placed under `docs/specs/active/`,
  - added to `docs/specs/README.md`,
  - classified in `docs/ARCHITECTURE_STATUS.md`, and
  - linked from `index.md` or `docs/DOCS_MAP.md` if they affect implementation flow.

## 3) Placement Rules

### 3.1 Root (`/`)

Place only:
- `README.md`
- `AGENTS.md`
- `index.md`
- workspace manifests and config files

Do not place:
- canonical specs
- implementation sketches
- migration notes
- temporary analyses
- generated files

### 3.2 `docs/`

Place:
- implementation plans
- architecture status and scope locks
- wedge/runbook documents
- process contracts
- navigation maps
- spec folders

Do not place loose canonical specs directly under `docs/`; use `docs/specs/active/` or `docs/specs/deferred/`.

### 3.3 `docs/specs/active/`

Place implementation-driving specs for the current operating cut.

### 3.4 `docs/specs/deferred/`

Place broader or older architecture specs that remain useful references but should not drive current milestones.

### 3.5 `reference/`

Place summaries, contract digests, and implementation notes. Each file should explicitly say it is non-authoritative.

### 3.6 `archive/`

Place superseded material kept for history.

### 3.7 `registry/`

Place bootstrap schema payloads and bootstrap capability snapshot payloads.

### 3.8 `crates/`

Place only Rust code, tests, and migration assets required by code.

## 4) Naming and Consistency Rules

- Use lowercase snake_case for spec filenames.
- Avoid duplicate concept files under alternate names.
- Keep endpoint and contract naming aligned with current canonical forms.
- Do not move a spec between active/deferred without updating scope docs.

## 5) Required Checks Before Merging Behavior Changes

Run:

```bash
./.codex/skills/adesh-spec-guard/scripts/check_spec_drift.sh .
CARGO_TARGET_DIR=/tmp/adesh-cargo-target cargo test --workspace
```

Required outcomes:

- no stale filenames/endpoints/field names
- no markdown wrapper artifacts in docs
- root markdown remains entry-only
- `docs/specs/README.md` matches active/deferred placement
- tests pass

## 6) Commit Hygiene

- Keep docs/contract repairs separate from behavior changes when possible.
- Keep generated artifacts out of Git.
- Do not mix archive/reference churn into implementation commits unless required.
- If a change touches behavior and specs, update specs first in the same PR or a preceding PR.

## 7) Fast Checklist

Before opening a PR:

1. Active specs are in `docs/specs/active/` and indexed.
2. Deferred specs are in `docs/specs/deferred/` and not described as current gates.
3. Root markdown contains only entry documents.
4. Guard script passes.
5. Tests pass.
6. No generated clutter is tracked.

## 8) Traversal Aids

Keep these navigation docs up to date when structure changes:

- `index.md`
- `docs/DOCS_MAP.md`
- `docs/CODEBASE_MAP.md`
- `docs/specs/README.md`
- `crates/README.md`
