#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: ./scripts/supervisory_trace_complex_simulation.sh [options]

Runs a deterministic, realistic supervisory-trace stress scenario through the
public host connector path. This is intentionally harder than the quick
simulation: multiple workspaces, overlapping workstreams, stale/conflicting
memory, sparse host payloads, failing/passing evidence, ignored/modified
directions, duplicate replay, and one controlled degraded host trace.

Options:
  --sessions N       Number of linked simulated sessions. Default: 24.
                     Use 50 to exercise the linked-outcome volume gate.
  --output-dir DIR   Directory for events/report/db. Default: /tmp/adesh-supervisory-complex-<run_id>.
  --db-url URL       SQLite URL. Default: sqlite://<output-dir>/complex.db?mode=rwc.
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
SESSIONS=24
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

if ! [[ "$SESSIONS" =~ ^[0-9]+$ ]] || [[ "$SESSIONS" -lt 6 ]]; then
  echo "--sessions must be an integer >= 6 for the complex scenario" >&2
  exit 1
fi

OUTPUT_DIR="${OUTPUT_DIR:-/tmp/adesh-supervisory-complex-${RUN_ID}}"
mkdir -p "$OUTPUT_DIR"

DB_URL="${DB_URL:-sqlite://${OUTPUT_DIR}/complex.db?mode=rwc}"
DB_PATH="${DB_URL#sqlite://}"
DB_PATH="${DB_PATH%%\?*}"
CARGO_TARGET_DIR="${ADESH_CARGO_TARGET_DIR:-/tmp/adesh-cargo-target}"
EVENTS_JSONL="${OUTPUT_DIR}/complex_events.jsonl"
REPORT_PATH="${OUTPUT_DIR}/supervisory_trace_complex_report.json"
: > "$EVENTS_JSONL"

CONNECTOR_ID="complex-supervisory-simulation"
CONNECTOR_KIND="offline_complex_simulation"
CONNECTOR_VERSION="0.1.0"
HOST_AGENT_ID="simulated-multi-agent-host"
HOST_AGENT_KIND="mixed-coding-agent"
HOST_MODEL="deterministic-complex-script"

WORKSPACE_PAYMENTS="workspace://complex-payments-service"
WORKSPACE_CONNECTORS="workspace://complex-connector-ecosystem"
WORKSPACE_EVAL="workspace://complex-eval-lab"

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
  local preference="$8"
  local risk="$9"
  local test_name="${10}"
  local test_status="${11}"
  local test_summary="${12}"

  local workspace
  workspace="$(workspace_json "$workspace_locator")"

  local payload
  payload="$(jq -n \
    --arg connector_id "$CONNECTOR_ID" \
    --arg connector_kind "$CONNECTOR_KIND" \
    --arg connector_version "$CONNECTOR_VERSION" \
    --arg session_id "complex-seed-${RUN_ID}-${task_hint}-${test_name}" \
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
    --arg preference "$preference" \
    --arg risk "$risk" \
    --arg test_name "$test_name" \
    --arg test_status "$test_status" \
    --arg test_summary "$test_summary" \
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
      files_touched: [
        "crates/adesh-daemon/src/cognition.rs",
        "crates/adesh-daemon/src/connector_adapter.rs",
        "docs/CONNECTOR_INTEGRATION_V0.md"
      ],
      tests: [
        {
          name: $test_name,
          status: $test_status,
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
      observed_preferences: [$preference],
      risk_signals: [$risk]
    }')"

  connector_event "$payload" >/dev/null
}

