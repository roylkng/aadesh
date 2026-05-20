ALTER TABLE intervention_context
  ADD COLUMN selected_direction_rank INTEGER;

ALTER TABLE intervention_context
  ADD COLUMN surfaced_directions_json TEXT;
