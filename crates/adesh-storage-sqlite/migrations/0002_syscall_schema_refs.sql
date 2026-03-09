CREATE TABLE syscalls_v2 (
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

INSERT INTO syscalls_v2 (
  syscall_id,
  operation_id,
  approval_id,
  created_at,
  updated_at,
  tool_name,
  action_name,
  args_schema_ref,
  result_schema_ref,
  status,
  args_json,
  result_ref,
  audit_trace_id
)
SELECT
  syscall_id,
  operation_id,
  approval_id,
  created_at,
  updated_at,
  tool_name,
  action_name,
  COALESCE(args_schema_ref, CASE
    WHEN tool_name = 'email' AND action_name = 'send' THEN 'schema:sha256:adesh-email-send-payload-v0_1'
    ELSE 'schema:sha256:unknown'
  END),
  COALESCE(result_schema_ref, CASE
    WHEN tool_name = 'email' AND action_name = 'send' THEN 'schema:sha256:adesh-email-send-result-v0_1'
    ELSE NULL
  END),
  status,
  args_json,
  result_ref,
  audit_trace_id
FROM syscalls;

DROP TABLE syscalls;
ALTER TABLE syscalls_v2 RENAME TO syscalls;

CREATE INDEX IF NOT EXISTS idx_syscalls_operation_created
  ON syscalls(operation_id, created_at);

CREATE INDEX IF NOT EXISTS idx_syscalls_status
  ON syscalls(status);