simulate_linked_session() {
  local idx="$1"
  local workspace_locator="$2"
  local task_hint="$3"
  local files_json="$4"
  local prompt="$5"
  local outcome="$6"
  local correction_summary="$7"
  local summary="$8"
  local decision="$9"
  local rationale="${10}"
  local unresolved="${11}"
  local risk="${12}"
  local test_name="${13}"
  local test_status="${14}"
  local test_summary="${15}"
  local replay_once="${16}"

  local session_id="complex-session-${RUN_ID}-${idx}"
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
      task_hint: (if $task_hint == "" then null else $task_hint end)
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
    --arg test_status "$test_status" \
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
      task_hint: (if $task_hint == "" then null else $task_hint end),
      summary: $summary,
      tests: [
        {
          name: $test_name,
          status: $test_status,
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

  if [[ "$replay_once" == "true" ]]; then
    connector_event "$end_payload" >/dev/null
  fi

  jq -n \
    --arg index "$idx" \
    --arg workspace_locator "$workspace_locator" \
    --arg task_hint "$task_hint" \
    --arg session_id "$session_id" \
    --arg context_id "$context_id" \
    --arg selected_direction "$selected_direction" \
    --arg outcome "$outcome" \
    --arg replay_once "$replay_once" \
    --argjson files "$files_json" \
    --argjson start_response "$start_response" \
    --argjson end_response "$end_response" \
    '{
      index: ($index | tonumber),
      workspace_locator: $workspace_locator,
      task_hint: (if $task_hint == "" then null else $task_hint end),
      files_in_focus_count: ($files | length),
      session_id: $session_id,
      context_id: (if $context_id == "" then null else $context_id end),
      selected_direction: $selected_direction,
      outcome: $outcome,
      duplicate_replay_attempted: ($replay_once == "true"),
      start_handled_as: $start_response.handled_as,
      end_handled_as: $end_response.handled_as,
      episode_id: $end_response.stored_episode.episode_id
    }' >> "$EVENTS_JSONL"

  printf 'complex session %02d workspace=%s hint=%s outcome=%s context=%s replay=%s\n' \
    "$idx" "$workspace_locator" "${task_hint:-none}" "$outcome" "${context_id:-missing}" "$replay_once"
}

write_degraded_trace() {
  local workspace
  workspace="$(workspace_json "$WORKSPACE_CONNECTORS")"

  local payload
  payload="$(jq -n \
    --arg connector_id "$CONNECTOR_ID" \
    --arg connector_kind "$CONNECTOR_KIND" \
    --arg connector_version "$CONNECTOR_VERSION" \
    --arg session_id "complex-degraded-${RUN_ID}" \
    --arg host_agent_id "$HOST_AGENT_ID" \
    --arg host_agent_kind "$HOST_AGENT_KIND" \
    --arg host_model "$HOST_MODEL" \
    --argjson workspace "$workspace" \
    '{
      connector_id: $connector_id,
      connector_kind: $connector_kind,
      connector_version: $connector_version,
      session_id: $session_id,
      host_agent_id: $host_agent_id,
      host_agent_kind: $host_agent_kind,
      host_model: $host_model,
      context_id: "stale-context-from-crashed-host",
      selected_next_direction: "Use stale context after host restart",
      outcome: "accepted",
      correction_summary: "This intentionally bad host trace must be stored but excluded from learning.",
      event_kind: "task_end",
      workspace: $workspace,
      task_prompt: "Host restarted and emitted an outcome with a stale context id.",
      task_hint: "connector-restart",
      summary: "Persisted a degraded trace with invalid context linkage.",
      files_touched: ["crates/adesh-daemon/src/connector_adapter.rs"],
      decisions: [
        {
          decision: "Do not learn from stale unlinked host outcomes",
          rationale: "Invalid context links can poison advisory learning"
        }
      ],
      unresolved_items: ["Need explicit host retry idempotency docs"],
      risk_signals: ["A restarted host can replay stale context ids"]
    }')"

  connector_event "$payload" >/dev/null
  jq -n '{
    degraded_trace: true,
    expected_learnability: false,
    reason: "invalid context_id should persist but remain unlearnable"
  }' >> "$EVENTS_JSONL"
  echo "complex degraded trace workspace=${WORKSPACE_CONNECTORS} expected_learnability=false"
}

echo "Running complex supervisory trace simulation..."
echo "DB_URL=${DB_URL}"
echo "linked_sessions=${SESSIONS}"

seed_episode \
  "$WORKSPACE_PAYMENTS" \
  "retry-hardening" \
  "Earlier retry patch moved retries into the transport adapter for speed." \
  "The transport-level retry shortcut created duplicate risk around idempotency keys." \
  "Do not keep retry policy hidden in the transport adapter" \
  "Transport-level retries make idempotency and timeout evidence hard to audit" \
  "Move retry state back to service-level explicit control flow" \
  "Prefer explicit reliability state over hidden transport convenience" \
  "Transport retries can duplicate writes under partial timeout" \
  "retry_idempotency_collision" \
  "fail" \
  "Duplicate write observed when timeout happens after upstream commit"

