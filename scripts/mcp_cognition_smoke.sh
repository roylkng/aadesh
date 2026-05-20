#!/usr/bin/env bash
set -euo pipefail

ADESH_DATABASE_URL="${ADESH_DATABASE_URL:-sqlite:///tmp/adesh-mcp-cognition-smoke.db?mode=rwc}"
MCP_TIMEOUT_SECONDS="${MCP_TIMEOUT_SECONDS:-20}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required" >&2
  exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required" >&2
  exit 1
fi
if ! command -v timeout >/dev/null 2>&1; then
  echo "timeout is required" >&2
  exit 1
fi

MCP_CALL_ID=0

mcp_call() {
  local method="$1"
  local params_json="$2"

  MCP_CALL_ID=$((MCP_CALL_ID + 1))
  local request_json
  request_json="$(jq -cn \
    --arg method "$method" \
    --argjson id "$MCP_CALL_ID" \
    --argjson params "$params_json" \
    '{jsonrpc:"2.0",id:$id,method:$method,params:$params}')"

  local content_length
  content_length="$(printf '%s' "$request_json" | wc -c | tr -d '[:space:]')"

  local raw_response
  raw_response="$(
    {
      printf 'Content-Length: %s\r\n\r\n%s' "$content_length" "$request_json"
    } | ADESH_DATABASE_URL="$ADESH_DATABASE_URL" \
      timeout "$MCP_TIMEOUT_SECONDS" \
      cargo run -q -p adesh-daemon -- host mcp-stdio
  )"

  local body
  body="$(printf '%s' "$raw_response" | tr -d '\r' | sed '1,/^$/d')"
  if [[ -z "$body" ]]; then
    echo "empty MCP response for method=$method" >&2
    exit 1
  fi
  if ! printf '%s' "$body" | jq -e . >/dev/null 2>&1; then
    echo "non-JSON MCP response for method=$method" >&2
    printf '%s\n' "$body" >&2
    exit 1
  fi

  printf '%s' "$body"
}

echo "[1] initialize"
init_params='{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"mcp-cognition-smoke","version":"0.1.0"}}'
init_response="$(mcp_call "initialize" "$init_params")"
printf '%s' "$init_response" | jq -e '.result.protocolVersion == "2024-11-05"' >/dev/null

echo "[2] tools/list"
tools_response="$(mcp_call "tools/list" "{}")"
printf '%s' "$tools_response" | jq -e '.result.tools | length >= 4' >/dev/null
printf '%s' "$tools_response" | jq -e '.result.tools[].name | select(. == "adesh.prepare_task_context")' >/dev/null
printf '%s' "$tools_response" | jq -e '.result.tools[].name | select(. == "adesh.store_work_episode")' >/dev/null
printf '%s' "$tools_response" | jq -e '.result.tools[].name | select(. == "adesh.recall_relevant_memory")' >/dev/null
printf '%s' "$tools_response" | jq -e '.result.tools[].name | select(. == "adesh.connector_event")' >/dev/null

cwd="$(pwd)"
workspace_json="$(jq -cn --arg cwd "$cwd" '{kind:"directory",locator:$cwd,cwd:$cwd,branch:null,external_ref:null}')"

echo "[3] tools/call -> adesh.store_work_episode"
store_arguments="$(jq -cn \
  --argjson workspace "$workspace_json" \
  '{
    workspace: $workspace,
    task_prompt: "MCP stdio smoke: store episode",
    summary: "Stored through MCP stdio bridge to validate frozen cognition integration.",
    files_touched: ["README.md"],
    tests: [{name:"mcp_stdio_smoke_store",status:"pass",summary:"store_work_episode via tools/call succeeded"}],
    decisions: [{decision:"Keep MCP adapter thin over cognition core",rationale:"Transport should not fork cognition behavior"}],
    unresolved_items: ["Need recurring MCP smoke in CI before broad host rollout"],
    observed_preferences: [],
    risk_signals: ["MCP adapter regressions could silently break host integrations"],
    issue_refs: [],
    artifact_refs: [],
    task_hint: "mcp-smoke",
    started_at: null,
    ended_at: null
  }')"
