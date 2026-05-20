# Specs Directory

Status: canonical/spec navigation.
Authority: `docs/ARCHITECTURE_STATUS.md` controls active vs deferred scope for the current implementation phase.

Specs are no longer kept at repository root. Root markdown is reserved for entry docs and agent guidance.

## Layout

- `docs/specs/active/`: specs used by the current continuity and supervisory-substrate implementation cut.
- `docs/specs/deferred/`: long-horizon architecture specs retained as references, not active milestone gates.

## Active Specs

These are canonical inputs for the current implementation phase:

- `docs/specs/active/artifact_normalization_contract.md`
- `docs/specs/active/fact_ledger_and_reflection_claims.md`
- `docs/specs/active/ingestion_pipeline_spec.md`
- `docs/specs/active/mcp_host_surface_contract_spec.md`
- `docs/specs/active/model_output_contract.md`
- `docs/specs/active/storage_provider_port_contract.md`
- `docs/specs/active/storage_schema.md`
- `docs/specs/active/storage_semantics_txn.md`

## Deferred Specs

These remain useful design references, but they must not be treated as current acceptance gates:

- `docs/specs/deferred/adaptive_interface.md`
- `docs/specs/deferred/api_batch_1.md`
- `docs/specs/deferred/api_batch_2.md`
- `docs/specs/deferred/api_batch_3.md`
- `docs/specs/deferred/approval_oob_spec.md`
- `docs/specs/deferred/audience_graph_and_disclosure_policy.md`
- `docs/specs/deferred/blobstore_provider_port_contract.md`
- `docs/specs/deferred/boot_sequence.md`
- `docs/specs/deferred/capability_mcp.md`
- `docs/specs/deferred/control_plane_api_spec.md`
- `docs/specs/deferred/data_classification_and_taint_labelling.md`
- `docs/specs/deferred/email_send_payload_contract.md`
- `docs/specs/deferred/error_remediation.md`
- `docs/specs/deferred/governance_kernel_logic.md`
- `docs/specs/deferred/interface_spec_contract.md`
- `docs/specs/deferred/jit_compiler.md`
- `docs/specs/deferred/jobqueue_provider_port_contract.md`
- `docs/specs/deferred/kernel_execution_loop.md`
- `docs/specs/deferred/model_provider_port_contract.md`
- `docs/specs/deferred/observability_audit.md`
- `docs/specs/deferred/operation_decomposition_ipc.md`
- `docs/specs/deferred/reflection_and_persona.md`
- `docs/specs/deferred/replay_and_deterministic_re_execution.md`
- `docs/specs/deferred/retention_and_data_lifecycle.md`
- `docs/specs/deferred/review_queue_and_control_plane.md`
- `docs/specs/deferred/sandboxed_actuator_capability.md`
- `docs/specs/deferred/sanitization_subsystem.md`
- `docs/specs/deferred/scheduler_concurrency.md`
- `docs/specs/deferred/schema_based_tools_and_actions.md`
- `docs/specs/deferred/schema_registry_and_versioning.md`
- `docs/specs/deferred/stack.md`
- `docs/specs/deferred/test_and_kri.md`
- `docs/specs/deferred/threat_model_spec.md`
- `docs/specs/deferred/tool_provider_port_contract.md`
- `docs/specs/deferred/ui_theme.md`
- `docs/specs/deferred/verification_core_ruleset.md`
- `docs/specs/deferred/version_diff_and_merge.md`
- `docs/specs/deferred/websocket_events_contract.md`
- `docs/specs/deferred/workflow_spec_contract.md`

## Contribution Rule

If a deferred spec becomes implementation-driving, update these files in the same change:

1. `docs/ARCHITECTURE_STATUS.md`
2. `docs/IMPLEMENTATION_PLAN.md`
3. `docs/specs/README.md`
4. `docs/DOCS_MAP.md`