seed_episode \
  "$WORKSPACE_PAYMENTS" \
  "retry-hardening" \
  "Follow-up retry work restored explicit service state." \
  "Resolved the transport-retry shortcut and left timeout benchmark as the remaining gate." \
  "Keep retry state explicit in service layer" \
  "The service layer can correlate idempotency, timeout, and incident evidence" \
  "Run degraded-network timeout benchmark before claiming retry safety" \
  "Prefer proof-first reliability changes with incident-backed tests" \
  "Retry changes can look safe while timeout coverage is incomplete" \
  "retry_state_service_boundary" \
  "pass" \
  "Explicit service-level retry state passed idempotency regression"

seed_episode \
  "$WORKSPACE_CONNECTORS" \
  "connector-integration" \
  "VS Code integration note originally implied native lifecycle callbacks." \
  "Corrected lifecycle wording: task_start/task_end are Aadesh adapter events, not VS Code-native hooks." \
  "Keep connector events host-neutral and adapter-owned" \
  "Different hosts expose different extension surfaces; Aadesh normalizes after host mapping" \
  "Show returned context_id being fed into task_end outcomes" \
  "Prefer concrete integration examples over broad lifecycle claims" \
  "Docs can overclaim automation without proving trace linkage" \
  "connector_context_linkage_example" \
  "fail" \
  "Docs lacked an accepted-vs-ignored context linkage example"

seed_episode \
  "$WORKSPACE_CONNECTORS" \
  "connector-integration" \
  "Qwen, Gemini, and OpenCode wrappers shared the same host-facing adapter shape." \
  "Wrapper behavior worked, but restart/idempotency behavior still needed explicit evidence." \
  "Keep host wrappers thin and push semantics into connector_event" \
  "Thin wrappers preserve multi-host extensibility" \
  "Add stale-context restart case to connector validation" \
  "Prefer one normalized connector contract over per-host cognition logic" \
  "Wrapper-specific logic can drift if trace semantics are duplicated" \
  "wrapper_restart_stale_context" \
  "fail" \
  "No test yet proves stale context outcomes stay unlearnable"

seed_episode \
  "$WORKSPACE_EVAL" \
  "wedge-evaluation" \
  "A single aggregate benchmark looked good but was too easy to game." \
  "Evaluation needed more discriminating tasks with stale, conflicting, and sparse inputs." \
  "Do not claim Phase E from a single aggregate harness run" \
  "Policy-state work needs repeated operational pressure, not one good benchmark" \
  "Add realistic multi-workspace trace simulation before Phase E discussion" \
  "Prefer discriminating proof over vanity aggregate scores" \
  "A simple seeded harness can hide ranking and linkage weaknesses" \
  "cognitive_eval_simple_harness" \
  "pass" \
  "The basic harness passed but did not stress degraded traces or cross-workspace behavior"

seed_episode \
  "$WORKSPACE_EVAL" \
  "wedge-evaluation" \
  "Later eval persistence stored structured summaries and artifact refs." \
  "Eval data is useful only when it can be connected to suggested directions and outcomes." \
  "Treat eval persistence as evidence storage, not controller behavior" \
  "Evaluation storage should support later analysis without becoming Design Lab" \
  "Correlate accepted directions with later benchmark outcomes" \
  "Prefer durable evidence over speculative policy-state modeling" \
  "Eval records can become noise if not linked to intervention outcomes" \
  "eval_artifact_linkage" \
  "pass" \
  "Eval artifact persisted with run metadata and promotion decision"

