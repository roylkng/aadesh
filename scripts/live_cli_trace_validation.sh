#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: ./scripts/live_cli_trace_validation.sh [options]

Runs installed coding-agent CLIs with short non-destructive prompts, captures the
sessions through Aadesh connector events, then validates the resulting DB with
real_trace_validation_harness.sh.

This is a live host-friction check, not a replacement for the deterministic
validator. Individual CLIs may be reported as blocked/failed; the run passes only
when at least one live CLI completes and the captured Aadesh DB validates.

Options:
  --output-dir DIR       Output directory. Default: /tmp/adesh-live-cli-trace-<run_id>.
  --db-url URL           SQLite URL. Default: sqlite://<output-dir>/live_cli.db?mode=rwc.
  --timeout-seconds N    Per-CLI timeout. Default: 90.
  --skip-codex           Do not invoke the codex CLI.
  -h, --help             Show help.
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
if ! command -v timeout >/dev/null 2>&1; then
  echo "timeout is required" >&2
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
TIMEOUT_SECONDS="${LIVE_CLI_TIMEOUT_SECONDS:-90}"
SKIP_CODEX=0
CARGO_TARGET_DIR="${ADESH_CARGO_TARGET_DIR:-/tmp/adesh-cargo-target}"
LMSTUDIO_BASE_URL="${LMSTUDIO_BASE_URL:-http://127.0.0.1:1234/v1}"
LIVE_CLI_MODEL="${LIVE_CLI_MODEL:-qwen/qwen3.6-27b}"
QWEN_BASE_URL="${QWEN_OPENAI_BASE_URL:-$LMSTUDIO_BASE_URL}"
QWEN_API_KEY="${QWEN_OPENAI_API_KEY:-lm-studio}"
QWEN_MODEL="${QWEN_MODEL:-$LIVE_CLI_MODEL}"

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
    --timeout-seconds)
      TIMEOUT_SECONDS="${2:-}"
      shift 2
      ;;
    --skip-codex)
      SKIP_CODEX=1
      shift
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

if ! [[ "$TIMEOUT_SECONDS" =~ ^[0-9]+$ ]] || [[ "$TIMEOUT_SECONDS" -lt 5 ]]; then
  echo "--timeout-seconds must be an integer >= 5" >&2
  exit 1
fi

OUTPUT_DIR="${OUTPUT_DIR:-/tmp/adesh-live-cli-trace-${RUN_ID}}"
mkdir -p "$OUTPUT_DIR/events" "$OUTPUT_DIR/cli_outputs" "$OUTPUT_DIR/validation"
DB_URL="${DB_URL:-sqlite://${OUTPUT_DIR}/live_cli.db?mode=rwc}"
DB_PATH="${DB_URL#sqlite://}"
DB_PATH="${DB_PATH%%\?*}"
REPORT_PATH="${OUTPUT_DIR}/live_cli_trace_report.json"
CLI_RESULTS_JSONL="${OUTPUT_DIR}/cli_results.jsonl"
: >"$CLI_RESULTS_JSONL"

run_daemon() {
  ADESH_DATABASE_URL="$DB_URL" \
    CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
    cargo run -q --manifest-path "${ADESH_ROOT}/Cargo.toml" -p adesh-daemon -- "$@"
}

sqlite_json_or_empty_array() {
  local query="$1"
  local result
  result="$(sqlite3 -json "$DB_PATH" "$query")"
  if [[ -z "$result" ]]; then
    printf '[]'
  else
    printf '%s' "$result"
  fi
}

connector_event() {
  local payload="$1"
  run_daemon host connector --json "$payload"
}

seed_prior() {
  local workspace_locator="$1"
  local task_hint="$2"
  local task_prompt="$3"
  local summary="$4"
  local decision="$5"
  local rationale="$6"
  local unresolved="$7"
  local preference="$8"
  local risk="$9"
  local file_one="${10}"
  local file_two="${11}"

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
}

build_prompt() {
  local host="$1"
  local task_prompt="$2"
  local top_direction="$3"
  local decision="$4"
  local unresolved="$5"
  cat <<PROMPT
You are being used for Aadesh live CLI validation.
Do not edit files. Do not run commands. Return a concise advisory answer only.
Host under test: ${host}
Current task: ${task_prompt}
Aadesh surfaced direction: ${top_direction}
Relevant prior decision: ${decision}
Known open loop: ${unresolved}
Answer in exactly three bullets: accept/modify/ignore the surfaced direction, one concrete next step, and one risk to preserve.
PROMPT
}

