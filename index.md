# Aadesh Repo Index and Reading Order

This is the root navigation entry.

Framing note: this index and linked plan define the next implementation phase only, not Aadesh's final product identity. The deferred long-horizon direction still includes a broader capability substrate for personalized agentic systems (memory, trace, policy-state, capability/runtime boundaries, and later adaptive workflow surfaces). The active implementation focus for this phase is continuity, supervisory observability, evaluation persistence, and advisory learning because they are the highest-confidence substrate work now.

Use this file with `docs/ARCHITECTURE_STATUS.md`, which is the source of truth for Active, Deferred, and Archived scope.

## 1) Correct Framing

Aadesh is currently being built in this phase as the active implementation focus: continuity-first cognitive substrate with supervisory observability for agents in bounded environments.

This repo is not currently executing a full governed-OS buildout.

## 2) Root Layout

Root markdown is intentionally entry-only:

- `README.md`: short product/repo entry.
- `index.md`: reading order and map.
- `AGENTS.md`: agent instructions.

Specs now live under `docs/specs/` so root stays readable.

## 3) Read Order For Active Work

1. `docs/ARCHITECTURE_STATUS.md`
2. `docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md`
3. `docs/IMPLEMENTATION_PLAN.md`
4. `docs/MEMORY_CONTRACT_VNEXT.md`
5. `docs/POLICY_STATE_DECISION_NOTE.md`
6. `docs/EVALUATION_PERSISTENCE_DESIGN.md`
7. `docs/COMPARISON_BENCHMARK.md`
8. `docs/DESIGN_LAB_BOUNDARY.md`
9. `docs/CONNECTOR_INTEGRATION_V0.md`
10. `docs/SUPERVISORY_LAYER_V1.md`
11. `docs/WEDGE_V0_RUNBOOK.md`
12. `docs/DOCS_MAP.md`
13. `docs/CODEBASE_MAP.md`
14. `docs/specs/README.md`

## 4) Active Substrate Specs To Use Now

These are canonical inputs for current implementation work:

- `docs/specs/active/storage_semantics_txn.md`
- `docs/specs/active/storage_provider_port_contract.md`
- `docs/specs/active/storage_schema.md`
- `docs/specs/active/fact_ledger_and_reflection_claims.md`
- `docs/specs/active/artifact_normalization_contract.md`
- `docs/specs/active/ingestion_pipeline_spec.md`
- `docs/specs/active/model_output_contract.md`
- `docs/specs/active/mcp_host_surface_contract_spec.md`

## 5) Deferred Architecture Sets

Deferred specs are under `docs/specs/deferred/`.

They remain useful references, but they are not active milestone gates. See file-level classification in `docs/ARCHITECTURE_STATUS.md` and `docs/specs/README.md`.

## 6) Historical Material

- `archive/`
- `docs/WEDGE_V0_EMAIL_DRAFT_AND_SEND.md`

Historical material is retained for context only.

## 7) Contribution Guardrail

Do not treat deferred specs as required implementation parity for the current phase.

If a PR expands scope into deferred architecture:
1. update `docs/ARCHITECTURE_STATUS.md`,
2. update `docs/IMPLEMENTATION_PLAN.md`,
3. update `docs/specs/README.md`, and
4. justify why active continuity/supervisory milestones are not being diluted.
