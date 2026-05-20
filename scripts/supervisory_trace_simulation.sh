#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: ./scripts/supervisory_trace_simulation.sh [options]

Runs a deterministic offline supervisory-trace simulation through the public
host connector path. This is the standard repeatable test for trace linkage,
learnability, and multi-workspace observability. It does not call a real LLM.

Options:
  --sessions N       Number of simulated completed sessions. Default: 20.
                     Use 50 to exercise the linked-outcome volume gate.
  --output-dir DIR   Directory for events/report/db. Default: /tmp/adesh-supervisory-sim-<run_id>.
  --db-url URL       SQLite URL. Default: sqlite://<output-dir>/simulation.db?mode=rwc.
  -h, --help         Show this help.

Environment:
  ADESH_DAEMON_ROOT       Override repo root.
  ADESH_CARGO_TARGET_DIR  Cargo target dir. Default: /tmp/adesh-cargo-target.
USAGE
}

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

ADESH_ROOT="${ADESH_DAEMON_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
if [[ ! -f "${ADESH_ROOT}/Cargo.toml" ]]; then
  echo "ADESH_DAEMON_ROOT does not point to an Aadesh repo: ${ADESH_ROOT}" >&2
  exit 1
fi

RUN_ID="$(date +%Y%m%d%H%M%S)"
SESSIONS=20
OUTPUT_DIR=""
DB_URL=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --sessions)
      SESSIONS="${2:-}"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="${2:-}"
      shift 2
      ;;
    --db-url)
      DB_URL="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if ! [[ "$SESSIONS" =~ ^[0-9]+$ ]] || [[ "$SESSIONS" -lt 1 ]]; then
  echo "--sessions must be a positive integer" >&2
  exit 1
fi

OUTPUT_DIR="${OUTPUT_DIR:-/tmp/adesh-supervisory-sim-${RUN_ID}}"
mkdir -p "$OUTPUT_DIR"

DB_URL="${DB_URL:-sqlite://${OUTPUT_DIR}/simulation.db?mode=rwc}"
DB_PATH="${DB_URL#sqlite://}"
DB_PATH="${DB_PATH%%\?*}"
CARGO_TARGET_DIR="${ADESH_CARGO_TARGET_DIR:-/tmp/adesh-cargo-target}"
EVENTS_JSONL="${OUTPUT_DIR}/events.jsonl"
REPORT_PATH="${OUTPUT_DIR}/supervisory_trace_simulation_report.json"
: > "$EVENTS_JSONL"

CONNECTOR_ID="standard-simulation"
CONNECTOR_KIND="offline_simulation"
CONNECTOR_VERSION="0.1.0"
HOST_AGENT_ID="simulated-coding-agent"
HOST_AGENT_KIND="standard-test-agent"
HOST_MODEL="deterministic-script"

WORKSPACE_A="workspace://sim-payments-service"
WORKSPACE_B="workspace://sim-docs-connector"
SCOPE_A="workspace:task_space:${WORKSPACE_A}"
SCOPE_B="workspace:task_space:${WORKSPACE_B}"

run_daemon() {
  ADESH_DATABASE_URL="$DB_URL" \
    CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
    cargo run -q --manifest-path "${ADESH_ROOT}/Cargo.toml" -p adesh-daemon -- "$@"
}

connector_event() {
  local payload="$1"
  run_daemon host connector --json "$payload"
}

workspace_json() {
  local locator="$1"
  jq -n --arg locator "$locator" '{
    kind: "task_space",
    locator: $locator,
    cwd: null,
    branch: null,
    external_ref: null
  }'
}

