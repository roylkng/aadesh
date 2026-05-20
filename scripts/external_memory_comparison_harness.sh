#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: ./scripts/external_memory_comparison_harness.sh [options]

Runs a deterministic comparison scenario for Aadesh vs adjacent memory systems.
The harness always evaluates Aadesh and baseline. External systems are recorded
as adapter stubs unless explicit exported result JSON is provided.

Options:
  --output-dir DIR              Directory for DBs and reports.
  --include-external-stubs      Include memd/knowns/openmemory/hermes rows as not-run adapter slots.
  --run-hermes-probe            Run a local Hermes runtime probe in an isolated HERMES_HOME.
  --run-hermes-benchmark        Run Hermes against the same comparison tasks and score its output.
  --run-memory-layer-probe      Probe local availability of knowns/openmemory/memd without installing them.
  --run-openmemory-direct-benchmark
                                Run a direct mem0/OpenMemory memory-layer comparator with Docker + LM Studio.
  --run-live-cli-trace          Run installed Qwen/OpenCode/Gemini/Codex CLIs through Aadesh trace validation.
  --run-hard-supervisory-benchmark
                                After this comparison, run the hard multi-week judge benchmark.
  --live-cli-timeout N          Per-CLI timeout for --run-live-cli-trace. Default: 180.
  --hard-days N                 Days for hard benchmark. Default: 14.
  --hard-sessions N             Deep sessions for hard benchmark. Default: 12.
  --stress-events N             Adversarial noise events for hard benchmark. Default: 36.
  --data-profile PROFILE        Hard benchmark data profile: standard or production. Default: production.
  --judge-mode MODE             local or lmstudio for hard benchmark. Default: local.
  --external-result NAME=PATH   Add external system result JSON for aggregation.
                                Expected schema documented in docs/COMPARISON_BENCHMARK.md.
  -h, --help                    Show this help.

Environment:
  ADESH_DAEMON_ROOT             Override repo root.
  ADESH_CARGO_TARGET_DIR        Cargo target dir. Default: /tmp/adesh-cargo-target.
  HERMES_BASE_URL               Hermes custom model base URL. Default: http://127.0.0.1:1234/v1.
  HERMES_MODEL                  Hermes model. Default: qwen/qwen3.6-27b.
  HERMES_TIMEOUT_SECONDS        Per-task Hermes timeout. Default: 180.
  OPENMEMORY_LMSTUDIO_BASE_URL  Host LM Studio OpenAI-compatible URL. Default: http://127.0.0.1:1234/v1.
  OPENMEMORY_LMSTUDIO_CHAT_URL  Host LM Studio chat URL for generated answers. Default: http://127.0.0.1:1234/api/v1/chat.
  OPENMEMORY_CONTAINER_LMSTUDIO_BASE_URL
                                Container-visible LM Studio URL. Default: http://host.docker.internal:1234/v1.
  OPENMEMORY_LLM_MODEL          Chat model. Default: HERMES_MODEL or qwen/qwen3.6-27b.
  OPENMEMORY_EMBED_MODEL        Embedding model. Default: text-embedding-nomic-embed-text-v1.5.
  OPENMEMORY_EMBED_DIMS         Embedding dimensions. Default: 768.
  OPENMEMORY_TASK_TIMEOUT_SECONDS
                                Per-task OpenMemory answer timeout. Default: 120.
  OPENMEMORY_KEEP_CONTAINERS    Keep comparator containers for debugging. Default: 0.
  LIVE_CLI_TIMEOUT_SECONDS      Per-CLI live trace timeout. Default: 180.
  LIVE_CLI_MODEL                Qwen CLI local model default. Default: qwen/qwen3.6-27b.

Purpose:
  This is not a vendor integration harness. It is the stable comparison contract:
  same multi-session scenario, same scoring fields, explicit setup/portability/
  outcome-trace dimensions, and explicit slots for external tools once adapters
  or exported reports exist.
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

ADESH_ROOT="${ADESH_DAEMON_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
if [[ ! -f "${ADESH_ROOT}/Cargo.toml" ]]; then
  echo "ADESH_DAEMON_ROOT does not point to an Aadesh repo: ${ADESH_ROOT}" >&2
  exit 1
fi

OUTPUT_DIR=""
INCLUDE_EXTERNAL_STUBS=0
RUN_HERMES_PROBE=0
RUN_HERMES_BENCHMARK=0
RUN_MEMORY_LAYER_PROBE=0
RUN_OPENMEMORY_DIRECT_BENCHMARK=0
RUN_LIVE_CLI_TRACE=0
RUN_HARD_SUPERVISORY_BENCHMARK=0
LIVE_CLI_TIMEOUT_SECONDS="${LIVE_CLI_TIMEOUT_SECONDS:-180}"
HARD_BENCHMARK_DAYS=14
HARD_BENCHMARK_SESSIONS=12
HARD_STRESS_EVENTS=36
HARD_DATA_PROFILE=production
HARD_JUDGE_MODE=local
EXTERNAL_RESULTS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-dir)
      OUTPUT_DIR="${2:-}"
      shift 2
      ;;
    --include-external-stubs)
      INCLUDE_EXTERNAL_STUBS=1
      shift
      ;;
    --run-hermes-probe)
      RUN_HERMES_PROBE=1
      shift
      ;;
    --run-hermes-benchmark)
      RUN_HERMES_BENCHMARK=1
      shift
      ;;
    --run-memory-layer-probe)
      RUN_MEMORY_LAYER_PROBE=1
      shift
      ;;
    --run-openmemory-direct-benchmark)
      RUN_OPENMEMORY_DIRECT_BENCHMARK=1
      shift
      ;;
    --run-live-cli-trace)
      RUN_LIVE_CLI_TRACE=1
      shift
      ;;
    --run-hard-supervisory-benchmark)
      RUN_HARD_SUPERVISORY_BENCHMARK=1
      shift
      ;;
    --live-cli-timeout)
      LIVE_CLI_TIMEOUT_SECONDS="${2:-}"
      shift 2
      ;;
    --hard-days)
      HARD_BENCHMARK_DAYS="${2:-}"
      shift 2
      ;;
    --hard-sessions)
      HARD_BENCHMARK_SESSIONS="${2:-}"
      shift 2
      ;;
    --stress-events)
      HARD_STRESS_EVENTS="${2:-}"
      shift 2
      ;;
    --data-profile)
      HARD_DATA_PROFILE="${2:-}"
      shift 2
      ;;
    --judge-mode)
      HARD_JUDGE_MODE="${2:-}"
      shift 2
      ;;
    --external-result)
      EXTERNAL_RESULTS+=("${2:-}")
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

if [[ "$HARD_DATA_PROFILE" != "standard" && "$HARD_DATA_PROFILE" != "production" ]]; then
  echo "--data-profile must be standard or production" >&2
  exit 1
fi
if ! [[ "$LIVE_CLI_TIMEOUT_SECONDS" =~ ^[0-9]+$ ]] || [[ "$LIVE_CLI_TIMEOUT_SECONDS" -lt 5 ]]; then
  echo "--live-cli-timeout must be an integer >= 5" >&2
  exit 1
fi

RUN_ID="$(date +%Y%m%d%H%M%S)"
OUTPUT_DIR="${OUTPUT_DIR:-/tmp/adesh-external-comparison-${RUN_ID}}"
mkdir -p "$OUTPUT_DIR"

CARGO_TARGET_DIR="${ADESH_CARGO_TARGET_DIR:-/tmp/adesh-cargo-target}"
BASELINE_DB="${OUTPUT_DIR}/baseline.db"
ADESH_DB="${OUTPUT_DIR}/aadesh.db"
BASELINE_URL="sqlite://${BASELINE_DB}?mode=rwc"
ADESH_URL="sqlite://${ADESH_DB}?mode=rwc"
REPORT_PATH="${OUTPUT_DIR}/comparison_report.json"
TASKS_FILE="${OUTPUT_DIR}/comparison_tasks.tsv"
ROWS_FILE="${OUTPUT_DIR}/comparison_rows.jsonl"
rm -f "$BASELINE_DB" "$ADESH_DB" "$ROWS_FILE"

