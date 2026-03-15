# Docs Map (Human + Agent Navigation)

Status: Navigation-only document.
Authority: Non-authoritative. Canonical behavior remains in root-level specs.

Use this file when you know the question but do not know which spec to open first.

## 1) Fast path by task

### Active v1 cognitive proof
- Start: `docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md`
- Sequencing: `docs/IMPLEMENTATION_PLAN.md`
- Storage ordering and fail-closed rules: `storage_semantics_txn.md`
- Storage contract: `storage_provider_port_contract.md`
- Fact and memory promotion substrate: `fact_ledger_and_reflection_claims.md`
- Ingestion and artifact normalization inputs:
  - `artifact_normalization_contract.md`
  - `ingestion_pipeline_spec.md`

### Request lifecycle and execution
- Start: `kernel_execution_loop.md`
- Governance gates: `governance_kernel_logic.md`
- Compilation and memory packing: `jit_compiler.md`
- Verification and deny behavior: `verification_core_ruleset.md`

### Transactions, durability, idempotency, and replay
- Transaction semantics: `storage_semantics_txn.md`
- Storage methods contract: `storage_provider_port_contract.md`
- Storage schema backing: `storage_schema.md`
- Replay contract: `replay_and_deterministic_re_execution.md`

### Approvals and OOB
- Approval/OOB behavior: `approval_oob_spec.md`
- Control-plane endpoints: `control_plane_api_spec.md`
- WS state/audit events: `websocket_events_contract.md`

### Capability and schema system
- Capability snapshots and MCP discovery: `capability_mcp.md`
- Schema registry versioning and integrity: `schema_registry_and_versioning.md`
- Generic externalized tools/actions: `schema_based_tools_and_actions.md`
- Model output schema: `model_output_contract.md`

### Data safety, taint, and disclosure
- Audience policy model: `audience_graph_and_disclosure_policy.md`
- Sensitivity and taint labels: `data_classification_and_taint_labelling.md`
- Sanitization semantics: `sanitization_subsystem.md`
- Threat model: `threat_model_spec.md`

### Async enrichment and governance extensions
- Reflection lifecycle: `reflection_and_persona.md`
- Review queue workflows: `review_queue_and_control_plane.md`
- Fact ledger and claim promotion: `fact_ledger_and_reflection_claims.md`
- Ingestion flow: `ingestion_pipeline_spec.md`
- Artifact normalization rules: `artifact_normalization_contract.md`

### Post-wedge composition contracts
- Declarative UI composition and bindings: `interface_spec_contract.md`
- Durable workflow composition and step semantics: `workflow_spec_contract.md`
- Generic schema-backed tool/action surface: `schema_based_tools_and_actions.md`

### Product wedge and scope lock
- Active wedge scope: `docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md`
- Implementation sequencing: `docs/IMPLEMENTATION_PLAN.md`
- Deferred legacy wedge scope: `docs/WEDGE_V0_EMAIL_DRAFT_AND_SEND.md`
- Deferred legacy wedge runbook: `docs/WEDGE_V0_RUNBOOK.md`
- Generic quickstart and current runtime demo docs:
  - `docs/QUICKSTART.md`
  - `docs/DEMO_RUNBOOK.md`

## 2) Endpoint-to-spec map

- `POST /v1/requests`: `control_plane_api_spec.md`, `kernel_execution_loop.md`, `storage_semantics_txn.md`
- `GET /v1/operations/{operation_id}` and related reads: `control_plane_api_spec.md`, batch contracts
- `POST /v1/approvals/{approval_id}`: `approval_oob_spec.md`, `control_plane_api_spec.md`
- `POST /v1/approvals/{approval_id}/oob/*`: `approval_oob_spec.md`, `control_plane_api_spec.md`
- `POST /v1/audit/{audit_trace_id}/replay`: `replay_and_deterministic_re_execution.md`, `control_plane_api_spec.md`
- `WS /v1/events`: `websocket_events_contract.md`

## 3) Contract-to-spec map

- Batch API schemas: `api_batch_1.md`, `api_batch_2.md`, `api_batch_3.md`
- Provider contracts:
  - `storage_provider_port_contract.md`
  - `tool_provider_port_contract.md`
  - `model_provider_port_contract.md`
  - `blobstore_provider_port_contract.md`
  - `jobqueue_provider_port_contract.md`

## 4) Read order when touching behavior

1. `index.md`
2. Target behavior spec(s) from section 1
3. `storage_semantics_txn.md` for ordering/fail-closed constraints
4. Relevant port contract
5. `docs/IMPLEMENTATION_PLAN.md` for milestone scope check

## 5) Change discipline

- If behavior is ambiguous, patch canonical spec first.
- Update this map only when it improves navigation, not to introduce behavior.
