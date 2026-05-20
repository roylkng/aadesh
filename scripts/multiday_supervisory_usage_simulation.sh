#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: ./scripts/multiday_supervisory_usage_simulation.sh [options]

Simulates multi-day / multi-week Aadesh usage through the public connector path.
The scenario is intentionally temporal: older open loops get resolved, host
outcomes change over time, several host agents contribute traces, and one
non-repo conversation workspace is included.

Options:
  --days N         Number of simulated days. Default: 21.
  --profile NAME   Data profile: standard or production. Default: standard.
  --output-dir D  Directory for report/db/probes. Default: /tmp/adesh-multiday-<run_id>.
  -h, --help      Show this help.

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
DAYS=21
PROFILE="standard"
OUTPUT_DIR=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --days)
      DAYS="${2:-}"
      shift 2
      ;;
    --profile)
      PROFILE="${2:-}"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="${2:-}"
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

if ! [[ "$DAYS" =~ ^[0-9]+$ ]] || [[ "$DAYS" -lt 14 ]]; then
  echo "--days must be an integer >= 14 for the multi-week scenario" >&2
  exit 1
fi
if [[ "$PROFILE" != "standard" && "$PROFILE" != "production" ]]; then
  echo "--profile must be standard or production" >&2
  exit 1
fi

OUTPUT_DIR="${OUTPUT_DIR:-/tmp/adesh-multiday-${RUN_ID}}"
mkdir -p "$OUTPUT_DIR"

DB_URL="sqlite://${OUTPUT_DIR}/multiday.db?mode=rwc"
DB_PATH="${DB_URL#sqlite://}"
DB_PATH="${DB_PATH%%\?*}"
CARGO_TARGET_DIR="${ADESH_CARGO_TARGET_DIR:-/tmp/adesh-cargo-target}"
EVENTS_JSONL="${OUTPUT_DIR}/multiday_events.jsonl"
PROBES_JSONL="${OUTPUT_DIR}/multiday_probes.jsonl"
REPORT_PATH="${OUTPUT_DIR}/multiday_supervisory_usage_report.json"
: > "$EVENTS_JSONL"
: > "$PROBES_JSONL"

run_daemon() {
  ADESH_DATABASE_URL="$DB_URL" \
    CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
    cargo run -q --manifest-path "${ADESH_ROOT}/Cargo.toml" -p adesh-daemon -- "$@"
}

connector_event() {
  local payload="$1"
  run_daemon host connector --json "$payload"
}

timestamp_for() {
  local day="$1"
  local hour="$2"
  date -u -d "2026-04-20 09:00:00 UTC +${day} days +${hour} hours" +"%Y-%m-%dT%H:%M:%SZ"
}

workspace_json() {
  local kind="$1"
  local locator="$2"
  jq -n --arg kind "$kind" --arg locator "$locator" '{
    kind: $kind,
    locator: $locator,
    cwd: null,
    branch: null,
    external_ref: null
  }'
}

host_for_day() {
  case $(( $1 % 4 )) in
    0) printf 'codex-vscode|codex-extension|gpt-5.4' ;;
    1) printf 'qwen-code|cli|qwen/qwen3.6-27b' ;;
    2) printf 'opencode|cli|opencode-default' ;;
    *) printf 'gemini-cli|cli|gemini-cli-default' ;;
  esac
}