seed_episode() {
  local workspace_locator="$1"
  local task_hint="$2"
  local prompt="$3"
  local summary="$4"
  local decision="$5"
  local rationale="$6"
  local unresolved="$7"
  local risk="$8"
  local preference="$9"

  local workspace
  workspace="$(workspace_json "$workspace_locator")"

  local payload
  payload="$(jq -n \
    --arg connector_id "$CONNECTOR_ID" \
    --arg connector_kind "$CONNECTOR_KIND" \
    --arg connector_version "$CONNECTOR_VERSION" \
    --arg session_id "seed-${RUN_ID}-${task_hint}" \
    --arg host_agent_id "$HOST_AGENT_ID" \
    --arg host_agent_kind "$HOST_AGENT_KIND" \
    --arg host_model "$HOST_MODEL" \
    --argjson workspace "$workspace" \
    --arg task_prompt "$prompt" \
    --arg task_hint "$task_hint" \
    --arg summary "$summary" \
    --arg decision "$decision" \
    --arg rationale "$rationale" \
    --arg unresolved "$unresolved" \
    --arg risk "$risk" \
    --arg preference "$preference" \
    '{
      connector_id: $connector_id,
      connector_kind: $connector_kind,
      connector_version: $connector_version,
      session_id: $session_id,
      host_agent_id: $host_agent_id,
      host_agent_kind: $host_agent_kind,
      host_model: $host_model,
      event_kind: "task_end",
      workspace: $workspace,
      task_prompt: $task_prompt,
      task_hint: $task_hint,
      summary: $summary,
      files_touched: ["crates/adesh-daemon/src/cognition.rs", "docs/CONNECTOR_INTEGRATION_V0.md"],
      tests: [
        {
          name: "seed_context",
          status: "pass",
          summary: "Seed episode supplies deterministic memory for simulation"
        }
      ],
      decisions: [
        {
          decision: $decision,
          rationale: $rationale
        }
      ],
      unresolved_items: [$unresolved],
      observed_preferences: [$preference],
      risk_signals: [$risk]
    }')"

  connector_event "$payload" >/dev/null
}