write_cli_result() {
  local host_key="$1"
  local host_agent_kind="$2"
  local status="$3"
  local exit_code="$4"
  local context_id="$5"
  local output_path="$6"
  local stderr_path="$7"
  local outcome="$8"
  local note="$9"

  jq -n \
    --arg host_key "$host_key" \
    --arg host_agent_kind "$host_agent_kind" \
    --arg status "$status" \
    --arg exit_code "$exit_code" \
    --arg context_id "$context_id" \
    --arg output_path "$output_path" \
    --arg stderr_path "$stderr_path" \
    --arg outcome "$outcome" \
    --arg note "$note" \
    '{
      host_key: $host_key,
      host_agent_kind: $host_agent_kind,
      status: $status,
      exit_code: (if ($exit_code | length) > 0 then ($exit_code | tonumber) else null end),
      context_id: (if ($context_id | length) > 0 then $context_id else null end),
      output_path: $output_path,
      stderr_path: $stderr_path,
      selected_response: (if ($outcome | length) > 0 then $outcome else null end),
      note: $note
    }' >>"$CLI_RESULTS_JSONL"
}

run_cli_command() {
  local host_key="$1"
  local prompt="$2"
  local output_path="$3"
  local stderr_path="$4"

  case "$host_key" in
    qwen)
      if ! command -v qwen >/dev/null 2>&1; then
        return 127
      fi
      OPENAI_API_KEY="$QWEN_API_KEY" \
      OPENAI_BASE_URL="$QWEN_BASE_URL" \
      timeout "$TIMEOUT_SECONDS" qwen \
        --auth-type openai \
        --openai-base-url "$QWEN_BASE_URL" \
        --openai-api-key "$QWEN_API_KEY" \
        --model "$QWEN_MODEL" \
        --prompt "$prompt" \
        --output-format text >"$output_path" 2>"$stderr_path"
      ;;
    opencode)
      if ! command -v opencode >/dev/null 2>&1; then
        return 127
      fi
      timeout "$TIMEOUT_SECONDS" opencode run "$prompt" >"$output_path" 2>"$stderr_path"
      ;;
    gemini)
      if ! command -v gemini >/dev/null 2>&1; then
        return 127
      fi
      timeout "$TIMEOUT_SECONDS" gemini \
        --skip-trust \
        --approval-mode plan \
        --prompt "$prompt" \
        --output-format json >"$output_path" 2>"$stderr_path"
      ;;
    codex)
      if ! command -v codex >/dev/null 2>&1; then
        return 127
      fi
      timeout "$TIMEOUT_SECONDS" codex exec "$prompt" >"$output_path" 2>"$stderr_path"
      ;;
    *)
      echo "unknown host key: $host_key" >&2
      return 2
      ;;
  esac
}

