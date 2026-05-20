# Policy-State Decision Note (vNext)

Status: decision note.

## Decision

Explicit policy-state is deferred by default for the next implementation phase.

If Phase E is triggered, the default implementation direction is **dedicated policy-state tables**, not claims/evidence-first modeling.

## Why policy-state is deferred now

Rationale:
- current priority is observability quality (trace linkage + evaluation persistence)
- many near-term supervision needs are covered by claims/evidence + intervention/evaluation memory
- adding policy-state too early risks premature abstraction without proven leverage

## What traces + eval already solve

With strong intervention/evaluation persistence, Aadesh can already answer:
- which suggestions were accepted/ignored/modified
- what outcomes followed
- which patterns improve quality
- where repeated failure clusters remain

## What explicit policy-state uniquely solves later

Policy-state becomes necessary when Aadesh needs durable lifecycle semantics for:
- stable active/candidate policy version pointers
- mutation lineage across revisions
- rollback/supersession history with explicit causality
- frequent policy comparison queries across scopes and time

## Default storage direction when Phase E is triggered

Use dedicated policy-state tables by default.

Justification:
- lifecycle semantics: policy objects have explicit state transitions (candidate -> active -> superseded/rolled back)
- query semantics: frequent policy lineage/comparison queries are clearer and cheaper on first-class policy tables
- versioning semantics: version pointers and revision chains are core fields, not incidental claim attributes
- rollback semantics: rollback reason and evidence should be directly addressable as policy lifecycle records

Claims/evidence remains important, but as linked evidence/rationale, not as the primary policy-state container.

## Minimal policy-state object

When Phase E is enabled, start with this minimal object:
- `policy_state_id`
- `scope_type`, `scope_key`
- `active_version`
- `candidate_version` (optional)
- `constraints_snapshot_ref`
- `mutation_rationale_ref`
- `promotion_evidence_refs[]`
- `rejection_or_rollback_reason_ref`
- timestamps

This remains storage/advisory only unless a future separate decision introduces controller behavior.
