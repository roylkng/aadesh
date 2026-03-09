CREATE TABLE IF NOT EXISTS review_queue_items (
  item_id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  status TEXT NOT NULL,
  source TEXT NOT NULL,
  target_domain TEXT NOT NULL,
  risk_r_estimate INTEGER NOT NULL,
  sensitivity_s_estimate INTEGER NOT NULL,
  requires_oob INTEGER NOT NULL,
  proposal_json TEXT NOT NULL,
  evidence_json TEXT NOT NULL,
  impact_json TEXT NOT NULL,
  base_version TEXT,
  resolved_version TEXT
);

CREATE INDEX IF NOT EXISTS idx_review_queue_items_status_created
  ON review_queue_items(status, created_at);

CREATE TABLE IF NOT EXISTS review_queue_decisions (
  decision_id TEXT PRIMARY KEY,
  item_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  decision TEXT NOT NULL,
  edited_payload_json TEXT,
  applied_version TEXT,
  FOREIGN KEY(item_id) REFERENCES review_queue_items(item_id)
);

CREATE INDEX IF NOT EXISTS idx_review_queue_decisions_item
  ON review_queue_decisions(item_id, created_at);
