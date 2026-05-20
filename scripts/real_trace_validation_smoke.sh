#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: ./scripts/real_trace_validation_smoke.sh [options]

Creates a small linked connector-trace DB, writes accepted/ignored/modified
outcomes through the public host connector path, then validates the captured DB
with real_trace_validation_harness.sh.

Options:
  --output-dir DIR      Output directory. Default: /tmp/adesh-real-trace-smoke-<run_id>.
  --db-url URL          SQLite URL. Default: sqlite://<output-dir>/real_trace_smoke.db?mode=rwc.
  -h, --help            Show help.
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
OUTPUT_DIR=""
DB_URL=""
CARGO_TARGET_DIR="${ADESH_CARGO_TARGET_DIR:-/tmp/adesh-cargo-target}"

while [[ $# -gt 0 ]]; do
  case "$1" in
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

OUTPUT_DIR="${OUTPUT_DIR:-/tmp/adesh-real-trace-smoke-${RUN_ID}}"
mkdir -p "$OUTPUT_DIR/events" "$OUTPUT_DIR/validation"
DB_URL="${DB_URL:-sqlite://${OUTPUT_DIR}/real_trace_smoke.db?mode=rwc}"
DB_PATH="${DB_URL#sqlite://}"
DB_PATH="${DB_PATH%%\?*}"
REPORT_PATH="${OUTPUT_DIR}/real_trace_smoke_report.json"

run_daemon() {
  ADESH_DATABASE_URL="$DB_URL" \
    CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
    cargo run -q --manifest-path "${ADESH_ROOT}/Cargo.toml" -p adesh-daemon -- "$@"
}

connector_event() {
  local payload="$1"
  run_daemon host connector --json "$payload"
}

run_case() {
  local idx="$1"
  local workspace_locator="$2"
  local task_hint="$3"
  local task_prompt="$4"
  local outcome="$5"
  local summary="$6"
  local decision="$7"
  local rationale="$8"
  local unresolved="$9"
  local preference="${10}"
  local risk="${11}"
  local file_one="${12}"
  local file_two="${13}"
  local correction_summary="${14:-}"

  local session_id="real-trace-smoke-${RUN_ID}-${idx}"
  local start_payload start_response context_id top_direction end_payload end_response

  # Seed one prior episode so task_start can surface a concrete direction and
  # return a context_id. Without this, the connector correctly degrades to an
  # unlinked outcome because there was no surfaced guidance to link.
  run_daemon host store \
    --workspace-kind task_space \
    --workspace-locator "$workspace_locator" \
    --task-hint "$task_hint" \
    --task "Prior trace for ${task_prompt}" \
    --summary "Prior captured trace for ${task_hint}: ${summary}" \
    --decision "$decision::$rationale" \
    --unresolved "$unresolved" \
    --preference "$preference" \
    --risk "$risk" \
    --file "$file_one" \
    --file "$file_two" >/dev/null

  start_payload="$(jq -n \
    --arg session_id "$session_id" \
    --arg workspace_locator "$workspace_locator" \
    --arg task_hint "$task_hint" \
    --arg task_prompt "$task_prompt" \
    --arg file_one "$file_one" \
    --arg file_two "$file_two" \
    '{
      connector_id: "real-trace-smoke",
      connector_kind: "deterministic_connector_smoke",
      connector_version: "0.1.0",
      session_id: $session_id,
      host_agent_id: "smoke-codex-host",
      host_agent_kind: "codex-vscode",
      host_model: "deterministic-smoke",
      event_kind: "task_start",
      workspace: {kind: "task_space", locator: $workspace_locator, cwd: null, branch: null, external_ref: null},
      task_prompt: $task_prompt,
      task_hint: $task_hint,
      files_in_focus: [$file_one, $file_two]
    }')"
  printf '%s\n' "$start_payload" >"${OUTPUT_DIR}/events/${idx}-task_start.json"
  start_response="$(connector_event "$start_payload")"
  printf '%s\n' "$start_response" >"${OUTPUT_DIR}/events/${idx}-task_start.response.json"

  context_id="$(printf '%s' "$start_response" | jq -r '.context_id // empty')"
  top_direction="$(printf '%s' "$start_response" | jq -r '.prepare_context.likely_next_directions[0].statement // empty')"
  if [[ -z "$top_direction" ]]; then
    top_direction="Continue captured work with evidence-backed next step for ${task_hint}"
  fi

  end_payload="$(jq -n \
    --arg session_id "$session_id" \
    --arg workspace_locator "$workspace_locator" \
    --arg task_hint "$task_hint" \
    --arg task_prompt "$task_prompt" \
    --arg context_id "$context_id" \
    --arg selected_next_direction "$top_direction" \
    --arg outcome "$outcome" \
    --arg correction_summary "$correction_summary" \
    --arg summary "$summary" \
    --arg decision "$decision" \
    --arg rationale "$rationale" \
    --arg unresolved "$unresolved" \
    --arg preference "$preference" \
    --arg risk "$risk" \
    --arg file_one "$file_one" \
    --arg file_two "$file_two" \
    '{
      connector_id: "real-trace-smoke",
      connector_kind: "deterministic_connector_smoke",
      connector_version: "0.1.0",
      session_id: $session_id,
      host_agent_id: "smoke-codex-host",
      host_agent_kind: "codex-vscode",
      host_model: "deterministic-smoke",
      event_kind: "task_end",
      context_id: (if ($context_id | length) > 0 then $context_id else null end),
      selected_next_direction: $selected_next_direction,
      outcome: $outcome,
      correction_summary: (if ($correction_summary | length) > 0 then $correction_summary else null end),
      workspace: {kind: "task_space", locator: $workspace_locator, cwd: null, branch: null, external_ref: null},
      task_prompt: $task_prompt,
      task_hint: $task_hint,
      summary: $summary,
      files_touched: [$file_one, $file_two],
      decisions: [{decision: $decision, rationale: $rationale}],
      unresolved_items: [$unresolved],
      observed_preferences: [$preference],
      risk_signals: [$risk],
      tests: [{name: ("real_trace_smoke_" + $session_id), status: "pass", summary: "deterministic connector trace case stored"}]
    }')"
  printf '%s\n' "$end_payload" >"${OUTPUT_DIR}/events/${idx}-task_end.json"
  end_response="$(connector_event "$end_payload")"
  printf '%s\n' "$end_response" >"${OUTPUT_DIR}/events/${idx}-task_end.response.json"
}

