# Adesh Spec Guard Checklist

## Canonical naming checks

These filenames are canonical and should not regress:

- `governance_kernel_logic.md`
- `jit_compiler.md`
- `control_plane_api_spec.md`
- `replay_and_deterministic_re_execution.md`
- `threat_model_spec.md`
- `model_provider_port_contract.md`
- `tool_provider_port_contract.md`
- `audience_graph_and_disclosure_policy.md`

## Common drift checks

- approval endpoints must use `approval_id`, never `operation_id`
- pinned versions must include:
  - `active_state_version`
  - `capability_snapshot_version`
  - `audience_graph_version`
- no `pinned_state_version` field in canonical specs
- no pasted wrapper markers like ````md id=` or `Goal understood:` in canonical docs
- root-level specs win over `reference/` and `archive/`
- root-level markdown files (except `README.md`, `AGENTS.md`) must be indexed in `index.md`
- `docs/REPO_ORGANIZATION.md` and `registry/README.md` must exist
- `docs/README.md`, `docs/DOCS_MAP.md`, `docs/CODEBASE_MAP.md`, and `crates/README.md` must exist
- `.gitignore` must include `/target/`

## Milestone checks

For Milestone 1:

- localhost bind only
- Root Owner HTTP/WS only
- request acceptance transaction
- idempotency
- operation leases
- fail-closed audit behavior

Do not implement Milestone 2+ behavior unless the active task explicitly calls for it.
