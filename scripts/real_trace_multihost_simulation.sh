#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: ./scripts/real_trace_multihost_simulation.sh [options]

Simulates production-shaped multi-host Aadesh usage through the public host
connector path, then validates the generated DB with real_trace_validation_harness.sh.

It covers:
  - codex-vscode, qwen-code-cli, and opencode-cli host kinds
  - multiple workspaces/workstreams
  - accepted, ignored, and modified outcomes
  - one intentional degraded unlinked outcome
  - unrelated workspace noise

Options:
  --output-dir DIR      Output directory. Default: /tmp/adesh-real-trace-multihost-<run_id>.
  --db-url URL          SQLite URL. Default: sqlite://<output-dir>/multihost.db?mode=rwc.
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

OUTPUT_DIR="${OUTPUT_DIR:-/tmp/adesh-real-trace-multihost-${RUN_ID}}"
mkdir -p "$OUTPUT_DIR/events" "$OUTPUT_DIR/validation"
DB_URL="${DB_URL:-sqlite://${OUTPUT_DIR}/multihost.db?mode=rwc}"
DB_PATH="${DB_URL#sqlite://}"
DB_PATH="${DB_PATH%%\?*}"
REPORT_PATH="${OUTPUT_DIR}/real_trace_multihost_report.json"

run_daemon() {
  ADESH_DATABASE_URL="$DB_URL" \
    CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
    cargo run -q --manifest-path "${ADESH_ROOT}/Cargo.toml" -p adesh-daemon -- "$@"
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

run_linked_case() {
  local idx="$1"
  local connector_id="$2"
  local connector_kind="$3"
  local host_agent_id="$4"
  local host_agent_kind="$5"
  local host_model="$6"
  local workspace_locator="$7"
  local task_hint="$8"
  local task_prompt="$9"
  local outcome="${10}"
  local summary="${11}"
  local decision="${12}"
  local rationale="${13}"
  local unresolved="${14}"
  local preference="${15}"
  local risk="${16}"
  local file_one="${17}"
  local file_two="${18}"
  local correction_summary="${19:-}"

  local session_id="multihost-${RUN_ID}-${idx}"
  local start_payload start_response context_id top_direction end_payload end_response

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
  printf '%s\n' "$start_payload" >"${OUTPUT_DIR}/events/${idx}-task_start.json"
  start_response="$(connector_event "$start_payload")"
  printf '%s\n' "$start_response" >"${OUTPUT_DIR}/events/${idx}-task_start.response.json"

  context_id="$(printf '%s' "$start_response" | jq -r '.context_id // empty')"
  top_direction="$(printf '%s' "$start_response" | jq -r '.prepare_context.likely_next_directions[0].statement // empty')"
  if [[ -z "$context_id" || -z "$top_direction" ]]; then
    echo "linked case ${idx} did not receive context_id/top direction" >&2
    exit 1
  fi

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
      tests: [{name: ("multihost_" + $session_id), status: "pass", summary: "deterministic multi-host trace case stored"}]
    }')"
  printf '%s\n' "$end_payload" >"${OUTPUT_DIR}/events/${idx}-task_end.json"
  end_response="$(connector_event "$end_payload")"
  printf '%s\n' "$end_response" >"${OUTPUT_DIR}/events/${idx}-task_end.response.json"
}

run_degraded_unlinked_case() {
  local idx="$1"
  local workspace_locator="$2"
  local task_hint="$3"
  local task_prompt="$4"
  local summary="$5"
  local decision="$6"
  local unresolved="$7"
  local preference="$8"
  local risk="$9"

  local session_id="multihost-${RUN_ID}-${idx}"
  local payload response
  payload="$(jq -n \
    --arg session_id "$session_id" \
    --arg workspace_locator "$workspace_locator" \
    --arg task_hint "$task_hint" \
    --arg task_prompt "$task_prompt" \
    --arg summary "$summary" \
    --arg decision "$decision" \
    --arg unresolved "$unresolved" \
    --arg preference "$preference" \
    --arg risk "$risk" \
    '{
      connector_id: "degraded-host-sim",
      connector_kind: "degraded_background_capture",
      connector_version: "0.1.0",
      session_id: $session_id,
      host_agent_id: "degraded-background-watcher",
      host_agent_kind: "background-watcher",
      host_model: "none",
      event_kind: "task_end",
      selected_next_direction: "degraded host wrote an outcome without returned context_id",
      outcome: "ignored",
      correction_summary: "No task_start context_id was available, so this should remain unlearnable.",
      workspace: {kind: "task_space", locator: $workspace_locator, cwd: null, branch: null, external_ref: null},
      task_prompt: $task_prompt,
      task_hint: $task_hint,
      summary: $summary,
      files_touched: ["docs/degraded-host-note.md"],
      decisions: [{decision: $decision, rationale: "Captured only from degraded background evidence"}],
      unresolved_items: [$unresolved],
      observed_preferences: [$preference],
      risk_signals: [$risk]
    }')"
  printf '%s\n' "$payload" >"${OUTPUT_DIR}/events/${idx}-degraded-task_end.json"
  response="$(connector_event "$payload")"
  printf '%s\n' "$response" >"${OUTPUT_DIR}/events/${idx}-degraded-task_end.response.json"
}