run_session() {
  local day="$1"
  local hour="$2"
  local name="$3"
  local workspace_kind="$4"
  local workspace_locator="$5"
  local task_hint="$6"
  local prompt="$7"
  local files_json="$8"
  local outcome="$9"
  local summary="${10}"
  local decision="${11}"
  local rationale="${12}"
  local unresolved="${13}"
  local risk="${14}"
  local test_name="${15}"
  local test_status="${16}"
  local test_summary="${17}"
  local correction_summary="${18}"

  local host_parts connector_id host_kind host_model
  host_parts="$(host_for_day "$day")"
  IFS='|' read -r connector_id host_kind host_model <<< "$host_parts"

  local started_at ended_at workspace start_payload start_response context_id selected_direction end_payload end_response
  started_at="$(timestamp_for "$day" "$hour")"
  ended_at="$(timestamp_for "$day" "$((hour + 1))")"
  workspace="$(workspace_json "$workspace_kind" "$workspace_locator")"

  start_payload="$(jq -n \
    --arg connector_id "$connector_id" \
    --arg host_kind "$host_kind" \
    --arg host_model "$host_model" \
    --arg run_id "$RUN_ID" \
    --arg name "$name" \
    --arg started_at "$started_at" \
    --argjson workspace "$workspace" \
    --arg task_hint "$task_hint" \
    --arg prompt "$prompt" \
    --argjson files "$files_json" \
    '{
      connector_id: $connector_id,
      connector_kind: $host_kind,
      connector_version: "0.1.0",
      session_id: ("multiday-" + $run_id + "-" + $name),
      host_agent_id: $connector_id,
      host_agent_kind: $host_kind,
      host_model: $host_model,
      event_kind: "task_start",
      workspace: $workspace,
      task_prompt: $prompt,
      task_hint: (if $task_hint == "" then null else $task_hint end),
      files_in_focus: $files,
      started_at: $started_at,
      ended_at: null
    }')"
  start_response="$(connector_event "$start_payload")"
  context_id="$(printf '%s' "$start_response" | jq -r '.context_id // empty')"
  selected_direction="$(printf '%s' "$start_response" | jq -r '.prepare_context.likely_next_directions[0].statement // empty')"

  end_payload="$(jq -n \
    --arg connector_id "$connector_id" \
    --arg host_kind "$host_kind" \
    --arg host_model "$host_model" \
    --arg run_id "$RUN_ID" \
    --arg name "$name" \
    --arg started_at "$started_at" \
    --arg ended_at "$ended_at" \
    --arg context_id "$context_id" \
    --arg selected_direction "$selected_direction" \
    --arg outcome "$outcome" \
    --arg correction_summary "$correction_summary" \
    --argjson workspace "$workspace" \
    --arg task_hint "$task_hint" \
    --arg prompt "$prompt" \
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
      connector_kind: $host_kind,
      connector_version: "0.1.0",
      session_id: ("multiday-" + $run_id + "-" + $name),
      host_agent_id: $connector_id,
      host_agent_kind: $host_kind,
      host_model: $host_model,
      context_id: (if $context_id == "" then null else $context_id end),
      selected_next_direction: (if $selected_direction == "" then null else $selected_direction end),
      outcome: $outcome,
      correction_summary: (if $correction_summary == "" then null else $correction_summary end),
      event_kind: "task_end",
      workspace: $workspace,
      task_prompt: $prompt,
      task_hint: (if $task_hint == "" then null else $task_hint end),
      files_in_focus: $files,
      files_touched: $files,
      started_at: $started_at,
      ended_at: $ended_at,
      summary: $summary,
      tests: [
        {
          name: $test_name,
          status: $test_status,
          summary: $test_summary
        }
      ],
      decisions: (
        if $decision == "" then []
        else [{decision: $decision, rationale: (if $rationale == "" then null else $rationale end)}]
        end
      ),
      unresolved_items: (if $unresolved == "" then [] else [$unresolved] end),
      observed_preferences: [],
      risk_signals: (if $risk == "" then [] else [$risk] end)
    }')"
  end_response="$(connector_event "$end_payload")"

  jq -n \
    --arg day "$day" \
    --arg name "$name" \
    --arg workspace_kind "$workspace_kind" \
    --arg workspace_locator "$workspace_locator" \
    --arg connector_id "$connector_id" \
    --arg host_kind "$host_kind" \
    --arg started_at "$started_at" \
    --arg ended_at "$ended_at" \
    --arg context_id "$context_id" \
    --arg selected_direction "$selected_direction" \
    --arg outcome "$outcome" \
    --argjson start_response "$start_response" \
    --argjson end_response "$end_response" \
    '{
      day: ($day | tonumber),
      name: $name,
      workspace_kind: $workspace_kind,
      workspace_locator: $workspace_locator,
      connector_id: $connector_id,
      host_kind: $host_kind,
      started_at: $started_at,
      ended_at: $ended_at,
      context_id: (if $context_id == "" then null else $context_id end),
      selected_direction: $selected_direction,
      outcome: $outcome,
      task_focus: $start_response.prepare_context.task_focus,
      top_next_direction: ($start_response.prepare_context.likely_next_directions[0].statement // null),
      episode_id: $end_response.stored_episode.episode_id
    }' >> "$EVENTS_JSONL"

  printf 'day %02d %-28s host=%s outcome=%s context=%s\n' \
    "$day" "$name" "$connector_id" "$outcome" "${context_id:-missing}"
}