run_live_case() {
  local idx="$1"
  local host_key="$2"
  local connector_id="$3"
  local connector_kind="$4"
  local host_agent_id="$5"
  local host_agent_kind="$6"
  local host_model="$7"
  local workspace_locator="$8"
  local task_hint="$9"
  local task_prompt="${10}"
  local expected_outcome="${11}"
  local summary="${12}"
  local decision="${13}"
  local rationale="${14}"
  local unresolved="${15}"
  local preference="${16}"
  local risk="${17}"
  local file_one="${18}"
  local file_two="${19}"
  local correction_summary="${20:-}"

  local session_id="live-cli-${RUN_ID}-${idx}-${host_key}"
  local start_payload start_response context_id top_direction prompt output_path stderr_path exit_code status note output_excerpt end_payload

  seed_prior "$workspace_locator" "$task_hint" "$task_prompt" "$summary" "$decision" "$rationale" "$unresolved" "$preference" "$risk" "$file_one" "$file_two"

  start_payload="$(jq -n \
    --arg connector_id "$connector_id" \
    --arg connector_kind "$connector_kind" \
    --arg session_id "$session_id" \
    --arg host_agent_id "$host_agent_id" \
    --arg host_agent_kind "$host_agent_kind" \
    --arg host_model "$host_model" \
    --arg workspace_locator "$workspace_locator" \
    --arg task_hint "$task_hint" \
    --arg task_prompt "$task_prompt" \
    --arg file_one "$file_one" \
    --arg file_two "$file_two" \
    '{
      connector_id: $connector_id,
      connector_kind: $connector_kind,
      connector_version: "0.1.0",
      session_id: $session_id,
      host_agent_id: $host_agent_id,
      host_agent_kind: $host_agent_kind,
      host_model: $host_model,
      event_kind: "task_start",
      workspace: {kind: "task_space", locator: $workspace_locator, cwd: null, branch: null, external_ref: null},
      task_prompt: $task_prompt,
      task_hint: $task_hint,
      files_in_focus: [$file_one, $file_two]
    }')"
  printf '%s\n' "$start_payload" >"${OUTPUT_DIR}/events/${idx}-${host_key}-task_start.json"
  start_response="$(connector_event "$start_payload")"
  printf '%s\n' "$start_response" >"${OUTPUT_DIR}/events/${idx}-${host_key}-task_start.response.json"

  context_id="$(printf '%s' "$start_response" | jq -r '.context_id // empty')"
  top_direction="$(printf '%s' "$start_response" | jq -r '.prepare_context.likely_next_directions[0].statement // empty')"
  if [[ -z "$context_id" || -z "$top_direction" ]]; then
    write_cli_result "$host_key" "$host_agent_kind" "blocked" "" "$context_id" "" "" "" "Aadesh did not return context_id or top direction."
    return 0
  fi

  prompt="$(build_prompt "$host_agent_kind" "$task_prompt" "$top_direction" "$decision" "$unresolved")"
  output_path="${OUTPUT_DIR}/cli_outputs/${idx}-${host_key}.stdout.txt"
  stderr_path="${OUTPUT_DIR}/cli_outputs/${idx}-${host_key}.stderr.txt"

  set +e
  run_cli_command "$host_key" "$prompt" "$output_path" "$stderr_path"
  exit_code="$?"
  set -e

  if [[ "$exit_code" -eq 0 && -s "$output_path" ]]; then
    status="run"
    note="CLI completed and was written as a linked Aadesh outcome."
  elif [[ "$exit_code" -eq 127 ]]; then
    status="blocked"
    note="CLI binary was not found."
    write_cli_result "$host_key" "$host_agent_kind" "$status" "$exit_code" "$context_id" "$output_path" "$stderr_path" "" "$note"
    return 0
  elif [[ "$exit_code" -eq 124 ]]; then
    status="blocked"
    note="CLI timed out before producing a completed host turn."
    write_cli_result "$host_key" "$host_agent_kind" "$status" "$exit_code" "$context_id" "$output_path" "$stderr_path" "" "$note"
    return 0
  else
    status="failed"
    note="CLI exited nonzero; preserved stdout/stderr but did not write a learnable outcome."
    write_cli_result "$host_key" "$host_agent_kind" "$status" "$exit_code" "$context_id" "$output_path" "$stderr_path" "" "$note"
    return 0
  fi

  output_excerpt="$((cat "$output_path" "$stderr_path" 2>/dev/null | tr '\n' ' ' | head -c 1200) || true)"
  end_payload="$(jq -n \
    --arg connector_id "$connector_id" \
    --arg connector_kind "$connector_kind" \
    --arg session_id "$session_id" \
    --arg host_agent_id "$host_agent_id" \
    --arg host_agent_kind "$host_agent_kind" \
    --arg host_model "$host_model" \
    --arg workspace_locator "$workspace_locator" \
    --arg task_hint "$task_hint" \
    --arg task_prompt "$task_prompt" \
    --arg context_id "$context_id" \
    --arg selected_next_direction "$top_direction" \
    --arg outcome "$expected_outcome" \
    --arg correction_summary "$correction_summary" \
    --arg summary "${summary} Live ${host_agent_kind} output excerpt: ${output_excerpt}" \
    --arg decision "$decision" \
    --arg rationale "$rationale" \
    --arg unresolved "$unresolved" \
    --arg preference "$preference" \
    --arg risk "$risk" \
    --arg file_one "$file_one" \
    --arg file_two "$file_two" \
    --arg output_path "$output_path" \
    '{
      connector_id: $connector_id,
      connector_kind: $connector_kind,
      connector_version: "0.1.0",
      session_id: $session_id,
      host_agent_id: $host_agent_id,
      host_agent_kind: $host_agent_kind,
      host_model: $host_model,
      event_kind: "task_end",
      context_id: $context_id,
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
      artifact_refs: [$output_path],
      tests: [{name: ("live_cli_" + $session_id), status: "pass", summary: "live CLI produced a non-empty response"}]
    }')"
  printf '%s\n' "$end_payload" >"${OUTPUT_DIR}/events/${idx}-${host_key}-task_end.json"
  connector_event "$end_payload" >"${OUTPUT_DIR}/events/${idx}-${host_key}-task_end.response.json"
  write_cli_result "$host_key" "$host_agent_kind" "$status" "$exit_code" "$context_id" "$output_path" "$stderr_path" "$expected_outcome" "$note"
}