simulate_session() {
  local idx="$1"
  local workspace_locator="$2"
  local task_hint="$3"
  local files_json="$4"
  local prompt="$5"
  local outcome="$6"
  local correction_summary="$7"

  local session_id="sim-session-${RUN_ID}-${idx}"
  local workspace
  workspace="$(workspace_json "$workspace_locator")"

  local start_payload
  start_payload="$(jq -n \
    --arg connector_id "$CONNECTOR_ID" \
    --arg connector_kind "$CONNECTOR_KIND" \
    --arg connector_version "$CONNECTOR_VERSION" \
    --arg session_id "$session_id" \
    --arg host_agent_id "$HOST_AGENT_ID" \
    --arg host_agent_kind "$HOST_AGENT_KIND" \
    --arg host_model "$HOST_MODEL" \
    --argjson workspace "$workspace" \
    --arg task_prompt "$prompt" \
    --arg task_hint "$task_hint" \
    --argjson files "$files_json" \
    '{
      connector_id: $connector_id,
      connector_kind: $connector_kind,
      connector_version: $connector_version,
      session_id: $session_id,
      host_agent_id: $host_agent_id,
      host_agent_kind: $host_agent_kind,
      host_model: $host_model,
      event_kind: "task_start",
      workspace: $workspace,
      task_prompt: $task_prompt,
      files_in_focus: $files,
      task_hint: $task_hint
    }')"

  local start_response
  start_response="$(connector_event "$start_payload")"

  local context_id
  local selected_direction
  context_id="$(printf '%s' "$start_response" | jq -r '.context_id // empty')"
  selected_direction="$(printf '%s' "$start_response" | jq -r '.prepare_context.likely_next_directions[0].statement // empty')"

  if [[ -z "$selected_direction" ]]; then
    selected_direction="No ranked direction returned"
  fi

  local summary
  local decision
  local rationale
  local unresolved
  local risk
  local test_name
  local test_summary

  if [[ "$workspace_locator" == "$WORKSPACE_A" ]]; then
    summary="Completed retry-hardening session ${idx}; used Aadesh guidance to choose validation order."
    decision="Keep retry hardening proof-first before refactoring cleanup"
    rationale="Timeout and incident evidence should drive ordering before implementation cleanup"
    unresolved="Need one more degraded-network timeout comparison after retry edge cases"
    risk="Retry changes can look safe while timeout coverage remains incomplete"
    test_name="retry_timeout_coverage_sim_${idx}"
    test_summary="Simulated timeout coverage check for retry hardening"
  else
    summary="Completed connector-doc session ${idx}; used Aadesh guidance to choose example order."
    decision="Keep connector docs example-first and host-neutral"
    rationale="Host integrations need concrete accepted/ignored outcome examples before new adapter scope"
    unresolved="Need one more accepted-vs-ignored connector trace example"
    risk="Connector docs can overstate automation without showing trace linkage"
    test_name="connector_docs_trace_example_sim_${idx}"
    test_summary="Simulated docs proof check for connector traces"
  fi

  local end_payload
  end_payload="$(jq -n \
    --arg connector_id "$CONNECTOR_ID" \
    --arg connector_kind "$CONNECTOR_KIND" \
    --arg connector_version "$CONNECTOR_VERSION" \
    --arg session_id "$session_id" \
    --arg host_agent_id "$HOST_AGENT_ID" \
    --arg host_agent_kind "$HOST_AGENT_KIND" \
    --arg host_model "$HOST_MODEL" \
    --arg context_id "$context_id" \
    --arg selected_next_direction "$selected_direction" \
    --arg outcome "$outcome" \
    --arg correction_summary "$correction_summary" \
    --argjson workspace "$workspace" \
    --arg task_prompt "$prompt" \
    --arg task_hint "$task_hint" \
    --argjson files "$files_json" \
    --arg summary "$summary" \
    --arg decision "$decision" \
    --arg rationale "$rationale" \
    --arg unresolved "$unresolved" \
    --arg risk "$risk" \
    --arg test_name "$test_name" \
    --arg test_summary "$test_summary" \
    '{
      connector_id: $connector_id,
      connector_kind: $connector_kind,
      connector_version: $connector_version,
      session_id: $session_id,
      host_agent_id: $host_agent_id,
      host_agent_kind: $host_agent_kind,
      host_model: $host_model,
      context_id: (if $context_id == "" then null else $context_id end),
      selected_next_direction: $selected_next_direction,
      outcome: $outcome,
      correction_summary: (if $correction_summary == "" then null else $correction_summary end),
      event_kind: "task_end",
      workspace: $workspace,
      task_prompt: $task_prompt,
      files_in_focus: $files,
      files_touched: $files,
      task_hint: $task_hint,
      summary: $summary,
      tests: [
        {
          name: $test_name,
          status: "pass",
          summary: $test_summary
        }
      ],
      decisions: [
        {
          decision: $decision,
          rationale: $rationale
        }
      ],
      unresolved_items: [$unresolved],
      risk_signals: [$risk]
    }')"

  local end_response
  end_response="$(connector_event "$end_payload")"

  jq -n \
    --arg index "$idx" \
    --arg workspace_locator "$workspace_locator" \
    --arg session_id "$session_id" \
    --arg context_id "$context_id" \
    --arg selected_direction "$selected_direction" \
    --arg outcome "$outcome" \
    --argjson start_response "$start_response" \
    --argjson end_response "$end_response" \
    '{
      index: ($index | tonumber),
      workspace_locator: $workspace_locator,
      session_id: $session_id,
      context_id: (if $context_id == "" then null else $context_id end),
      selected_direction: $selected_direction,
      outcome: $outcome,
      start_handled_as: $start_response.handled_as,
      end_handled_as: $end_response.handled_as,
      episode_id: $end_response.stored_episode.episode_id
    }' >> "$EVENTS_JSONL"

  printf 'session %02d workspace=%s outcome=%s context=%s\n' \
    "$idx" "$workspace_locator" "$outcome" "${context_id:-missing}"
}

echo "Running deterministic supervisory trace simulation..."
echo "DB_URL=${DB_URL}"
echo "sessions=${SESSIONS}"

seed_episode \
  "$WORKSPACE_A" \
  "retry-hardening" \
  "Prior retry hardening work left timeout coverage unresolved." \
  "Added retry scaffolding but deferred degraded-network timeout comparison." \
  "Keep retry state explicit in service layer" \
  "Failure-path audits are easier with explicit control flow" \
  "Need degraded-network timeout benchmark before retry confidence is claimed" \
  "Without timeout benchmark, retry confidence may be overstated" \
  "Prefer proof-first retry changes with concrete timeout evidence"

seed_episode \
  "$WORKSPACE_A" \
  "retry-hardening" \
  "Review incident-oriented retry risks before cleanup." \
  "Incident notes showed timeout behavior matters more than local cleanup debt." \
  "Prioritize timeout coverage before retry metric polish" \
  "Safety gaps should outrank cosmetic cleanup when validating retry behavior" \
  "Compare timeout behavior under degraded-network simulation" \
  "Incident evidence can be lost if ranking favors generic cleanup" \
  "Prefer keeping safety gaps visible until passing evidence exists"

