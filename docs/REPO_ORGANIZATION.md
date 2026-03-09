# Repository Organization Contract

Status: Process contract for repository layout and contribution hygiene.
Scope: Humans and coding agents working in this repository.

This document defines where content belongs and what checks must pass before behavior-changing work proceeds.

## 1) Top-level layout

Authoritative layout:

- root-level `*.md`: canonical system specifications
- `docs/`: implementation sequencing and product-scope constraints
- `crates/`: Rust implementation
- `registry/`: bootstrap schema and capability artifacts
- `reference/`: non-authoritative summaries and planning notes
- `archive/`: legacy material only
- `.codex/skills/`: local agent skills and guard scripts

Anything outside this structure must be justified in PR description.

## 2) Canonicality rules

- Root-level specs are behavior source-of-truth unless explicitly marked otherwise.
- `docs/IMPLEMENTATION_PLAN.md` is sequencing guidance, not a behavior override.
- `reference/` and `archive/` must never be used to override canonical behavior.
- New canonical behavior specs must be:
  - created at repo root
  - added to `index.md`
  - referenced in `README.md` if they affect implementation flow

## 3) Placement rules

### 3.1 Root (`/`)
Place only:
- canonical specs
- repo policy files (`README.md`, `AGENTS.md`)
- workspace manifests (`Cargo.toml`, `Cargo.lock`)

Do not place:
- implementation sketches
- migration notes
- temporary analyses
- generated files

### 3.2 `docs/`
Place only:
- implementation plans
- wedge/scope lock documents
- process contracts (like this file)
- navigation maps for specs/code traversal

Do not place canonical behavior specs here.

### 3.3 `reference/`
Place:
- summaries
- contract digests
- implementation notes

Each file should explicitly say it is non-authoritative.

### 3.4 `archive/`
Place only superseded material kept for history.

### 3.5 `registry/`
Place:
- bootstrap schema payloads
- bootstrap capability snapshot payloads

All files should map to canonical schema registry / capability snapshot contracts.

### 3.6 `crates/`
Place only Rust code, tests, and migration assets required by code.

## 4) Naming and consistency rules

- Use lowercase snake_case for canonical spec filenames.
- Keep endpoint and contract naming aligned with current canonical forms:
  - `control_plane_api_spec.md`
  - approval routes keyed by `approval_id`
  - pinned versions include:
    - `active_state_version`
    - `capability_snapshot_version`
    - `audience_graph_version`
- Avoid duplicate concept files under alternate names.

## 5) Required checks before merging behavior changes

Run:

```bash
./.codex/skills/adesh-spec-guard/scripts/check_spec_drift.sh
cargo test --workspace
```

Required outcomes:

- no stale filenames/endpoints/field names
- no markdown wrapper artifacts in canonical docs
- root-level docs remain indexed and resolvable
- tests pass

## 6) Commit hygiene

- Keep docs/contract repairs separate from behavior changes when possible.
- Keep generated artifacts out of Git.
- Do not mix archive/reference churn into implementation commits unless required.
- If a change touches behavior and specs, update specs first in the same PR or a preceding PR.

## 7) Fast checklist

Before opening a PR:

1. Canonical behavior in root specs is updated and indexed.
2. New docs are in the correct folder.
3. Guard script passes.
4. Tests pass.
5. No generated clutter is tracked.

## 8) Traversal aids

Keep these navigation docs up to date when structure changes:

- `docs/DOCS_MAP.md`
- `docs/CODEBASE_MAP.md`
- `crates/README.md`
