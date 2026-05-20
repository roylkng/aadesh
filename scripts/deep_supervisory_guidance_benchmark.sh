#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: ./scripts/deep_supervisory_guidance_benchmark.sh [options]

Runs the long-form Aadesh real-mode benchmark:
1. executes the complex supervisory trace simulation through the public connector path
2. probes the resulting memory with fresh task_start requests
3. asserts guidance quality, not just trace persistence

Options:
  --sessions N       Number of linked simulated sessions. Default: 60.
  --output-dir DIR   Directory for report/db/probes. Default: /tmp/adesh-deep-supervisory-<run_id>.
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
SESSIONS=60
OUTPUT_DIR=""

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

if ! [[ "$SESSIONS" =~ ^[0-9]+$ ]] || [[ "$SESSIONS" -lt 12 ]]; then
  echo "--sessions must be an integer >= 12 for the deep benchmark" >&2
  exit 1
fi

OUTPUT_DIR="${OUTPUT_DIR:-/tmp/adesh-deep-supervisory-${RUN_ID}}"
mkdir -p "$OUTPUT_DIR"

CARGO_TARGET_DIR="${ADESH_CARGO_TARGET_DIR:-/tmp/adesh-cargo-target}"
COMPLEX_REPORT="${OUTPUT_DIR}/supervisory_trace_complex_report.json"
DB_URL="sqlite://${OUTPUT_DIR}/complex.db?mode=rwc"
PROBES_JSONL="${OUTPUT_DIR}/deep_guidance_probes.jsonl"
REPORT_PATH="${OUTPUT_DIR}/deep_supervisory_guidance_report.json"
: > "$PROBES_JSONL"

run_daemon() {
  ADESH_DATABASE_URL="$DB_URL" \
    CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
    cargo run -q --manifest-path "${ADESH_ROOT}/Cargo.toml" -p adesh-daemon -- "$@"
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

run_probe() {
  local name="$1"
  local workspace_locator="$2"
  local task_hint="$3"
  local prompt="$4"
  local files_json="$5"
  local workspace
  workspace="$(workspace_json "$workspace_locator")"

  local payload
  payload="$(jq -n \
    --arg name "$name" \
    --argjson workspace "$workspace" \
    --arg task_hint "$task_hint" \
    --arg prompt "$prompt" \
    --argjson files "$files_json" \
    '{
      connector_id: "deep-supervisory-guidance-benchmark",
      connector_kind: "offline_deep_benchmark",
      connector_version: "0.1.0",
      session_id: ("deep-probe-" + $name),
      host_agent_id: "deep-benchmark-agent",
      host_agent_kind: "mixed-coding-agent",
      host_model: "deterministic-deep-benchmark",
      event_kind: "task_start",
      workspace: $workspace,
      task_prompt: $prompt,
      task_hint: (if $task_hint == "" then null else $task_hint end),
      files_in_focus: $files
    }')"

  local response
  response="$(run_daemon host connector --json "$payload")"
  jq -c --arg name "$name" --arg workspace_locator "$workspace_locator" --arg prompt "$prompt" '{
    name: $name,
    workspace_locator: $workspace_locator,
    prompt: $prompt,
    context_id,
    task_focus: .prepare_context.task_focus,
    relevant_decisions: .prepare_context.relevant_decisions,
    open_loops: .prepare_context.open_loops,
    risk_flags: .prepare_context.risk_flags,
    likely_next_directions: .prepare_context.likely_next_directions,
    uncertainties: .prepare_context.uncertainties
  }' <<< "$response" >> "$PROBES_JSONL"
}

echo "Running long complex trace simulation..."
"${ADESH_ROOT}/scripts/supervisory_trace_complex_simulation.sh" \
  --sessions "$SESSIONS" \
  --output-dir "$OUTPUT_DIR" \
  --db-url "$DB_URL"

echo
echo "Running deep guidance probes..."
run_probe \
  "payments_vague_release" \
  "workspace://complex-payments-service" \
  "" \
  "This release still worries me. What should I validate before cleanup?" \
  '[]'
run_probe \
  "connector_new_adapter" \
  "workspace://complex-connector-ecosystem" \
  "connector-integration" \
  "Before adding another adapter, what trace behavior should we harden?" \
  '["crates/adesh-daemon/src/connector_adapter.rs","crates/adesh-contracts/src/lib.rs"]'