for ((i = 1; i <= SESSIONS; i++)); do
  case $(( (i - 1) % 6 )) in
    0)
      workspace_locator="$WORKSPACE_PAYMENTS"
      task_hint="retry-hardening"
      files_json='["crates/adesh-daemon/src/cognition.rs","crates/adesh-daemon/tests/cognitive_proof.rs","docs/specs/active/storage_schema.md"]'
      prompt="The retry rollout still feels risky; what should I validate next?"
      summary="Balanced timeout coverage, idempotency, and incident risk before making retry cleanup changes."
      decision="Keep retry hardening blocked on degraded-network timeout evidence"
      rationale="Incident and failing-test evidence should outrank generic cleanup"
      unresolved="Compare timeout behavior under packet loss and partial upstream commit"
      risk="Retry cleanup can mask duplicate-write risk without degraded-network evidence"
      test_name="retry_degraded_network_complex_${i}"
      test_status="pass"
      test_summary="Simulated degraded-network proof kept retry safety visible"
      ;;
    1)
      workspace_locator="$WORKSPACE_CONNECTORS"
      task_hint="connector-integration"
      files_json='["docs/CONNECTOR_INTEGRATION_V0.md","scripts/supervisory_trace_real_runs.sh","scripts/supervisory_trace_complex_simulation.sh"]'
      prompt="The connector integration story is still ambiguous across VS Code, Qwen, and OpenCode."
      summary="Kept connector semantics host-neutral and focused docs on returned context_id linkage."
      decision="Require connector examples to show returned context_id used on task_end"
      rationale="Accepted/ignored outcomes are only learnable when linked to the prepared context"
      unresolved="Document stale-context restart behavior for host wrappers"
      risk="Host wrappers can silently emit unlearnable outcomes if context_id propagation is omitted"
      test_name="connector_context_linkage_complex_${i}"
      test_status="pass"
      test_summary="Simulated linked connector outcome across host wrapper style"
      ;;
    2)
      workspace_locator="$WORKSPACE_EVAL"
      task_hint="wedge-evaluation"
      files_json='["scripts/cognitive_eval_harness.sh","docs/IMPLEMENTATION_PLAN.md","docs/WEDGE_V0_RUNBOOK.md"]'
      prompt="Are we actually proving the wedge, or just passing a narrow seeded harness?"
      summary="Used the harder trace simulation as a discriminating check before interpreting aggregate metrics."
      decision="Keep Phase E gated until repeated operational pressure appears"
      rationale="The simulation validates trace mechanics but not the real two-week observation window"
      unresolved="Collect real host outcomes over time and inspect whether policy lineage gaps repeat"
      risk="A single synthetic pass can be mistaken for production evidence"
      test_name="eval_gate_complex_${i}"
      test_status="pass"
      test_summary="Simulated gate check separates trace mechanics from real observation window"
      ;;
    3)
      workspace_locator="$WORKSPACE_PAYMENTS"
      task_hint=""
      files_json='[]'
      prompt="This release still worries me, but the host only sent a vague prompt."
      summary="Recovered useful retry guidance from workspace memory despite sparse host payload."
      decision="When host payload is sparse, prefer workspace-backed safety evidence over generic cleanup"
      rationale="The prompt lacked files/task_hint, so scoped memory carried the useful context"
      unresolved="Ask host adapters to include task_hint and files when available"
      risk="Sparse host payloads reduce retrieval precision and can over-rank generic advice"
      test_name="sparse_payload_retry_complex_${i}"
      test_status="pass"
      test_summary="Sparse prompt still produced a linked learnable trace"
      ;;
    4)
      workspace_locator="$WORKSPACE_CONNECTORS"
      task_hint="connector-integration"
      files_json='["crates/adesh-daemon/src/connector_adapter.rs","crates/adesh-contracts/src/lib.rs"]'
      prompt="Before adding another adapter, what trace behavior should we harden?"
      summary="Modified the suggested docs-first direction into a narrower stale-context validation task."
      decision="Before adding adapters, prove stale-context outcomes stay unlearnable"
      rationale="Bad host linkage should be observed but excluded from advisory learning"
      unresolved="Add host-facing warning examples for unlearnable outcome traces"
      risk="Adding adapters before stale-context proof multiplies bad trace patterns"
      test_name="stale_context_complex_${i}"
      test_status="pass"
      test_summary="Simulated stale-context hardening path"
      ;;
    *)
      workspace_locator="$WORKSPACE_EVAL"
      task_hint="wedge-evaluation"
      files_json='["docs/IMPLEMENTATION_PLAN.md","docs/POLICY_STATE_DECISION_NOTE.md"]'
      prompt="The metrics look good; should we start policy-state now?"
      summary="Ignored the tempting policy-state expansion and kept focus on observational evidence."
      decision="Do not open policy-state from synthetic metrics alone"
      rationale="Policy-state requires repeated lineage, rollback, explanation, or comparison pressure"
      unresolved="Review real host traces for repeated policy-lineage reconstruction gaps"
      risk="Premature policy-state modeling can turn observability into controller theater"
      test_name="phase_e_gate_complex_${i}"
      test_status="pass"
      test_summary="Simulated decision keeps Phase E gated"
      ;;
  esac

  if (( i % 7 == 0 )); then
    outcome="ignored"
    correction_summary="Host chose local cleanup first; trace should count against this suggestion pattern."
  elif (( i % 5 == 0 )); then
    outcome="modified"
    correction_summary="Host accepted the direction but narrowed it to the concrete failing evidence."
  else
    outcome="accepted"
    correction_summary=""
  fi

  replay_once="false"
  if (( i == 3 )); then
    replay_once="true"
  fi

  simulate_linked_session \
    "$i" \
    "$workspace_locator" \
    "$task_hint" \
    "$files_json" \
    "$prompt" \
    "$outcome" \
    "$correction_summary" \
    "$summary" \
    "$decision" \
    "$rationale" \
    "$unresolved" \
    "$risk" \
    "$test_name" \
    "$test_status" \
    "$test_summary" \
    "$replay_once"
