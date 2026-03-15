CREATE TABLE IF NOT EXISTS ingest_jobs (
  job_id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  status TEXT NOT NULL,
  source_count INTEGER NOT NULL,
  artifacts_total INTEGER NOT NULL,
  artifacts_succeeded INTEGER NOT NULL,
  artifacts_failed INTEGER NOT NULL,
  bytes_ingested INTEGER NOT NULL,
  options_json TEXT NOT NULL,
  error_summary TEXT
);

CREATE INDEX IF NOT EXISTS idx_ingest_jobs_status_created
  ON ingest_jobs(status, created_at);

CREATE TABLE IF NOT EXISTS ingest_job_items (
  job_id TEXT NOT NULL,
  item_key TEXT NOT NULL,
  status TEXT NOT NULL,
  artifact_id TEXT,
  error_json TEXT,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(job_id, item_key),
  FOREIGN KEY(job_id) REFERENCES ingest_jobs(job_id)
);

CREATE INDEX IF NOT EXISTS idx_ingest_job_items_status
  ON ingest_job_items(job_id, status);

CREATE TABLE IF NOT EXISTS artifacts (
  artifact_id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  ingest_job_id TEXT,
  kind TEXT NOT NULL,
  content_ref TEXT NOT NULL,
  parent_artifact_id TEXT,
  dedupe_key TEXT,
  meta_json TEXT NOT NULL,
  FOREIGN KEY(ingest_job_id) REFERENCES ingest_jobs(job_id),
  FOREIGN KEY(parent_artifact_id) REFERENCES artifacts(artifact_id),
  FOREIGN KEY(content_ref) REFERENCES blob_objects(content_ref)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_artifacts_dedupe
  ON artifacts(dedupe_key);

CREATE INDEX IF NOT EXISTS idx_artifacts_job_created
  ON artifacts(ingest_job_id, created_at);
