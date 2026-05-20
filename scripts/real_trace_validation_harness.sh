#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: ./scripts/real_trace_validation_harness.sh [options]

Validates Aadesh guidance quality against an existing real/session-learning DB.
Use --seed-fixture for a deterministic local proof, or point --db-path/--db-url
at a DB populated by session_learning_capture.sh, session_learning_watcher.sh, or
host connector events.

Options:
  --db-url URL          SQLite URL to validate, e.g. sqlite:///tmp/aadesh.db?mode=rwc.
  --db-path PATH        SQLite DB path to validate. Converted to sqlite://PATH?mode=rwc.
  --output-dir DIR      Output directory. Default: /tmp/adesh-real-trace-validation-<run_id>.
  --limit N             Max recent episodes to convert into validation cases. Default: 9.
  --min-cases N         Minimum cases required for pass. Default: 3.
  --task-hint HINT      Optional task hint filter, without task:hint: prefix.
  --seed-fixture        Create a deterministic fixture DB before validating.
  --strict              Exit nonzero if the validation gate fails.
  -h, --help            Show help.

Report:
  <output-dir>/real_trace_validation_report.json
  <output-dir>/cases.jsonl
  <output-dir>/prepare/*.json
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
DB_URL="${ADESH_DATABASE_URL:-}"
DB_PATH=""
LIMIT=9
MIN_CASES=3
TASK_HINT=""
SEED_FIXTURE=0
STRICT=0
CARGO_TARGET_DIR="${ADESH_CARGO_TARGET_DIR:-/tmp/adesh-cargo-target}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --db-url)
      DB_URL="${2:-}"
      shift 2
      ;;
    --db-path)
      DB_PATH="${2:-}"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="${2:-}"
      shift 2
      ;;
    --limit)
      LIMIT="${2:-}"
      shift 2
      ;;
    --min-cases)
      MIN_CASES="${2:-}"
      shift 2
      ;;
    --task-hint)
      TASK_HINT="${2:-}"
      shift 2
      ;;
    --seed-fixture)
      SEED_FIXTURE=1
      shift
      ;;
    --strict)
      STRICT=1
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

if ! [[ "$LIMIT" =~ ^[0-9]+$ ]] || [[ "$LIMIT" -lt 1 ]]; then
  echo "--limit must be a positive integer" >&2
  exit 1
fi
if ! [[ "$MIN_CASES" =~ ^[0-9]+$ ]] || [[ "$MIN_CASES" -lt 1 ]]; then
  echo "--min-cases must be a positive integer" >&2
  exit 1
fi

OUTPUT_DIR="${OUTPUT_DIR:-/tmp/adesh-real-trace-validation-${RUN_ID}}"
mkdir -p "$OUTPUT_DIR/prepare"
REPORT_PATH="${OUTPUT_DIR}/real_trace_validation_report.json"
CASES_JSONL="${OUTPUT_DIR}/cases.jsonl"
: >"$CASES_JSONL"

if [[ -n "$DB_PATH" ]]; then
  DB_URL="sqlite://${DB_PATH}?mode=rwc"
elif [[ -n "$DB_URL" ]]; then
  case "$DB_URL" in
    sqlite://*)
      DB_PATH="${DB_URL#sqlite://}"
      DB_PATH="${DB_PATH%%\?*}"
      ;;
    *)
      echo "only sqlite:// DB URLs are supported" >&2
      exit 1
      ;;
  esac
else
  DB_PATH="${OUTPUT_DIR}/real_trace_fixture.db"
  DB_URL="sqlite://${DB_PATH}?mode=rwc"
fi

run_daemon() {
  ADESH_DATABASE_URL="$DB_URL" \
    CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
    cargo run -q --manifest-path "${ADESH_ROOT}/Cargo.toml" -p adesh-daemon -- "$@"
}

normalize_hint() {
  printf '%s' "$1" \
    | tr '[:upper:]' '[:lower:]' \
    | sed -E 's/[^a-z0-9]+/_/g' \
    | sed -E 's/^_+|_+$//g'
}

seed_fixture_store() {
  local workspace_locator="$1"
  local task_hint="$2"
  local task="$3"
  local summary="$4"
  local decision="$5"
  local unresolved="$6"
  local preference="$7"
  local risk="$8"
  shift 8
  local files=("$@")

  local cmd=(host store
    --workspace-kind task_space
    --workspace-locator "$workspace_locator"
    --task-hint "$task_hint"
    --task "$task"
    --summary "$summary"
    --decision "$decision"
    --unresolved "$unresolved"
    --preference "$preference"
    --risk "$risk")
  for file in "${files[@]}"; do
    cmd+=(--file "$file")
  done
  run_daemon "${cmd[@]}" >/dev/null
}

seed_fixture_linked_outcome() {
  local workspace_locator="$1"
  local task_hint="$2"
  local task="$3"
  local summary="$4"
  local outcome="$5"

  local start_payload
  start_payload="$(jq -n \
    --arg workspace_locator "$workspace_locator" \
    --arg task_hint "$task_hint" \
    --arg task "$task" \
    --arg session_id "real-trace-fixture-${RUN_ID}-${task_hint}" \
    '{
      connector_id: "fixture-codex-vscode",
      connector_kind: "chat_extension",
      connector_version: "0.1.0",
      session_id: $session_id,
      host_agent_id: "fixture-codex",
      host_agent_kind: "codex-vscode",
      host_model: "fixture-local",
      event_kind: "task_start",
      workspace: {kind: "task_space", locator: $workspace_locator, cwd: null, branch: null, external_ref: null},
      task_prompt: $task,
      task_hint: $task_hint,
      files_in_focus: []
    }')"

  local start_response context_id top_direction
  start_response="$(run_daemon host connector --json "$start_payload")"
  context_id="$(printf '%s' "$start_response" | jq -r '.context_id // empty')"
  top_direction="$(printf '%s' "$start_response" | jq -r '.prepare_context.likely_next_directions[0].statement // "No direction surfaced"')"

  local end_payload
  end_payload="$(jq -n \
    --arg workspace_locator "$workspace_locator" \
    --arg task_hint "$task_hint" \
    --arg task "$task" \
    --arg summary "$summary" \
    --arg session_id "real-trace-fixture-${RUN_ID}-${task_hint}" \
    --arg context_id "$context_id" \
    --arg top_direction "$top_direction" \
    --arg outcome "$outcome" \
    '{
      connector_id: "fixture-codex-vscode",
      connector_kind: "chat_extension",
      connector_version: "0.1.0",
      session_id: $session_id,
      host_agent_id: "fixture-codex",
      host_agent_kind: "codex-vscode",
      host_model: "fixture-local",
      context_id: (if ($context_id | length) > 0 then $context_id else null end),
      selected_next_direction: $top_direction,
      outcome: $outcome,
      event_kind: "task_end",
      workspace: {kind: "task_space", locator: $workspace_locator, cwd: null, branch: null, external_ref: null},
      task_prompt: $task,
      task_hint: $task_hint,
      summary: $summary,
      files_touched: ["scripts/real_trace_validation_harness.sh"],
      decisions: [{decision: "Preserve the supervisory trace path as advisory-only", rationale: "Real-use validation should not introduce controller behavior"}],
      unresolved_items: ["Collect more real host traces before changing ranking weights"],
      observed_preferences: ["Prefer evidence-backed validation before expanding integrations"],
      risk_signals: ["Single-session traces can overstate guidance quality"]
    }')"

  run_daemon host connector --json "$end_payload" >/dev/null
}

if [[ "$SEED_FIXTURE" -eq 1 ]]; then
  rm -f "$DB_PATH"
  seed_fixture_store \
    "workspace://real-trace-payments" \
    "payment-reliability" \
    "Stabilize retry idempotency under packet loss" \
    "Captured a real coding-session style episode: service-boundary idempotency stayed fixed, timeout coverage remained open, and retry replay needed evidence." \
    "Keep idempotency at the service boundary" \
    "Add timeout coverage for retry under packet loss" \
    "Prefer service-boundary tests for reliability-sensitive retry work" \
    "Retry replay can duplicate charges without timeout evidence" \
    "src/payments/retry_worker.rs" "tests/payments/retry_timeout.rs"

  seed_fixture_store \
    "workspace://real-trace-connectors" \
    "connector-observability" \
    "Harden connector context_id round trip examples" \
    "Captured connector work: context_id propagation was the durable decision, accepted/ignored examples stayed missing, and docs needed concrete outcome traces." \
    "Propagate returned context_id from task_start to task_end" \
    "Add one accepted and one ignored outcome example with context_id" \
    "Prefer concrete connector examples over abstract lifecycle language" \
    "Without context_id examples, hosts may write unlearnable outcome traces" \
    "crates/adesh-daemon/src/connector_adapter.rs" "docs/CONNECTOR_INTEGRATION_V0.md"

  seed_fixture_store \
    "workspace://real-trace-eval" \
    "proof-validation" \
    "Validate the Aadesh wedge against memory-only competitors" \
    "Captured benchmark work: OpenMemory nearly matched recall, so the wedge must stay centered on outcome traces and cross-host supervision." \
    "Do not claim differentiation on memory recall alone" \
    "Run real host trace validation before adding new features" \
    "Prefer competitor comparisons that separate recall from outcome-trace learning" \
    "A generic memory-server direction would erase the supervisory wedge" \
    "scripts/external_memory_comparison_harness.sh" "docs/COMPARISON_BENCHMARK.md"

  seed_fixture_linked_outcome \
    "workspace://real-trace-eval" \
    "proof-validation" \
    "Continue wedge validation using captured host traces" \
    "Accepted the proof-validation direction and created a trace-backed validation path." \
    "accepted"
fi

if [[ ! -f "$DB_PATH" ]]; then
  echo "database file not found: $DB_PATH" >&2
  exit 1
fi

where_parts=("1=1")
if [[ -n "$TASK_HINT" ]]; then
  normalized_hint="$(normalize_hint "$TASK_HINT")"
  where_parts+=("task_scope_key = 'task:hint:${normalized_hint}'")
fi
where_clause="$(printf ' AND %s' "${where_parts[@]}")"
where_clause="${where_clause# AND }"

sql="
SELECT
  episode_id,
  scope_type,
  scope_key,
  task_scope_key,
  workspace_json,
  task_prompt,
  summary,
  files_touched_json,
  decisions_json,
  unresolved_items_json,
  observed_preferences_json,
  risk_signals_json,
  ended_at
FROM episodes
WHERE ${where_clause}
  AND (
    json_array_length(decisions_json) > 0
    OR json_array_length(unresolved_items_json) > 0
    OR json_array_length(observed_preferences_json) > 0
    OR json_array_length(risk_signals_json) > 0
  )
ORDER BY ended_at DESC, created_at DESC
LIMIT ${LIMIT};
"

episodes_json="$(sqlite3 -json "$DB_PATH" "$sql")"

cases_json="$(printf '%s' "$episodes_json" | jq '
  def parse_json_array($s): (($s // "[]") | fromjson? // []);
  def string_value($v):
    if ($v | type) == "object" then
      ($v.decision // $v.statement // $v.item // $v.summary // ($v | tostring))
    else ($v // "" | tostring) end;
  def first_text($items): ([$items[]? | string_value(.) | select(length > 0)] | first // "");
  def task_hint_from_scope($scope): (($scope // "") | sub("^task:hint:"; ""));
  map(
    .decisions = parse_json_array(.decisions_json)
    | .unresolved_items = parse_json_array(.unresolved_items_json)
    | .observed_preferences = parse_json_array(.observed_preferences_json)
    | .risk_signals = parse_json_array(.risk_signals_json)
    | .files_touched = parse_json_array(.files_touched_json)
    | .workspace = ((.workspace_json // "{}") | fromjson? // {})
    | {
        case_id: ("case-" + ((input_line_number // 0) | tostring) + "-" + (.episode_id | gsub("[^A-Za-z0-9_-]"; "_"))),
        source_episode_id: .episode_id,
        workspace_kind: (.workspace.kind // "task_space"),
        workspace_locator: (.workspace.locator // .scope_key),
        task_scope_key: .task_scope_key,
        task_hint: task_hint_from_scope(.task_scope_key),
        task_prompt: ("Continue this captured work: " + .task_prompt),
        files_in_focus: (.files_touched[0:5]),
        expected_decision: first_text(.decisions),
        expected_open_loop: first_text(.unresolved_items),
        expected_preference: first_text(.observed_preferences),
        expected_risk_or_direction: (first_text(.unresolved_items) // first_text(.risk_signals)),
        source_summary: .summary
      }
  )
')"

case_count="$(printf '%s' "$cases_json" | jq 'length')"
if [[ "$case_count" -eq 0 ]]; then
  jq -n \
    --arg db_path "$DB_PATH" \
    --arg report_path "$REPORT_PATH" \
    '{
      status: "no_cases",
      validation_pass: false,
      db_path: $db_path,
      report_path: $report_path,
      reason: "No episodes with decisions, unresolved items, preferences, or risks were found. Capture sessions first or use --seed-fixture."
    }' >"$REPORT_PATH"
  cat "$REPORT_PATH"
  if [[ "$STRICT" -eq 1 ]]; then
    exit 1
  fi
  exit 0
fi

score_prepare() {
  local response_path="$1"
  local expected_decision="$2"
  local expected_open_loop="$3"
  local expected_preference="$4"
  local expected_direction="$5"

  jq \
    --arg expected_decision "$expected_decision" \
    --arg expected_open_loop "$expected_open_loop" \
    --arg expected_preference "$expected_preference" \
    --arg expected_direction "$expected_direction" \
    '
      def text_of($items):
        [$items[]? | [.statement, .basis, ((.evidence_refs // []) | join(" "))] | map(. // "") | join(" ") | ascii_downcase] | join(" ");
      def keywords($s):
        ($s // "" | ascii_downcase | gsub("[^a-z0-9_]+"; " ") | split(" ")
          | map(select(length >= 5))
          | map(select(. as $w | (["about", "after", "before", "still", "there", "their", "which", "should", "would", "could", "under", "using", "without", "captured"] | index($w) | not)))
          | unique | .[0:6]);
      def hit($haystack; $expected):
        (keywords($expected)) as $kw
        | ([$kw[]? | select($haystack | contains(.))]) as $matched
        | {
            expected: $expected,
            keywords: $kw,
            matched_keywords: $matched,
            hit: (if ($kw | length) == 0 then null else (($matched | length) >= (if ($kw | length) >= 2 then 2 else 1 end)) end)
          };
      (text_of(.relevant_decisions)) as $decision_text
      | (text_of(.open_loops)) as $open_text
      | (text_of(.applicable_preferences)) as $preference_text
      | (text_of(.likely_next_directions)) as $direction_text
      | (text_of(.risk_flags)) as $risk_text
      | (hit($decision_text; $expected_decision)) as $decision
      | (hit($open_text; $expected_open_loop)) as $open_loop
      | (hit($preference_text; $expected_preference)) as $preference
      | (hit(($direction_text + " " + $risk_text + " " + $open_text); $expected_direction)) as $direction
      | {
          decision: $decision,
          open_loop: $open_loop,
          preference: $preference,
          direction: $direction,
          score: ([
            $decision.hit,
            $open_loop.hit,
            $preference.hit,
            $direction.hit
          ] | map(select(. != null)) as $checks | if ($checks | length) == 0 then 0 else (($checks | map(if . then 1 else 0 end) | add) / ($checks | length)) end),
          unsupported_items: ([.relevant_decisions[], .applicable_preferences[], .open_loops[], .risk_flags[], .likely_next_directions[]?] | map(select((.evidence_refs // [] | length) == 0)) | length)
        }
    ' "$response_path"
}

case_index=0
printf '%s' "$cases_json" | jq -c '.[]' | while IFS= read -r case_json; do
  case_index=$((case_index + 1))
  case_id="$(printf '%s' "$case_json" | jq -r '.case_id')"
  response_path="${OUTPUT_DIR}/prepare/${case_index}-${case_id}.json"

  task_prompt="$(printf '%s' "$case_json" | jq -r '.task_prompt')"
  workspace_kind="$(printf '%s' "$case_json" | jq -r '.workspace_kind')"
  workspace_locator="$(printf '%s' "$case_json" | jq -r '.workspace_locator')"
  task_hint="$(printf '%s' "$case_json" | jq -r '.task_hint')"
  mapfile -t files < <(printf '%s' "$case_json" | jq -r '.files_in_focus[]?')

  cmd=(host prepare
    --workspace-kind "$workspace_kind"
    --workspace-locator "$workspace_locator"
    --task "$task_prompt")
  if [[ -n "$task_hint" && "$task_hint" != "null" ]]; then
    cmd+=(--task-hint "$task_hint")
  fi
  for file in "${files[@]}"; do
    cmd+=(--file "$file")
  done

  run_daemon "${cmd[@]}" >"$response_path"

  metrics="$(score_prepare \
    "$response_path" \
    "$(printf '%s' "$case_json" | jq -r '.expected_decision')" \
    "$(printf '%s' "$case_json" | jq -r '.expected_open_loop')" \
    "$(printf '%s' "$case_json" | jq -r '.expected_preference')" \
    "$(printf '%s' "$case_json" | jq -r '.expected_risk_or_direction')")"

  jq -cn \
    --argjson case "$case_json" \
    --arg response_path "$response_path" \
    --argjson metrics "$metrics" \
    '$case + {response_path: $response_path, metrics: $metrics}' >>"$CASES_JSONL"
done

outcome_summary="$(sqlite3 -json "$DB_PATH" "
SELECT selected_response AS outcome, COUNT(*) AS count, SUM(CASE WHEN learn_from_this THEN 1 ELSE 0 END) AS learnable_count
FROM intervention_outcomes
GROUP BY selected_response
ORDER BY count DESC;
" 2>/dev/null || true)"
if [[ -z "$outcome_summary" ]] || ! printf '%s' "$outcome_summary" | jq -e . >/dev/null 2>&1; then
  outcome_summary='[]'
fi

linked_outcome_count="$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM intervention_outcomes WHERE context_ref IS NOT NULL AND context_ref != '';" 2>/dev/null || true)"
unlinked_outcome_count="$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM intervention_outcomes WHERE context_ref IS NULL OR context_ref = '';" 2>/dev/null || true)"
if ! [[ "$linked_outcome_count" =~ ^[0-9]+$ ]]; then
  linked_outcome_count=0
fi
if ! [[ "$unlinked_outcome_count" =~ ^[0-9]+$ ]]; then
  unlinked_outcome_count=0
fi

jq -s \
  --arg db_path "$DB_PATH" \
  --arg db_url "$DB_URL" \
  --arg cases_path "$CASES_JSONL" \
  --arg output_dir "$OUTPUT_DIR" \
  --argjson min_cases "$MIN_CASES" \
  --argjson outcome_summary "$outcome_summary" \
  --argjson linked_outcome_count "$linked_outcome_count" \
  --argjson unlinked_outcome_count "$unlinked_outcome_count" \
  '
    . as $cases
    | ($cases | length) as $case_count
    | {
        status: "run",
        db_path: $db_path,
        db_url: $db_url,
        output_dir: $output_dir,
        cases_path: $cases_path,
        case_count: $case_count,
        min_cases: $min_cases,
        mean_score: (if $case_count == 0 then 0 else (($cases | map(.metrics.score) | add) / $case_count) end),
        decision_hit_rate: (if $case_count == 0 then 0 else (($cases | map(if .metrics.decision.hit == true then 1 else 0 end) | add) / $case_count) end),
        open_loop_hit_rate: (if $case_count == 0 then 0 else (($cases | map(if .metrics.open_loop.hit == true then 1 else 0 end) | add) / $case_count) end),
        preference_hit_rate: (if $case_count == 0 then 0 else (($cases | map(if .metrics.preference.hit == true then 1 else 0 end) | add) / $case_count) end),
        direction_hit_rate: (if $case_count == 0 then 0 else (($cases | map(if .metrics.direction.hit == true then 1 else 0 end) | add) / $case_count) end),
        unsupported_items: ($cases | map(.metrics.unsupported_items) | add),
        outcome_trace_summary: {
          by_outcome: $outcome_summary,
          linked_outcome_count: $linked_outcome_count,
          unlinked_outcome_count: $unlinked_outcome_count
        },
        validation_pass: (
          $case_count >= $min_cases
          and (if $case_count == 0 then false else (($cases | map(.metrics.score) | add) / $case_count) >= 0.75 end)
          and ($cases | map(.metrics.unsupported_items) | add) == 0
        ),
        weak_cases: ($cases | map(select(.metrics.score < 0.75)) | map({case_id, source_episode_id, task_scope_key, metrics})),
        cases: $cases
      }
  ' "$CASES_JSONL" >"$REPORT_PATH"

cat "$REPORT_PATH"

if [[ "$STRICT" -eq 1 ]] && [[ "$(jq -r '.validation_pass' "$REPORT_PATH")" != "true" ]]; then
  exit 1
fi