done

write_degraded_trace

events_json="$(jq -s '.' "$EVENTS_JSONL")"

db_counts_json="$(sqlite3 -json "$DB_PATH" "
SELECT
  (SELECT COUNT(*) FROM episodes) AS stored_episodes,
  (SELECT COUNT(*) FROM intervention_context) AS intervention_contexts,
  (SELECT COUNT(*) FROM intervention_outcomes) AS intervention_outcomes,
  (SELECT COUNT(*) FROM intervention_outcomes WHERE learn_from_this = 1) AS learnable_outcomes,
  (SELECT COUNT(*) FROM intervention_outcomes WHERE learn_from_this = 0) AS unlearnable_outcomes,
  (SELECT COUNT(DISTINCT scope_key) FROM intervention_context) AS distinct_context_scopes,
  (SELECT COUNT(*) FROM intervention_context WHERE surfaced_directions_json IS NOT NULL AND surfaced_directions_json <> '') AS contexts_with_direction_snapshot,
  (SELECT COUNT(*) FROM intervention_outcomes WHERE context_ref IS NULL) AS unlinked_outcomes;
")"

outcome_breakdown_json="$(sqlite3 -json "$DB_PATH" "
SELECT selected_response AS outcome, learn_from_this, COUNT(*) AS count
FROM intervention_outcomes
GROUP BY selected_response, learn_from_this
ORDER BY learn_from_this DESC, count DESC, outcome ASC;
")"

scope_breakdown_json="$(sqlite3 -json "$DB_PATH" "
SELECT scope_key, COUNT(*) AS context_count
FROM intervention_context
GROUP BY scope_key
ORDER BY context_count DESC, scope_key ASC;
")"

correlation_json="$(sqlite3 -json "$DB_PATH" "
SELECT
  c.scope_key,
  o.selected_response AS outcome,
  o.learn_from_this,
  o.surfaced_direction,
  COUNT(*) AS count