run_case \
  "01" \
  "workspace://real-trace-smoke-payments" \
  "payment-reliability" \
  "Continue retry idempotency hardening from the current coding session" \
  "accepted" \
  "Accepted the service-boundary guidance and kept timeout coverage as the next proof item." \
  "Keep payment idempotency at the service boundary" \
  "Service-boundary evidence is easier to audit across hosts" \
  "Add timeout coverage for retry under packet loss" \
  "Prefer service-boundary tests for reliability-sensitive retry work" \
  "Retry replay can duplicate charges without timeout evidence" \
  "src/payments/retry_worker.rs" \
  "tests/payments/retry_timeout.rs"

run_case \
  "02" \
  "workspace://real-trace-smoke-connectors" \
  "connector-observability" \
  "Continue connector context_id outcome trace hardening" \
  "ignored" \
  "Ignored the returned context_id example direction and did local cleanup first; the concrete accepted/ignored examples remained open." \
  "Propagate returned context_id from task_start to task_end" \
  "Linked outcomes require stable context references" \
  "Add one accepted and one ignored outcome example with context_id" \
  "Prefer concrete connector examples over abstract lifecycle language" \
  "Without context_id examples, hosts may write unlearnable outcome traces" \
  "crates/adesh-daemon/src/connector_adapter.rs" \
  "docs/CONNECTOR_INTEGRATION_V0.md" \
  "Host chose cleanup before evidence examples, so the suggestion was not followed."

run_case \
  "03" \
  "workspace://real-trace-smoke-eval" \
  "proof-validation" \
  "Continue benchmark validation after OpenMemory nearly matched recall" \
  "modified" \
  "Modified the suggestion: kept proof validation, but narrowed it to real trace validation before adding integrations." \
  "Do not claim differentiation on memory recall alone" \
  "OpenMemory can match recall, so Aadesh must prove outcome-aware supervision" \
  "Run real host trace validation before adding new features" \
  "Prefer competitor comparisons that separate recall from outcome-trace learning" \
  "A generic memory-server direction would erase the supervisory wedge" \
  "scripts/external_memory_comparison_harness.sh" \
  "scripts/real_trace_validation_harness.sh" \
  "Suggestion was narrowed to the real-trace validation gate."

"${ADESH_ROOT}/scripts/real_trace_validation_harness.sh" \
  --db-path "$DB_PATH" \
  --output-dir "${OUTPUT_DIR}/validation" \
  --min-cases 3 \
  --strict >"${OUTPUT_DIR}/validation.stdout.json"

validation_report="${OUTPUT_DIR}/validation/real_trace_validation_report.json"

outcome_gate="$(jq -r '
  (.outcome_trace_summary.linked_outcome_count >= 3)
  and (([.outcome_trace_summary.by_outcome[]?.outcome] | index("accepted")) != null)
  and (([.outcome_trace_summary.by_outcome[]?.outcome] | index("ignored")) != null)
  and (([.outcome_trace_summary.by_outcome[]?.outcome] | index("modified")) != null)
' "$validation_report")"

jq -n \
  --arg db_path "$DB_PATH" \
  --arg db_url "$DB_URL" \
  --arg output_dir "$OUTPUT_DIR" \
  --arg validation_report "$validation_report" \
  --argjson validation "$(cat "$validation_report")" \
  --argjson outcome_gate "$outcome_gate" \
  '{
    status: "run",
    db_path: $db_path,
    db_url: $db_url,
    output_dir: $output_dir,
    validation_report: $validation_report,
    validation_pass: ($validation.validation_pass == true and $outcome_gate == true),
    outcome_gate_pass: $outcome_gate,
    case_count: $validation.case_count,
    mean_score: $validation.mean_score,
    outcome_trace_summary: $validation.outcome_trace_summary,
    weak_cases: $validation.weak_cases
  }' >"$REPORT_PATH"

cat "$REPORT_PATH"

if [[ "$(jq -r '.validation_pass' "$REPORT_PATH")" != "true" ]]; then
  exit 1
fi
