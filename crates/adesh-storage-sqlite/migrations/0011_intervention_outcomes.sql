CREATE TABLE IF NOT EXISTS intervention_context (
  context_id TEXT PRIMARY KEY,
  scope_type TEXT NOT NULL,
  scope_key TEXT NOT NULL,
  task_prompt TEXT NOT NULL,
  prepared_at TEXT NOT NULL,
  host_agent_id TEXT,
  host_agent_kind TEXT,
  host_model TEXT,
  selected_direction TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_intervention_context_scope_time
  ON intervention_context(scope_type, scope_key, prepared_at);

CREATE TABLE IF NOT EXISTS intervention_outcomes (
  intervention_id TEXT PRIMARY KEY,
  episode_id TEXT,
  surfaced_direction TEXT NOT NULL,
  context_ref TEXT,
  surfaced_at TEXT NOT NULL,
  selected_response TEXT NOT NULL CHECK (selected_response IN ('accepted', 'ignored', 'modified')),
  modified_payload TEXT,
  outcome_ref TEXT,
  correction_summary TEXT,
  learn_from_this BOOLEAN NOT NULL DEFAULT FALSE,
  idempotency_key TEXT UNIQUE,
  created_at TEXT NOT NULL,
  FOREIGN KEY(context_ref) REFERENCES intervention_context(context_id)
);

CREATE INDEX IF NOT EXISTS idx_intervention_outcomes_episode
  ON intervention_outcomes(episode_id);

CREATE INDEX IF NOT EXISTS idx_intervention_outcomes_context
  ON intervention_outcomes(context_ref);

CREATE INDEX IF NOT EXISTS idx_intervention_outcomes_learnability
  ON intervention_outcomes(learn_from_this)
  WHERE learn_from_this = TRUE;