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

ADESH_ROOT="${ADESH_DAEMON_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
if [[ ! -f "${ADESH_ROOT}/Cargo.toml" ]]; then
  echo "ADESH_DAEMON_ROOT does not point to an Aadesh repo: ${ADESH_ROOT}" >&2
  exit 1
fi

DB_URL="${ADESH_DATABASE_URL:-sqlite:///tmp/adesh-connector-smoke.db?mode=rwc}"
CARGO_TARGET_DIR="${ADESH_CARGO_TARGET_DIR:-/tmp/adesh-cargo-target}"

run_daemon() {
  ADESH_DATABASE_URL="$DB_URL" CARGO_TARGET_DIR="$CARGO_TARGET_DIR" cargo run -q --manifest-path "${ADESH_ROOT}/Cargo.toml" -p adesh-daemon -- "$@"
}

start_payload="$(jq -n '{
  connector_id: "codex-vscode",
  connector_kind: "chat_extension",
  connector_version: "0.1.0",
  session_id: "smoke-session-1",
  event_kind: "task_start",
  workspace: {
    kind: "task_space",
    locator: "workspace://connector-smoke",
    cwd: null,
    branch: null,
    external_ref: null
  },
  task_prompt: "What should I focus on next to finish retry hardening safely?",
  files_in_focus: ["src/retry.rs"],
  task_hint: "retry-hardening"
}')"

end_payload="$(jq -n '{
  connector_id: "codex-vscode",
  connector_kind: "chat_extension",
  connector_version: "0.1.0",
  session_id: "smoke-session-1",
  event_kind: "task_end",
  workspace: {
    kind: "task_space",
    locator: "workspace://connector-smoke",
    cwd: null,
    branch: null,
    external_ref: null
  },
  task_prompt: "Finish retry hardening safely",
  files_in_focus: ["src/retry.rs"],
  task_hint: "retry-hardening",
  summary: "Kept retry state explicit and left one benchmark open.",
  files_touched: ["src/retry.rs", "tests/retry.rs"],
  decisions: [
    {
      decision: "Keep retry state explicit in service layer",
      rationale: "Failure-path audits are easier with explicit control flow"
    }
  ],
  unresolved_items: ["Need degraded-network timeout benchmark"],
  risk_signals: ["Without timeout benchmark, retry confidence may be overstated"],
  tests: [
    {
      name: "retry_backoff_bounds",
      status: "pass",
      summary: "Backoff envelope remains within policy limits"
    }
  ]
}')"

echo "=== task_start -> prepare_task_context ==="
run_daemon host connector --json "$start_payload" \
  | jq '{handled_as, prepare_context: (.prepare_context | {task_focus, likely_next_directions})}'

echo
echo "=== task_end -> store_work_episode ==="
run_daemon host connector --json "$end_payload" \
  | jq '{handled_as, stored_episode: (.stored_episode | {episode_id, task_scope_key, decisions, unresolved_items})}'