seed_episode \
  "$WORKSPACE_B" \
  "connector-docs" \
  "Prior connector docs review found lifecycle wording too abstract." \
  "Docs were clearer after adding accepted-vs-ignored trace examples." \
  "Keep connector docs example-first and host-neutral" \
  "Different hosts need the same trace semantics without assuming native lifecycle callbacks" \
  "Add concrete accepted-vs-ignored intervention outcome example" \
  "Connector docs can overclaim automation if examples do not show actual linkage" \
  "Prefer concrete host-neutral examples over broad architecture prose"

seed_episode \
  "$WORKSPACE_B" \
  "connector-docs" \
  "Review host integration guidance after VS Code correction." \
  "Kept connector events as Aadesh adapter semantics, not native VS Code callbacks." \
  "Separate Aadesh connector events from host-native lifecycle APIs" \
  "This keeps the integration extensible across CLI and IDE hosts" \
  "Show how returned context_id is fed into task_end outcomes" \
  "Missing context_id propagation makes outcome learning unreliable" \
  "Prefer trace-linkage examples before adding new host adapters"

for ((i = 1; i <= SESSIONS; i++)); do
  if (( i % 2 == 1 )); then
    workspace_locator="$WORKSPACE_A"
    task_hint="retry-hardening"
    files_json='["crates/adesh-daemon/src/cognition.rs","crates/adesh-daemon/tests/cognitive_proof.rs"]'
    prompt="What should I do next in retry hardening session ${i}?"
  else
    workspace_locator="$WORKSPACE_B"
    task_hint="connector-docs"
    files_json='["docs/CONNECTOR_INTEGRATION_V0.md","scripts/supervisory_trace_simulation.sh"]'
    prompt="What should I do next in connector documentation session ${i}?"
  fi

  if (( i % 5 == 0 )); then
    outcome="modified"
    correction_summary="Used the suggested direction but narrowed it to the highest-risk subtask."
  elif (( i % 4 == 0 )); then
    outcome="ignored"
    correction_summary="Skipped the suggested direction to do local cleanup first."
  else
    outcome="accepted"
    correction_summary=""
  fi

  simulate_session "$i" "$workspace_locator" "$task_hint" "$files_json" "$prompt" "$outcome" "$correction_summary"
done

events_json="$(jq -s '.' "$EVENTS_JSONL")"

db_counts_json="$(sqlite3 -json "$DB_PATH" "
SELECT
  (SELECT COUNT(*) FROM episodes) AS stored_episodes,
  (SELECT COUNT(*) FROM intervention_context) AS intervention_contexts,
  (SELECT COUNT(*) FROM intervention_outcomes) AS intervention_outcomes,
  (SELECT COUNT(*) FROM intervention_outcomes WHERE learn_from_this = 1) AS learnable_outcomes,
  (SELECT COUNT(*) FROM intervention_outcomes WHERE learn_from_this = 0) AS unlearnable_outcomes,
  (SELECT COUNT(DISTINCT scope_key) FROM intervention_context WHERE scope_key IN ('$SCOPE_A', '$SCOPE_B')) AS distinct_context_scopes,
  (SELECT COUNT(*) FROM intervention_context WHERE surfaced_directions_json IS NOT NULL AND surfaced_directions_json <> '') AS contexts_with_direction_snapshot;
")"

outcome_breakdown_json="$(sqlite3 -json "$DB_PATH" "
SELECT selected_response AS outcome, COUNT(*) AS count
FROM intervention_outcomes
GROUP BY selected_response
ORDER BY count DESC, outcome ASC;
")"

correlation_json="$(sqlite3 -json "$DB_PATH" "
SELECT
  c.scope_key,
  o.selected_response AS outcome,
  o.surfaced_direction,
  COUNT(*) AS count
FROM intervention_outcomes o
JOIN intervention_context c ON c.context_id = o.context_ref
GROUP BY c.scope_key, o.selected_response, o.surfaced_direction
ORDER BY count DESC, c.scope_key ASC, outcome ASC
LIMIT 20;
")"

field_quality_json="$(sqlite3 -json "$DB_PATH" "
SELECT
  'context_ref' AS field,
  COUNT(*) AS total,
  SUM(CASE WHEN context_ref IS NOT NULL AND context_ref <> '' THEN 1 ELSE 0 END) AS present_count