# Core cross-host trace set.
run_linked_case "01" "codex-vscode" "chat_extension" "codex-vscode-user" "codex-vscode" "gpt-5-codex" \
  "workspace://aadesh-core" "proof-validation" \
  "Validate Aadesh after OpenMemory nearly matched recall" "accepted" \
  "Accepted proof-first guidance and narrowed the next step to real trace validation before adding integrations." \
  "Do not claim differentiation on memory recall alone" \
  "OpenMemory can match recall, so Aadesh must prove outcome-aware supervision" \
  "Run real host trace validation before adding new features" \
  "Prefer competitor comparisons that separate recall from outcome-trace learning" \
  "A generic memory-server direction would erase the supervisory wedge" \
  "scripts/external_memory_comparison_harness.sh" "scripts/real_trace_validation_harness.sh"

run_linked_case "02" "qwen-code-cli" "cli_wrapper" "qwen-code-user" "qwen-code-cli" "qwen/qwen3.6-27b" \
  "workspace://aadesh-core" "connector-observability" \
  "Harden connector context_id examples for CLI hosts" "ignored" \
  "Ignored the evidence-example direction and did wrapper cleanup first; accepted/ignored examples stayed open." \
  "Propagate returned context_id from task_start to task_end" \
  "Linked outcomes require stable context references" \
  "Add one accepted and one ignored outcome example with context_id" \
  "Prefer concrete connector examples over abstract lifecycle language" \
  "Without context_id examples, hosts may write unlearnable outcome traces" \
  "crates/adesh-daemon/src/connector_adapter.rs" "docs/CONNECTOR_INTEGRATION_V0.md" \
  "Host chose cleanup before evidence examples."

run_linked_case "03" "opencode-cli" "cli_wrapper" "opencode-user" "opencode-cli" "opencode-local" \
  "workspace://payments-service" "payment-reliability" \
  "Continue retry idempotency hardening" "modified" \
  "Modified the suggestion by keeping service-boundary idempotency but switching the first proof to packet-loss timeout coverage." \
  "Keep payment idempotency at the service boundary" \
  "Service-boundary evidence is easier to audit across hosts" \
  "Add timeout coverage for retry under packet loss" \
  "Prefer service-boundary tests for reliability-sensitive retry work" \
  "Retry replay can duplicate charges without timeout evidence" \
  "src/payments/retry_worker.rs" "tests/payments/retry_timeout.rs" \
  "Suggestion was narrowed to timeout coverage before replay proof."

run_linked_case "04" "codex-vscode" "chat_extension" "codex-vscode-user" "codex-vscode" "gpt-5-codex" \
  "workspace://aadesh-docs" "docs-polish" \
  "Write project status without overstating readiness" "accepted" \
  "Accepted the metrics-first status direction and included benchmark evidence before conclusions." \
  "Status updates must include benchmark metrics before readiness claims" \
  "The comparison showed OpenMemory strong on recall but absent outcome traces" \
  "Add a metrics table before any status conclusion" \
  "Prefer evidence-first status updates over broad roadmap claims" \
  "Status notes without metrics can misstate confidence" \
  "docs/COMPARISON_BENCHMARK.md" "docs/IMPLEMENTATION_REPORT.md"

run_linked_case "05" "qwen-code-cli" "cli_wrapper" "qwen-code-user" "qwen-code-cli" "qwen/qwen3.6-27b" \
  "workspace://nonrepo-research" "research-synthesis" \
  "Summarize competitor findings for a non-repo planning task" "modified" \
  "Modified the guidance by separating Hermes runtime comparison from OpenMemory retrieval comparison." \
  "Treat Hermes as a host/runtime comparator, not a memory-layer replacement" \
  "Collect one concrete Hermes plugin/integration path before deciding integration priority" \
  "Prefer separate comparator classes for runtime agents and memory layers" \
  "Collapsing Hermes and OpenMemory into one bucket hides product tradeoffs" \
  "notes/hermes-openmemory-comparison.md" "docs/COMPETITOR_TESTING_NOTES.md" \
  "Kept comparison but split runtime vs memory-layer interpretation."

# Unrelated noise that should not break target retrieval but should still validate.
run_linked_case "06" "opencode-cli" "cli_wrapper" "opencode-user" "opencode-cli" "opencode-local" \
  "workspace://unrelated-ui-polish" "ui-copy" \
  "Polish onboarding copy for an unrelated UI task" "ignored" \
  "Ignored the evidence-first recommendation and edited headings first; the concrete before/after example remained open." \
  "Open onboarding with one concrete before-after example" \
  "Concrete examples prevent abstract copy from sounding generic" \
  "Add one before-after onboarding example before typography cleanup" \
  "Prefer example-led UX copy for onboarding changes" \
  "Typography polish can hide missing onboarding evidence" \
  "apps/web/src/onboarding/Intro.tsx" "docs/onboarding-copy.md" \
  "Host did visual cleanup before adding the example."