FROM intervention_outcomes o
LEFT JOIN intervention_context c ON c.context_id = o.context_ref
GROUP BY c.scope_key, o.selected_response, o.learn_from_this, o.surfaced_direction
ORDER BY count DESC, scope_key ASC, outcome ASC
LIMIT 30;
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
  --argjson sessions_requested "$SESSIONS" \
  --argjson events "$events_json" \
  --argjson counts "$db_counts_json" \
  --argjson outcome_breakdown "$outcome_breakdown_json" \
  --argjson scope_breakdown "$scope_breakdown_json" \
  --argjson correlation "$correlation_json" \
  --argjson field_quality "$field_quality_json" \
  '
  ($counts[0] // {}) as $c
  | ($outcome_breakdown | map(select(.learn_from_this == 1)) | map({key: .outcome, value: .count}) | from_entries) as $learnable_by_outcome
  | {
      metadata: {
        scenario: "complex_supervisory_trace",
        run_id: $run_id,
        db_url: $db_url,
        db_path: $db_path,
        report_path: $report_path,
        events_path: $events_path
      },
      scenario_dimensions: {
        workspaces: [
          "payments retry/idempotency rollout",
          "connector ecosystem and host wrappers",
          "evaluation/policy-gate evidence"
        ],
        hard_cases: [
          "stale/conflicting prior decision",
          "resolved vs unresolved safety loops",
          "sparse host payload",
          "ignored and modified suggested directions",
          "duplicate event replay",
          "invalid stale context that must remain unlearnable"
        ]
      },
      totals: {
        linked_sessions_requested: $sessions_requested,
        stored_episodes: ($c.stored_episodes // 0),
        intervention_contexts: ($c.intervention_contexts // 0),
        intervention_outcomes: ($c.intervention_outcomes // 0),
        learnable_outcomes: ($c.learnable_outcomes // 0),
        unlearnable_outcomes: ($c.unlearnable_outcomes // 0),
        unlinked_outcomes: ($c.unlinked_outcomes // 0),
        distinct_context_scopes: ($c.distinct_context_scopes // 0),
        contexts_with_direction_snapshot: ($c.contexts_with_direction_snapshot // 0),
        duplicate_replay_attempts: ($events | map(select(.duplicate_replay_attempted == true)) | length),
        degraded_trace_events: ($events | map(select(.degraded_trace == true)) | length),
        sparse_payload_sessions: ($events | map(select(.files_in_focus_count == 0 or .task_hint == null)) | length)
      },
      outcome_breakdown: $outcome_breakdown,
      scope_breakdown: $scope_breakdown,
      direction_outcome_correlation: $correlation,
      field_quality: (
        $field_quality
        | map(. + {
            present_ratio: (if .total == 0 then 0 else (.present_count / .total) end),
            classification: (
              if .present_count == .total then "complete_signal"
              elif .present_count == 0 then "missing_signal"
              else "partial_signal_expected_in_complex_run"
              end
            )
          })
      ),
      complex_assertions: {
        linked_sessions_all_learnable: (($c.learnable_outcomes // 0) == $sessions_requested),
        exactly_one_controlled_unlearnable_trace: (($c.unlearnable_outcomes // 0) == 1 and ($c.unlinked_outcomes // 0) == 1),
        duplicate_replay_did_not_add_outcome: (($c.intervention_outcomes // 0) == ($sessions_requested + 1)),
        multi_workspace_coverage: (($c.distinct_context_scopes // 0) >= 3),
        direction_snapshots_complete_for_linked_sessions: (($c.contexts_with_direction_snapshot // 0) >= $sessions_requested),
        mixed_outcomes_present: (
          (($learnable_by_outcome.accepted // 0) > 0)
          and (($learnable_by_outcome.ignored // 0) > 0)
          and (($learnable_by_outcome.modified // 0) > 0)
        ),
        sparse_payload_exercised: (($events | map(select(.files_in_focus_count == 0 or .task_hint == null)) | length) > 0)
      },
      complex_simulation_pass: (
        (($c.learnable_outcomes // 0) == $sessions_requested)
        and (($c.unlearnable_outcomes // 0) == 1)
        and (($c.unlinked_outcomes // 0) == 1)
        and (($c.intervention_outcomes // 0) == ($sessions_requested + 1))
        and (($c.distinct_context_scopes // 0) >= 3)
        and (($c.contexts_with_direction_snapshot // 0) >= $sessions_requested)
        and (($learnable_by_outcome.accepted // 0) > 0)
        and (($learnable_by_outcome.ignored // 0) > 0)
        and (($learnable_by_outcome.modified // 0) > 0)
        and (($events | map(select(.files_in_focus_count == 0 or .task_hint == null)) | length) > 0)
      ),
      operational_gate_progress: {
        two_week_window_required: true,
        completed_sessions_required: 20,
        completed_sessions_current: $sessions_requested,
        distinct_workspaces_required: 2,
        distinct_workspaces_current: ($c.distinct_context_scopes // 0),
        linked_learnable_outcomes_required: 50,
        linked_learnable_outcomes_current: ($c.learnable_outcomes // 0),
        note: "This complex deterministic simulation validates difficult trace mechanics. It still does not satisfy the real-time two-week observation gate by itself."
      },
      simulated_events: $events
    }
  ')"

printf '%s\n' "$report_json" > "$REPORT_PATH"

echo
echo "Complex supervisory trace simulation report:"
echo "  $REPORT_PATH"
echo
printf '%s\n' "$report_json" | jq '{
  complex_simulation_pass,
  totals,
  complex_assertions,
  outcome_breakdown,
  scope_breakdown,
  operational_gate_progress,
  field_quality
}'

if [[ "$(printf '%s\n' "$report_json" | jq -r '.complex_simulation_pass')" != "true" ]]; then
  echo "complex supervisory trace simulation failed" >&2
  exit 1
fi
