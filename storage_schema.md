# Storage Schema v0.1 (Vendor-neutral)

## Design principles

* **Append-only Experience Log**: immutable event records, indexed.
* **Versioned Active State**: state mutations create new versions; operations pin versions.
* **Audit is replay-first**: store AuditTrace and references to linked objects.
* **Objects are stored as JSON with typed columns for indexing**: strong structure in the app layer (Batch schemas), flexible persistence for evolution.
* **Idempotency support**: store idempotency keys for side-effecting writes.

## Core tables (logical)

1. `experience_events` (append-only)
2. `operations` (current state) + `operation_transitions` (append-only) + `operation_leases`
3. `gate_decisions`
4. `compiled_slices`
5. `syscalls` + `syscall_denies`
6. `approval_items` + `approval_item_syscalls` + `oob_challenges`
7. `ipc_artifacts`
8. `audit_traces`
9. `active_state_versions` + `current_versions`
10. `capability_snapshots`
11. `schema_registry_entries`
12. `workflow_specs` + `workflow_instances` + `workflow_instance_transitions` + `workflow_step_states` + `workflow_step_transitions`
13. `interface_specs` + `interface_instances`
14. `audience_graph_nodes`, `audience_graph_edges`, `audience_graph_scopes`
15. `review_queue_items` + `review_queue_decisions`
16. `idempotency_keys`
17. `claims` + `claim_evidence` + `claim_conflicts`
18. `ingest_jobs` + `ingest_job_items`
19. `artifacts`
20. `jobs` (reflection / async work queue)
21. `blob_objects` (metadata only, content in filesystem/S3)

Minimal indexes are included below.

---

# SQLite DDL v0.1 (reference backend)

> Notes:
>
> * Enable WAL mode in runtime (`PRAGMA journal_mode=WAL;`).
> * Use `TEXT` for UUIDs.
> * JSON is stored as `TEXT` (validated by app).
> * Add `STRICT` tables if you want stronger typing (SQLite 3.37+), optional.

