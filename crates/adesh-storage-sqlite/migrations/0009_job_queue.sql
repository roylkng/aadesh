CREATE TABLE IF NOT EXISTS jobs (
  job_id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  run_after TEXT,
  leased_until TEXT,
  lease_owner TEXT,
  lease_epoch INTEGER NOT NULL,
  status TEXT NOT NULL,
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

CREATE INDEX IF NOT EXISTS idx_jobs_type_status_run_after
  ON jobs(job_type, status, run_after);

CREATE INDEX IF NOT EXISTS idx_jobs_dedupe_status
  ON jobs(dedupe_key, status);