# One intentional degraded trace: should be persisted, unlinked, and unlearnable.
run_degraded_unlinked_case "07" \
  "workspace://aadesh-core" "background-capture" \
  "Background watcher noticed local cleanup but no task_start context" \
  "A degraded watcher captured cleanup activity without a returned context_id." \
  "Do not learn from unlinked degraded watcher outcomes" \
  "Re-run with explicit task_start if this cleanup matters" \
  "Prefer explicit connector sessions when outcome learning is required" \
  "Unlinked watcher traces can poison ranking if treated as learnable"

"${ADESH_ROOT}/scripts/real_trace_validation_harness.sh" \
  --db-path "$DB_PATH" \
  --output-dir "${OUTPUT_DIR}/validation" \
  --min-cases 6 \
  --limit 32 \
  --strict >"${OUTPUT_DIR}/validation.stdout.json"

validation_report="${OUTPUT_DIR}/validation/real_trace_validation_report.json"

host_summary="$(sqlite3 -json "$DB_PATH" '
SELECT host_agent_kind, COUNT(*) AS context_count
FROM intervention_context
GROUP BY host_agent_kind
ORDER BY host_agent_kind;
')"

outcome_summary="$(sqlite3 -json "$DB_PATH" '
SELECT selected_response AS outcome, COUNT(*) AS count, SUM(CASE WHEN learn_from_this THEN 1 ELSE 0 END) AS learnable_count
FROM intervention_outcomes
GROUP BY selected_response
ORDER BY selected_response;
')"

linked_outcomes="$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM intervention_outcomes WHERE context_ref IS NOT NULL AND context_ref != '';" )"
unlinked_outcomes="$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM intervention_outcomes WHERE context_ref IS NULL OR context_ref = '';" )"
learnable_outcomes="$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM intervention_outcomes WHERE learn_from_this = 1;" )"
host_kind_count="$(printf '%s' "$host_summary" | jq 'map(.host_agent_kind) | unique | length')"

accepted_present="$(printf '%s' "$outcome_summary" | jq 'map(.outcome) | index("accepted") != null')"
ignored_present="$(printf '%s' "$outcome_summary" | jq 'map(.outcome) | index("ignored") != null')"
modified_present="$(printf '%s' "$outcome_summary" | jq 'map(.outcome) | index("modified") != null')"

jq -n \
  --arg db_path "$DB_PATH" \
  --arg db_url "$DB_URL" \
  --arg output_dir "$OUTPUT_DIR" \
  --arg validation_report "$validation_report" \
  --argjson validation "$(cat "$validation_report")" \
  --argjson host_summary "$host_summary" \
  --argjson outcome_summary "$outcome_summary" \
  --argjson linked_outcomes "$linked_outcomes" \
  --argjson unlinked_outcomes "$unlinked_outcomes" \
  --argjson learnable_outcomes "$learnable_outcomes" \
  --argjson host_kind_count "$host_kind_count" \
  --argjson accepted_present "$accepted_present" \
  --argjson ignored_present "$ignored_present" \
  --argjson modified_present "$modified_present" \
  '(
    ($validation.validation_pass == true)
    and ($host_kind_count >= 3)
    and ($linked_outcomes >= 6)
    and ($unlinked_outcomes == 1)
    and ($learnable_outcomes == $linked_outcomes)
    and $accepted_present
    and $ignored_present
    and $modified_present
    and (($validation.weak_cases | length) == 0)
  ) as $pass
  | {
      status: "run",
      validation_pass: $pass,
      db_path: $db_path,
      db_url: $db_url,
      output_dir: $output_dir,
      validation_report: $validation_report,
      case_count: $validation.case_count,
      mean_score: $validation.mean_score,
      weak_cases: $validation.weak_cases,
      host_summary: $host_summary,
      outcome_summary: $outcome_summary,
      linked_outcomes: $linked_outcomes,
      unlinked_outcomes: $unlinked_outcomes,
      learnable_outcomes: $learnable_outcomes,
      gates: {
        validator_passed: ($validation.validation_pass == true),
        at_least_three_host_kinds: ($host_kind_count >= 3),
        linked_outcomes_at_least_six: ($linked_outcomes >= 6),
        exactly_one_degraded_unlinked_trace: ($unlinked_outcomes == 1),
        all_linked_outcomes_learnable: ($learnable_outcomes == $linked_outcomes),
        accepted_present: $accepted_present,
        ignored_present: $ignored_present,
        modified_present: $modified_present,
        no_weak_cases: (($validation.weak_cases | length) == 0)
      }
    }' >"$REPORT_PATH"

cat "$REPORT_PATH"

if [[ "$(jq -r '.validation_pass' "$REPORT_PATH")" != "true" ]]; then
  exit 1
fi
