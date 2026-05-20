# Architecture Status (Truth Source)

Status: active classification document.
Authority: this file defines which spec groups are active targets vs deferred vs historical.

Framing note: this status applies to the next implementation phase only, not Aadesh's final product identity. The deferred long-horizon direction still includes a broader capability substrate for personalized agentic systems (memory, trace, policy-state, capability/runtime boundaries, and later adaptive workflow surfaces). This phase intentionally focuses on continuity, supervisory observability, evaluation persistence, and advisory learning as the highest-confidence substrate work now.

## 1) Product Truth

Aadesh is a supervisory continuity, memory, intervention, and policy-state substrate for agents operating in bounded environments.

Active direction in this repo:
- cross-session continuity and personalization for agent work
- scoped memory with conservative promotion and evidence links
- ranked context preparation and next-direction guidance
- host-friendly wrappers and connector adapters
- supervisory trace capture (adoption/outcome/correction signals)

Not the current implementation target:
- full governed execution OS buildout
- full governance kernel/JIT/approval/OOB expansion
- full workflow/interface runtime expansion
- full audience graph/sanitization expansion
- broad new public tool surface

## 2) Classification Model

- Active: governs current implementation and test priorities.
- Deferred: valid long-horizon architecture; keep as reference, not active milestone driver.
- Archived/Historical: retained record only; not implementation input.

## 3) Active Docs / Specs

Primary docs:
- `README.md`
- `index.md`
- `docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md`
- `docs/IMPLEMENTATION_PLAN.md`
- `docs/WEDGE_V0_RUNBOOK.md`
- `docs/CONNECTOR_INTEGRATION_V0.md`
- `docs/SUPERVISORY_LAYER_V1.md`
- `docs/DOCS_MAP.md`
- `docs/CODEBASE_MAP.md`
- `docs/MEMORY_CONTRACT_VNEXT.md`
- `docs/POLICY_STATE_DECISION_NOTE.md`
- `docs/EVALUATION_PERSISTENCE_DESIGN.md`
- `docs/COMPARISON_BENCHMARK.md`
- `docs/DESIGN_LAB_BOUNDARY.md`
- `docs/specs/README.md`

Active substrate specs:
- `docs/specs/active/fact_ledger_and_reflection_claims.md`
- `docs/specs/active/storage_semantics_txn.md`
- `docs/specs/active/storage_provider_port_contract.md`
- `docs/specs/active/storage_schema.md`
- `docs/specs/active/artifact_normalization_contract.md`
- `docs/specs/active/ingestion_pipeline_spec.md`
- `docs/specs/active/model_output_contract.md`
- `docs/specs/active/mcp_host_surface_contract_spec.md`

## 4) Deferred Docs / Specs

Deferred architecture specs live under `docs/specs/deferred/`.

Major deferred groups:
- governed execution kernel and verification expansion
- approval/OOB/control-plane expansion
- policy graph/sanitization/disclosure expansion
- runtime composition expansion
- broad schema/capability/provider platform expansion
- legacy API batches and OS-era operational specs

Rule:
- deferred docs remain valuable references, but they are not acceptance gates for current continuity/supervisory-substrate phases.

See `docs/specs/README.md` for the full file inventory.

## 5) Archived / Historical Docs

- `archive/*`
- `docs/WEDGE_V0_EMAIL_DRAFT_AND_SEND.md`

These are retained for context only.

## 6) Scope Freeze For Current Phase

Current phase focus:
- continuity core quality and retrieval/ranking quality
- supervisory traces and evaluation persistence primitives
- real host usage validation
- policy-state only if trace/eval evidence shows a concrete gap

Out of scope now:
- control-plane-led OS feature expansion
- new runtime governance planes
- protocol-heavy redesign

## 7) Change Control Rule

Any change that re-expands scope into deferred groups must:
1. update this file,
2. update `docs/IMPLEMENTATION_PLAN.md`,
3. update `docs/specs/README.md`, and
4. include explicit rationale for why active milestones are not being diluted.

## 8) OutcomeProfile And Advisory Learning Semantics

The `OutcomeProfile` is the central data structure for advisory learning from intervention outcomes.

### Data Structure

```rust
struct OutcomeProfile {
    accepted_count: usize,
    ignored_count: usize,
    modified_count: usize,
    accepted_tokens: HashSet<String>,
    ignored_tokens: HashSet<String>,
    modified_tokens: HashSet<String>,
}
```

### Collection Scope

- Workspace-scoped: `collect_workspace_outcome_profile()` fetches intervention contexts for current workspace scope.
- Learn-from-this only: only outcomes with `learn_from_this = true` are collected.
- Token extraction: from `surfaced_direction` + `correction_summary` text.

### Boost Calculation (`outcome_boost_bonus_for_claim`)

1. Token overlap: compute overlap between claim statement tokens and outcome tokens.
2. Accepted boost:
   - +6 per accepted token overlap (capped at overlap*2)
   - +4 for accepted count bonus (capped at count/2)
   - +8 if acceptance rate >70% and 2+ accepted
   - +3 if acceptance rate >50% and 1+ accepted
3. Penalty: -2 if ignored overlap > accepted overlap.
4. Modified bonus: +2 per modified token overlap (capped at modified_overlap).

### Workspace Boundary

Outcome profiles are scoped per workspace to prevent cross-contamination:
- intervention contexts are found by `scope_type` + `scope_key`
- only outcomes linked to those contexts are included
- ranking applies boost only for claims matching workspace outcome tokens

### Non-Goals

- no autonomous policy gating
- no veto or approval routing
- ranking influence is advisory only
- system remains fail-closed for safety-critical paths