```sql
-- =========================
-- 1) Active state versions
-- =========================
CREATE TABLE IF NOT EXISTS active_state_versions (
  state_version TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  parent_version TEXT,
  content_hash TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  provenance_refs_json TEXT NOT NULL,
  notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_active_state_versions_created
  ON active_state_versions(created_at);

CREATE INDEX IF NOT EXISTS idx_active_state_versions_parent
  ON active_state_versions(parent_version);

CREATE TABLE IF NOT EXISTS current_versions (
  version_kind TEXT PRIMARY KEY,       -- active_state|audience_graph|capability_snapshot
  version_id TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- =========================
-- 2) Experience Log (append-only)
-- =========================
CREATE TABLE IF NOT EXISTS experience_events (
  event_ref TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  source_class TEXT NOT NULL,         -- user_statement, tool_trace, telemetry, etc.
  author TEXT,                        -- optional
  audience_id TEXT,                   -- Audience Graph node id
  sensitivity_s INTEGER NOT NULL,     -- 0..4
  taint_s INTEGER NOT NULL,           -- 0..4
  kind TEXT NOT NULL,                 -- request, syscall_result, approval, etc.
  content_ref TEXT,                   -- optional blob ref
  json_payload TEXT NOT NULL          -- canonical event object
);

CREATE INDEX IF NOT EXISTS idx_experience_events_created
  ON experience_events(created_at);

CREATE INDEX IF NOT EXISTS idx_experience_events_audience
  ON experience_events(audience_id);

CREATE INDEX IF NOT EXISTS idx_experience_events_kind
  ON experience_events(kind);

-- =========================
-- 3) Operations and transitions
-- =========================
CREATE TABLE IF NOT EXISTS operations (
  operation_id TEXT PRIMARY KEY,
  parent_request_id TEXT NOT NULL,
  isolation_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  state TEXT NOT NULL,                -- created|compiled|awaiting_approval|running|blocked|...
  state_reason TEXT,
  requesting_audience_id TEXT NOT NULL,
  pinned_active_state_version TEXT,
  pinned_capability_snapshot_version TEXT,
  pinned_audience_graph_version TEXT,
  budgets_json TEXT NOT NULL,         -- includes token_budget + block budgets
  operation_goal_json TEXT NOT NULL,  -- from OperationSpec.operation_goal
  ipc_json TEXT                        -- consumes_artifacts, inherits_sensitivity, etc.
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_operations_isolation
  ON operations(isolation_id);

CREATE INDEX IF NOT EXISTS idx_operations_state
  ON operations(state);

CREATE TABLE IF NOT EXISTS operation_transitions (
  transition_id TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL,
  ts TEXT NOT NULL,
  from_state TEXT,
  to_state TEXT NOT NULL,
  reason TEXT,
  audit_trace_id TEXT,
  FOREIGN KEY(operation_id) REFERENCES operations(operation_id)
);

CREATE INDEX IF NOT EXISTS idx_operation_transitions_op_ts
  ON operation_transitions(operation_id, ts);

CREATE TABLE IF NOT EXISTS operation_leases (
  operation_id TEXT PRIMARY KEY,
  lease_owner TEXT NOT NULL,
  leased_until TEXT NOT NULL,
  lease_epoch INTEGER NOT NULL,
  last_heartbeat_at TEXT,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(operation_id) REFERENCES operations(operation_id)
);

CREATE INDEX IF NOT EXISTS idx_operation_leases_until
  ON operation_leases(leased_until);

CREATE INDEX IF NOT EXISTS idx_operation_leases_owner_until
  ON operation_leases(lease_owner, leased_until);

-- =========================
-- 4) Gate decisions (Batch 2)
-- =========================
CREATE TABLE IF NOT EXISTS gate_decisions (
  gate_decision_id TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL,
  isolation_id TEXT NOT NULL,
  evaluated_at TEXT NOT NULL,
  active_state_version TEXT NOT NULL,
  capability_snapshot_version TEXT NOT NULL,
  audience_graph_version TEXT NOT NULL,
  risk_r INTEGER NOT NULL,
  sensitivity_s INTEGER NOT NULL,
  max_gate INTEGER NOT NULL,
  approval_mode TEXT NOT NULL,        -- none|confirm|diff|oob_required|refuse
  requesting_audience_id TEXT NOT NULL,
  scopes_allowed_json TEXT NOT NULL,
  scopes_denied_json TEXT NOT NULL,
  sensitivity_ceiling_s INTEGER NOT NULL,
  predicates_json TEXT NOT NULL,      -- fired predicates
  constraints_json TEXT NOT NULL,     -- negative memory, token budgets, taint policy, etc.
  json_payload TEXT NOT NULL,
  audit_trace_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_gate_decisions_op_time
  ON gate_decisions(operation_id, evaluated_at);

-- =========================
-- 5) Compiled slices (Batch 2)
-- =========================
CREATE TABLE IF NOT EXISTS compiled_slices (
  compiled_slice_id TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL,
  isolation_id TEXT NOT NULL,
  compiled_at TEXT NOT NULL,
  active_state_version TEXT NOT NULL,
  capability_snapshot_version TEXT NOT NULL,
  audience_graph_version TEXT NOT NULL,
  risk_r INTEGER NOT NULL,
  sensitivity_s INTEGER NOT NULL,
  max_gate INTEGER NOT NULL,
  approval_mode TEXT NOT NULL,
  operation_max_taint_s INTEGER NOT NULL,
  did_omit INTEGER NOT NULL,          -- 0/1
  omissions_json TEXT NOT NULL,
  provenance_summary_json TEXT NOT NULL,
  intent_anchor_json TEXT NOT NULL,
  blocks_json TEXT NOT NULL,          -- policy/capability/context/evidence/scratch blocks
  json_payload TEXT NOT NULL,
  audit_trace_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_compiled_slices_op_time
  ON compiled_slices(operation_id, compiled_at);

-- =========================
-- 6) Syscalls + denials (Batch 3)
-- =========================
CREATE TABLE IF NOT EXISTS syscalls (
  syscall_id TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL,
  isolation_id TEXT NOT NULL,
  issued_at TEXT NOT NULL,
  caller_component TEXT NOT NULL,
  target_kind TEXT NOT NULL,          -- sensor|actuator|ipc|sanitizer|memory_read
  target_name TEXT NOT NULL,
  provider TEXT NOT NULL,             -- mcp|adapter|internal
  status TEXT NOT NULL,               -- proposed|permitted|denied|awaiting_approval|executed|failed
  declared_effect TEXT,
  declared_audience_id TEXT,
  risk_r INTEGER NOT NULL,
  sensitivity_s INTEGER NOT NULL,
  max_gate INTEGER NOT NULL,
  approval_mode TEXT NOT NULL,
  taint_in_s INTEGER NOT NULL,
  output_ref TEXT,
  output_sensitivity_s INTEGER,
  output_taint_s INTEGER,
  error_code TEXT,
  error_message TEXT,
  retryable INTEGER,
  json_payload TEXT NOT NULL,
  audit_trace_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_syscalls_op_time
  ON syscalls(operation_id, issued_at);

CREATE INDEX IF NOT EXISTS idx_syscalls_status
  ON syscalls(status);

CREATE TABLE IF NOT EXISTS syscall_denies (
  syscall_id TEXT PRIMARY KEY,
  denied_at TEXT NOT NULL,
  deny_class TEXT NOT NULL,
  violations_json TEXT NOT NULL,
  retry_policy_json TEXT NOT NULL,
  remediation_json TEXT NOT NULL,
  json_payload TEXT NOT NULL,
  audit_trace_id TEXT,
  FOREIGN KEY(syscall_id) REFERENCES syscalls(syscall_id)
);

-- =========================
-- 7) Approvals + OOB
-- =========================
CREATE TABLE IF NOT EXISTS approval_items (
  approval_id TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL,
  isolation_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  approval_mode TEXT NOT NULL,        -- confirm|diff|oob_required
  status TEXT NOT NULL,               -- pending|approved|denied|expired|consumed|superseded
  proposal_bundle_json TEXT NOT NULL,
  diff_payload_json TEXT,
  prompt TEXT NOT NULL,
  expires_at TEXT,
  audit_trace_id TEXT,
  FOREIGN KEY(operation_id) REFERENCES operations(operation_id)
);

CREATE INDEX IF NOT EXISTS idx_approval_items_op_status
  ON approval_items(operation_id, status);

CREATE INDEX IF NOT EXISTS idx_approval_items_expires
  ON approval_items(expires_at);

CREATE TABLE IF NOT EXISTS approval_item_syscalls (
  approval_id TEXT NOT NULL,
  syscall_id TEXT NOT NULL,
  PRIMARY KEY (approval_id, syscall_id),
  FOREIGN KEY(approval_id) REFERENCES approval_items(approval_id),
  FOREIGN KEY(syscall_id) REFERENCES syscalls(syscall_id)
);

CREATE TABLE IF NOT EXISTS oob_challenges (
  challenge_id TEXT PRIMARY KEY,
  approval_id TEXT NOT NULL,
  challenge_type TEXT NOT NULL,
  nonce_hash TEXT NOT NULL,
  status TEXT NOT NULL,               -- pending|verified|consumed|expired|failed
  issued_at TEXT NOT NULL,
  verified_at TEXT,
  consumed_at TEXT,
  expires_at TEXT NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY(approval_id) REFERENCES approval_items(approval_id)
);

CREATE INDEX IF NOT EXISTS idx_oob_challenges_approval_status
  ON oob_challenges(approval_id, status);

CREATE INDEX IF NOT EXISTS idx_oob_challenges_expires
  ON oob_challenges(expires_at);

-- =========================
-- 8) IPC artifacts (Batch 3)
-- =========================
CREATE TABLE IF NOT EXISTS ipc_artifacts (
  artifact_id TEXT PRIMARY KEY,
  produced_by_operation_id TEXT NOT NULL,
  produced_at TEXT NOT NULL,
  kind TEXT NOT NULL,
  content_ref TEXT NOT NULL,
  sensitivity_s INTEGER NOT NULL,
  taint_s INTEGER NOT NULL,
  provenance_refs_json TEXT NOT NULL,
  audience_scope_tag_json TEXT NOT NULL,
  ipc_rules_json TEXT,
  json_payload TEXT NOT NULL,
  audit_trace_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_ipc_artifacts_producer_time
  ON ipc_artifacts(produced_by_operation_id, produced_at);

-- =========================
-- 9) Audit traces (Batch 3)
-- =========================
CREATE TABLE IF NOT EXISTS audit_traces (
  audit_trace_id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  request_id TEXT NOT NULL,
  operation_id TEXT NOT NULL,
  isolation_id TEXT NOT NULL,
  pinned_json TEXT NOT NULL,
  summary_json TEXT NOT NULL,
  timeline_json TEXT NOT NULL,
  attachments_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_audit_traces_op
  ON audit_traces(operation_id);

-- =========================
-- 10) Capability snapshots
-- =========================
CREATE TABLE IF NOT EXISTS capability_snapshots (
  capability_snapshot_version TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  parent_version TEXT,
  content_hash TEXT NOT NULL,
  json_payload TEXT NOT NULL,
  notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_capability_snapshots_created
  ON capability_snapshots(created_at);

CREATE INDEX IF NOT EXISTS idx_capability_snapshots_parent
  ON capability_snapshots(parent_version);

-- =========================
-- 11) Schema registry
-- =========================
CREATE TABLE IF NOT EXISTS schema_registry_entries (
  schema_ref TEXT PRIMARY KEY,
  schema_kind TEXT NOT NULL,
  name TEXT NOT NULL,
  semver TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  created_at TEXT NOT NULL,
  status TEXT NOT NULL,
  compatibility TEXT NOT NULL,
  payload_json TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_schema_registry_name_semver
  ON schema_registry_entries(name, semver);

CREATE INDEX IF NOT EXISTS idx_schema_registry_hash
  ON schema_registry_entries(content_hash);

-- =========================
-- 12) Workflow specs and instances
-- =========================
CREATE TABLE IF NOT EXISTS workflow_specs (
  workflow_ref TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  author TEXT NOT NULL,
  tags_json TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  payload_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workflow_specs_name_created
  ON workflow_specs(name, created_at);

CREATE TABLE IF NOT EXISTS workflow_instances (
  workflow_instance_id TEXT PRIMARY KEY,
  workflow_ref TEXT NOT NULL,
  parent_request_id TEXT,
  parent_operation_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  state TEXT NOT NULL,                -- created|running|awaiting_approval|blocked|completed|failed|cancelled
  state_reason TEXT,
  pinned_active_state_version TEXT NOT NULL,
  pinned_capability_snapshot_version TEXT NOT NULL,
  pinned_audience_graph_version TEXT NOT NULL,
  inputs_json TEXT NOT NULL,
  outputs_json TEXT,
  FOREIGN KEY(workflow_ref) REFERENCES workflow_specs(workflow_ref),
  FOREIGN KEY(parent_operation_id) REFERENCES operations(operation_id)
);

CREATE INDEX IF NOT EXISTS idx_workflow_instances_ref_state
  ON workflow_instances(workflow_ref, state);

CREATE TABLE IF NOT EXISTS workflow_instance_transitions (
  transition_id TEXT PRIMARY KEY,
  workflow_instance_id TEXT NOT NULL,
  ts TEXT NOT NULL,
  from_state TEXT,
  to_state TEXT NOT NULL,
  reason TEXT,
  FOREIGN KEY(workflow_instance_id) REFERENCES workflow_instances(workflow_instance_id)
);

CREATE INDEX IF NOT EXISTS idx_workflow_instance_transitions_instance_ts
  ON workflow_instance_transitions(workflow_instance_id, ts);

CREATE TABLE IF NOT EXISTS workflow_step_states (
  workflow_instance_id TEXT NOT NULL,
  step_id TEXT NOT NULL,
  step_type TEXT NOT NULL,            -- transform|model_call|syscall|subworkflow
  state TEXT NOT NULL,                -- pending|running|awaiting_approval|blocked|completed|failed|skipped|cancelled
  attempt INTEGER NOT NULL DEFAULT 0,
  operation_id TEXT,
  approval_id TEXT,
  syscall_id TEXT,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(workflow_instance_id, step_id),
  FOREIGN KEY(workflow_instance_id) REFERENCES workflow_instances(workflow_instance_id),
  FOREIGN KEY(operation_id) REFERENCES operations(operation_id),
  FOREIGN KEY(approval_id) REFERENCES approval_items(approval_id),
  FOREIGN KEY(syscall_id) REFERENCES syscalls(syscall_id)
);

CREATE INDEX IF NOT EXISTS idx_workflow_step_states_state
  ON workflow_step_states(workflow_instance_id, state);

CREATE TABLE IF NOT EXISTS workflow_step_transitions (
  transition_id TEXT PRIMARY KEY,
  workflow_instance_id TEXT NOT NULL,
  step_id TEXT NOT NULL,
  ts TEXT NOT NULL,
  from_state TEXT,
  to_state TEXT NOT NULL,
  reason TEXT,
  linked_operation_id TEXT,
  linked_approval_id TEXT,
  linked_syscall_id TEXT,
  FOREIGN KEY(workflow_instance_id, step_id) REFERENCES workflow_step_states(workflow_instance_id, step_id)
);

CREATE INDEX IF NOT EXISTS idx_workflow_step_transitions_instance_ts
  ON workflow_step_transitions(workflow_instance_id, ts);

-- =========================
-- 13) Interface specs and instances
-- =========================
CREATE TABLE IF NOT EXISTS interface_specs (
  interface_ref TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  author TEXT NOT NULL,
  tags_json TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  payload_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_interface_specs_name_created
  ON interface_specs(name, created_at);

CREATE TABLE IF NOT EXISTS interface_instances (
  interface_instance_id TEXT PRIMARY KEY,
  interface_ref TEXT NOT NULL,
  operation_id TEXT,
  workflow_instance_id TEXT,
  created_at TEXT NOT NULL,
  viewer_audience_id TEXT NOT NULL,
  pinned_active_state_version TEXT NOT NULL,
  pinned_capability_snapshot_version TEXT NOT NULL,
  pinned_audience_graph_version TEXT NOT NULL,
  gate_summary_json TEXT NOT NULL,
  blocks_json TEXT NOT NULL,
  bindings_json TEXT NOT NULL,
  taint_summary_json TEXT NOT NULL,
  state TEXT NOT NULL,                -- ready|stale
  FOREIGN KEY(interface_ref) REFERENCES interface_specs(interface_ref),
  FOREIGN KEY(operation_id) REFERENCES operations(operation_id),
  FOREIGN KEY(workflow_instance_id) REFERENCES workflow_instances(workflow_instance_id),
  CHECK (
    (operation_id IS NOT NULL AND workflow_instance_id IS NULL) OR
    (operation_id IS NULL AND workflow_instance_id IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS idx_interface_instances_operation
  ON interface_instances(operation_id);

CREATE INDEX IF NOT EXISTS idx_interface_instances_workflow
  ON interface_instances(workflow_instance_id);

-- =========================
-- 14) Audience Graph
-- =========================
CREATE TABLE IF NOT EXISTS audience_graph_nodes (
  node_id TEXT PRIMARY KEY,
  node_type TEXT NOT NULL,            -- person|group|role|channel|public|root_owner|agent_client
  label TEXT,
  props_json TEXT NOT NULL,
  graph_version TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audience_nodes_version
  ON audience_graph_nodes(graph_version);

CREATE TABLE IF NOT EXISTS audience_graph_edges (
  edge_id TEXT PRIMARY KEY,
  src_id TEXT NOT NULL,
  dst_id TEXT NOT NULL,
  edge_type TEXT NOT NULL,            -- relationship type
  props_json TEXT NOT NULL,
  graph_version TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audience_edges_version
  ON audience_graph_edges(graph_version);

CREATE INDEX IF NOT EXISTS idx_audience_edges_src_dst
  ON audience_graph_edges(src_id, dst_id);

CREATE TABLE IF NOT EXISTS audience_graph_scopes (
  scope_id TEXT PRIMARY KEY,
  src_id TEXT NOT NULL,
  dst_id TEXT NOT NULL,
  allowed_scopes_json TEXT NOT NULL,
  sensitivity_ceiling_s INTEGER NOT NULL,
  graph_version TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audience_scopes_version
  ON audience_graph_scopes(graph_version);

-- =========================
-- 15) Review queue (hypothesis promotion)
-- =========================
CREATE TABLE IF NOT EXISTS review_queue_items (
  item_id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  status TEXT NOT NULL,               -- pending|resolved
  summary TEXT NOT NULL,
  proposed_change_json TEXT NOT NULL,
  evidence_refs_json TEXT NOT NULL,
  risk_r INTEGER NOT NULL,
  sensitivity_s INTEGER NOT NULL,
  requires_owner_confirmation INTEGER NOT NULL,
  audit_trace_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_review_queue_status
  ON review_queue_items(status);

CREATE TABLE IF NOT EXISTS review_queue_decisions (
  decision_id TEXT PRIMARY KEY,
  item_id TEXT NOT NULL,
  decided_at TEXT NOT NULL,
  decision TEXT NOT NULL,             -- approve|reject|edit
  edited_payload_json TEXT,
  change_id TEXT,
  new_state_version TEXT,
  audit_trace_id TEXT,
  FOREIGN KEY(item_id) REFERENCES review_queue_items(item_id)
);

-- =========================
-- 16) Idempotency keys
-- =========================
CREATE TABLE IF NOT EXISTS idempotency_keys (
  endpoint_scope TEXT NOT NULL,
  idempotency_key TEXT NOT NULL,
  request_id TEXT NOT NULL,
  response_json TEXT NOT NULL,
  response_hash TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT,
  PRIMARY KEY(endpoint_scope, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_idempotency_keys_expires
  ON idempotency_keys(expires_at);

-- =========================
-- 17) Fact Ledger claims
-- =========================
CREATE TABLE IF NOT EXISTS episodes (
  episode_id TEXT PRIMARY KEY,
  scope_type TEXT NOT NULL,          -- user_global|workspace|task_or_workstream|artifact|episode
  scope_key TEXT NOT NULL,
  task_scope_key TEXT,
  workspace_json TEXT NOT NULL,
  workspace_resolution_basis_json TEXT NOT NULL,
  workspace_resolution_confidence REAL NOT NULL,
  branch TEXT,
  task_prompt TEXT NOT NULL,
  summary TEXT NOT NULL,
  files_touched_json TEXT NOT NULL,
  decisions_json TEXT NOT NULL,
  unresolved_items_json TEXT NOT NULL,
  tests_json TEXT NOT NULL,
  observed_preferences_json TEXT NOT NULL,
  risk_signals_json TEXT NOT NULL,
  issue_refs_json TEXT NOT NULL,
  artifact_refs_json TEXT NOT NULL,
  started_at TEXT NOT NULL,
  ended_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_episodes_scope_time
  ON episodes(scope_type, scope_key, ended_at);

CREATE INDEX IF NOT EXISTS idx_episodes_task_scope_time
  ON episodes(task_scope_key, ended_at);

CREATE TABLE IF NOT EXISTS episode_artifacts (
  episode_id TEXT NOT NULL,
  artifact_ref TEXT NOT NULL,
  PRIMARY KEY(episode_id, artifact_ref),
  FOREIGN KEY(episode_id) REFERENCES episodes(episode_id)
);

CREATE TABLE IF NOT EXISTS search_documents (
  doc_id TEXT PRIMARY KEY,
  scope_type TEXT NOT NULL,
  scope_key TEXT NOT NULL,
  source_type TEXT NOT NULL,         -- episode|claim
  source_ref TEXT NOT NULL,
  title TEXT,
  body_text TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_search_documents_scope_updated
  ON search_documents(scope_type, scope_key, updated_at);

CREATE TABLE IF NOT EXISTS claims (
  claim_id TEXT PRIMARY KEY,
  claim_type TEXT NOT NULL,
  claim_key TEXT NOT NULL,
  scope_type TEXT NOT NULL,          -- user_global|workspace|task_or_workstream|artifact|episode
  scope_key TEXT NOT NULL,
  subject_key TEXT NOT NULL,
  status TEXT NOT NULL,               -- candidate|accepted|superseded|rejected
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  created_by TEXT NOT NULL,           -- reflection|owner
  confidence REAL NOT NULL,
  value_json TEXT NOT NULL,
  context_predicates_json TEXT NOT NULL,
  time_start TEXT,
  time_end TEXT,
  evidence_quality_json TEXT NOT NULL,
  promotion_ref TEXT
);

CREATE INDEX IF NOT EXISTS idx_claims_key_status
  ON claims(claim_key, status);

CREATE INDEX IF NOT EXISTS idx_claims_type_status
  ON claims(claim_type, status);

CREATE INDEX IF NOT EXISTS idx_claims_scope_status
  ON claims(scope_type, scope_key, status);

CREATE INDEX IF NOT EXISTS idx_claims_scope_subject_status
  ON claims(scope_type, scope_key, subject_key, status);

CREATE TABLE IF NOT EXISTS claim_evidence (
  claim_id TEXT NOT NULL,
  evidence_ref TEXT NOT NULL,
  evidence_kind TEXT NOT NULL,        -- artifact|experience_event
  locator_json TEXT,
  PRIMARY KEY(claim_id, evidence_ref),
  FOREIGN KEY(claim_id) REFERENCES claims(claim_id)
);

CREATE TABLE IF NOT EXISTS claim_conflicts (
  claim_id TEXT NOT NULL,
  conflicting_claim_id TEXT NOT NULL,
  reason_code TEXT NOT NULL,
  detected_at TEXT NOT NULL,
  PRIMARY KEY(claim_id, conflicting_claim_id),
  FOREIGN KEY(claim_id) REFERENCES claims(claim_id),
  FOREIGN KEY(conflicting_claim_id) REFERENCES claims(claim_id)
);

-- =========================
-- 18) Ingest jobs
-- =========================
CREATE TABLE IF NOT EXISTS ingest_jobs (
  job_id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  status TEXT NOT NULL,               -- pending|running|completed|failed|cancelled
  source_count INTEGER NOT NULL,
  artifacts_total INTEGER NOT NULL,
  artifacts_succeeded INTEGER NOT NULL,
  artifacts_failed INTEGER NOT NULL,
  bytes_ingested INTEGER NOT NULL,
  options_json TEXT NOT NULL,
  error_summary TEXT
);

CREATE INDEX IF NOT EXISTS idx_ingest_jobs_status_created
  ON ingest_jobs(status, created_at);

CREATE TABLE IF NOT EXISTS ingest_job_items (
  job_id TEXT NOT NULL,
  item_key TEXT NOT NULL,
  status TEXT NOT NULL,               -- pending|running|completed|failed|cancelled
  artifact_id TEXT,
  error_json TEXT,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(job_id, item_key),
  FOREIGN KEY(job_id) REFERENCES ingest_jobs(job_id)
);

CREATE INDEX IF NOT EXISTS idx_ingest_job_items_status
  ON ingest_job_items(job_id, status);

-- =========================
-- 19) Artifact registry
-- =========================
CREATE TABLE IF NOT EXISTS artifacts (
  artifact_id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  ingest_job_id TEXT,
  kind TEXT NOT NULL,
  content_ref TEXT NOT NULL,
  parent_artifact_id TEXT,
  dedupe_key TEXT,
  meta_json TEXT NOT NULL,
  FOREIGN KEY(ingest_job_id) REFERENCES ingest_jobs(job_id),
  FOREIGN KEY(parent_artifact_id) REFERENCES artifacts(artifact_id),
  FOREIGN KEY(content_ref) REFERENCES blob_objects(content_ref)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_artifacts_dedupe
  ON artifacts(dedupe_key);

CREATE INDEX IF NOT EXISTS idx_artifacts_job_created
  ON artifacts(ingest_job_id, created_at);

-- =========================
-- 20) Jobs queue (reflection / async work)
-- =========================
CREATE TABLE IF NOT EXISTS jobs (
  job_id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  run_after TEXT,
  leased_until TEXT,
  lease_owner TEXT,
  lease_epoch INTEGER NOT NULL,
  status TEXT NOT NULL,               -- pending|leased|completed|failed|dead_lettered|cancelled
  attempt_count INTEGER NOT NULL,
  max_attempts INTEGER NOT NULL,
  job_type TEXT NOT NULL,
  dedupe_key TEXT,
  payload_json TEXT NOT NULL,
  sensitivity_s INTEGER NOT NULL,
  taint_s INTEGER NOT NULL,
  provenance_refs_json TEXT NOT NULL,
  last_error_code TEXT,
  last_error_message TEXT,
  completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_jobs_status_run_after
  ON jobs(status, run_after);

CREATE UNIQUE INDEX IF NOT EXISTS idx_jobs_dedupe_active
  ON jobs(dedupe_key, status);

-- =========================
-- 21) Blob metadata (content stored in FS/S3)
-- =========================
CREATE TABLE IF NOT EXISTS blob_objects (
  content_ref TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  content_type TEXT,
  size_bytes INTEGER NOT NULL,
  checksum TEXT,
  sensitivity_s INTEGER NOT NULL,
  taint_s INTEGER NOT NULL,
  provenance_refs_json TEXT NOT NULL,
  storage_backend TEXT NOT NULL,      -- fs|s3|other
  storage_path TEXT NOT NULL          -- path or key
);

CREATE INDEX IF NOT EXISTS idx_blob_created
  ON blob_objects(created_at);
```