store_params="$(jq -cn --arg name "adesh.store_work_episode" --argjson arguments "$store_arguments" '{name:$name,arguments:$arguments}')"
store_response="$(mcp_call "tools/call" "$store_params")"
episode_id="$(printf '%s' "$store_response" | jq -r '.result.structuredContent.episode_id')"
if [[ -z "$episode_id" || "$episode_id" == "null" ]]; then
  echo "store_work_episode did not return episode_id" >&2
  printf '%s\n' "$store_response" >&2
  exit 1
fi

echo "[4] tools/call -> adesh.store_work_episode (reinforcement)"
store_reinforce_arguments="$(jq -cn \
  --argjson workspace "$workspace_json" \
  '{
    workspace: $workspace,
    task_prompt: "MCP stdio smoke: reinforce episode",
    summary: "Second MCP smoke episode to reinforce memory promotion for recall validation.",
    files_touched: ["crates/adesh-daemon/src/mcp_stdio.rs"],
    tests: [{name:"mcp_stdio_smoke_store_reinforce",status:"pass",summary:"second store_work_episode via tools/call succeeded"}],
    decisions: [{decision:"Keep MCP adapter thin over cognition core",rationale:"Transport should remain an adapter"}],
    unresolved_items: ["Need recurring MCP smoke in CI before broad host rollout"],
    observed_preferences: [],
    risk_signals: ["MCP adapter regressions could silently break host integrations"],
    issue_refs: [],
    artifact_refs: [],
    task_hint: "mcp-smoke",
    started_at: null,
    ended_at: null
  }')"
store_reinforce_params="$(jq -cn --arg name "adesh.store_work_episode" --argjson arguments "$store_reinforce_arguments" '{name:$name,arguments:$arguments}')"
store_reinforce_response="$(mcp_call "tools/call" "$store_reinforce_params")"
printf '%s' "$store_reinforce_response" | jq -e '.result.structuredContent.episode_id | type == "string"' >/dev/null

echo "[5] tools/call -> adesh.prepare_task_context"
prepare_arguments="$(jq -cn \
  --argjson workspace "$workspace_json" \
  '{
    workspace: $workspace,
    task_prompt: "What should I prioritize next for MCP host integration?",
    files_in_focus: ["crates/adesh-daemon/src/mcp_stdio.rs"],
    task_hint: "mcp-smoke"
  }')"
prepare_params="$(jq -cn --arg name "adesh.prepare_task_context" --argjson arguments "$prepare_arguments" '{name:$name,arguments:$arguments}')"
prepare_response="$(mcp_call "tools/call" "$prepare_params")"
printf '%s' "$prepare_response" | jq -e '.result.structuredContent.likely_next_directions | length >= 1' >/dev/null
printf '%s' "$prepare_response" | jq -e '.result.structuredContent.open_loops | length >= 1' >/dev/null

echo "[6] tools/call -> adesh.recall_relevant_memory"
recall_arguments="$(jq -cn \
  --argjson workspace "$workspace_json" \
  '{
    workspace: $workspace,
    query: "mcp adapter",
    task_hint: "mcp-smoke",
    memory_types: ["decision","open_loop","risk"],
    limit: 5
  }')"
recall_params="$(jq -cn --arg name "adesh.recall_relevant_memory" --argjson arguments "$recall_arguments" '{name:$name,arguments:$arguments}')"
recall_response="$(mcp_call "tools/call" "$recall_params")"
printf '%s' "$recall_response" | jq -e '.result.structuredContent.memories | length >= 1' >/dev/null

top_direction="$(printf '%s' "$prepare_response" | jq -r '.result.structuredContent.likely_next_directions[0].statement')"
echo "mcp cognition smoke completed successfully"
echo "episode_id=${episode_id}"
echo "top_next_direction=${top_direction}"