run_host() {
  local db_url="$1"
  shift
  ADESH_DATABASE_URL="$db_url" \
    CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
    cargo run -q --manifest-path "${ADESH_ROOT}/Cargo.toml" -p adesh-daemon -- host "$@"
}

run_store() {
  local db_url="$1"
  shift
  run_host "$db_url" store "$@" >/dev/null
}

run_prepare() {
  local db_url="$1"
  shift
  run_host "$db_url" prepare "$@"
}

score_response() {
  local response_path="$1"
  local decision_kw="$2"
  local open_kw="$3"
  local preference_kw="$4"
  local direction_kw="$5"

  jq \
    --arg decision_kw "$decision_kw" \
    --arg open_kw "$open_kw" \
    --arg preference_kw "$preference_kw" \
    --arg direction_kw "$direction_kw" \
    '
      def item_text(item):
        [item.statement, item.basis, item.confidence, (item.evidence_refs // [] | join(" "))]
        | map(. // "")
        | join(" ")
        | ascii_downcase;

      def has_kw(items; kw):
        if (kw | length) == 0 then false
        else [items[]? | item_text(.) | contains(kw | ascii_downcase)] | any
        end;

      def unsupported_count(items):
        [items[]? | select((.evidence_refs // [] | length) == 0)] | length;

      . as $r
      | (has_kw($r.relevant_decisions; $decision_kw)) as $decision_hit
      | (has_kw($r.open_loops; $open_kw)) as $open_hit
      | (has_kw($r.applicable_preferences; $preference_kw)) as $preference_hit
      | (has_kw($r.likely_next_directions; $direction_kw)) as $direction_hit
      | (
          unsupported_count($r.relevant_decisions)
          + unsupported_count($r.applicable_preferences)
          + unsupported_count($r.open_loops)
          + unsupported_count($r.risk_flags)
          + unsupported_count($r.likely_next_directions)
        ) as $unsupported
      | {
          decision_hit: $decision_hit,
          open_loop_hit: $open_hit,
          preference_hit: $preference_hit,
          direction_hit: $direction_hit,
          false_memory_clear: ($unsupported == 0),
          unsupported_count: $unsupported,
          surfaced_items: (
            (($r.relevant_decisions | length) // 0)
            + (($r.applicable_preferences | length) // 0)
            + (($r.open_loops | length) // 0)
            + (($r.risk_flags | length) // 0)
            + (($r.likely_next_directions | length) // 0)
          ),
          score: (
            (if $decision_hit then 1 else 0 end)
            + (if $open_hit then 1 else 0 end)
            + (if $preference_hit then 1 else 0 end)
            + (if $direction_hit then 1 else 0 end)
            + (if $unsupported == 0 then 1 else 0 end)
          )
        }
    ' "$response_path"
}

score_plain_text() {
  local response_path="$1"
  local decision_kw="$2"
  local open_kw="$3"
  local preference_kw="$4"
  local direction_kw="$5"

  jq -Rs \
    --arg decision_kw "$decision_kw" \
    --arg open_kw "$open_kw" \
    --arg preference_kw "$preference_kw" \
    --arg direction_kw "$direction_kw" \
    '
      def has_kw($text; kw):
          if (kw | length) == 0 then false
          else ($text | contains(kw | ascii_downcase))
          end;
      (ascii_downcase) as $text
      | (
          [
            "do not know",
            "no memory",
            "not enough context",
            "cannot determine",
            "no relevant memory",
            "failed to load model",
            "model is unloaded",
            "error code:",
            "invalid_request",
            "no memory comparator response",
            "session_id:"
          ]
          | map(. as $term | select($text | contains($term)))
          | length
        ) as $unsupported
      | {
          decision_hit: has_kw($text; $decision_kw),
          open_loop_hit: has_kw($text; $open_kw),
          preference_hit: has_kw($text; $preference_kw),
          direction_hit: has_kw($text; $direction_kw),
          false_memory_clear: ($unsupported == 0),
          unsupported_count: $unsupported,
          surfaced_items: null,
          score: (
            (if has_kw($text; $decision_kw) then 1 else 0 end)
            + (if has_kw($text; $open_kw) then 1 else 0 end)
            + (if has_kw($text; $preference_kw) then 1 else 0 end)
            + (if has_kw($text; $direction_kw) then 1 else 0 end)
            + (if $unsupported == 0 then 1 else 0 end)
          )
        }
    ' "$response_path"
}

seed_aadesh_memory() {
  echo "Seeding Aadesh comparison memory..." >&2

  run_store "$ADESH_URL" \
    --workspace-kind task_space \
    --workspace-locator "workspace://compare-payments" \
    --task "Stabilize payment retry ingestion" \
    --task-hint payment-reliability \
    --summary "Kept idempotency at service boundary; timeout coverage and webhook replay proof remain open." \
    --file src/payments/retry_worker.rs \
    --file src/payments/idempotency.rs \
    --decision "Keep payment idempotency at the service boundary, not inside transport retry helpers" \
    --unresolved "Timeout coverage is still missing for payment retry under packet loss" \
    --preference "Prefer explicit service-boundary tests for reliability-sensitive changes" \
    --risk "Webhook replay may duplicate charges if timeout behavior is not covered" \
    --test "fail::payment_retry_timeout::timeout path still fails under packet loss"

  run_store "$ADESH_URL" \
    --workspace-kind task_space \
    --workspace-locator "workspace://compare-payments" \
    --task "Resolve stale payment retry plan" \
    --task-hint payment-reliability \
    --summary "Older helper-level idempotency idea was rejected after review; do not revive it." \
    --file src/payments/retry_worker.rs \
    --decision "Do not move idempotency into retry helper internals" \
    --unresolved "Need incident-style replay test for duplicate-charge prevention" \
    --preference "Prefer explicit service-boundary tests for reliability-sensitive changes" \
    --risk "Stale helper-level plans can reintroduce duplicate-charge risk" \
    --test "pass::idempotency_boundary::service layer owns duplicate protection"

  run_store "$ADESH_URL" \
    --workspace-kind task_space \
    --workspace-locator "workspace://compare-connectors" \
    --task "Improve multi-agent connector trace quality" \
    --task-hint connector-supervision \
    --summary "Codex and Qwen traces linked correctly; Gemini traces are sparse and should remain non-learnable until context IDs round-trip." \
    --file scripts/qwen_code_with_aadesh.sh \
    --file scripts/gemini_with_aadesh.sh \
    --decision "Only linked accepted outcomes should influence advisory ranking" \
    --unresolved "Gemini wrapper still needs reliable context_id round-trip before learning from its outcomes" \
    --preference "Prefer generic connector events over per-agent cognition branches" \
    --risk "Unlinked sparse traces can pollute outcome learning if treated as learnable"

  run_store "$ADESH_URL" \
    --workspace-kind task_space \
    --workspace-locator "workspace://compare-eval" \
    --task "Prove Aadesh against external memory systems" \
    --task-hint external-comparison \
    --summary "Next proof should compare Aadesh against memd, Knowns, OpenMemory, and Hermes on the same tasks; success depends on outcome-aware guidance, not plain memory recall." \
    --file docs/COMPARISON_BENCHMARK.md \
    --decision "Aadesh only has a wedge if intervention/outcome-aware guidance beats memory-only recall" \
    --unresolved "Need external comparison report with baseline, Aadesh, memd, Knowns, OpenMemory, and Hermes rows" \
    --preference "Prefer measured comparison over broad architecture claims" \
    --risk "If Aadesh only matches memory recall, it should become a layer over an existing memory backend"
}

write_tasks() {
  cat >"$TASKS_FILE" <<'EOF_TASKS'
payment-01|workspace://compare-payments|payment-reliability|What should I do first to safely finish payment retry reliability?|service boundary|timeout coverage|service-boundary tests|timeout
payment-02|workspace://compare-payments|payment-reliability|I am about to touch retry helpers. What stale plan should I avoid?|retry helper internals|incident-style replay test|service-boundary tests|duplicate-charge
payment-03|workspace://compare-payments|payment-reliability|Prioritize between small cleanup and proving webhook replay safety.|service boundary|replay test|reliability-sensitive|replay
connector-01|workspace://compare-connectors|connector-supervision|What matters before learning from Gemini CLI traces?|linked accepted outcomes|context_id round-trip|generic connector events|context_id
connector-02|workspace://compare-connectors|connector-supervision|What should the connector layer preserve across Codex and Qwen?|linked accepted outcomes|context_id round-trip|generic connector events|context_id
connector-03|workspace://compare-connectors|connector-supervision|What is the risk in sparse host traces?|linked accepted outcomes|Gemini wrapper|generic connector events|unlinked sparse traces
eval-01|workspace://compare-eval|external-comparison|How do we prove whether Aadesh is better than memory-only tools?|outcome-aware guidance|external comparison report|measured comparison|memory-only recall
eval-02|workspace://compare-eval|external-comparison|What should happen if Aadesh only matches memd, Knowns, or Hermes on recall?|intervention/outcome-aware guidance|baseline, Aadesh, memd|measured comparison|existing memory backend
eval-03|workspace://compare-eval|external-comparison|What is the next defensible validation step before broadening scope?|intervention/outcome-aware guidance|external comparison report|measured comparison|comparison
EOF_TASKS
}

write_hermes_memory_seed() {
  local hermes_home="$1"
  mkdir -p "${hermes_home}/memories"
  cat >"${hermes_home}/memories/MEMORY.md" <<'EOF_MEMORY'
# Aadesh external comparison memory seed

## Payment reliability
- Decision: Payment retry idempotency stays at the service boundary, not inside retry helper internals.
- Open loop: Timeout coverage is still missing for payment retry reliability.
- Preference: Prefer service-boundary tests for reliability-sensitive payment work.
- Risk: Webhook replay can cause duplicate charges unless covered by an incident-style replay test.
- Stale plan to avoid: Do not move idempotency into retry helper internals.

## Connector supervision
- Decision: Only linked accepted outcomes should influence advisory ranking.
- Open loop: Gemini wrapper must round-trip context_id from TaskStart to TaskEnd before traces become learnable.
- Preference: Preserve generic connector events across Codex, Qwen, Gemini, OpenCode, and other hosts.
- Risk: Unlinked sparse traces can pollute outcome learning and should remain non-learnable.

## External comparison
- Decision: Aadesh's wedge depends on intervention/outcome-aware guidance beating memory-only recall.
- Open loop: Need external comparison report with baseline, Aadesh, memd, Knowns, OpenMemory, and Hermes.
- Preference: Prefer measured comparison before broadening scope.
- Fallback: If Aadesh only matches memory recall, narrow around supervisory traces/eval learning or layer over an existing memory backend.
EOF_MEMORY
}

run_system_rows() {
  local system_name="$1"
  local db_url="$2"

  while IFS='|' read -r task_id workspace_locator task_hint prompt decision_kw open_kw preference_kw direction_kw; do
    local response_path="${OUTPUT_DIR}/${system_name}_${task_id}.json"
    run_prepare "$db_url" \
      --workspace-kind task_space \
      --workspace-locator "$workspace_locator" \
      --task-hint "$task_hint" \
      --task "$prompt" >"$response_path"

    local metrics
    metrics="$(score_response "$response_path" "$decision_kw" "$open_kw" "$preference_kw" "$direction_kw")"

    jq -n \
      --arg system "$system_name" \
      --arg task_id "$task_id" \
      --arg response_path "$response_path" \
      --argjson metrics "$metrics" \
      '{system: $system, task_id: $task_id, response_path: $response_path, metrics: $metrics}' \
      >>"$ROWS_FILE"
  done <"$TASKS_FILE"
}

add_external_stub() {
  local system_name="$1"
  local comparator_class="${2:-memory_layer}"
  jq -n \
    --arg system "$system_name" \
    --arg comparator_class "$comparator_class" \
    '{
      system: $system,
      comparator_class: $comparator_class,
      status: "not_run",
      reason: "adapter/export not provided; slot exists so external runs use the same report schema",
      aggregate: null,
      dimensions: {
        memory_recall_quality: null,
        next_direction_quality: null,
        setup_friction: null,
        cross_host_portability: null,
        outcome_trace_learning: null
      }
    }' >>"${OUTPUT_DIR}/external_rows.jsonl"
}

run_hermes_probe() {
  local hermes_home="${OUTPUT_DIR}/hermes_home"
  local version_path="${OUTPUT_DIR}/hermes_version.txt"
  local memory_status_path="${OUTPUT_DIR}/hermes_memory_status.txt"
  local config_path="${OUTPUT_DIR}/hermes_config.txt"
  local chat_probe_path="${OUTPUT_DIR}/hermes_chat_probe.txt"
  mkdir -p "$hermes_home"

  if ! command -v hermes >/dev/null 2>&1; then
    jq -n \
      '{
        system: "hermes",
        comparator_class: "host_runtime",
        status: "not_installed",
        reason: "hermes command not found",
        aggregate: null,
        dimensions: {
          memory_recall_quality: null,
          next_direction_quality: null,
          setup_friction: {score: 0, note: "not installed"},
          cross_host_portability: null,
          outcome_trace_learning: null
        }
      }' >>"${OUTPUT_DIR}/external_rows.jsonl"
    return 0
  fi

  HERMES_HOME="$hermes_home" hermes --version >"$version_path" 2>&1 || true
  HERMES_HOME="$hermes_home" hermes memory status >"$memory_status_path" 2>&1 || true
  HERMES_HOME="$hermes_home" hermes config show >"$config_path" 2>&1 || true

  local chat_exit=0
  if command -v timeout >/dev/null 2>&1; then
    HERMES_HOME="$hermes_home" timeout 20s hermes chat -Q -q "Say READY in one word." >"$chat_probe_path" 2>&1 || chat_exit=$?
  else
    HERMES_HOME="$hermes_home" hermes chat -Q -q "Say READY in one word." >"$chat_probe_path" 2>&1 || chat_exit=$?
  fi

  local status="run"
  local task_quality_note="Hermes chat probe completed; inspect artifact before treating as scored task-quality evidence."
  if [[ "$chat_exit" -ne 0 ]]; then
    status="blocked_unconfigured"
    task_quality_note="Hermes is installed, but isolated HERMES_HOME has no model/API configuration; task-quality comparison not claimed."
  fi

  jq -n \
    --arg status "$status" \
    --arg version_path "$version_path" \
    --arg memory_status_path "$memory_status_path" \
    --arg config_path "$config_path" \
    --arg chat_probe_path "$chat_probe_path" \
    --arg task_quality_note "$task_quality_note" \
    '{
      system: "hermes",
      comparator_class: "host_runtime",
      status: $status,
      aggregate: null,
      dimensions: {
        memory_recall_quality: {
          score: null,
          note: "Hermes has built-in memory/session-search/provider mechanisms, but this probe did not run the benchmark tasks."
        },
        next_direction_quality: {
          score: null,
          note: $task_quality_note
        },
        setup_friction: {
          score: (if $status == "run" then 1 else 0.4 end),
          note: "CLI installed locally; isolated benchmark home needs model/provider configuration before task prompts can run."
        },
        cross_host_portability: {
          score: 0.5,
          note: "Hermes is a host/runtime with CLI/gateway/plugins; it is portable as an agent, not a host-neutral cross-agent substrate."
        },
        outcome_trace_learning: {
          score: 0,
          note: "No first-class accepted/ignored/modified intervention-outcome trace contract was exercised by this probe."
        }
      },
      artifacts: {
        version: $version_path,
        memory_status: $memory_status_path,
        config: $config_path,
        chat_probe: $chat_probe_path
      },
      notes: "Real local Hermes probe. Do not score Hermes recall/next-direction quality until configured and run against comparison_tasks.tsv."
    }' >>"${OUTPUT_DIR}/external_rows.jsonl"
}

run_hermes_benchmark() {
  local hermes_home="${OUTPUT_DIR}/hermes_home"
  local hermes_rows_path="${OUTPUT_DIR}/hermes_rows.jsonl"
  local hermes_output_dir="${OUTPUT_DIR}/hermes_outputs"
  local hermes_base_url="${HERMES_BASE_URL:-http://127.0.0.1:1234/v1}"
  local hermes_model="${HERMES_MODEL:-qwen/qwen3.6-27b}"
  local hermes_timeout="${HERMES_TIMEOUT_SECONDS:-180}"
  mkdir -p "$hermes_home" "$hermes_output_dir"
  : >"$hermes_rows_path"

  if ! command -v hermes >/dev/null 2>&1; then
    jq -n \
      '{
        system: "hermes",
        comparator_class: "host_runtime",
        status: "not_installed",
        reason: "hermes command not found",
        aggregate: null,
        dimensions: {
          memory_recall_quality: null,
          next_direction_quality: null,
          setup_friction: {score: 0, note: "not installed"},
          cross_host_portability: null,
          outcome_trace_learning: null
        }
      }' >>"${OUTPUT_DIR}/external_rows.jsonl"
    return 0
  fi

  write_hermes_memory_seed "$hermes_home"
  HERMES_HOME="$hermes_home" hermes config set model.provider custom >/dev/null
  HERMES_HOME="$hermes_home" hermes config set model.base_url "$hermes_base_url" >/dev/null
  HERMES_HOME="$hermes_home" hermes config set model.default "$hermes_model" >/dev/null
  HERMES_HOME="$hermes_home" hermes config set memory.memory_enabled true >/dev/null
  HERMES_HOME="$hermes_home" hermes config set memory.user_profile_enabled true >/dev/null

  while IFS='|' read -r task_id workspace_locator task_hint prompt decision_kw open_kw preference_kw direction_kw; do
    local response_path="${hermes_output_dir}/${task_id}.txt"
    local task_exit=0
    local full_prompt
    full_prompt="You are being benchmarked as a coding-agent memory system. Use only relevant persistent memory. Current workspace: ${workspace_locator}. Current task scope: ${task_hint}. Current task: ${prompt}. Return compact guidance with decisions, open loops, preferences, risks, and likely next directions. Do not use tools."

    if command -v timeout >/dev/null 2>&1; then
      HERMES_HOME="$hermes_home" timeout "${hermes_timeout}s" hermes chat -Q --max-turns 1 -t memory -q "$full_prompt" >"$response_path" 2>&1 || task_exit=$?
    else
      HERMES_HOME="$hermes_home" hermes chat -Q --max-turns 1 -t memory -q "$full_prompt" >"$response_path" 2>&1 || task_exit=$?
    fi

    local response_status="run"
    if [[ "$task_exit" -ne 0 ]] || grep -Eiq 'failed to load model|model is unloaded|invalid_request|error code:|^session_id:[[:space:]]*$' "$response_path"; then
      response_status="blocked_generation"
    fi

    local metrics
    metrics="$(score_plain_text "$response_path" "$decision_kw" "$open_kw" "$preference_kw" "$direction_kw")"
    jq -cn \
      --arg system "hermes" \
      --arg task_id "$task_id" \
      --arg response_path "$response_path" \
      --arg response_status "$response_status" \
      --argjson exit_code "$task_exit" \
      --argjson metrics "$metrics" \
      '{system: $system, task_id: $task_id, response_path: $response_path, response_status: $response_status, exit_code: $exit_code, metrics: $metrics}' \
      >>"$hermes_rows_path"
  done <"$TASKS_FILE"

  jq -s \
    --arg rows_path "$hermes_rows_path" \
    --arg output_dir "$hermes_output_dir" \
    --arg hermes_home "$hermes_home" \
    --arg hermes_base_url "$hermes_base_url" \
    --arg hermes_model "$hermes_model" \
    '[.[]] as $rows
     | {
        system: "hermes",
        comparator_class: "host_runtime",
        status: (
          if ($rows | map(select(.response_status == "blocked_generation")) | length) == ($rows | length) then "blocked_generation"
          elif ($rows | map(select(.exit_code != 0 or .response_status != "run")) | length) == 0 then "run"
          else "partial" end
        ),
        tasks: ($rows | length),
        blocked_generation_count: ($rows | map(select(.response_status == "blocked_generation")) | length),
        mean_score: (($rows | map(.metrics.score) | add) / ($rows | length)),
        decision_recall: (($rows | map(if .metrics.decision_hit then 1 else 0 end) | add) / ($rows | length)),
        open_loop_recall: (($rows | map(if .metrics.open_loop_hit then 1 else 0 end) | add) / ($rows | length)),
        preference_recall: (($rows | map(if .metrics.preference_hit then 1 else 0 end) | add) / ($rows | length)),
        next_direction_acceptance_proxy: (($rows | map(if .metrics.direction_hit then 1 else 0 end) | add) / ($rows | length)),
        false_memory_rate_proxy: (($rows | map(if .metrics.false_memory_clear then 0 else 1 end) | add) / ($rows | length)),
        unsupported_count: ($rows | map(.metrics.unsupported_count) | add),
        dimensions: {
          memory_recall_quality: {
            decision_recall: (($rows | map(if .metrics.decision_hit then 1 else 0 end) | add) / ($rows | length)),
            open_loop_recall: (($rows | map(if .metrics.open_loop_hit then 1 else 0 end) | add) / ($rows | length)),
            preference_recall: (($rows | map(if .metrics.preference_hit then 1 else 0 end) | add) / ($rows | length))
          },
          next_direction_quality: {
            acceptance_proxy: (($rows | map(if .metrics.direction_hit then 1 else 0 end) | add) / ($rows | length)),
            unsupported_count: ($rows | map(.metrics.unsupported_count) | add)
          },
          setup_friction: {
            score: 0.4,
            note: "Hermes required configured local model endpoint and seeded isolated MEMORY.md"
          },
          cross_host_portability: {
            score: 0.5,
            note: "Hermes is a host/runtime; portability is via Hermes plugins/gateway, not host-neutral memory substrate"
          },
          outcome_trace_learning: {
            score: 0,
            note: "No first-class accepted/ignored/modified outcome trace learning was exercised"
          }
        },
        artifacts: {
          rows: $rows_path,
          output_dir: $output_dir,
          hermes_home: $hermes_home
        },
        notes: ("Hermes task benchmark used local model endpoint " + $hermes_base_url + " with model " + $hermes_model + ". Scoring is lexical over generated guidance.")
      }' "$hermes_rows_path" >>"${OUTPUT_DIR}/external_rows.jsonl"
}

add_openmemory_direct_blocked_row() {
  local status="$1"
  local reason="$2"
  local artifact_path="${3:-}"

  jq -n \
    --arg status "$status" \
    --arg reason "$reason" \
    --arg artifact_path "$artifact_path" \
    '{
      system: "openmemory",
      comparator_class: "memory_layer",
      status: $status,
      reason: $reason,
      aggregate: null,
      dimensions: {
        memory_recall_quality: null,
        next_direction_quality: null,
        setup_friction: {
          score: 0,
          note: $reason
        },
        cross_host_portability: null,
        outcome_trace_learning: null
      },
      artifacts: (if ($artifact_path | length) > 0 then {probe: $artifact_path} else {} end)
    }' >>"${OUTPUT_DIR}/external_rows.jsonl"
}

write_openmemory_direct_scripts() {
  local bench_dir="$1"

  cat >"${bench_dir}/openmemory_seed.py" <<'PY'
import contextlib
import json
import os
import sys
from mem0 import Memory


def build_client() -> Memory:
    config = {
        "vector_store": {
            "provider": "qdrant",
            "config": {
                "collection_name": "openmemory",
                "host": "mem0_store",
                "port": 6333,
                "embedding_model_dims": int(os.environ.get("OPENMEMORY_EMBED_DIMS", "768")),
            },
        },
        "llm": {
            "provider": "lmstudio",
            "config": {
                "model": os.environ["OPENMEMORY_LLM_MODEL"],
                "lmstudio_base_url": os.environ["OPENMEMORY_CONTAINER_LMSTUDIO_BASE_URL"],
            },
        },
        "embedder": {
            "provider": "lmstudio",
            "config": {
                "model": os.environ["OPENMEMORY_EMBED_MODEL"],
                "embedding_dims": int(os.environ.get("OPENMEMORY_EMBED_DIMS", "768")),
                "lmstudio_base_url": os.environ["OPENMEMORY_CONTAINER_LMSTUDIO_BASE_URL"],
            },
        },
    }
    with contextlib.redirect_stdout(sys.stderr):
        return Memory.from_config(config)


MEMORIES = [
    """
Payment reliability memory.
Decision: Payment retry idempotency stays at the service boundary, not inside retry helper internals.
Open loop: Timeout coverage is still missing for payment retry reliability.
Open loop: Need incident-style replay test for duplicate-charge prevention.
Preference: Prefer service-boundary tests for reliability-sensitive payment work.
Risk: Webhook replay can cause duplicate charges unless covered by an incident-style replay test.
Stale plan to avoid: Do not move idempotency into retry helper internals.
""",
    """
Connector supervision memory.
Decision: Only linked accepted outcomes should influence advisory ranking.
Open loop: Gemini wrapper must round-trip context_id from TaskStart to TaskEnd before traces become learnable.
Preference: Preserve generic connector events across Codex, Qwen, Gemini, OpenCode, and other hosts.
Risk: Unlinked sparse traces can pollute outcome learning and should remain non-learnable.
""",
    """
External comparison memory.
Decision: Aadesh's wedge depends on intervention/outcome-aware guidance beating memory-only recall.
Open loop: Need external comparison report with baseline, Aadesh, memd, Knowns, OpenMemory, and Hermes.
Preference: Prefer measured comparison before broadening scope.
Fallback: If Aadesh only matches memory recall, narrow around supervisory traces/eval learning or layer over an existing memory backend.
""",
]


def main() -> None:
    client = build_client()
    for memory in MEMORIES:
        with contextlib.redirect_stdout(sys.stderr):
            client.add(memory, user_id="aadesh-comparison", infer=False)
    print(json.dumps({"seeded": len(MEMORIES)}))


if __name__ == "__main__":
    main()
PY

  cat >"${bench_dir}/openmemory_search.py" <<'PY'
import contextlib
import json
import os
import sys
from mem0 import Memory


def build_client() -> Memory:
    config = {
        "vector_store": {
            "provider": "qdrant",
            "config": {
                "collection_name": "openmemory",
                "host": "mem0_store",
                "port": 6333,
                "embedding_model_dims": int(os.environ.get("OPENMEMORY_EMBED_DIMS", "768")),
            },
        },
        "llm": {
            "provider": "lmstudio",
            "config": {
                "model": os.environ["OPENMEMORY_LLM_MODEL"],
                "lmstudio_base_url": os.environ["OPENMEMORY_CONTAINER_LMSTUDIO_BASE_URL"],
            },
        },
        "embedder": {
            "provider": "lmstudio",
            "config": {
                "model": os.environ["OPENMEMORY_EMBED_MODEL"],
                "embedding_dims": int(os.environ.get("OPENMEMORY_EMBED_DIMS", "768")),
                "lmstudio_base_url": os.environ["OPENMEMORY_CONTAINER_LMSTUDIO_BASE_URL"],
            },
        },
    }
    with contextlib.redirect_stdout(sys.stderr):
        return Memory.from_config(config)


def main() -> None:
    query = os.environ["OPENMEMORY_QUERY"]
    limit = int(os.environ.get("OPENMEMORY_LIMIT", "3"))
    client = build_client()
    with contextlib.redirect_stdout(sys.stderr):
        result = client.search(query, user_id="aadesh-comparison", limit=limit)
    print(json.dumps(result, default=str))


if __name__ == "__main__":
    main()
PY
}

run_openmemory_direct_benchmark() {
  local openmemory_rows_path="${OUTPUT_DIR}/openmemory_rows.jsonl"
  local openmemory_output_dir="${OUTPUT_DIR}/openmemory_outputs"
  local openmemory_retrieval_dir="${OUTPUT_DIR}/openmemory_retrieval"
  local openmemory_host_base_url="${OPENMEMORY_LMSTUDIO_BASE_URL:-http://127.0.0.1:1234/v1}"
  local openmemory_chat_url="${OPENMEMORY_LMSTUDIO_CHAT_URL:-http://127.0.0.1:1234/api/v1/chat}"
  local openmemory_container_base_url="${OPENMEMORY_CONTAINER_LMSTUDIO_BASE_URL:-http://host.docker.internal:1234/v1}"
  local openmemory_llm_model="${OPENMEMORY_LLM_MODEL:-${HERMES_MODEL:-qwen/qwen3.6-27b}}"
  local openmemory_embed_model="${OPENMEMORY_EMBED_MODEL:-text-embedding-nomic-embed-text-v1.5}"
  local openmemory_embed_dims="${OPENMEMORY_EMBED_DIMS:-768}"
  local openmemory_task_timeout="${OPENMEMORY_TASK_TIMEOUT_SECONDS:-120}"
  local keep_containers="${OPENMEMORY_KEEP_CONTAINERS:-0}"
  local probe_path="${OUTPUT_DIR}/openmemory_direct_probe.txt"
  local network_name="aadesh-openmemory-${RUN_ID}-$$"
  local qdrant_container="aadesh-openmemory-qdrant-${RUN_ID}-$$"
  local runner_container="aadesh-openmemory-runner-${RUN_ID}-$$"
  mkdir -p "$openmemory_output_dir" "$openmemory_retrieval_dir"
  : >"$openmemory_rows_path"

  if ! command -v docker >/dev/null 2>&1; then
    echo "docker command not found" >"$probe_path"
    add_openmemory_direct_blocked_row "blocked_environment" "docker command not found" "$probe_path"
    return 0
  fi

  if ! docker info >"$probe_path" 2>&1; then
    add_openmemory_direct_blocked_row "blocked_environment" "Docker is unavailable or permission-denied" "$probe_path"
    return 0
  fi

  if ! docker image inspect mem0/openmemory-mcp:latest >/dev/null 2>&1; then
    echo "mem0/openmemory-mcp:latest image not present locally" >"$probe_path"
    add_openmemory_direct_blocked_row "not_installed" "mem0/openmemory-mcp:latest image is not present locally; harness does not pull/install external tools" "$probe_path"
    return 0
  fi

  if ! docker image inspect qdrant/qdrant:latest >/dev/null 2>&1; then
    echo "qdrant/qdrant:latest image not present locally" >"$probe_path"
    add_openmemory_direct_blocked_row "not_installed" "qdrant/qdrant:latest image is not present locally; harness does not pull/install external tools" "$probe_path"
    return 0
  fi

  if ! curl -sS "${openmemory_host_base_url%/}/models" >/dev/null 2>"$probe_path"; then
    add_openmemory_direct_blocked_row "blocked_environment" "LM Studio endpoint is unavailable at ${openmemory_host_base_url}" "$probe_path"
    return 0
  fi

  cleanup_openmemory_direct() {
    if [[ "$keep_containers" == "1" ]]; then
      echo "Keeping OpenMemory comparator containers: ${qdrant_container}, ${runner_container}, network ${network_name}" >&2
      return 0
    fi
    docker rm -f "$runner_container" "$qdrant_container" >/dev/null 2>&1 || true
    docker network rm "$network_name" >/dev/null 2>&1 || true
  }
  trap cleanup_openmemory_direct EXIT

  docker network create "$network_name" >"$probe_path"
  docker run -d \
    --name "$qdrant_container" \
    --network "$network_name" \
    --network-alias mem0_store \
    qdrant/qdrant:latest >/dev/null
  docker run -d \
    --name "$runner_container" \
    --network "$network_name" \
    --add-host host.docker.internal:host-gateway \
    -v "${OUTPUT_DIR}:/bench" \
    --entrypoint sleep \
    mem0/openmemory-mcp:latest infinity >/dev/null

  local qdrant_ready=0
  for _ in {1..30}; do
    if docker exec "$runner_container" python -c 'import socket; s=socket.create_connection(("mem0_store", 6333), timeout=1); s.close()' >/dev/null 2>&1; then
      qdrant_ready=1
      break
    fi
    sleep 1
  done
  if [[ "$qdrant_ready" -ne 1 ]]; then
    add_openmemory_direct_blocked_row "blocked_environment" "Qdrant did not become reachable from the OpenMemory runner container" "$probe_path"
    cleanup_openmemory_direct
    trap - EXIT
    return 0
  fi

  write_openmemory_direct_scripts "$OUTPUT_DIR"

  local seed_exit=0
  docker exec \
    -e OPENMEMORY_CONTAINER_LMSTUDIO_BASE_URL="$openmemory_container_base_url" \
    -e OPENMEMORY_LLM_MODEL="$openmemory_llm_model" \
    -e OPENMEMORY_EMBED_MODEL="$openmemory_embed_model" \
    -e OPENMEMORY_EMBED_DIMS="$openmemory_embed_dims" \
    "$runner_container" python /bench/openmemory_seed.py >"${OUTPUT_DIR}/openmemory_seed.json" 2>"${OUTPUT_DIR}/openmemory_seed.stderr" || seed_exit=$?

  if [[ "$seed_exit" -ne 0 ]]; then
    add_openmemory_direct_blocked_row "blocked_environment" "OpenMemory direct seed failed; inspect openmemory_seed.stderr" "${OUTPUT_DIR}/openmemory_seed.stderr"
    cleanup_openmemory_direct
    trap - EXIT
    return 0
  fi

  while IFS='|' read -r task_id workspace_locator task_hint prompt decision_kw open_kw preference_kw direction_kw; do
    local retrieval_path="${openmemory_retrieval_dir}/${task_id}.json"
    local retrieval_stderr="${openmemory_retrieval_dir}/${task_id}.stderr"
    local raw_response_path="${openmemory_output_dir}/${task_id}.raw.json"
    local response_path="${openmemory_output_dir}/${task_id}.txt"
    local search_exit=0
    docker exec \
      -e OPENMEMORY_CONTAINER_LMSTUDIO_BASE_URL="$openmemory_container_base_url" \
      -e OPENMEMORY_LLM_MODEL="$openmemory_llm_model" \
      -e OPENMEMORY_EMBED_MODEL="$openmemory_embed_model" \
      -e OPENMEMORY_EMBED_DIMS="$openmemory_embed_dims" \
      -e OPENMEMORY_QUERY="${workspace_locator} ${task_hint} ${prompt}" \
      -e OPENMEMORY_LIMIT=3 \
      "$runner_container" python /bench/openmemory_search.py >"$retrieval_path" 2>"$retrieval_stderr" || search_exit=$?

    local memory_context=""
    if [[ "$search_exit" -eq 0 ]]; then
      memory_context="$(jq -r '
        def memory_text:
          .memory // .text // .content // .payload // .metadata.text // empty;
        if type == "array" then
          .[]? | memory_text
        elif (.results? | type) == "array" then
          .results[]? | memory_text
        elif (.memories? | type) == "array" then
          .memories[]? | memory_text
        else
          empty
        end
      ' "$retrieval_path" | head -n 6)"
    fi
    if [[ -z "$memory_context" ]]; then
      memory_context="No relevant OpenMemory entries retrieved."
    fi

    local lm_prompt
    lm_prompt="Workspace: ${workspace_locator}
Task scope: ${task_hint}
Current task: ${prompt}

Retrieved OpenMemory entries:
${memory_context}

Return compact coding-agent guidance. Include decisions, open loops, preferences, risks, and likely next directions. Use only retrieved memory. Do not invent missing facts."

    local payload
    payload="$(jq -n \
      --arg model "$openmemory_llm_model" \
      --arg system_prompt "/no_think You are a memory-layer comparator. Use only the supplied retrieved memories. If the memory is insufficient, say what is missing briefly. Return the final answer in the message, not only reasoning." \
      --arg input "$lm_prompt" \
      '{
        model: $model,
        system_prompt: $system_prompt,
        input: $input
      }')"

    local chat_exit=0
    curl -sS \
      --max-time "$openmemory_task_timeout" \
      -H "Content-Type: application/json" \
      -d "$payload" \
      "$openmemory_chat_url" >"$raw_response_path" 2>"${raw_response_path}.stderr" || chat_exit=$?

    if [[ "$chat_exit" -eq 0 ]]; then
      jq -r '
        def nonempty_string:
          select(type == "string" and length > 0);
        def output_messages($items):
          ([$items[]? | select(.type == "message") | .content | nonempty_string] | join("\n")) as $message
          | if ($message | length) > 0 then
              $message
            else
              ([$items[]? | select(.type == "reasoning") | .content | nonempty_string] | join("\n"))
            end;
        if type == "array" then
          output_messages(.)
        elif (.output? | type) == "array" then
          output_messages(.output)
        else
          (.output | nonempty_string)
          // (.content | nonempty_string)
          // (.response | nonempty_string)
          // (.choices[0].message.content | nonempty_string)
          // (.choices[0].message.reasoning_content | nonempty_string)
          // .error.message
          // (if .error == null then empty else (.error | tostring) end)
          // tostring
        end
      ' "$raw_response_path" >"$response_path"
    else
      echo "no memory comparator response; curl failed" >"$response_path"
    fi

    local response_status="run"
    if [[ "$chat_exit" -ne 0 ]] || jq -e '.error? != null' "$raw_response_path" >/dev/null 2>&1 || grep -Eiq 'failed to load model|model is unloaded|invalid_request|error code:|no memory comparator response' "$response_path"; then
      response_status="blocked_generation"
    fi

    local retrieval_hit="false"
    if [[ "$search_exit" -eq 0 ]] && jq -e '
      if type == "array" then length > 0
      elif (.results? | type) == "array" then (.results | length) > 0
      elif (.memories? | type) == "array" then (.memories | length) > 0
      else false end
    ' "$retrieval_path" >/dev/null 2>&1; then
      retrieval_hit="true"
    fi

    local metrics
    metrics="$(score_plain_text "$response_path" "$decision_kw" "$open_kw" "$preference_kw" "$direction_kw")"
    jq -cn \
      --arg system "openmemory" \
      --arg task_id "$task_id" \
      --arg response_path "$response_path" \
      --arg retrieval_path "$retrieval_path" \
      --arg response_status "$response_status" \
      --argjson search_exit "$search_exit" \
      --argjson chat_exit "$chat_exit" \
      --argjson retrieval_hit "$retrieval_hit" \
      --argjson metrics "$metrics" \
      '{
        system: $system,
        task_id: $task_id,
        response_path: $response_path,
        retrieval_path: $retrieval_path,
        response_status: $response_status,
        retrieval_hit: $retrieval_hit,
        search_exit: $search_exit,
        chat_exit: $chat_exit,
        metrics: $metrics
      }' >>"$openmemory_rows_path"
  done <"$TASKS_FILE"

  jq -s \
    --arg rows_path "$openmemory_rows_path" \
    --arg output_dir "$openmemory_output_dir" \
    --arg retrieval_dir "$openmemory_retrieval_dir" \
    --arg host_base_url "$openmemory_host_base_url" \
    --arg chat_url "$openmemory_chat_url" \
    --arg container_base_url "$openmemory_container_base_url" \
    --arg llm_model "$openmemory_llm_model" \
    --arg embed_model "$openmemory_embed_model" \
    '[.[]] as $rows
     | {
        system: "openmemory",
        comparator_class: "memory_layer",
        status: (
          if ($rows | map(select(.response_status == "blocked_generation")) | length) == ($rows | length) then "blocked_generation"
          elif ($rows | map(select(.search_exit != 0 or .chat_exit != 0 or .response_status != "run")) | length) == 0 then "run"
          else "partial" end
        ),
        tasks: ($rows | length),
        blocked_generation_count: ($rows | map(select(.response_status == "blocked_generation")) | length),
        retrieval_success_rate: (($rows | map(if .retrieval_hit then 1 else 0 end) | add) / ($rows | length)),
        mean_score: (($rows | map(.metrics.score) | add) / ($rows | length)),
        decision_recall: (($rows | map(if .metrics.decision_hit then 1 else 0 end) | add) / ($rows | length)),
        open_loop_recall: (($rows | map(if .metrics.open_loop_hit then 1 else 0 end) | add) / ($rows | length)),
        preference_recall: (($rows | map(if .metrics.preference_hit then 1 else 0 end) | add) / ($rows | length)),
        next_direction_acceptance_proxy: (($rows | map(if .metrics.direction_hit then 1 else 0 end) | add) / ($rows | length)),
        false_memory_rate_proxy: (($rows | map(if .metrics.false_memory_clear then 0 else 1 end) | add) / ($rows | length)),
        unsupported_count: ($rows | map(.metrics.unsupported_count) | add),
        dimensions: {
          memory_recall_quality: {
            decision_recall: (($rows | map(if .metrics.decision_hit then 1 else 0 end) | add) / ($rows | length)),
            open_loop_recall: (($rows | map(if .metrics.open_loop_hit then 1 else 0 end) | add) / ($rows | length)),
            preference_recall: (($rows | map(if .metrics.preference_hit then 1 else 0 end) | add) / ($rows | length))
          },
          next_direction_quality: {
            acceptance_proxy: (($rows | map(if .metrics.direction_hit then 1 else 0 end) | add) / ($rows | length)),
            unsupported_count: ($rows | map(.metrics.unsupported_count) | add)
          },
          setup_friction: {
            score: 0.25,
            note: "Direct mem0/OpenMemory comparator required Docker, Qdrant, local OpenMemory image, LM Studio chat model, and LM Studio embedding model"
          },
          cross_host_portability: {
            score: 0.7,
            note: "Memory layer can be host-neutral, but does not define Aadesh-style connector events or outcome semantics"
          },
          outcome_trace_learning: {
            score: 0,
            note: "No first-class accepted/ignored/modified intervention-outcome trace contract was exercised"
          }
        },
        artifacts: {
          rows: $rows_path,
          output_dir: $output_dir,
          retrieval_dir: $retrieval_dir
        },
        notes: ("Direct mem0/OpenMemory no-infer seed/search comparator used local LM Studio host URL " + $host_base_url + ", chat URL " + $chat_url + ", container URL " + $container_base_url + ", chat model " + $llm_model + ", embedding model " + $embed_model + ". It reports retrieval success separately from generated-guidance quality and does not exercise Aadesh supervisory traces.")
      }' "$openmemory_rows_path" >>"${OUTPUT_DIR}/external_rows.jsonl"

  cleanup_openmemory_direct
  trap - EXIT
}

run_memory_layer_probe() {
  local docker_probe_path="${OUTPUT_DIR}/docker_probe.txt"
  local knowns_probe_path="${OUTPUT_DIR}/knowns_probe.txt"
  local openmemory_probe_path="${OUTPUT_DIR}/openmemory_probe.txt"
  local memd_probe_path="${OUTPUT_DIR}/memd_probe.txt"

  local docker_exit=0
  if command -v docker >/dev/null 2>&1; then
    docker info >"$docker_probe_path" 2>&1 || docker_exit=$?
  else
    docker_exit=127
    echo "docker command not found" >"$docker_probe_path"
  fi

  probe_memory_command() {
    local system_name="$1"
    local command_name="$2"
    local probe_path="$3"
    local install_note="$4"

    if command -v "$command_name" >/dev/null 2>&1; then
      "$command_name" --version >"$probe_path" 2>&1 || "$command_name" --help >"$probe_path" 2>&1 || true
      jq -n \
        --arg system "$system_name" \
        --arg command_name "$command_name" \
        --arg probe_path "$probe_path" \
        '{
          system: $system,
          comparator_class: "memory_layer",
          status: "installed_unscored",
          aggregate: null,
          dimensions: {
            memory_recall_quality: {score: null, note: "installed, but not run against comparison_tasks.tsv"},
            next_direction_quality: {score: null, note: "not run against task prompts"},
            setup_friction: {score: 0.7, note: ("local command available: " + $command_name)},
            cross_host_portability: {score: null, note: "requires actual adapter run to score"},
            outcome_trace_learning: {score: null, note: "requires actual adapter run to verify"}
          },
          artifacts: {probe: $probe_path},
          notes: "Availability probe only. Import a scored external result before making task-quality claims."
        }' >>"${OUTPUT_DIR}/external_rows.jsonl"
    else
      echo "$command_name command not found" >"$probe_path"
      jq -n \
        --arg system "$system_name" \
        --arg command_name "$command_name" \
        --arg probe_path "$probe_path" \
        --arg install_note "$install_note" \
        '{
          system: $system,
          comparator_class: "memory_layer",
          status: "not_installed",
          reason: ($command_name + " command not found"),
          aggregate: null,
          dimensions: {
            memory_recall_quality: null,
            next_direction_quality: null,
            setup_friction: {score: 0, note: $install_note},
            cross_host_portability: null,
            outcome_trace_learning: null
          },
          artifacts: {probe: $probe_path}
        }' >>"${OUTPUT_DIR}/external_rows.jsonl"
    fi
  }

  probe_memory_command knowns knowns "$knowns_probe_path" "Knowns is not installed; do not use unverified npx execution inside this repo."
  probe_memory_command memd memd "$memd_probe_path" "memd is not installed."

  if [[ "$RUN_OPENMEMORY_DIRECT_BENCHMARK" -eq 1 ]]; then
    echo "openmemory direct benchmark requested; skipping command-only OpenMemory probe" >"$openmemory_probe_path"
  elif command -v openmemory >/dev/null 2>&1; then
    probe_memory_command openmemory openmemory "$openmemory_probe_path" "openmemory command probe"
  else
    echo "openmemory command not found" >"$openmemory_probe_path"
    jq -n \
      --arg docker_probe_path "$docker_probe_path" \
      --arg openmemory_probe_path "$openmemory_probe_path" \
      --argjson docker_exit "$docker_exit" \
      '{
        system: "openmemory",
        comparator_class: "memory_layer",
        status: (if $docker_exit == 0 then "not_installed" else "blocked_environment" end),
        reason: (if $docker_exit == 0 then "openmemory command not found" else "openmemory command not found and Docker is unavailable/permission-denied" end),
        aggregate: null,
        dimensions: {
          memory_recall_quality: null,
          next_direction_quality: null,
          setup_friction: {
            score: (if $docker_exit == 0 then 0.2 else 0 end),
            note: "OpenMemory commonly requires Docker plus model/API configuration; no task-quality run was performed."
          },
          cross_host_portability: null,
          outcome_trace_learning: null
        },
        artifacts: {
          docker_probe: $docker_probe_path,
          openmemory_probe: $openmemory_probe_path
        }
      }' >>"${OUTPUT_DIR}/external_rows.jsonl"
  fi
}

load_external_result() {
  local spec="$1"
  local name="${spec%%=*}"
  local path="${spec#*=}"
  if [[ -z "$name" || "$name" == "$spec" || ! -f "$path" ]]; then
    echo "invalid --external-result; expected NAME=PATH and existing file: $spec" >&2
    exit 1
  fi

  jq -n \
    --arg system "$name" \
    --arg source_path "$path" \
    --slurpfile result "$path" \
    '{system: $system, status: "imported", source_path: $source_path, result: $result[0]}' \
    >>"${OUTPUT_DIR}/external_rows.jsonl"
}

aggregate_internal() {
  local system_name="$1"
  jq -s \
    --arg system "$system_name" \
    '[.[] | select(.system == $system)] as $rows
     | {
        system: $system,
        comparator_class: (if $system == "aadesh" then "supervisory_substrate" else "empty_baseline" end),
        status: "run",
        tasks: ($rows | length),
        mean_score: (($rows | map(.metrics.score) | add) / ($rows | length)),
        decision_recall: (($rows | map(if .metrics.decision_hit then 1 else 0 end) | add) / ($rows | length)),
        open_loop_recall: (($rows | map(if .metrics.open_loop_hit then 1 else 0 end) | add) / ($rows | length)),
        preference_recall: (($rows | map(if .metrics.preference_hit then 1 else 0 end) | add) / ($rows | length)),
        next_direction_acceptance_proxy: (($rows | map(if .metrics.direction_hit then 1 else 0 end) | add) / ($rows | length)),
        false_memory_rate_proxy: (($rows | map(if .metrics.false_memory_clear then 0 else 1 end) | add) / ($rows | length)),
        unsupported_count: ($rows | map(.metrics.unsupported_count) | add),
        dimensions: {
          memory_recall_quality: {
            decision_recall: (($rows | map(if .metrics.decision_hit then 1 else 0 end) | add) / ($rows | length)),
            open_loop_recall: (($rows | map(if .metrics.open_loop_hit then 1 else 0 end) | add) / ($rows | length)),
            preference_recall: (($rows | map(if .metrics.preference_hit then 1 else 0 end) | add) / ($rows | length))
          },
          next_direction_quality: {
            acceptance_proxy: (($rows | map(if .metrics.direction_hit then 1 else 0 end) | add) / ($rows | length)),
            unsupported_count: ($rows | map(.metrics.unsupported_count) | add)
          },
          setup_friction: {
            score: (if $system == "aadesh" then 0.8 else 1 end),
            note: (if $system == "aadesh" then "local cargo/SQLite setup; no cloud service required" else "empty local baseline" end)
          },
          cross_host_portability: {
            score: (if $system == "aadesh" then 1 else 0 end),
            note: (if $system == "aadesh" then "host-neutral workspace/task payload and connector event model" else "no cross-host substrate" end)
          },
          outcome_trace_learning: {
            score: (if $system == "aadesh" then 1 else 0 end),
            note: (if $system == "aadesh" then "supports linked accepted/ignored/modified traces and outcome-aware ranking" else "no outcome trace learning" end)
          }
        }
      }' "$ROWS_FILE"
}

seed_aadesh_memory
write_tasks
run_system_rows baseline "$BASELINE_URL"
run_system_rows aadesh "$ADESH_URL"

: >"${OUTPUT_DIR}/external_rows.jsonl"
if [[ "$INCLUDE_EXTERNAL_STUBS" -eq 1 ]]; then
  if [[ "$RUN_MEMORY_LAYER_PROBE" -ne 1 ]]; then
    add_external_stub memd memory_layer
    add_external_stub knowns memory_layer
    if [[ "$RUN_OPENMEMORY_DIRECT_BENCHMARK" -ne 1 ]]; then
      add_external_stub openmemory memory_layer
    fi
  fi
  if [[ "$RUN_HERMES_PROBE" -ne 1 ]]; then
    add_external_stub hermes host_runtime
  fi
fi
if [[ "$RUN_HERMES_PROBE" -eq 1 ]]; then
  run_hermes_probe
fi
if [[ "$RUN_HERMES_BENCHMARK" -eq 1 ]]; then
  run_hermes_benchmark
fi
if [[ "$RUN_MEMORY_LAYER_PROBE" -eq 1 ]]; then
  run_memory_layer_probe
fi
if [[ "$RUN_OPENMEMORY_DIRECT_BENCHMARK" -eq 1 ]]; then
  run_openmemory_direct_benchmark
fi
for external in "${EXTERNAL_RESULTS[@]}"; do
  load_external_result "$external"
done

live_cli_trace_report=""
live_cli_trace_stdout=""
live_cli_trace_summary="null"
if [[ "$RUN_LIVE_CLI_TRACE" -eq 1 ]]; then
  echo >&2
  echo "Running live CLI trace validation..." >&2
  live_cli_trace_dir="${OUTPUT_DIR}/live_cli_trace"
  live_cli_trace_stdout="${OUTPUT_DIR}/live_cli_trace.stdout.json"
  "${ADESH_ROOT}/scripts/live_cli_trace_validation.sh" \
    --output-dir "$live_cli_trace_dir" \
    --timeout-seconds "$LIVE_CLI_TIMEOUT_SECONDS" \
    >"$live_cli_trace_stdout"
  live_cli_trace_report="${live_cli_trace_dir}/live_cli_trace_report.json"
  live_cli_trace_summary="$(cat "$live_cli_trace_report")"
fi

baseline_aggregate="$(aggregate_internal baseline)"
aadesh_aggregate="$(aggregate_internal aadesh)"
external_rows="$(jq -s '.' "${OUTPUT_DIR}/external_rows.jsonl")"

jq -n \
  --arg run_id "$RUN_ID" \
  --arg output_dir "$OUTPUT_DIR" \
  --arg tasks_file "$TASKS_FILE" \
  --arg rows_file "$ROWS_FILE" \
  --arg baseline_db "$BASELINE_DB" \
  --arg aadesh_db "$ADESH_DB" \
  --arg live_cli_trace_report "$live_cli_trace_report" \
  --arg live_cli_trace_stdout "$live_cli_trace_stdout" \
  --argjson baseline "$baseline_aggregate" \
  --argjson aadesh "$aadesh_aggregate" \
  --argjson external "$external_rows" \
  --argjson live_cli_trace "$live_cli_trace_summary" \
  '{
    run_id: $run_id,
    output_dir: $output_dir,
    scenario: "external-memory-comparison-v0",
    compared_systems: ([$baseline, $aadesh] + $external),
    artifacts: {
      tasks_file: $tasks_file,
      rows_file: $rows_file,
      baseline_db: $baseline_db,
      aadesh_db: $aadesh_db,
      live_cli_trace_report: (if ($live_cli_trace_report | length) > 0 then $live_cli_trace_report else null end),
      live_cli_trace_stdout: (if ($live_cli_trace_stdout | length) > 0 then $live_cli_trace_stdout else null end)
    },
    optional_validations: {
      live_cli_trace: $live_cli_trace
    },
    interpretation: {
      wedge_claim: "Aadesh must beat baseline and show value beyond memory-only recall via outcome-aware advisory guidance.",
      protocol_dimensions: [
        "memory_recall_quality",
        "next_direction_quality",
        "setup_friction",
        "cross_host_portability",
        "outcome_trace_learning"
      ],
      external_status: "External systems require explicit adapter/export runs before direct task-quality claims are valid. Runtime probes are allowed but not scored as task-quality evidence.",
      next_action: "Run at least one configured host/runtime comparator and one memory-layer comparator against comparison_tasks.tsv, then import their JSON with --external-result."
    }
  }' >"$REPORT_PATH"

cat "$REPORT_PATH"
echo >&2
echo "Comparison report: $REPORT_PATH" >&2

if [[ "$RUN_HARD_SUPERVISORY_BENCHMARK" -eq 1 ]]; then
  echo >&2
  echo "Running hard supervisory benchmark..." >&2
  "${ADESH_ROOT}/scripts/hard_supervisory_comparison_benchmark.sh" \
    --comparison-report "$REPORT_PATH" \
    --output-dir "${OUTPUT_DIR}/hard_supervisory" \
    --days "$HARD_BENCHMARK_DAYS" \
    --sessions "$HARD_BENCHMARK_SESSIONS" \
    --stress-events "$HARD_STRESS_EVENTS" \
    --data-profile "$HARD_DATA_PROFILE" \
    --judge-mode "$HARD_JUDGE_MODE"
fi
