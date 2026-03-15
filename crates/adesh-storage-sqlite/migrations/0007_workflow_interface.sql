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
  state TEXT NOT NULL,
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
  step_type TEXT NOT NULL,
  state TEXT NOT NULL,
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
  state TEXT NOT NULL,
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
