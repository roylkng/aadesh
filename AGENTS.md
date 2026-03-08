# Adesh OS Repository Instructions

## Scope

These instructions apply to the entire repository rooted here.

## Source of Truth

- Root-level `.md` files are canonical specifications unless explicitly marked otherwise.
- `docs/IMPLEMENTATION_PLAN.md` is the implementation sequencing document, not a behavior override.
- `reference/` is non-authoritative support material.
- `archive/` is legacy material and must not be used to define behavior.

## Spec Precedence

When documents overlap, use this order:

1. `index.md`
2. `storage_semantics_txn.md`
3. `governance_kernel_logic.md`
4. `verification_core_ruleset.md`
5. `control_plane_api_spec.md` and `mcp_host_surface_contract_spec.md`
6. Port contracts
7. `docs/IMPLEMENTATION_PLAN.md`
8. `reference/`
9. `archive/`

## Non-Negotiable Invariants

- `max_gate = max(R, S)`
- Audit never fails open
- No side effects without persisted syscall pre-image
- Persist before emit
- Default deny audience graph
- Explicit IPC only
- No taint laundering without explicit sanitization and verification
- OOB is approval-bound and single-use
- In-flight operations use pinned versions only

## Implementation Rules

- Do not invent unspecified behavior.
- If behavior is ambiguous, patch the relevant spec before changing code.
- Do not weaken fail-closed or audit-critical paths for convenience.
- Do not implement beyond the active milestone scope unless the spec or plan is updated first.
- Keep provider interfaces backend-agnostic even when implementing the SQLite reference backend.

## Milestone Discipline

- Start from Milestone 1 in `docs/IMPLEMENTATION_PLAN.md`.
- Keep changes vertical, compileable, and testable.
- Do not merge partial unsafe behavior behind TODO comments on critical paths.

## Repo Hygiene

- Keep canonical specs in the repo root.
- Put summaries, sketches, and migration notes in `reference/`.
- Put superseded material in `archive/`.
- Maintain filename consistency with `index.md`.

## Validation Before Behavior Changes

Before implementing behavior-changing code:

- confirm the relevant canonical spec exists and is current
- confirm filenames and endpoints are current
- confirm pinned-version fields are consistent across docs and contracts
- update tests or add placeholders only when the milestone explicitly includes them