---

# Postgres DDL v0.1 (reference backend, server profile)

> Notes:
>
> * Use `JSONB` for payloads.
> * Use `UUID` types if desired; below uses `TEXT` for portability with the same contract IDs.
> * Consider partitioning `experience_events` by time later.

```sql
-- =========================
-- 1) Active state versions
-- =========================
CREATE TABLE IF NOT EXISTS active_state_versions (
  state_version TEXT PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL,
  parent_version TEXT,
  content_hash TEXT NOT NULL,
  payload_json JSONB NOT NULL,
  provenance_refs_json JSONB NOT NULL,
  notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_active_state_versions_created
  ON active_state_versions(created_at);

CREATE INDEX IF NOT EXISTS idx_active_state_versions_parent
  ON active_state_versions(parent_version);

CREATE TABLE IF NOT EXISTS current_versions (
  version_kind TEXT PRIMARY KEY,
  version_id TEXT NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL
);

-- =========================
-- 2) Experience Log
-- =========================
CREATE TABLE IF NOT EXISTS experience_events (
  event_ref TEXT PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL,
  source_class TEXT NOT NULL,
  author TEXT,
  audience_id TEXT,
  sensitivity_s INT NOT NULL,
  taint_s INT NOT NULL,
  kind TEXT NOT NULL,
  content_ref TEXT,
  json_payload JSONB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_experience_events_created
  ON experience_events(created_at);

CREATE INDEX IF NOT EXISTS idx_experience_events_audience
  ON experience_events(audience_id);

CREATE INDEX IF NOT EXISTS idx_experience_events_kind
  ON experience_events(kind);

-- =========================
-- 3) Operations and transitions
-- =========================
CREATE TABLE IF NOT EXISTS operations (
  operation_id TEXT PRIMARY KEY,
  parent_request_id TEXT NOT NULL,
  isolation_id TEXT NOT NULL UNIQUE,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  state TEXT NOT NULL,
  state_reason TEXT,
  requesting_audience_id TEXT NOT NULL,
  pinned_active_state_version TEXT,
  pinned_capability_snapshot_version TEXT,
  pinned_audience_graph_version TEXT,
  budgets_json JSONB NOT NULL,
  operation_goal_json JSONB NOT NULL,
  ipc_json JSONB
);

CREATE INDEX IF NOT EXISTS idx_operations_state
  ON operations(state);

CREATE TABLE IF NOT EXISTS operation_transitions (
  transition_id TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL REFERENCES operations(operation_id),
  ts TIMESTAMPTZ NOT NULL,
  from_state TEXT,
  to_state TEXT NOT NULL,
  reason TEXT,
  audit_trace_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_operation_transitions_op_ts
  ON operation_transitions(operation_id, ts);

CREATE TABLE IF NOT EXISTS operation_leases (
  operation_id TEXT PRIMARY KEY REFERENCES operations(operation_id),
  lease_owner TEXT NOT NULL,
  leased_until TIMESTAMPTZ NOT NULL,
  lease_epoch INT NOT NULL,
  last_heartbeat_at TIMESTAMPTZ,
  updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_operation_leases_until
  ON operation_leases(leased_until);

CREATE INDEX IF NOT EXISTS idx_operation_leases_owner_until
  ON operation_leases(lease_owner, leased_until);

-- =========================
-- 4) Gate decisions
-- =========================
CREATE TABLE IF NOT EXISTS gate_decisions (
  gate_decision_id TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL REFERENCES operations(operation_id),
  isolation_id TEXT NOT NULL,
  evaluated_at TIMESTAMPTZ NOT NULL,
  active_state_version TEXT NOT NULL,
  capability_snapshot_version TEXT NOT NULL,
  audience_graph_version TEXT NOT NULL,
  risk_r INT NOT NULL,
  sensitivity_s INT NOT NULL,
  max_gate INT NOT NULL,
  approval_mode TEXT NOT NULL,
  requesting_audience_id TEXT NOT NULL,
  scopes_allowed_json JSONB NOT NULL,
  scopes_denied_json JSONB NOT NULL,
  sensitivity_ceiling_s INT NOT NULL,
  predicates_json JSONB NOT NULL,
  constraints_json JSONB NOT NULL,
  json_payload JSONB NOT NULL,
  audit_trace_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_gate_decisions_op_time
  ON gate_decisions(operation_id, evaluated_at);

-- =========================
-- 5) Compiled slices
-- =========================
CREATE TABLE IF NOT EXISTS compiled_slices (
  compiled_slice_id TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL REFERENCES operations(operation_id),
  isolation_id TEXT NOT NULL,
  compiled_at TIMESTAMPTZ NOT NULL,
  active_state_version TEXT NOT NULL,
  capability_snapshot_version TEXT NOT NULL,
  audience_graph_version TEXT NOT NULL,
  risk_r INT NOT NULL,
  sensitivity_s INT NOT NULL,
  max_gate INT NOT NULL,
  approval_mode TEXT NOT NULL,
  operation_max_taint_s INT NOT NULL,
  did_omit BOOLEAN NOT NULL,
  omissions_json JSONB NOT NULL,
  provenance_summary_json JSONB NOT NULL,
  intent_anchor_json JSONB NOT NULL,
  blocks_json JSONB NOT NULL,
  json_payload JSONB NOT NULL,
  audit_trace_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_compiled_slices_op_time
  ON compiled_slices(operation_id, compiled_at);

-- =========================
-- 6) Syscalls + denials
-- =========================
CREATE TABLE IF NOT EXISTS syscalls (
  syscall_id TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL REFERENCES operations(operation_id),
  isolation_id TEXT NOT NULL,
  issued_at TIMESTAMPTZ NOT NULL,
  caller_component TEXT NOT NULL,
  target_kind TEXT NOT NULL,
  target_name TEXT NOT NULL,
  provider TEXT NOT NULL,
  status TEXT NOT NULL,
  declared_effect TEXT,
  declared_audience_id TEXT,
  risk_r INT NOT NULL,
  sensitivity_s INT NOT NULL,
  max_gate INT NOT NULL,
  approval_mode TEXT NOT NULL,
  taint_in_s INT NOT NULL,
  output_ref TEXT,
  output_sensitivity_s INT,
  output_taint_s INT,
  error_code TEXT,
  error_message TEXT,
  retryable BOOLEAN,
  json_payload JSONB NOT NULL,
  audit_trace_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_syscalls_op_time
  ON syscalls(operation_id, issued_at);

CREATE INDEX IF NOT EXISTS idx_syscalls_status
  ON syscalls(status);

CREATE TABLE IF NOT EXISTS syscall_denies (
  syscall_id TEXT PRIMARY KEY REFERENCES syscalls(syscall_id),
  denied_at TIMESTAMPTZ NOT NULL,
  deny_class TEXT NOT NULL,
  violations_json JSONB NOT NULL,
  retry_policy_json JSONB NOT NULL,
  remediation_json JSONB NOT NULL,
  json_payload JSONB NOT NULL,
  audit_trace_id TEXT
);

-- =========================
-- 7) Approvals + OOB
-- =========================
CREATE TABLE IF NOT EXISTS approval_items (
  approval_id TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL REFERENCES operations(operation_id),
  isolation_id TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  approval_mode TEXT NOT NULL,
  status TEXT NOT NULL,
  proposal_bundle_json JSONB NOT NULL,
  diff_payload_json JSONB,
  prompt TEXT NOT NULL,
  expires_at TIMESTAMPTZ,
  audit_trace_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_approval_items_op_status
  ON approval_items(operation_id, status);

CREATE INDEX IF NOT EXISTS idx_approval_items_expires
  ON approval_items(expires_at);

CREATE TABLE IF NOT EXISTS approval_item_syscalls (
  approval_id TEXT NOT NULL REFERENCES approval_items(approval_id),
  syscall_id TEXT NOT NULL REFERENCES syscalls(syscall_id),
  PRIMARY KEY (approval_id, syscall_id)
);

CREATE TABLE IF NOT EXISTS oob_challenges (
  challenge_id TEXT PRIMARY KEY,
  approval_id TEXT NOT NULL REFERENCES approval_items(approval_id),
  challenge_type TEXT NOT NULL,
  nonce_hash TEXT NOT NULL,
  status TEXT NOT NULL,
  issued_at TIMESTAMPTZ NOT NULL,
  verified_at TIMESTAMPTZ,
  consumed_at TIMESTAMPTZ,
  expires_at TIMESTAMPTZ NOT NULL,
  attempts INT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_oob_challenges_approval_status
  ON oob_challenges(approval_id, status);

CREATE INDEX IF NOT EXISTS idx_oob_challenges_expires
  ON oob_challenges(expires_at);

-- =========================
-- 8) IPC artifacts
-- =========================
CREATE TABLE IF NOT EXISTS ipc_artifacts (
  artifact_id TEXT PRIMARY KEY,
  produced_by_operation_id TEXT NOT NULL REFERENCES operations(operation_id),
  produced_at TIMESTAMPTZ NOT NULL,
  kind TEXT NOT NULL,
  content_ref TEXT NOT NULL,
  sensitivity_s INT NOT NULL,
  taint_s INT NOT NULL,
  provenance_refs_json JSONB NOT NULL,
  audience_scope_tag_json JSONB NOT NULL,
  ipc_rules_json JSONB,
  json_payload JSONB NOT NULL,
  audit_trace_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_ipc_artifacts_producer_time
  ON ipc_artifacts(produced_by_operation_id, produced_at);

-- =========================
-- 9) Audit traces
-- =========================
CREATE TABLE IF NOT EXISTS audit_traces (
  audit_trace_id TEXT PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL,
  request_id TEXT NOT NULL,
  operation_id TEXT NOT NULL,
  isolation_id TEXT NOT NULL,
  pinned_json JSONB NOT NULL,
  summary_json JSONB NOT NULL,
  timeline_json JSONB NOT NULL,
  attachments_json JSONB
);

CREATE INDEX IF NOT EXISTS idx_audit_traces_op
  ON audit_traces(operation_id);

-- =========================
-- 10) Capability snapshots
-- =========================
CREATE TABLE IF NOT EXISTS capability_snapshots (
  capability_snapshot_version TEXT PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL,
  parent_version TEXT,
  content_hash TEXT NOT NULL,
  json_payload JSONB NOT NULL,
  notes TEXT
);

CREATE INDEX IF NOT EXISTS idx_capability_snapshots_created
  ON capability_snapshots(created_at);

CREATE INDEX IF NOT EXISTS idx_capability_snapshots_parent
  ON capability_snapshots(parent_version);

-- =========================
-- 11) Schema registry
-- =========================
CREATE TABLE IF NOT EXISTS schema_registry_entries (
  schema_ref TEXT PRIMARY KEY,
  schema_kind TEXT NOT NULL,
  name TEXT NOT NULL,
  semver TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  status TEXT NOT NULL,
  compatibility TEXT NOT NULL,
  payload_json JSONB NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_schema_registry_name_semver
  ON schema_registry_entries(name, semver);

CREATE INDEX IF NOT EXISTS idx_schema_registry_hash
  ON schema_registry_entries(content_hash);

-- =========================
-- 12) Workflow specs and instances
-- =========================
CREATE TABLE IF NOT EXISTS workflow_specs (
  workflow_ref TEXT PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  author TEXT NOT NULL,
  tags_json JSONB NOT NULL,
  content_hash TEXT NOT NULL,
  payload_json JSONB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workflow_specs_name_created
  ON workflow_specs(name, created_at);

CREATE TABLE IF NOT EXISTS workflow_instances (
  workflow_instance_id TEXT PRIMARY KEY,
  workflow_ref TEXT NOT NULL REFERENCES workflow_specs(workflow_ref),
  parent_request_id TEXT,
  parent_operation_id TEXT REFERENCES operations(operation_id),
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  state TEXT NOT NULL,
  state_reason TEXT,
  pinned_active_state_version TEXT NOT NULL,
  pinned_capability_snapshot_version TEXT NOT NULL,
  pinned_audience_graph_version TEXT NOT NULL,
  inputs_json JSONB NOT NULL,
  outputs_json JSONB
);

CREATE INDEX IF NOT EXISTS idx_workflow_instances_ref_state
  ON workflow_instances(workflow_ref, state);

CREATE TABLE IF NOT EXISTS workflow_instance_transitions (
  transition_id TEXT PRIMARY KEY,
  workflow_instance_id TEXT NOT NULL REFERENCES workflow_instances(workflow_instance_id),
  ts TIMESTAMPTZ NOT NULL,
  from_state TEXT,
  to_state TEXT NOT NULL,
  reason TEXT
);

CREATE INDEX IF NOT EXISTS idx_workflow_instance_transitions_instance_ts
  ON workflow_instance_transitions(workflow_instance_id, ts);

CREATE TABLE IF NOT EXISTS workflow_step_states (
  workflow_instance_id TEXT NOT NULL REFERENCES workflow_instances(workflow_instance_id),
  step_id TEXT NOT NULL,
  step_type TEXT NOT NULL,
  state TEXT NOT NULL,
  attempt INT NOT NULL DEFAULT 0,
  operation_id TEXT REFERENCES operations(operation_id),
  approval_id TEXT REFERENCES approval_items(approval_id),
  syscall_id TEXT REFERENCES syscalls(syscall_id),
  updated_at TIMESTAMPTZ NOT NULL,
  PRIMARY KEY(workflow_instance_id, step_id)
);

CREATE INDEX IF NOT EXISTS idx_workflow_step_states_state
  ON workflow_step_states(workflow_instance_id, state);

CREATE TABLE IF NOT EXISTS workflow_step_transitions (
  transition_id TEXT PRIMARY KEY,
  workflow_instance_id TEXT NOT NULL,
  step_id TEXT NOT NULL,
  ts TIMESTAMPTZ NOT NULL,
  from_state TEXT,
  to_state TEXT NOT NULL,
  reason TEXT,
  linked_operation_id TEXT,
  linked_approval_id TEXT,
  linked_syscall_id TEXT,
  FOREIGN KEY(workflow_instance_id, step_id) REFERENCES workflow_step_states(workflow_instance_id, step_id)
);

CREATE INDEX IF NOT EXISTS idx_workflow_step_transitions_instance_ts
  ON workflow_step_transitions(workflow_instance_id, ts);

-- =========================
-- 13) Interface specs and instances
-- =========================
CREATE TABLE IF NOT EXISTS interface_specs (
  interface_ref TEXT PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  author TEXT NOT NULL,
  tags_json JSONB NOT NULL,
  content_hash TEXT NOT NULL,
  payload_json JSONB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_interface_specs_name_created
  ON interface_specs(name, created_at);

CREATE TABLE IF NOT EXISTS interface_instances (
  interface_instance_id TEXT PRIMARY KEY,
  interface_ref TEXT NOT NULL REFERENCES interface_specs(interface_ref),
  operation_id TEXT REFERENCES operations(operation_id),
  workflow_instance_id TEXT REFERENCES workflow_instances(workflow_instance_id),
  created_at TIMESTAMPTZ NOT NULL,
  viewer_audience_id TEXT NOT NULL,
  pinned_active_state_version TEXT NOT NULL,
  pinned_capability_snapshot_version TEXT NOT NULL,
  pinned_audience_graph_version TEXT NOT NULL,
  gate_summary_json JSONB NOT NULL,
  blocks_json JSONB NOT NULL,
  bindings_json JSONB NOT NULL,
  taint_summary_json JSONB NOT NULL,
  state TEXT NOT NULL,
  CHECK (
    (operation_id IS NOT NULL AND workflow_instance_id IS NULL) OR
    (operation_id IS NULL AND workflow_instance_id IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS idx_interface_instances_operation
  ON interface_instances(operation_id);

CREATE INDEX IF NOT EXISTS idx_interface_instances_workflow
  ON interface_instances(workflow_instance_id);

-- =========================
-- 14) Audience Graph tables (versioned)
-- =========================
CREATE TABLE IF NOT EXISTS audience_graph_nodes (
  node_id TEXT NOT NULL,
  graph_version TEXT NOT NULL,
  node_type TEXT NOT NULL,
  label TEXT,
  props_json JSONB NOT NULL,
  PRIMARY KEY(node_id, graph_version)
);

CREATE INDEX IF NOT EXISTS idx_audience_nodes_version
  ON audience_graph_nodes(graph_version);

CREATE TABLE IF NOT EXISTS audience_graph_edges (
  edge_id TEXT NOT NULL,
  graph_version TEXT NOT NULL,
  src_id TEXT NOT NULL,
  dst_id TEXT NOT NULL,
  edge_type TEXT NOT NULL,
  props_json JSONB NOT NULL,
  PRIMARY KEY(edge_id, graph_version)
);

CREATE INDEX IF NOT EXISTS idx_audience_edges_version
  ON audience_graph_edges(graph_version);

CREATE INDEX IF NOT EXISTS idx_audience_edges_src_dst
  ON audience_graph_edges(src_id, dst_id);

CREATE TABLE IF NOT EXISTS audience_graph_scopes (
  scope_id TEXT NOT NULL,
  graph_version TEXT NOT NULL,
  src_id TEXT NOT NULL,
  dst_id TEXT NOT NULL,
  allowed_scopes_json JSONB NOT NULL,
  sensitivity_ceiling_s INT NOT NULL,
  PRIMARY KEY(scope_id, graph_version)
);

CREATE INDEX IF NOT EXISTS idx_audience_scopes_version
  ON audience_graph_scopes(graph_version);

-- =========================
-- 15) Review queue
-- =========================
CREATE TABLE IF NOT EXISTS review_queue_items (
  item_id TEXT PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL,
  status TEXT NOT NULL,
  summary TEXT NOT NULL,
  proposed_change_json JSONB NOT NULL,
  evidence_refs_json JSONB NOT NULL,
  risk_r INT NOT NULL,
  sensitivity_s INT NOT NULL,
  requires_owner_confirmation BOOLEAN NOT NULL,
  audit_trace_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_review_queue_status
  ON review_queue_items(status);

CREATE TABLE IF NOT EXISTS review_queue_decisions (
  decision_id TEXT PRIMARY KEY,
  item_id TEXT NOT NULL REFERENCES review_queue_items(item_id),
  decided_at TIMESTAMPTZ NOT NULL,
  decision TEXT NOT NULL,
  edited_payload_json JSONB,
  change_id TEXT,
  new_state_version TEXT,
  audit_trace_id TEXT
);

-- =========================
-- 16) Idempotency keys
-- =========================
CREATE TABLE IF NOT EXISTS idempotency_keys (
  endpoint_scope TEXT NOT NULL,
  idempotency_key TEXT NOT NULL,
  request_id TEXT NOT NULL,
  response_json JSONB NOT NULL,
  response_hash TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  expires_at TIMESTAMPTZ,
  PRIMARY KEY (endpoint_scope, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_idempotency_keys_expires
  ON idempotency_keys(expires_at);

-- =========================
-- 17) Fact Ledger claims
-- =========================
CREATE TABLE IF NOT EXISTS episodes (
  episode_id TEXT PRIMARY KEY,
  scope_type TEXT NOT NULL,
  scope_key TEXT NOT NULL,
  task_scope_key TEXT,
  workspace_json JSONB NOT NULL,
  workspace_resolution_basis_json JSONB NOT NULL,
  workspace_resolution_confidence DOUBLE PRECISION NOT NULL,
  branch TEXT,
  task_prompt TEXT NOT NULL,
  summary TEXT NOT NULL,
  files_touched_json JSONB NOT NULL,
  decisions_json JSONB NOT NULL,
  unresolved_items_json JSONB NOT NULL,
  tests_json JSONB NOT NULL,
  observed_preferences_json JSONB NOT NULL,
  risk_signals_json JSONB NOT NULL,
  issue_refs_json JSONB NOT NULL,
  artifact_refs_json JSONB NOT NULL,
  started_at TIMESTAMPTZ NOT NULL,
  ended_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_episodes_scope_time
  ON episodes(scope_type, scope_key, ended_at);

CREATE INDEX IF NOT EXISTS idx_episodes_task_scope_time
  ON episodes(task_scope_key, ended_at);

CREATE TABLE IF NOT EXISTS episode_artifacts (
  episode_id TEXT NOT NULL REFERENCES episodes(episode_id),
  artifact_ref TEXT NOT NULL,
  PRIMARY KEY(episode_id, artifact_ref)
);

CREATE TABLE IF NOT EXISTS search_documents (
  doc_id TEXT PRIMARY KEY,
  scope_type TEXT NOT NULL,
  scope_key TEXT NOT NULL,
  source_type TEXT NOT NULL,
  source_ref TEXT NOT NULL,
  title TEXT,
  body_text TEXT NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_search_documents_scope_updated
  ON search_documents(scope_type, scope_key, updated_at);

CREATE TABLE IF NOT EXISTS claims (
  claim_id TEXT PRIMARY KEY,
  claim_type TEXT NOT NULL,
  claim_key TEXT NOT NULL,
  scope_type TEXT NOT NULL,
  scope_key TEXT NOT NULL,
  subject_key TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  created_by TEXT NOT NULL,
  confidence DOUBLE PRECISION NOT NULL,
  value_json JSONB NOT NULL,
  context_predicates_json JSONB NOT NULL,
  time_start TIMESTAMPTZ,
  time_end TIMESTAMPTZ,
  evidence_quality_json JSONB NOT NULL,
  promotion_ref TEXT
);

CREATE INDEX IF NOT EXISTS idx_claims_key_status
  ON claims(claim_key, status);

CREATE INDEX IF NOT EXISTS idx_claims_type_status
  ON claims(claim_type, status);

CREATE INDEX IF NOT EXISTS idx_claims_scope_status
  ON claims(scope_type, scope_key, status);

CREATE INDEX IF NOT EXISTS idx_claims_scope_subject_status
  ON claims(scope_type, scope_key, subject_key, status);

CREATE TABLE IF NOT EXISTS claim_evidence (
  claim_id TEXT NOT NULL REFERENCES claims(claim_id),
  evidence_ref TEXT NOT NULL,
  evidence_kind TEXT NOT NULL,
  locator_json JSONB,
  PRIMARY KEY(claim_id, evidence_ref)
);

CREATE TABLE IF NOT EXISTS claim_conflicts (
  claim_id TEXT NOT NULL REFERENCES claims(claim_id),
  conflicting_claim_id TEXT NOT NULL REFERENCES claims(claim_id),
  reason_code TEXT NOT NULL,
  detected_at TIMESTAMPTZ NOT NULL,
  PRIMARY KEY(claim_id, conflicting_claim_id)
);

-- =========================
-- 18) Ingest jobs
-- =========================
CREATE TABLE IF NOT EXISTS ingest_jobs (
  job_id TEXT PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  status TEXT NOT NULL,
  source_count INT NOT NULL,
  artifacts_total INT NOT NULL,
  artifacts_succeeded INT NOT NULL,
  artifacts_failed INT NOT NULL,
  bytes_ingested BIGINT NOT NULL,
  options_json JSONB NOT NULL,
  error_summary TEXT
);

CREATE INDEX IF NOT EXISTS idx_ingest_jobs_status_created
  ON ingest_jobs(status, created_at);

CREATE TABLE IF NOT EXISTS ingest_job_items (
  job_id TEXT NOT NULL REFERENCES ingest_jobs(job_id),
  item_key TEXT NOT NULL,
  status TEXT NOT NULL,
  artifact_id TEXT,
  error_json JSONB,
  updated_at TIMESTAMPTZ NOT NULL,
  PRIMARY KEY(job_id, item_key)
);

CREATE INDEX IF NOT EXISTS idx_ingest_job_items_status
  ON ingest_job_items(job_id, status);

-- =========================
-- 19) Artifact registry
-- =========================
CREATE TABLE IF NOT EXISTS artifacts (
  artifact_id TEXT PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL,
  ingest_job_id TEXT REFERENCES ingest_jobs(job_id),
  kind TEXT NOT NULL,
  content_ref TEXT NOT NULL REFERENCES blob_objects(content_ref),
  parent_artifact_id TEXT REFERENCES artifacts(artifact_id),
  dedupe_key TEXT,
  meta_json JSONB NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_artifacts_dedupe
  ON artifacts(dedupe_key);

CREATE INDEX IF NOT EXISTS idx_artifacts_job_created
  ON artifacts(ingest_job_id, created_at);

-- =========================
-- 20) Jobs queue
-- =========================
CREATE TABLE IF NOT EXISTS jobs (
  job_id TEXT PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  run_after TIMESTAMPTZ,
  leased_until TIMESTAMPTZ,
  lease_owner TEXT,
  lease_epoch INT NOT NULL,
  status TEXT NOT NULL,
  attempt_count INT NOT NULL,
  max_attempts INT NOT NULL,
  job_type TEXT NOT NULL,
  dedupe_key TEXT,
  payload_json JSONB NOT NULL,
  sensitivity_s INT NOT NULL,
  taint_s INT NOT NULL,
  provenance_refs_json JSONB NOT NULL,
  last_error_code TEXT,
  last_error_message TEXT,
  completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_jobs_status_run_after
  ON jobs(status, run_after);

CREATE UNIQUE INDEX IF NOT EXISTS idx_jobs_dedupe_active
  ON jobs(dedupe_key, status);

-- =========================
-- 21) Blob metadata
-- =========================
CREATE TABLE IF NOT EXISTS blob_objects (
  content_ref TEXT PRIMARY KEY,
  created_at TIMESTAMPTZ NOT NULL,
  content_type TEXT,
  size_bytes BIGINT NOT NULL,
  checksum TEXT,
  sensitivity_s INT NOT NULL,
  taint_s INT NOT NULL,
  provenance_refs_json JSONB NOT NULL,
  storage_backend TEXT NOT NULL,
  storage_path TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_blob_created
  ON blob_objects(created_at);
```

---

# Notes: how this supports swappable backends

* SQLite and Postgres store the same logical objects with the same IDs. Providers can swap without changing kernel semantics.
* Graph support is relational everywhere; Apache AGE can be layered later for traversal queries without affecting correctness.
* Job queue is DB-backed first; can be swapped with Redis/Kafka later via `JobQueue` interface.
* Blob content is not stored inline; `content_ref` abstracts FS vs S3.
