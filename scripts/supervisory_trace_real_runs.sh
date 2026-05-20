#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required" >&2
  exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required" >&2
  exit 1
fi
if ! command -v sqlite3 >/dev/null 2>&1; then
  echo "sqlite3 is required" >&2
  exit 1
fi
if ! command -v qwen >/dev/null 2>&1; then
  echo "qwen is required for this real-host observability run" >&2
  exit 1
fi

ADESH_ROOT="${ADESH_DAEMON_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
if [[ ! -f "${ADESH_ROOT}/Cargo.toml" ]]; then
  echo "ADESH_DAEMON_ROOT does not point to an Aadesh repo: ${ADESH_ROOT}" >&2
  exit 1
fi

run_id="$(date +%s)"
DB_URL="${ADESH_DATABASE_URL:-sqlite:///tmp/adesh-supervisory-trace-${run_id}.db?mode=rwc}"
DB_PATH="${DB_URL#sqlite://}"
DB_PATH="${DB_PATH%%\?*}"
WORKSPACE_LOCATOR="${SUPERVISORY_WORKSPACE_LOCATOR:-workspace://real-host-supervisory-trace}"
CONNECTOR_ID="${SUPERVISORY_CONNECTOR_ID:-qwen-cli}"
CONNECTOR_KIND="${SUPERVISORY_CONNECTOR_KIND:-cli_wrapper}"
HOST_AGENT_ID="${SUPERVISORY_HOST_AGENT_ID:-qwen-local-user}"
HOST_AGENT_KIND="${SUPERVISORY_HOST_AGENT_KIND:-qwen-cli}"
HOST_MODEL="${SUPERVISORY_HOST_MODEL:-qwen3-coder-plus}"
REPORT_PATH="${SUPERVISORY_REPORT_PATH:-/tmp/adesh-supervisory-trace-report-${run_id}.json}"
QWEN_TIMEOUT_SECONDS="${QWEN_TIMEOUT_SECONDS:-35}"

run_connector_event() {
  local payload="$1"
  ADESH_DATABASE_URL="$DB_URL" cargo run -q --manifest-path "${ADESH_ROOT}/Cargo.toml" -p adesh-daemon -- host connector --json "$payload"
}

