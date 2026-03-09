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
