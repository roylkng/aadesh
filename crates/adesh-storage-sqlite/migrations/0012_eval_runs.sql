CREATE TABLE IF NOT EXISTS eval_runs (
    run_id TEXT PRIMARY KEY,
    eval_name TEXT NOT NULL,
    eval_version TEXT,
    run_started_at TEXT NOT NULL,
    run_completed_at TEXT,
    baseline_summary TEXT,
    treatment_summary TEXT,
    judge_summary TEXT,
    failure_tags TEXT,
    promotion_decision TEXT CHECK (promotion_decision IN ('promote', 'reject', 'hold')),
    idempotency_key TEXT UNIQUE,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_eval_runs_name_version
  ON eval_runs(eval_name, eval_version);

CREATE INDEX IF NOT EXISTS idx_eval_runs_promotion
  ON eval_runs(promotion_decision)
  WHERE promotion_decision IS NOT NULL;

CREATE TABLE IF NOT EXISTS eval_artifacts (
    artifact_id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    artifact_kind TEXT NOT NULL CHECK (artifact_kind IN ('transcript', 'output', 'metrics', 'context')),
    file_path TEXT NOT NULL,
    mime_type TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY(run_id) REFERENCES eval_runs(run_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_eval_artifacts_run
  ON eval_artifacts(run_id);