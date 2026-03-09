PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS current_versions (
  version_kind TEXT PRIMARY KEY,
  version_id TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

INSERT OR IGNORE INTO current_versions (version_kind, version_id, updated_at) VALUES
  ('active_state', 'state:bootstrap', CURRENT_TIMESTAMP),
  ('audience_graph', 'aud:bootstrap', CURRENT_TIMESTAMP),
  ('capability_snapshot', 'cap:bootstrap', CURRENT_TIMESTAMP);

CREATE TABLE IF NOT EXISTS experience_events (
  event_ref TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  source_class TEXT NOT NULL,
  author TEXT,
  audience_id TEXT,
  sensitivity_s INTEGER NOT NULL,
  taint_s INTEGER NOT NULL,
  kind TEXT NOT NULL,
  content_ref TEXT,
  json_payload TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_experience_events_created
  ON experience_events(created_at);

CREATE TABLE IF NOT EXISTS operations (
  operation_id TEXT PRIMARY KEY,
  parent_request_id TEXT NOT NULL,
  isolation_id TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  state TEXT NOT NULL,
  state_reason TEXT,
  requesting_audience_id TEXT NOT NULL,
  pinned_active_state_version TEXT,
  pinned_capability_snapshot_version TEXT,
  pinned_audience_graph_version TEXT,
  budgets_json TEXT NOT NULL,
  operation_goal_json TEXT NOT NULL,
  ipc_json TEXT
);

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

CREATE TABLE IF NOT EXISTS gate_decisions (
  gate_decision_id TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL UNIQUE,
  isolation_id TEXT NOT NULL,
  evaluated_at TEXT NOT NULL,
  active_state_version TEXT NOT NULL,
  capability_snapshot_version TEXT NOT NULL,
  audience_graph_version TEXT NOT NULL,
  risk_r INTEGER NOT NULL,
  sensitivity_s INTEGER NOT NULL,
  max_gate INTEGER NOT NULL,
  approval_mode TEXT NOT NULL,
  requesting_audience_id TEXT NOT NULL,
  scopes_allowed_json TEXT NOT NULL,
  scopes_denied_json TEXT NOT NULL,
  sensitivity_ceiling_s INTEGER NOT NULL,
  predicates_json TEXT NOT NULL,
  constraints_json TEXT NOT NULL,
  json_payload TEXT NOT NULL,
  audit_trace_id TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_gate_decisions_op
  ON gate_decisions(operation_id);

CREATE TABLE IF NOT EXISTS compiled_slices (
  compiled_slice_id TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL UNIQUE,
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
  did_omit INTEGER NOT NULL,
  omissions_json TEXT NOT NULL,
  provenance_summary_json TEXT NOT NULL,
  intent_anchor_json TEXT NOT NULL,
  blocks_json TEXT NOT NULL,
  json_payload TEXT NOT NULL,
  audit_trace_id TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_compiled_slices_op
  ON compiled_slices(operation_id);

CREATE TABLE IF NOT EXISTS approval_items (
  approval_id TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  status TEXT NOT NULL,
  approval_mode TEXT NOT NULL,
  proposal_bundle_json TEXT NOT NULL,
  diff_payload_json TEXT NOT NULL,
  prompt TEXT NOT NULL,
  expires_at TEXT,
  audit_trace_id TEXT NOT NULL,
  FOREIGN KEY(operation_id) REFERENCES operations(operation_id)
);

CREATE INDEX IF NOT EXISTS idx_approval_items_op_status
  ON approval_items(operation_id, status);

CREATE TABLE IF NOT EXISTS syscalls (
  syscall_id TEXT PRIMARY KEY,
  operation_id TEXT NOT NULL,
  approval_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  tool_name TEXT NOT NULL,
  action_name TEXT NOT NULL,
  args_schema_ref TEXT NOT NULL,
  result_schema_ref TEXT,
  status TEXT NOT NULL,
  args_json TEXT NOT NULL,
  result_ref TEXT,
  audit_trace_id TEXT NOT NULL,
  FOREIGN KEY(operation_id) REFERENCES operations(operation_id),
  FOREIGN KEY(approval_id) REFERENCES approval_items(approval_id),
  UNIQUE(approval_id, tool_name, action_name)
);

CREATE INDEX IF NOT EXISTS idx_syscalls_operation_created
  ON syscalls(operation_id, created_at);

CREATE INDEX IF NOT EXISTS idx_syscalls_status
  ON syscalls(status);

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
