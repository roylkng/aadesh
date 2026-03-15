CREATE TABLE IF NOT EXISTS blob_objects (
  content_ref TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  content_type TEXT,
  size_bytes INTEGER NOT NULL,
  checksum TEXT,
  sensitivity_s INTEGER NOT NULL,
  taint_s INTEGER NOT NULL,
  provenance_refs_json TEXT NOT NULL,
  storage_backend TEXT NOT NULL,
  storage_path TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS episodes (
  episode_id TEXT PRIMARY KEY,
  event_ref TEXT NOT NULL UNIQUE,
  scope_type TEXT NOT NULL,
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
  source_type TEXT NOT NULL,
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
  scope_type TEXT NOT NULL,
  scope_key TEXT NOT NULL,
  subject_key TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  created_by TEXT NOT NULL,
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
  evidence_kind TEXT NOT NULL,
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