run_probe() {
  local name="$1"
  local workspace_kind="$2"
  local workspace_locator="$3"
  local task_hint="$4"
  local prompt="$5"
  local files_json="$6"
  local day="${7:-$DAYS}"

  local workspace started_at payload response
  workspace="$(workspace_json "$workspace_kind" "$workspace_locator")"
  started_at="$(timestamp_for "$day" 7)"
  payload="$(jq -n \
    --arg name "$name" \
    --arg started_at "$started_at" \
    --argjson workspace "$workspace" \
    --arg task_hint "$task_hint" \
    --arg prompt "$prompt" \
    --argjson files "$files_json" \
    '{
      connector_id: "multiday-probe",
      connector_kind: "offline_probe",
      connector_version: "0.1.0",
      session_id: ("multiday-probe-" + $name),
      host_agent_id: "multiday-probe",
      host_agent_kind: "offline_probe",
      host_model: "deterministic",
      event_kind: "task_start",
      workspace: $workspace,
      task_prompt: $prompt,
      task_hint: (if $task_hint == "" then null else $task_hint end),
      files_in_focus: $files,
      started_at: $started_at
    }')"
  response="$(connector_event "$payload")"
  jq -c --arg name "$name" --arg prompt "$prompt" '{
    name: $name,
    prompt: $prompt,
    task_focus: .prepare_context.task_focus,
    decisions: .prepare_context.relevant_decisions,
    open_loops: .prepare_context.open_loops,
    next_directions: .prepare_context.likely_next_directions,
    uncertainties: .prepare_context.uncertainties
  }' <<< "$response" >> "$PROBES_JSONL"
}

echo "Running multi-day supervisory usage simulation..."
echo "DB_URL=${DB_URL}"
echo "days=${DAYS}"
echo "profile=${PROFILE}"

run_session 0 0 "seed-payments-timeout" "task_space" "workspace://multi-payments" "retry-hardening" \
  "Retry hardening started with a timeout coverage gap." \
  '["src/retry/service.rs","tests/retry_timeout.rs"]' \
  "accepted" \
  "Initial retry work exposed timeout risk under partial upstream commit." \
  "Keep retry hardening blocked on degraded-network timeout evidence" \
  "Incident and failing-test evidence should outrank cleanup" \
  "Run degraded-network timeout benchmark before claiming retry safety" \
  "Retry cleanup can mask duplicate writes without timeout evidence" \
  "retry_timeout_gap" "fail" "Timeout path still lacks coverage" ""

run_session 0 2 "seed-connector-stale" "task_space" "workspace://multi-connectors" "connector-integration" \
  "Connector integration needs stale-context proof." \
  '["crates/adesh-daemon/src/connector_adapter.rs"]' \
  "accepted" \
  "Connector wrappers need proof that invalid context outcomes remain unlearnable." \
  "Before adding adapters, prove stale-context outcomes stay unlearnable" \
  "Bad host linkage should be observed but excluded from advisory learning" \
  "Add stale-context restart case to connector validation" \
  "Adapters can multiply bad trace patterns" \
  "stale_context_restart" "fail" "No stale-context proof yet" ""

run_session 0 4 "seed-eval-gate" "task_space" "workspace://multi-eval" "wedge-evaluation" \
  "The metrics look good; should policy-state start?" \
  '["docs/IMPLEMENTATION_PLAN.md"]' \
  "accepted" \
  "Synthetic metrics are insufficient evidence for Phase E." \
  "Do not open policy-state from synthetic metrics alone" \
  "Policy-state needs repeated lineage or rollback pressure" \
  "Review real host traces for repeated policy-lineage reconstruction gaps" \
  "Premature policy-state turns observability into controller theater" \
  "phase_e_gate" "pass" "Gate remained deferred" ""

