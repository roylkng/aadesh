# Docs Map (Human + Agent Navigation)

Status: navigation-only document.
Authority: non-authoritative. Scope status comes from `docs/ARCHITECTURE_STATUS.md`; spec placement comes from `docs/specs/README.md`.

## 1) Fast Path For Current Work

### Active Continuity + Supervisory-Substrate Path

- Scope/status truth: `docs/ARCHITECTURE_STATUS.md`
- Active wedge: `docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md`
- Sequencing: `docs/IMPLEMENTATION_PLAN.md`
- Memory contract (vNext design): `docs/MEMORY_CONTRACT_VNEXT.md`
- Policy-state decision note: `docs/POLICY_STATE_DECISION_NOTE.md`
- Evaluation persistence design: `docs/EVALUATION_PERSISTENCE_DESIGN.md`
- External comparison benchmark: `docs/COMPARISON_BENCHMARK.md`
- Competitor testing notes: `docs/COMPETITOR_TESTING_NOTES.md`
- Design Lab boundary: `docs/DESIGN_LAB_BOUNDARY.md`
- Connector integration: `docs/CONNECTOR_INTEGRATION_V0.md`
- Supervisory evolution note: `docs/SUPERVISORY_LAYER_V1.md`
- Runbook: `docs/WEDGE_V0_RUNBOOK.md`
- Code navigation: `docs/CODEBASE_MAP.md`
- Spec inventory: `docs/specs/README.md`

### Active Substrate Contracts Used Now

- `docs/specs/active/storage_semantics_txn.md`
- `docs/specs/active/storage_provider_port_contract.md`
- `docs/specs/active/storage_schema.md`
- `docs/specs/active/fact_ledger_and_reflection_claims.md`
- `docs/specs/active/artifact_normalization_contract.md`
- `docs/specs/active/ingestion_pipeline_spec.md`
- `docs/specs/active/model_output_contract.md`
- `docs/specs/active/mcp_host_surface_contract_spec.md`

## 2) Deferred Spec Groups

Use only when intentionally working in deferred architecture scope.

### Governed Execution / Kernel / JIT / Verification

- `docs/specs/deferred/kernel_execution_loop.md`
- `docs/specs/deferred/governance_kernel_logic.md`
- `docs/specs/deferred/jit_compiler.md`
- `docs/specs/deferred/verification_core_ruleset.md`
- `docs/specs/deferred/threat_model_spec.md`
- `docs/specs/deferred/test_and_kri.md`

### Approval / OOB / Control Plane

- `docs/specs/deferred/approval_oob_spec.md`
- `docs/specs/deferred/control_plane_api_spec.md`
- `docs/specs/deferred/websocket_events_contract.md`
- `docs/specs/deferred/review_queue_and_control_plane.md`
- `docs/specs/deferred/email_send_payload_contract.md`

### Policy Graph / Sanitization / Disclosure

- `docs/specs/deferred/audience_graph_and_disclosure_policy.md`
- `docs/specs/deferred/data_classification_and_taint_labelling.md`
- `docs/specs/deferred/sanitization_subsystem.md`
- `docs/specs/deferred/retention_and_data_lifecycle.md`

### Interface / Workflow Runtime

- `docs/specs/deferred/interface_spec_contract.md`
- `docs/specs/deferred/workflow_spec_contract.md`
- `docs/specs/deferred/adaptive_interface.md`
- `docs/specs/deferred/ui_theme.md`

### Broad Schema / Capability / Provider Platform

- `docs/specs/deferred/schema_based_tools_and_actions.md`
- `docs/specs/deferred/schema_registry_and_versioning.md`
- `docs/specs/deferred/capability_mcp.md`
- `docs/specs/deferred/model_provider_port_contract.md`
- `docs/specs/deferred/tool_provider_port_contract.md`
- `docs/specs/deferred/blobstore_provider_port_contract.md`
- `docs/specs/deferred/jobqueue_provider_port_contract.md`

### Other Deferred Architecture References

- `docs/specs/deferred/api_batch_1.md`
- `docs/specs/deferred/api_batch_2.md`
- `docs/specs/deferred/api_batch_3.md`
- `docs/specs/deferred/boot_sequence.md`
- `docs/specs/deferred/error_remediation.md`
- `docs/specs/deferred/observability_audit.md`
- `docs/specs/deferred/operation_decomposition_ipc.md`
- `docs/specs/deferred/reflection_and_persona.md`
- `docs/specs/deferred/replay_and_deterministic_re_execution.md`
- `docs/specs/deferred/sandboxed_actuator_capability.md`
- `docs/specs/deferred/scheduler_concurrency.md`
- `docs/specs/deferred/stack.md`
- `docs/specs/deferred/version_diff_and_merge.md`

## 3) Archived / Historical

- `archive/*`
- `docs/WEDGE_V0_EMAIL_DRAFT_AND_SEND.md`

## 4) Change Discipline

- Do not pull deferred docs into active milestones by default.
- If behavior is ambiguous, patch the active status/plan/spec first.
- If scope changes, update `docs/ARCHITECTURE_STATUS.md`, `docs/IMPLEMENTATION_PLAN.md`, `docs/specs/README.md`, and this map.