run_probe \
  "eval_policy_gate" \
  "workspace://complex-eval-lab" \
  "wedge-evaluation" \
  "The metrics look good; should we start policy-state now?" \
  '["docs/IMPLEMENTATION_PLAN.md","docs/POLICY_STATE_DECISION_NOTE.md"]'
run_probe \
  "connector_scope_negative" \
  "workspace://complex-connector-ecosystem" \
  "connector-integration" \
  "The retry rollout still feels risky; what should I validate next?" \
  '["crates/adesh-daemon/src/connector_adapter.rs"]'
run_probe \
  "payments_scope_negative" \
  "workspace://complex-payments-service" \
  "retry-hardening" \
  "Before adding another adapter, what trace behavior should we harden?" \
  '["crates/adesh-daemon/src/cognition.rs"]'

probe_assertions="$(jq -s '
  def byname($name): map(select(.name == $name))[0];
  def statements($items): (($items // []) | map(.statement // "") | join(" ") | ascii_downcase);
  def focus_is_current($name): (byname($name).task_focus == byname($name).prompt);
  {
    payments_vague_release_focus_current: focus_is_current("payments_vague_release"),
    payments_vague_release_timeout_open_loop: (statements(byname("payments_vague_release").open_loops) | contains("timeout")),
    payments_vague_release_timeout_next_direction: (statements(byname("payments_vague_release").likely_next_directions) | contains("timeout")),
    connector_adapter_unlearnable_trace_guidance: (
      (statements(byname("connector_new_adapter").open_loops) + " " + statements(byname("connector_new_adapter").likely_next_directions))
      | (contains("unlearnable") or contains("stale-context") or contains("stale context"))
    ),
    eval_policy_gate_does_not_start_policy_state: (
      (statements(byname("eval_policy_gate").relevant_decisions) + " " + statements(byname("eval_policy_gate").likely_next_directions))
      | ((contains("do not open policy-state") or contains("phase e gated")) and (contains("policy-lineage") or contains("real host traces") or contains("operational pressure")))
    ),
    connector_scope_does_not_leak_retry_timeout: (statements(byname("connector_scope_negative").likely_next_directions) | (contains("timeout") | not)),
    payments_scope_does_not_leak_connector_stale_context: (statements(byname("payments_scope_negative").likely_next_directions) | ((contains("stale-context") or contains("unlearnable")) | not))
  }
' "$PROBES_JSONL")"

jq -n \
  --slurpfile complex "$COMPLEX_REPORT" \
  --slurpfile probes "$PROBES_JSONL" \
  --argjson probe_assertions "$probe_assertions" \
  --arg report_path "$REPORT_PATH" \
  '{
    metadata: {
      scenario: "deep_supervisory_guidance_benchmark",
      report_path: $report_path,
      complex_report_path: $complex[0].metadata.report_path,
      db_path: $complex[0].metadata.db_path
    },
    complex_trace_result: {
      pass: $complex[0].complex_simulation_pass,
      totals: $complex[0].totals,
      assertions: $complex[0].complex_assertions,
      operational_gate_progress: $complex[0].operational_gate_progress
    },
    guidance_probe_assertions: $probe_assertions,
    guidance_probe_summary: (
      $probes
      | map({
          name,
          task_focus,
          top_decision: (.relevant_decisions[0].statement // null),
          top_open_loop: (.open_loops[0].statement // null),
          top_next_direction: (.likely_next_directions[0].statement // null),
          uncertainty_count: ((.uncertainties // []) | length)
        })
    ),
    guidance_probe_raw_path: "'$PROBES_JSONL'",
    deep_benchmark_pass: (
      ($complex[0].complex_simulation_pass == true)
      and (($probe_assertions | to_entries | map(.value == true) | index(false)) == null)
    )
  }' > "$REPORT_PATH"

echo
echo "Deep supervisory guidance report:"
echo "  $REPORT_PATH"
jq '{
  deep_benchmark_pass,
  complex_trace_result,
  guidance_probe_assertions,
  guidance_probe_summary
}' "$REPORT_PATH"

if [[ "$(jq -r '.deep_benchmark_pass' "$REPORT_PATH")" != "true" ]]; then
  echo "deep supervisory guidance benchmark failed" >&2
  exit 1
fi