for ((day = 1; day <= DAYS; day++)); do
  if (( day <= 5 )); then
    run_session "$day" 0 "payments-timeout-day-${day}" "task_space" "workspace://multi-payments" "retry-hardening" \
      "This retry release still worries me. What should I validate before cleanup?" \
      '["src/retry/service.rs","tests/retry_timeout.rs"]' \
      "accepted" \
      "Kept retry work focused on degraded-network timeout evidence." \
      "Keep retry hardening blocked on degraded-network timeout evidence" \
      "Timeout proof is the safety gate before cleanup" \
      "Run degraded-network timeout benchmark before claiming retry safety" \
      "Retry cleanup can hide duplicate-write risk" \
      "retry_timeout_multiday" "fail" "Timeout benchmark still incomplete" ""
  elif (( day == 6 )); then
    run_session "$day" 0 "payments-timeout-resolved" "task_space" "workspace://multi-payments" "retry-hardening" \
      "The degraded-network timeout benchmark passed. What remains before release?" \
      '["src/retry/service.rs","tests/retry_timeout.rs","docs/retry_release_notes.md"]' \
      "modified" \
      "Resolved degraded-network timeout benchmark after pass; remaining work is release notes and rollback docs." \
      "Retry hardening can proceed after degraded-network timeout evidence passed" \
      "The original safety gate is closed, but release evidence still needs packaging" \
      "Document retry rollback notes and release evidence" \
      "Release notes can omit why the retry safety gate was closed" \
      "retry_timeout_multiday" "pass" "Degraded-network timeout benchmark passed" \
      "Accepted safety proof, narrowed next work to release documentation."
  elif (( day <= 10 )); then
    run_session "$day" 0 "connector-stale-day-${day}" "task_space" "workspace://multi-connectors" "connector-integration" \
      "Before adding another adapter, what trace behavior should we harden?" \
      '["crates/adesh-daemon/src/connector_adapter.rs","docs/CONNECTOR_INTEGRATION_V0.md"]' \
      "accepted" \
      "Focused connector work on stale-context validation before adding new adapters." \
      "Before adding adapters, prove stale-context outcomes stay unlearnable" \
      "Linked outcomes are safe only when context propagation is valid" \
      "Add host-facing warning examples for unlearnable outcome traces" \
      "Stale context outcomes can poison advisory learning if treated as linked" \
      "connector_stale_context" "pass" "Unlearnable stale-context behavior stayed visible" ""
  elif (( day == 11 )); then
    run_session "$day" 0 "connector-stale-resolved" "task_space" "workspace://multi-connectors" "connector-integration" \
      "The stale-context validation passed. What should connector docs show next?" \
      '["docs/CONNECTOR_INTEGRATION_V0.md","scripts/supervisory_trace_real_runs.sh"]' \
      "modified" \
      "Resolved stale-context restart validation; next work is showing context_id round trip examples." \
      "Connector docs should show returned context_id being fed into task_end" \
      "A concrete round trip is more useful than lifecycle claims" \
      "Add one Codex/Qwen/OpenCode example showing context_id round trip" \
      "Docs can overclaim automation if the round trip is invisible" \
      "connector_context_round_trip" "pass" "Stale context validation passed" \
      "Accepted stale-context proof, narrowed follow-up to concrete docs."
  elif (( day <= 16 && !(DAYS <= 16 && day == DAYS) )); then
    run_session "$day" 0 "eval-gate-day-${day}" "task_space" "workspace://multi-eval" "wedge-evaluation" \
      "The metrics look good; should we start policy-state now?" \
      '["docs/IMPLEMENTATION_PLAN.md","docs/POLICY_STATE_DECISION_NOTE.md"]' \
      "accepted" \
      "Kept Phase E gated and reviewed traces for repeated policy-lineage pressure." \
      "Do not open policy-state from synthetic metrics alone" \
      "Repeated operational pressure, not one benchmark, should trigger policy-state" \
      "Review real host traces for repeated policy-lineage reconstruction gaps" \
      "Synthetic benchmark volume can be mistaken for real policy evolution need" \
      "phase_e_multiday_gate" "pass" "Phase E remained gated" ""
  else
    run_session "$day" 0 "conversation-memory-day-${day}" "conversation" "personal-agent-workflows" "personal-continuity" \
      "What should my personal agent setup remember across tools?" \
      '[]' \
      "accepted" \
      "Non-repo continuity should preserve personal workflow preferences across agent tools." \
      "Aadesh should support non-repo task spaces, not only coding repositories" \
      "Workspace identity must stay generic for conversations and personal workflows" \
      "Keep a non-repo continuity smoke path in the benchmark" \
      "Overfitting to git repos would shrink the product wedge incorrectly" \
      "conversation_continuity" "pass" "Conversation workspace continuity worked" ""
  fi
done

run_session "$((DAYS + 1))" 0 "controlled-invalid-context" "task_space" "workspace://multi-connectors" "connector-integration" \
  "Host restarted and emitted stale context." \
  '["crates/adesh-daemon/src/connector_adapter.rs"]' \
  "accepted" \
  "Stored an invalid context outcome for observability only." \
  "Do not learn from invalid context outcomes" \
  "Invalid context links can poison advisory learning" \
  "Document invalid context behavior" \
  "A restarted host can replay stale context ids" \
  "invalid_context_multiday" "fail" "Invalid context was expected" ""

