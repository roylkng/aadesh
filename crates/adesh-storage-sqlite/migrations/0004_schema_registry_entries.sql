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