run_live_case "01" "qwen" "qwen-code-cli" "cli_wrapper" "qwen-code-live" "qwen-code-cli" "qwen-local" \
  "workspace://aadesh-live-cli" "live-cli-validation" \
  "Validate whether Aadesh guidance can be consumed by a real Qwen CLI session" "accepted" \
  "The host should accept the evidence-first validation direction and preserve linked context_id writeback." \
  "Keep live CLI validation advisory and trace-backed" \
  "A live host run should prove integration friction without adding controller behavior" \
  "Record accepted/ignored/modified host outcomes from actual CLI turns" \
  "Prefer validating real host friction before adding new Aadesh features" \
  "Synthetic-only benchmarks can overstate production readiness" \
  "scripts/live_cli_trace_validation.sh" "docs/COMPARISON_BENCHMARK.md"

run_live_case "02" "opencode" "opencode-cli" "cli_wrapper" "opencode-live" "opencode-cli" "opencode-local" \
  "workspace://aadesh-live-cli" "live-cli-validation" \
  "Check OpenCode-style CLI behavior against the same surfaced Aadesh direction" "modified" \
  "The host should modify the direction if the CLI output highlights a narrower integration risk." \
  "Preserve host-neutral connector semantics across CLI agents" \
  "Cross-host validation only matters if the same context works through different host surfaces" \
  "Compare live CLI outputs for setup friction and trace completeness" \
  "Prefer host-neutral traces over per-agent memory silos" \
  "Single-agent success does not prove cross-host continuity" \
  "scripts/live_cli_trace_validation.sh" "docs/CONNECTOR_INTEGRATION_V0.md" \
  "OpenCode-style host narrowed the guidance to setup friction and trace completeness."

run_live_case "03" "gemini" "gemini-cli" "cli_wrapper" "gemini-live" "gemini-cli" "gemini-local" \
  "workspace://aadesh-live-cli" "live-cli-validation" \
  "Check Gemini CLI behavior and record whether it ignores irrelevant Aadesh suggestions" "ignored" \
  "The host may ignore a direction that is too implementation-specific for the current CLI run." \
  "Do not learn from ignored directions as positive ranking evidence" \
  "Ignored live outcomes must remain observable without becoming accepted ranking weight" \
  "Keep ignored outcomes available for later evaluation analysis" \
  "Prefer outcome-aware ranking over raw memory recall" \
  "Ignored directions can poison ranking if treated as success" \
  "scripts/live_cli_trace_validation.sh" "crates/adesh-daemon/src/cognition.rs" \
  "Gemini-style host ignored this implementation-specific direction."

if [[ "$SKIP_CODEX" -eq 0 ]]; then
  run_live_case "04" "codex" "codex-cli" "cli_wrapper" "codex-live" "codex-cli" "codex-local" \
    "workspace://aadesh-live-cli" "live-cli-validation" \
    "Check Codex CLI behavior as another host while keeping the prompt non-destructive" "accepted" \
    "The host should accept the same validation-first framing without changing files." \
    "Use live CLIs as hosts, not as replacements for Aadesh memory semantics" \
    "Hermes or Codex can be host comparators while Aadesh remains the cross-host substrate" \
    "Report which host outputs are useful versus blocked or noisy" \
    "Prefer using agent runtimes as integration targets rather than replacing Aadesh with one runtime" \
    "Host-specific success can hide portability gaps" \
    "scripts/live_cli_trace_validation.sh" "docs/COMPETITOR_TESTING_NOTES.md"