# Force the last event to be invalid by writing a direct stale-context outcome through connector.
invalid_payload="$(jq -n \
  --arg started_at "$(timestamp_for "$((DAYS + 1))" 2)" \
  --arg ended_at "$(timestamp_for "$((DAYS + 1))" 3)" \
  --argjson workspace "$(workspace_json "task_space" "workspace://multi-connectors")" \
  '{
    connector_id: "codex-vscode",
    connector_kind: "codex-extension",
    connector_version: "0.1.0",
    session_id: "multiday-invalid-context-direct",
    host_agent_id: "codex-vscode",
    host_agent_kind: "codex-extension",
    host_model: "gpt-test",
    context_id: "stale-context-from-crashed-host",
    selected_next_direction: "Use stale context after host restart",
    outcome: "accepted",
    correction_summary: "This direct invalid trace must be stored but excluded from learning.",
    event_kind: "task_end",
    workspace: $workspace,
    task_prompt: "Host restarted and emitted a stale context id.",
    task_hint: "connector-integration",
    summary: "Persisted invalid context trace for observability only.",
    files_touched: ["crates/adesh-daemon/src/connector_adapter.rs"],
    decisions: [{decision: "Invalid context outcomes must remain unlearnable", rationale: "Stale context is not trustworthy learning evidence"}],
    unresolved_items: ["Document invalid context behavior"],
    risk_signals: ["Stale context replay can poison advisory learning"],
    started_at: $started_at,
    ended_at: $ended_at
  }')"
connector_event "$invalid_payload" >/dev/null
echo "day $((DAYS + 1)) controlled-invalid-context-direct host=codex-vscode outcome=accepted context=stale-context-from-crashed-host"

if [[ "$PROFILE" == "production" ]]; then
  run_session "$((DAYS + 2))" 0 "prod-ci-flake-repro" "task_space" "workspace://multi-payments" "ci-release-flake" \
    "Release-only CI failed again; should I clean up retry code or preserve evidence first?" \
    '["src/retry/service.rs","tests/retry_release_ci.rs",".github/workflows/release.yml"]' \
    "accepted" \
    "Release-only retry flake must be reproduced with seed, release flags, and logs before cleanup." \
    "Reproduce release-only retry flake with seed and log capture before cleanup" \
    "Cleanup can erase the evidence needed to prove the release-only failure mode" \
    "Capture failing seed, release build flags, and CI logs for the retry flake" \
    "Cleanup can erase flaky failure evidence before root cause is known" \
    "retry_release_ci_flake" "fail" "Release-only CI flake reproduced once but evidence is incomplete" ""

  run_session "$((DAYS + 3))" 0 "prod-pr-review-context-docs" "task_space" "workspace://multi-connectors" "review-comment" \
    "Reviewer says the context_id docs are vague. What should be patched before claiming automation?" \
    '["docs/CONNECTOR_INTEGRATION_V0.md","crates/adesh-contracts/src/lib.rs"]' \
    "modified" \
    "Review feedback narrowed connector docs to exact context_id round-trip examples." \
    "Document the context_id round trip before claiming adapter automation" \
    "VS Code and CLI hosts still need explicit event mapping into Aadesh connector events" \
    "Add one accepted and one ignored outcome example with returned context_id" \
    "Docs can claim automation while host-specific mapping remains manual" \
    "connector_context_docs_review" "pass" "Review comment converted into concrete docs work" \
    "Modified original broad docs direction into exact round-trip examples."

  run_session "$((DAYS + 4))" 0 "prod-sandbox-blocked-comparator" "task_space" "workspace://multi-eval" "external-comparison" \
    "OpenMemory or Hermes could not run because Docker or local model access was blocked. How should that be recorded?" \
    '["scripts/external_memory_comparison_harness.sh","docs/COMPARISON_BENCHMARK.md"]' \
    "modified" \
    "Blocked external comparator runs must be stored as blocked/not-run environment evidence, not competitor failure." \
    "Record blocked comparator runs as not-run environment evidence, not product weakness" \
    "Sandbox or local model blockage is observability data, not competitor quality evidence" \
    "Persist blocked comparator artifact refs and rerun only when local prerequisites are available" \
    "Treating sandbox blockage as competitor weakness poisons comparison evidence" \
    "external_comparator_blocked" "fail" "Docker/model prerequisite unavailable in blocked environment" \
    "Modified benchmark interpretation to preserve blocked status."

  run_session "$((DAYS + 5))" 0 "prod-cross-workspace-noise" "task_space" "workspace://unrelated-maintenance" "lint-cleanup" \
    "Unrelated lint cleanup happened in another workspace while retry release evidence is still open." \
    '["scripts/format.sh","docs/style.md"]' \
    "ignored" \
    "Unrelated maintenance trace should remain isolated from payment retry release guidance." \
    "Do not let unrelated lint cleanup influence payment retry release safety guidance" \
    "Scope isolation is part of production-quality memory behavior" \
    "Keep cross-workspace leakage checks in the production benchmark" \
    "Cross-workspace leakage can make memory sound helpful while being wrong" \
    "unrelated_lint_cleanup" "pass" "Lint cleanup was intentionally unrelated" ""
