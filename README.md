# Adesh OS

Adesh OS is a governed execution kernel for Root Owner requests. It decomposes a request into isolated operations, pins the active state, capability snapshot, and audience graph versions per operation, computes governance gates from action risk and data sensitivity, compiles a taint-aware working slice, verifies structured model output before any side effects, and persists audit anchors so execution and replay are deterministic.

This repository is the **specification source of truth** and the **implementation planning baseline** for Adesh OS.

## Status

Pre-implementation / implementation kickoff.

- Root-level `*.md` files are canonical specs unless explicitly marked otherwise.
- `docs/IMPLEMENTATION_PLAN.md` is the current execution plan.
- `reference/` contains non-authoritative summaries and planning aids.
- `archive/` contains retained legacy material and is not source of truth.

## Core invariants

Non-negotiable:

- `max_gate = max(R, S)` governs operations and syscalls.
- Audit never fails open. Missing anchors or audit-critical persistence failures are hard failures.
- No side effect may execute without a persisted syscall pre-image (`SyscallEnvelope`).
- Persist-before-emit applies to operation state, approvals, denies, syscalls, and audit updates.
- Cross-operation data transfer is explicit only through `IPCArtifact` (no implicit piping).
- Taint laundering is prohibited without explicit sanitization and verification.
- OOB authorization is **approval-bound**, single-use, and never elevates a session globally.
- Audience Graph is default deny for unknown nodes, edges, scopes, or ceilings.
- HTTP/WS control plane is Root Owner only. External agents integrate via MCP Host only.
- In-flight operations use pinned versions only:
  - `active_state_version`
  - `capability_snapshot_version`
  - `audience_graph_version`

## Start here

1. Read `index.md` first.
2. Then read `docs/IMPLEMENTATION_PLAN.md` for sequencing and milestone gates.
3. Use canonical specs (root-level docs) for behavioral truth.
4. For fast traversal:
   - `docs/DOCS_MAP.md` (spec lookup by task/endpoint)
   - `docs/CODEBASE_MAP.md` (code entrypoints and edit surfaces)

## Minimum reading order for implementation work

1. `index.md`
2. `kernel_execution_loop.md`
3. `governance_kernel_logic.md`
4. `jit_compiler.md`
5. `verification_core_ruleset.md`
6. `storage_semantics_txn.md`
7. `storage_provider_port_contract.md`
8. `storage_schema.md`
9. `approval_oob_spec.md`
10. `operation_decomposition_ipc.md`
11. `scheduler_concurrency.md`
12. `model_output_contract.md`
13. `model_provider_port_contract.md`
14. `artifact_normalization_contract.md`
15. `schema_based_tools_and_actions.md`
16. `ingestion_pipeline_spec.md`
17. `fact_ledger_and_reflection_claims.md`
18. `sandboxed_actuator_capability.md`
19. `adaptive_interface.md`
20. `ui_theme.md`
21. `control_plane_api_spec.md`
22. `email_send_payload_contract.md`
23. `docs/IMPLEMENTATION_PLAN.md`

For the full ordered map and precedence rules, use `index.md`.

## Repository layout

### Canonical specs (root)
Root-level `*.md` files define canonical behavior unless explicitly marked otherwise.

Key entry points:
- `index.md`
- `kernel_execution_loop.md`
- `governance_kernel_logic.md`
- `jit_compiler.md`
- `verification_core_ruleset.md`
- `storage_semantics_txn.md`
- `approval_oob_spec.md`
- `operation_decomposition_ipc.md`
- `artifact_normalization_contract.md`
- `schema_based_tools_and_actions.md`
- `ingestion_pipeline_spec.md`
- `fact_ledger_and_reflection_claims.md`
- `sandboxed_actuator_capability.md`
- `adaptive_interface.md`
- `ui_theme.md`
- `scheduler_concurrency.md`
- `control_plane_api_spec.md`
- `email_send_payload_contract.md`
- `schema_registry_and_versioning.md`
- `data_classification_and_taint_labelling.md`
- `sanitization_subsystem.md`
- `replay_and_deterministic_re_execution.md`
- `threat_model_spec.md`
- Port contracts:
  - `storage_provider_port_contract.md`
  - `blobstore_provider_port_contract.md`
  - `jobqueue_provider_port_contract.md`
  - `tool_provider_port_contract.md`
  - `model_provider_port_contract.md`

### Implementation plan
- `docs/README.md`
- `docs/IMPLEMENTATION_PLAN.md`
- `docs/REPO_ORGANIZATION.md`
- `docs/DOCS_MAP.md`
- `docs/CODEBASE_MAP.md`

This plan is the implementation baseline. It is not a substitute for canonical specs.

### Reference docs (non-authoritative)
- `reference/README.md`

If a `reference/` file conflicts with a canonical spec, the canonical spec wins.

### Archive docs (legacy)
- `archive/README.md`

Archive files are retained for historical context only and must not be used as source of truth unless reintroduced into canonical specs.

## Implementation start

Coding begins at **Milestone 1** in `docs/IMPLEMENTATION_PLAN.md`.

Current starting assumptions:
- language/runtime: Rust
- async runtime: Tokio
- HTTP/WS: Axum
- architecture: pluggable providers behind explicit port contracts
- storage: implement SQLite first as a **reference backend** without baking SQLite assumptions into kernel behavior

Milestone 1 scope (first allowed slice):
- localhost HTTP + WS
- Root Owner-only control plane
- request acceptance and operation creation
- audit skeleton persistence
- idempotency keys
- operation leases
- fail-closed behavior on audit-critical paths

## Validation and drift checks

These checks intentionally avoid `ripgrep`.

### 1) No stale filename references
```sh
grep -RInE "Audience_graph_and_disclosure_policy\.md|JIT_compiler\.md|control_plane-apispec\.md|governanace_kernal_logic\.md|replay_and_deterministic_re-exection\.md|threat_mode\.spec\.md|modelprovider_port_contract\.md|toolprovider_port_contract\.md|Provider_Interfaces\.md|contracts\.md|rust_contracts\.md|Problem\.md|task\.md|code_skeleton\.md|Api_spec\.md" .
```

### 2) No stale approval endpoint paths

```sh
grep -RInE "/v1/approvals/\{operation_id\}|approvals/\{operation_id\}" .
```

### 3) No stale pinned-state fields

```sh
grep -RIn --exclude=README.md --exclude-dir=.codex "pinned_state_version" .
```

Expect zero hits for all three.

## Contribution rule

Specs before behavior:

* If behavior is missing or ambiguous, update the spec first.
* Implementations must follow canonical specs and port contracts.
* Keep file placement aligned with `docs/REPO_ORGANIZATION.md`.