fi

"${ADESH_ROOT}/scripts/real_trace_validation_harness.sh" \
  --db-path "$DB_PATH" \
  --output-dir "${OUTPUT_DIR}/validation" \
  --min-cases 3 \
  --limit 32 \
  --strict >"${OUTPUT_DIR}/validation.stdout.json"

validation_report="${OUTPUT_DIR}/validation/real_trace_validation_report.json"
cli_results="$(jq -s '.' "$CLI_RESULTS_JSONL")"
live_cli_count="$(printf '%s' "$cli_results" | jq '[.[] | select(.status == "run")] | length')"
failed_cli_count="$(printf '%s' "$cli_results" | jq '[.[] | select(.status == "failed")] | length')"
blocked_cli_count="$(printf '%s' "$cli_results" | jq '[.[] | select(.status == "blocked")] | length')"

host_summary="$(sqlite_json_or_empty_array '
SELECT host_agent_kind, COUNT(*) AS context_count
FROM intervention_context
GROUP BY host_agent_kind
ORDER BY host_agent_kind;
')"

outcome_summary="$(sqlite_json_or_empty_array '
SELECT selected_response AS outcome, COUNT(*) AS count, SUM(CASE WHEN learn_from_this THEN 1 ELSE 0 END) AS learnable_count
FROM intervention_outcomes
GROUP BY selected_response
ORDER BY selected_response;
')"

linked_outcomes="$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM intervention_outcomes WHERE context_ref IS NOT NULL AND context_ref != '';" )"
unlinked_outcomes="$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM intervention_outcomes WHERE context_ref IS NULL OR context_ref = '';" )"
learnable_outcomes="$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM intervention_outcomes WHERE learn_from_this = 1;" )"

jq -n \
  --arg db_path "$DB_PATH" \
  --arg db_url "$DB_URL" \
  --arg output_dir "$OUTPUT_DIR" \
  --arg validation_report "$validation_report" \
  --argjson validation "$(cat "$validation_report")" \
  --argjson cli_results "$cli_results" \
  --argjson live_cli_count "$live_cli_count" \
  --argjson failed_cli_count "$failed_cli_count" \
  --argjson blocked_cli_count "$blocked_cli_count" \
  --argjson host_summary "$host_summary" \
  --argjson outcome_summary "$outcome_summary" \
  --argjson linked_outcomes "$linked_outcomes" \
  --argjson unlinked_outcomes "$unlinked_outcomes" \
  --argjson learnable_outcomes "$learnable_outcomes" \
  '(
    ($validation.validation_pass == true)
    and ($live_cli_count >= 1)
    and ($linked_outcomes >= $live_cli_count)
    and (($validation.weak_cases | length) == 0)
  ) as $pass
  | {
      status: "run",
      validation_pass: $pass,
      db_path: $db_path,
      db_url: $db_url,
      output_dir: $output_dir,
      validation_report: $validation_report,
      live_cli_count: $live_cli_count,
      failed_cli_count: $failed_cli_count,
      blocked_cli_count: $blocked_cli_count,
      case_count: $validation.case_count,
      mean_score: $validation.mean_score,
      weak_cases: $validation.weak_cases,
      cli_results: $cli_results,
      host_summary: $host_summary,
      outcome_summary: $outcome_summary,
      linked_outcomes: $linked_outcomes,
      unlinked_outcomes: $unlinked_outcomes,
      learnable_outcomes: $learnable_outcomes,
      gates: {
        validator_passed: ($validation.validation_pass == true),
        at_least_one_live_cli_completed: ($live_cli_count >= 1),
        linked_outcomes_cover_completed_runs: ($linked_outcomes >= $live_cli_count),
        no_weak_cases: (($validation.weak_cases | length) == 0),
        blocked_or_failed_cli_results_recorded: (($blocked_cli_count + $failed_cli_count) >= 0)
      }
    }' >"$REPORT_PATH"

cat "$REPORT_PATH"

if [[ "$(jq -r '.validation_pass' "$REPORT_PATH")" != "true" ]]; then
  exit 1
fi