run_episode() {
  local episode_idx="$1"
  local task_prompt="$2"
  local task_hint="$3"
  local end_summary="$4"
  local unresolved_item="$5"
  local risk_signal="$6"
  local outcome="$7"
  local correction_summary="$8"

  local session_id="session-${run_id}-${episode_idx}"

  local start_payload
  start_payload="$(jq -n \
    --arg connector_id "$CONNECTOR_ID" \
    --arg connector_kind "$CONNECTOR_KIND" \
    --arg connector_version "0.1.0" \
    --arg session_id "$session_id" \
    --arg host_agent_id "$HOST_AGENT_ID" \
    --arg host_agent_kind "$HOST_AGENT_KIND" \
    --arg host_model "$HOST_MODEL" \
    --arg workspace_locator "$WORKSPACE_LOCATOR" \
    --arg task_prompt "$task_prompt" \
    --arg task_hint "$task_hint" \
    '{
      connector_id: $connector_id,
      connector_kind: $connector_kind,
      connector_version: $connector_version,
      session_id: $session_id,
      host_agent_id: $host_agent_id,
      host_agent_kind: $host_agent_kind,
      host_model: $host_model,
      event_kind: "task_start",
      workspace: {
        kind: "task_space",
        locator: $workspace_locator,
        cwd: null,
        branch: null,
        external_ref: null
      },
      task_prompt: $task_prompt,
      files_in_focus: ["crates/adesh-daemon/src/cognition.rs"],
      task_hint: $task_hint
    }')"

  local start_response
  start_response="$(run_connector_event "$start_payload")"
  local context_id
  context_id="$(printf '%s' "$start_response" | jq -r '.context_id // empty')"
  local top_direction
  top_direction="$(printf '%s' "$start_response" | jq -r '.prepare_context.likely_next_directions[0].statement // empty')"
  if [[ -z "$top_direction" ]]; then
    top_direction="No ranked direction returned; proceed with direct task decomposition"
  fi

  local qwen_prompt
  qwen_prompt="$(printf 'Task: %s\nAadesh top suggested direction: %s\nGive a concise 2-step coding plan.' "$task_prompt" "$top_direction")"
  timeout "${QWEN_TIMEOUT_SECONDS}s" qwen --prompt "$qwen_prompt" -o json >/dev/null

  local end_payload
  end_payload="$(jq -n \
    --arg connector_id "$CONNECTOR_ID" \
    --arg connector_kind "$CONNECTOR_KIND" \
    --arg connector_version "0.1.0" \
    --arg session_id "$session_id" \
    --arg host_agent_id "$HOST_AGENT_ID" \
    --arg host_agent_kind "$HOST_AGENT_KIND" \
    --arg host_model "$HOST_MODEL" \
    --arg context_id "$context_id" \
    --arg selected_next_direction "$top_direction" \
    --arg outcome "$outcome" \
    --arg correction_summary "$correction_summary" \
    --arg workspace_locator "$WORKSPACE_LOCATOR" \
    --arg task_prompt "$task_prompt" \
    --arg task_hint "$task_hint" \
    --arg summary "$end_summary" \
    --arg unresolved "$unresolved_item" \
    --arg risk "$risk_signal" \
    '{
      connector_id: $connector_id,
      connector_kind: $connector_kind,
      connector_version: $connector_version,
      session_id: $session_id,
      host_agent_id: $host_agent_id,
      host_agent_kind: $host_agent_kind,
      host_model: $host_model,
      context_id: (if ($context_id | length) > 0 then $context_id else null end),
      selected_next_direction: $selected_next_direction,
      outcome: $outcome,
      correction_summary: (if ($correction_summary | length) > 0 then $correction_summary else null end),
      event_kind: "task_end",
      workspace: {
        kind: "task_space",
        locator: $workspace_locator,
        cwd: null,
        branch: null,
        external_ref: null
      },
      task_prompt: $task_prompt,
      files_touched: ["crates/adesh-daemon/src/cognition.rs", "crates/adesh-daemon/src/connector_adapter.rs"],
      task_hint: $task_hint,
      summary: $summary,
      decisions: [
        {
          decision: "Keep cognition core advisory-only for v0",
          rationale: "Observability pass should not introduce controller behavior"
        }
      ],
      unresolved_items: [$unresolved],
      risk_signals: [$risk],
      tests: [
        {
          name: "cargo test -p adesh-daemon --lib",
          status: "pass",
          summary: "Core cognition + connector tests remain green"
        }
      ]
    }')"

  local end_response
  end_response="$(run_connector_event "$end_payload")"
  printf '%s\n' "$end_response" | jq -r --arg idx "$episode_idx" --arg direction "$top_direction" --arg context_id "$context_id" '
    "episode \($idx): handled_as=\(.handled_as) episode_id=\(.stored_episode.episode_id) context_id=\($context_id) outcome_direction=\($direction)"
  '
}

echo "Running real-host supervisory trace observability flow..."
echo "DB_URL=${DB_URL}"

run_episode \
  "1" \
  "Validate whether next-direction ranking improves continuity quality for coding tasks." \
  "validation-proof" \
  "Accepted the suggested validation-first direction and executed proof checks before cleanup." \
  "Need to compare baseline vs treatment acceptance rates in harness output" \
  "Without acceptance metrics, improvements can be anecdotal" \
  "accepted" \
  ""

run_episode \
  "2" \
  "Polish docs and examples for connector usage clarity." \
  "docs-polish" \
  "Ignored the suggestion and did formatting cleanup first, then had to revisit missing examples." \
  "Still need one concrete accepted-vs-ignored example in docs" \
  "Cosmetic cleanup can hide missing behavioral evidence" \
  "ignored" \
  "Chose local cleanup over suggested evidence-focused update; required follow-up."

run_episode \
  "3" \
  "Run a discriminating quality check to verify guidance relevance under sparse payloads." \
  "quality-check" \
  "Accepted suggested focus on unresolved validation loops and produced targeted checks." \
  "Need broader host-side sample size beyond single connector" \
  "Sparse payload quality can regress without repeated checks" \
  "accepted" \
  ""