fi

run_probe "payments_after_resolution" "task_space" "workspace://multi-payments" "retry-hardening" \
  "The timeout benchmark passed. What remains before release?" \
  '["docs/retry_release_notes.md","src/retry/service.rs"]'
run_probe "eval_phase_e_gate_after_weeks" "task_space" "workspace://multi-eval" "wedge-evaluation" \
  "After weeks of metrics, should we start policy-state now?" \
  '["docs/IMPLEMENTATION_PLAN.md","docs/POLICY_STATE_DECISION_NOTE.md"]'
run_probe "conversation_non_repo_continuity" "conversation" "personal-agent-workflows" "personal-continuity" \
  "What should my personal agent setup remember across tools?" \
  '[]'

if [[ "$PROFILE" == "production" ]]; then
  run_probe "prod_ci_flake_followup" "task_space" "workspace://multi-payments" "ci-release-flake" \
    "Release-only CI failed again. What should happen before cleanup?" \
    '["src/retry/service.rs","tests/retry_release_ci.rs",".github/workflows/release.yml"]'
  run_probe "prod_review_context_docs_followup" "task_space" "workspace://multi-connectors" "review-comment" \
    "A reviewer says context_id docs are vague. What exact patch should I make?" \
    '["docs/CONNECTOR_INTEGRATION_V0.md","crates/adesh-contracts/src/lib.rs"]'
  run_probe "prod_sandbox_blockage_followup" "task_space" "workspace://multi-eval" "external-comparison" \
    "External comparator was blocked by sandbox or local model access. Should I count that as competitor failure?" \
    '["scripts/external_memory_comparison_harness.sh","docs/COMPARISON_BENCHMARK.md"]'
  run_probe "prod_scope_noise_negative" "task_space" "workspace://multi-payments" "ci-release-flake" \
    "Should unrelated lint cleanup change the retry release-flake priority?" \
    '["src/retry/service.rs","tests/retry_release_ci.rs"]'
fi

events_json="$(jq -s '.' "$EVENTS_JSONL")"
probes_json="$(jq -s '.' "$PROBES_JSONL")"

db_counts_json="$(sqlite3 -json "$DB_PATH" "
SELECT
  (SELECT COUNT(*) FROM episodes) AS stored_episodes,
  (SELECT COUNT(*) FROM intervention_context) AS intervention_contexts,
  (SELECT COUNT(*) FROM intervention_outcomes) AS intervention_outcomes,
  (SELECT COUNT(*) FROM intervention_outcomes WHERE learn_from_this = 1) AS learnable_outcomes,
  (SELECT COUNT(*) FROM intervention_outcomes WHERE learn_from_this = 0) AS unlearnable_outcomes,
  (SELECT COUNT(DISTINCT scope_key) FROM episodes) AS distinct_workspaces,
  (SELECT COUNT(DISTINCT scope_type) FROM episodes) AS distinct_workspace_kinds,
  (SELECT COUNT(*) FROM episodes WHERE scope_key LIKE '%conversation%') AS non_repo_episodes,
  (SELECT julianday(MAX(ended_at)) - julianday(MIN(started_at)) FROM episodes) AS simulated_days_span;
")"

host_counts_json="$(sqlite3 -json "$DB_PATH" "
SELECT replace(artifact_ref, 'trace://host-agent-id/', '') AS host_agent, COUNT(*) AS count
FROM episode_artifacts
WHERE artifact_ref LIKE 'trace://host-agent-id/%'
GROUP BY artifact_ref
ORDER BY count DESC, host_agent ASC;
")"

claims_json="$(sqlite3 -json "$DB_PATH" "
SELECT claim_type, status, value_json, COUNT(*) AS count
FROM claims
WHERE value_json LIKE '%timeout benchmark%'
   OR value_json LIKE '%policy-state%'
   OR value_json LIKE '%non-repo%'
   OR value_json LIKE '%context_id%'
GROUP BY claim_type, status, value_json
ORDER BY claim_type, status, count DESC;
")"