FROM intervention_outcomes
UNION ALL
SELECT
  'surfaced_direction',
  COUNT(*),
  SUM(CASE WHEN surfaced_direction IS NOT NULL AND surfaced_direction <> '' THEN 1 ELSE 0 END)
FROM intervention_outcomes
UNION ALL
SELECT
  'surface_snapshot',
  COUNT(*),
  SUM(CASE WHEN surfaced_directions_json IS NOT NULL AND surfaced_directions_json <> '' THEN 1 ELSE 0 END)
FROM intervention_context;
")"

report_json="$(jq -n \
  --arg run_id "$RUN_ID" \
  --arg db_url "$DB_URL" \
  --arg db_path "$DB_PATH" \
  --arg report_path "$REPORT_PATH" \
  --arg events_path "$EVENTS_JSONL" \
  --arg workspace_a "$WORKSPACE_A" \
  --arg workspace_b "$WORKSPACE_B" \
  --argjson sessions_requested "$SESSIONS" \
  --argjson events "$events_json" \
  --argjson counts "$db_counts_json" \
  --argjson outcome_breakdown "$outcome_breakdown_json" \
  --argjson correlation "$correlation_json" \
  --argjson field_quality "$field_quality_json" \
  '
  ($counts[0] // {}) as $c
  | ($outcome_breakdown | map({key: .outcome, value: .count}) | from_entries) as $by_outcome
  | {
      metadata: {
        run_id: $run_id,
        db_url: $db_url,
        db_path: $db_path,
        report_path: $report_path,
        events_path: $events_path,
        workspaces: [$workspace_a, $workspace_b]
      },
      totals: {
        sessions_requested: $sessions_requested,
        simulated_sessions: ($events | length),
        stored_episodes: ($c.stored_episodes // 0),
        intervention_contexts: ($c.intervention_contexts // 0),
        intervention_outcomes: ($c.intervention_outcomes // 0),
        learnable_outcomes: ($c.learnable_outcomes // 0),
        unlearnable_outcomes: ($c.unlearnable_outcomes // 0),
        distinct_context_scopes: ($c.distinct_context_scopes // 0),
        contexts_with_direction_snapshot: ($c.contexts_with_direction_snapshot // 0)
      },
      outcome_breakdown: $outcome_breakdown,
      direction_outcome_correlation: $correlation,
      field_quality: (
        $field_quality
        | map(. + {
            present_ratio: (if .total == 0 then 0 else (.present_count / .total) end),
            classification: (
              if .present_count == .total then "complete_signal"
              elif .present_count == 0 then "missing_signal"
              else "partial_signal"
              end
            )
          })
      ),
      standard_simulation_pass: (
        ($events | length) == $sessions_requested
        and (($c.intervention_contexts // 0) >= $sessions_requested)
        and (($c.intervention_outcomes // 0) == $sessions_requested)
        and (($c.learnable_outcomes // 0) == $sessions_requested)
        and (($c.unlearnable_outcomes // 0) == 0)
        and (($c.distinct_context_scopes // 0) >= 2)
        and (($c.contexts_with_direction_snapshot // 0) >= $sessions_requested)
        and (($by_outcome.accepted // 0) > 0)
        and (($by_outcome.ignored // 0) > 0)
        and (($by_outcome.modified // 0) > 0)
      ),
      operational_gate_progress: {
        two_week_window_required: true,
        completed_sessions_required: 20,
        completed_sessions_current: ($events | length),
        distinct_workspaces_required: 2,
        distinct_workspaces_current: ($c.distinct_context_scopes // 0),
        linked_learnable_outcomes_required: 50,
        linked_learnable_outcomes_current: ($c.learnable_outcomes // 0),
        note: "This deterministic simulation validates trace mechanics. It does not satisfy the real-time two-week observation gate by itself."
      },
      simulated_events: $events
    }
  ')"

printf '%s\n' "$report_json" > "$REPORT_PATH"

echo
echo "Supervisory trace simulation report:"
echo "  $REPORT_PATH"
echo
printf '%s\n' "$report_json" | jq '{
  standard_simulation_pass,
  totals,
  outcome_breakdown,
  operational_gate_progress,
  field_quality
}'

if [[ "$(printf '%s\n' "$report_json" | jq -r '.standard_simulation_pass')" != "true" ]]; then
  echo "standard supervisory trace simulation failed" >&2
  exit 1
fi