run_episode \
  "4" \
  "Prepare a quick wrap-up note for stakeholders on project status." \
  "status-note" \
  "Ignored suggested proof-first path and drafted status first; had to add missing metrics afterward." \
  "Need explicit metrics table in wrap-up output" \
  "Status updates without metrics can misstate confidence" \
  "ignored" \
  "Prioritized summary drafting before evidence extraction."

episodes_sql="
SELECT
  e.episode_id,
  e.ended_at,
  e.task_prompt,
  e.summary,
  c.context_id,
  c.scope_key,
  c.host_agent_id,
  c.host_agent_kind,
  c.host_model,
  c.selected_direction,
  c.selected_direction_rank,
  o.selected_response AS outcome,
  o.correction_summary,
  o.learn_from_this
FROM intervention_outcomes o
LEFT JOIN intervention_context c ON c.context_id = o.context_ref
LEFT JOIN episodes e ON e.episode_id = o.episode_id
WHERE c.scope_key = 'workspace:task_space:${WORKSPACE_LOCATOR}'
   OR json_extract(e.workspace_json, '\$.locator') = '${WORKSPACE_LOCATOR}'
ORDER BY o.created_at;
"

episodes_json="$(sqlite3 -json "$DB_PATH" "$episodes_sql")"

outcomes_json="$(printf '%s' "$episodes_json" | jq '
  group_by(.outcome) | map({
    outcome: (.[0].outcome // "missing"),
    count: length
  }) | sort_by(.count) | reverse
')"

direction_outcome_json="$(printf '%s' "$episodes_json" | jq '
  group_by({selected_direction, outcome}) | map({
    selected_direction: (.[0].selected_direction // "missing"),
    outcome: (.[0].outcome // "missing"),
    count: length
  }) | sort_by(.count) | reverse
')"

field_stats_json="$(printf '%s' "$episodes_json" | jq '
  def field_stat($field):
    {
      field: $field,
      present_count: (map(select(.[$field] != null and .[$field] != "")) | length),
      distinct_count: (map(.[$field]) | map(select(. != null and . != "")) | unique | length)
    };
  [
    field_stat("host_agent_id"),
    field_stat("host_agent_kind"),
    field_stat("host_model"),
    field_stat("context_id"),
    field_stat("selected_direction"),
    field_stat("outcome"),
    field_stat("learn_from_this"),
    field_stat("correction_summary")
  ]
')"

report_json="$(jq -n \
  --arg db_url "$DB_URL" \
  --arg db_path "$DB_PATH" \
  --arg workspace_locator "$WORKSPACE_LOCATOR" \
  --arg connector_id "$CONNECTOR_ID" \
  --arg host_agent_kind "$HOST_AGENT_KIND" \
  --arg host_model "$HOST_MODEL" \
  --argjson episodes "$episodes_json" \
  --argjson outcomes "$outcomes_json" \
  --argjson direction_outcome "$direction_outcome_json" \
  --argjson field_stats "$field_stats_json" \
  '
  def classify($s; $total):
    if $s.field == "selected_direction" or $s.field == "outcome" then
      "useful_core_signal"
    elif $s.field == "context_id" and $s.present_count == $total then
      "useful_linking_signal"
    elif $s.distinct_count <= 1 then
      "likely_noise_or_low_signal_for_single-host-run"
    elif $s.present_count < ($total / 2) then
      "conditional_signal"
    else
      "potentially_useful"
    end;
  {
    metadata: {
      db_url: $db_url,
      db_path: $db_path,
      workspace_locator: $workspace_locator,
      connector_id: $connector_id,
      host_agent_kind: $host_agent_kind,
      host_model: $host_model
    },
    totals: {
      episodes: ($episodes | length),
      accepted: ($episodes | map(select(.outcome == "accepted")) | length),
      ignored: ($episodes | map(select(.outcome == "ignored")) | length)
    },
    episodes: $episodes,
    outcome_breakdown: $outcomes,
    direction_outcome_correlation: $direction_outcome,
    field_signal_assessment: (
      ($episodes | length) as $total
      | $field_stats
      | map(. + {classification: classify(.; $total)})
    )
  }')"

printf '%s\n' "$report_json" > "$REPORT_PATH"

echo
echo "Supervisory trace observability report:"
echo "  $REPORT_PATH"
echo
printf '%s\n' "$report_json" | jq '{
  totals,
  outcome_breakdown,
  field_signal_assessment
}'
