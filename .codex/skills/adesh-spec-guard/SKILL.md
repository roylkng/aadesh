---
name: adesh-spec-guard
description: Use this skill when working in the Adesh OS repository to validate spec drift, enforce canonical file usage, check stale filenames/endpoints/pinned-version fields, and confirm implementation work still matches the current milestone and root-level specs.
---

# Adesh Spec Guard

Use this skill for any docs or code change in Adesh OS that could drift from the canonical spec set.

## What this skill does

- checks filename consistency against the canonical renamed files
- checks approval endpoints stay `approval_id`-scoped
- checks stale pinned-version fields such as `pinned_state_version`
- checks prompt-artifact spillover and pasted markdown wrapper noise
- reminds the agent to prefer root-level specs over `reference/` and `archive/`

## When to use it

Use this skill when:

- editing any root-level spec
- adding or changing Rust code
- updating routes, approvals, replay, storage, or versioning logic
- preparing a PR and wanting a fast drift pass

## Workflow

1. Read `index.md` and `docs/IMPLEMENTATION_PLAN.md` if the task touches behavior.
2. Use `scripts/check_spec_drift.sh` from this skill.
3. If the script finds drift:
   - fix canonical specs first
   - then fix supporting docs
   - only then change code
4. If the task touches milestone scope, compare the change against `docs/IMPLEMENTATION_PLAN.md`.
5. If ambiguity remains, patch the canonical spec before coding.

## References

- For the exact checks and interpretation, read `references/checklist.md`.

## Notes

- Root-level specs are canonical.
- `reference/` is non-authoritative.
- `archive/` is legacy only.
