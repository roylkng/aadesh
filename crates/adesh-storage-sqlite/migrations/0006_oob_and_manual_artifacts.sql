CREATE TABLE IF NOT EXISTS oob_challenges (
  challenge_id TEXT PRIMARY KEY,
  approval_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  status TEXT NOT NULL,
  verified_at TEXT,
  consumed_at TEXT,
  response_json TEXT,
  FOREIGN KEY(approval_id) REFERENCES approval_items(approval_id)
);

CREATE INDEX IF NOT EXISTS idx_oob_challenges_approval_status
  ON oob_challenges(approval_id, status);

CREATE INDEX IF NOT EXISTS idx_oob_challenges_expires
  ON oob_challenges(expires_at);

CREATE TABLE IF NOT EXISTS manual_artifacts (
  artifact_id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  filename TEXT NOT NULL,
  media_type TEXT NOT NULL,
  content_base64 TEXT NOT NULL,
  text_preview TEXT,
  byte_size INTEGER NOT NULL,
  sensitivity_hint INTEGER
);

CREATE INDEX IF NOT EXISTS idx_manual_artifacts_created
  ON manual_artifacts(created_at);