probe_assertions="$(jq -n \
  --argjson probes "$probes_json" \
  '
  def byname($name): $probes | map(select(.name == $name))[0];
  def statements($items): (($items // []) | map(.statement // "") | join(" ") | ascii_downcase);
  {
    payment_resolution_does_not_restart_closed_timeout_benchmark: (
      statements(byname("payments_after_resolution").next_directions)
      | (contains("run degraded-network timeout benchmark before claiming retry safety") | not)
    ),
    payment_resolution_mentions_release_or_docs: (
      statements(byname("payments_after_resolution").next_directions)
      | (contains("release") or contains("rollback") or contains("document"))
    ),
    phase_e_still_gated_after_weeks: (
      statements(byname("eval_phase_e_gate_after_weeks").decisions)
      | (contains("do not open policy-state") or contains("gated"))
    ),
    phase_e_probe_points_to_trace_review: (
      statements(byname("eval_phase_e_gate_after_weeks").next_directions)
      | (contains("policy-lineage") or contains("real host traces") or contains("operational pressure"))
    ),
    non_repo_continuity_surfaces_personal_workspace_memory: (
      (statements(byname("conversation_non_repo_continuity").decisions) + " " + statements(byname("conversation_non_repo_continuity").next_directions))
      | (contains("non-repo") or contains("conversation") or contains("personal"))
    )
  }
  ')"

production_assertions="$(jq -n \
  --arg profile "$PROFILE" \
  --argjson probes "$probes_json" \
  '
  def byname($name): $probes | map(select(.name == $name))[0];
  def statements($items): (($items // []) | map(.statement // "") | join(" ") | ascii_downcase);
  def top_statement($items): (($items[0].statement // "") | ascii_downcase);
  if $profile != "production" then {}
  else {
    ci_flake_prioritizes_repro_evidence: (
      statements(byname("prod_ci_flake_followup").next_directions)
      | ((contains("seed") or contains("log") or contains("release-only") or contains("reproduce"))
         and (contains("cleanup") or contains("evidence")))
    ),
    review_comment_surfaces_context_round_trip: (
      statements(byname("prod_review_context_docs_followup").next_directions)
      | (contains("context_id") or contains("round trip") or contains("accepted") or contains("ignored"))
    ),
    sandbox_blockage_stays_observational: (
      statements(byname("prod_sandbox_blockage_followup").next_directions)
      | (contains("blocked") or contains("not-run") or contains("environment") or contains("prerequisite"))
    ),
    cross_workspace_noise_does_not_dominate: (
      (top_statement(byname("prod_scope_noise_negative").next_directions) | contains("lint cleanup") | not)
      and (
        statements(byname("prod_scope_noise_negative").next_directions)
        | (contains("retry") or contains("release") or contains("flake") or contains("evidence"))
      )
    )
  }
  end
  ')"

report_json="$(jq -n \
  --arg report_path "$REPORT_PATH" \
  --arg db_path "$DB_PATH" \
  --arg events_path "$EVENTS_JSONL" \
  --arg probes_path "$PROBES_JSONL" \
  --arg profile "$PROFILE" \
  --argjson days "$DAYS" \
  --argjson events "$events_json" \
  --argjson probes "$probes_json" \
  --argjson counts "$db_counts_json" \
  --argjson host_counts "$host_counts_json" \
  --argjson claims "$claims_json" \
  --argjson probe_assertions "$probe_assertions" \
  --argjson production_assertions "$production_assertions" \
  '
  ($counts[0] // {}) as $c
  | def byname($name): $probes | map(select(.name == $name))[0];
    def observed_case($name): {
      task_focus: (byname($name).task_focus // null),
      top_decision: (byname($name).decisions[0].statement // null),
      top_open_loop: (byname($name).open_loops[0].statement // null),
      top_next_direction: (byname($name).next_directions[0].statement // null),
      uncertainty_count: ((byname($name).uncertainties // []) | length)
    };
    def production_case($case_id; $failure_mode; $expected_evidence; $assertion_key; $if_fail):
      {
        case_id: $case_id,
        failure_mode: $failure_mode,
        expected_evidence: $expected_evidence,
        assertion_key: $assertion_key,
        assertion_passed: ($production_assertions[$assertion_key] // false),
        observed: observed_case($case_id),
        diagnostic: (
          if ($production_assertions[$assertion_key] // false)
          then "passed: observed guidance contained the expected production evidence"
          else $if_fail
          end
        )
      };
  {
      metadata: {
        scenario: "multiday_supervisory_usage",
        data_profile: $profile,
        report_path: $report_path,
        db_path: $db_path,
        events_path: $events_path,
        probes_path: $probes_path
      },
      simulated_usage: {
        data_profile: $profile,
        requested_days: $days,
        simulated_days_span: ($c.simulated_days_span // 0),
        host_counts: $host_counts,
        events: $events
      },
      storage_totals: $c,
      claim_status_samples: $claims,
      probe_assertions: $probe_assertions,
      production_assertions: $production_assertions,
      production_case_report: (
        if $profile != "production" then []
        else [
          production_case(
            "prod_ci_flake_followup";
            "release-only CI flake should preserve seed/log evidence before cleanup";
            ["seed", "log", "release-only", "reproduce", "cleanup/evidence"];
            "ci_flake_prioritizes_repro_evidence";
            "failed: ranking likely let generic cleanup or stale retry guidance outrank release-flake evidence"
          ),
          production_case(
            "prod_review_context_docs_followup";
            "review correction should become concrete context_id round-trip documentation work";
            ["context_id", "round trip", "accepted outcome example", "ignored outcome example"];
            "review_comment_surfaces_context_round_trip";
            "failed: retrieval likely recalled connector work but did not turn review feedback into a concrete patch"
          ),
          production_case(
            "prod_sandbox_blockage_followup";
            "blocked external comparator should remain observability data, not competitor failure";
            ["blocked", "not-run", "environment", "prerequisite"];
            "sandbox_blockage_stays_observational";
            "failed: benchmark interpretation likely converted environment blockage into system-quality evidence"
          ),
          production_case(
            "prod_scope_noise_negative";
            "unrelated workspace lint cleanup must not dominate payment release-flake guidance";
            ["retry", "release", "flake", "evidence", "not lint cleanup"];
            "cross_workspace_noise_does_not_dominate";
            "failed: scope isolation likely allowed unrelated maintenance memory to leak into current guidance"
          )
        ]
        end
      ),
      synthetic_benchmark_quality: {
        data_profile: $profile,
        controlled_synthetic: true,
        competitor_testing_patterns_adopted: [
          "per-case expected evidence",
          "aggregate score plus case diagnostics",
          "blocked prerequisite recorded as blocked/not-run",
          "isolated temporary DB and artifacts"
        ],
        remaining_limitations: [
          "seeded traces are hand-authored rather than harvested from real sessions",
          "assertions are lexical and can miss semantically correct alternatives",
          "competitor systems still need real exported runs for task-quality claims"
        ]
      },
      probe_summary: (
        $probes
        | map({
            name,
            task_focus,
            top_decision: (.decisions[0].statement // null),
            top_open_loop: (.open_loops[0].statement // null),
            top_next_direction: (.next_directions[0].statement // null),
            uncertainty_count: ((.uncertainties // []) | length)
          })
      ),
      multiday_assertions: {
        temporal_span_at_least_two_weeks: (($c.simulated_days_span // 0) >= 14),
        temporal_span_at_least_requested_minus_one: (($c.simulated_days_span // 0) >= ($days - 1)),
        multiple_host_kinds: (($host_counts | length) >= 3),
        non_repo_workspace_exercised: (($c.non_repo_episodes // 0) > 0),
        learnable_and_unlearnable_traces_present: (($c.learnable_outcomes // 0) > 0 and ($c.unlearnable_outcomes // 0) > 0),
        mixed_outcomes_present: (
          (($events | map(select(.outcome == "accepted")) | length) > 0)
          and (($events | map(select(.outcome == "modified")) | length) > 0)
        ),
        production_profile_checks_passed: (
          if $profile == "production" then
            (($production_assertions | to_entries | map(.value == true) | index(false)) == null)
          else true end
        )
      },
      production_profile_pass: (
        if $profile == "production" then
          (($production_assertions | to_entries | map(.value == true) | index(false)) == null)
        else true end
      ),
      multiday_simulation_pass: (
        (($c.simulated_days_span // 0) >= 14)
        and (($host_counts | length) >= 3)
        and (($c.non_repo_episodes // 0) > 0)
        and (($c.learnable_outcomes // 0) > 0)
        and (($c.unlearnable_outcomes // 0) > 0)
        and (($probe_assertions | to_entries | map(.value == true) | index(false)) == null)
        and (
          if $profile == "production" then
            (($production_assertions | to_entries | map(.value == true) | index(false)) == null)
          else true end
        )
      )
    }
  ')"

printf '%s\n' "$report_json" > "$REPORT_PATH"

echo
echo "Multi-day supervisory usage report:"
echo "  $REPORT_PATH"
printf '%s\n' "$report_json" | jq '{
  multiday_simulation_pass,
  production_profile_pass,
  storage_totals,
  simulated_usage: {data_profile: .simulated_usage.data_profile, requested_days: .simulated_usage.requested_days, simulated_days_span: .simulated_usage.simulated_days_span, host_counts: .simulated_usage.host_counts},
  multiday_assertions,
  production_assertions,
  production_case_report,
  synthetic_benchmark_quality,
  probe_assertions,
  probe_summary
}'

if [[ "$(printf '%s\n' "$report_json" | jq -r '.multiday_simulation_pass')" != "true" ]]; then
  echo "multi-day supervisory usage simulation failed" >&2
  exit 1
fi
