#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: ./scripts/adversarial_long_memory_stress_benchmark.sh [options]

Runs a longitudinal adversarial-memory pressure benchmark through the public
connector path. The benchmark seeds one important target workstream, injects
many noisy/confusable episodes across hosts and workspaces, probes at increasing
memory-load milestones, and reports whether current-task guidance degrades.

Options:
  --noise-events N  Number of adversarial noise episodes. Default: 36.
  --output-dir DIR  Directory for report/db/probes. Default: /tmp/adesh-adversarial-stress-<run_id>.
  --db-url URL      SQLite URL. Default: sqlite://<output-dir>/stress.db?mode=rwc.
  -h, --help        Show this help.

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
NOISE_EVENTS=36
OUTPUT_DIR=""
DB_URL=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --noise-events)
      NOISE_EVENTS="${2:-}"
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

if ! [[ "$NOISE_EVENTS" =~ ^[0-9]+$ ]] || [[ "$NOISE_EVENTS" -lt 12 ]]; then
  echo "--noise-events must be an integer >= 12" >&2
  exit 1
fi

OUTPUT_DIR="${OUTPUT_DIR:-/tmp/adesh-adversarial-stress-${RUN_ID}}"
mkdir -p "$OUTPUT_DIR"

DB_URL="${DB_URL:-sqlite://${OUTPUT_DIR}/stress.db?mode=rwc}"
DB_PATH="${DB_URL#sqlite://}"
DB_PATH="${DB_PATH%%\?*}"
CARGO_TARGET_DIR="${ADESH_CARGO_TARGET_DIR:-/tmp/adesh-cargo-target}"
EVENTS_JSONL="${OUTPUT_DIR}/adversarial_events.jsonl"
PROBES_JSONL="${OUTPUT_DIR}/adversarial_probes.jsonl"
REPORT_PATH="${OUTPUT_DIR}/adversarial_long_memory_stress_report.json"
: > "$EVENTS_JSONL"
: > "$PROBES_JSONL"

TARGET_WORKSPACE="workspace://stress-payments-release"
TARGET_TASK_HINT="ci-release-flake"
TARGET_FILES='["src/retry/service.rs","tests/retry_release_ci.rs",".github/workflows/release.yml"]'

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
  local event_index="$1"
  date -u -d "2026-05-01 09:00:00 UTC +${event_index} hours" +"%Y-%m-%dT%H:%M:%SZ"
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

host_for_index() {
  case $(( $1 % 4 )) in
    0) printf 'codex-vscode|codex-extension|gpt-5.4' ;;
    1) printf 'qwen-code|cli|qwen/qwen3.6-35b-a3b' ;;
    2) printf 'opencode|cli|opencode-default' ;;
    *) printf 'gemini-cli|cli|gemini-cli-default' ;;
  esac
}

store_episode() {
  local idx="$1"
  local name="$2"
  local workspace_locator="$3"
  local task_hint="$4"
  local prompt="$5"
  local files_json="$6"
  local outcome="$7"
  local summary="$8"
  local decision="$9"
  local rationale="${10}"
  local unresolved="${11}"
  local risk="${12}"
  local test_name="${13}"
  local test_status="${14}"
  local test_summary="${15}"

  local host_parts connector_id host_kind host_model workspace payload response
  host_parts="$(host_for_index "$idx")"
  IFS='|' read -r connector_id host_kind host_model <<< "$host_parts"
  workspace="$(workspace_json "$workspace_locator")"

  payload="$(jq -n \
    --arg connector_id "$connector_id" \
    --arg host_kind "$host_kind" \
    --arg host_model "$host_model" \
    --arg session_id "adversarial-${RUN_ID}-${name}-${idx}" \
    --argjson workspace "$workspace" \
    --arg task_prompt "$prompt" \
    --arg task_hint "$task_hint" \
    --argjson files "$files_json" \
    --arg started_at "$(timestamp_for "$idx")" \
    --arg ended_at "$(timestamp_for "$((idx + 1))")" \
    --arg outcome "$outcome" \
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
      session_id: $session_id,
      host_agent_id: $connector_id,
      host_agent_kind: $host_kind,
      host_model: $host_model,
      event_kind: "task_end",
      workspace: $workspace,
      task_prompt: $task_prompt,
      task_hint: (if $task_hint == "" then null else $task_hint end),
      files_in_focus: $files,
      files_touched: $files,
      started_at: $started_at,
      ended_at: $ended_at,
      outcome: $outcome,
      summary: $summary,
      tests: [{name: $test_name, status: $test_status, summary: $test_summary}],
      decisions: [{decision: $decision, rationale: $rationale}],
      unresolved_items: [$unresolved],
      observed_preferences: [],
      risk_signals: [$risk]
    }')"

  response="$(connector_event "$payload")"
  jq -n \
    --arg idx "$idx" \
    --arg name "$name" \
    --arg workspace_locator "$workspace_locator" \
    --arg task_hint "$task_hint" \
    --arg connector_id "$connector_id" \
    --arg outcome "$outcome" \
    --argjson response "$response" \
    '{
      event_index: ($idx | tonumber),
      name: $name,
      workspace_locator: $workspace_locator,
      task_hint: (if $task_hint == "" then null else $task_hint end),
      connector_id: $connector_id,
      outcome: $outcome,
      episode_id: ($response.stored_episode.episode_id // null)
    }' >> "$EVENTS_JSONL"
}

run_probe() {
  local milestone="$1"
  local label="$2"
  local workspace started_at payload response
  workspace="$(workspace_json "$TARGET_WORKSPACE")"
  started_at="$(timestamp_for "$((1000 + milestone))")"
  payload="$(jq -n \
    --arg session_id "adversarial-probe-${RUN_ID}-${label}" \
    --argjson workspace "$workspace" \
    --argjson files "$TARGET_FILES" \
    --arg started_at "$started_at" \
    '{
      connector_id: "adversarial-probe",
      connector_kind: "offline_probe",
      connector_version: "0.1.0",
      session_id: $session_id,
      host_agent_id: "adversarial-probe",
      host_agent_kind: "offline_probe",
      host_model: "deterministic",
      event_kind: "task_start",
      workspace: $workspace,
      task_prompt: "Release-only CI failed again. Should I clean up retry code or preserve evidence first?",
      task_hint: "ci-release-flake",
      files_in_focus: $files,
      started_at: $started_at
    }')"
  response="$(connector_event "$payload")"
  jq -c \
    --arg milestone "$milestone" \
    --arg label "$label" \
    '{
      milestone: ($milestone | tonumber),
      label: $label,
      task_focus: .prepare_context.task_focus,
      decisions: .prepare_context.relevant_decisions,
      open_loops: .prepare_context.open_loops,
      risks: .prepare_context.risk_flags,
      next_directions: .prepare_context.likely_next_directions,
      uncertainties: .prepare_context.uncertainties
    }' <<< "$response" >> "$PROBES_JSONL"
}

store_target_seed() {
  store_episode 0 "target-release-flake-seed" "$TARGET_WORKSPACE" "$TARGET_TASK_HINT" \
    "Release-only retry flake started after a retry cleanup branch." \
    "$TARGET_FILES" \
    "accepted" \
    "The important workstream is to preserve release-only failure evidence before cleanup." \
    "Reproduce release-only retry flake with seed and log capture before cleanup" \
    "Release-only behavior must be understood before cleanup changes erase evidence" \
    "Capture failing seed, release build flags, and CI logs for the retry flake" \
    "Cleanup can erase flaky failure evidence before root cause is known" \
    "release_retry_flake_seed" "fail" "Release-only flake reproduced once without full logs"
}

store_noise_event() {
  local i="$1"
  case $(( i % 6 )) in
    0)
      store_episode "$i" "noise-maintenance-lint-${i}" "workspace://stress-maintenance" "lint-cleanup" \
        "Large lint cleanup touched unrelated scripts." \
        '["scripts/format.sh","docs/style.md"]' \
        "ignored" \
        "Unrelated lint cleanup should stay isolated from retry release guidance." \
        "Do not let unrelated lint cleanup influence payment retry release-flake priority" \
        "Scope isolation should defeat attractive but irrelevant maintenance memories" \
        "Keep unrelated maintenance work out of payment retry decisions" \
        "Cross-workspace leakage can make wrong memory sound useful" \
        "lint_cleanup_noise_${i}" "pass" "Maintenance-only trace"
      ;;
    1)
      store_episode "$i" "noise-payment-cleanup-${i}" "$TARGET_WORKSPACE" "cleanup-debt" \
        "A cleanup-only payment task removed dead retry logs after safety evidence was archived." \
        '["src/retry/service.rs","docs/cleanup.md"]' \
        "modified" \
        "Cleanup is allowed only after release-only flake evidence is preserved." \
        "Do retry cleanup only after release-only seed and logs are captured" \
        "This is related but secondary to the active release-only flake task" \
        "Archive cleanup notes separately from flake reproduction evidence" \
        "Cleanup-first advice can be harmful if evidence has not been captured" \
        "payment_cleanup_noise_${i}" "pass" "Cleanup trace intentionally secondary"
      ;;
    2)
      store_episode "$i" "noise-connector-context-${i}" "workspace://stress-connectors" "connector-integration" \
        "Connector docs discussed context_id linkage and accepted outcomes." \
        '["docs/CONNECTOR_INTEGRATION_V0.md","crates/adesh-contracts/src/lib.rs"]' \
        "accepted" \
        "Connector trace linkage matters but is unrelated to payment release-flake handling." \
        "Document context_id round trip for connector outcomes" \
        "Host linkage evidence is not payment retry evidence" \
        "Add accepted and ignored outcome examples for connector docs" \
        "Connector docs can distract from payment incident work" \
        "connector_noise_${i}" "pass" "Connector trace was linked"
      ;;
    3)
      store_episode "$i" "noise-eval-policy-${i}" "workspace://stress-eval" "wedge-evaluation" \
        "Evaluation metrics looked strong, but policy-state stayed gated." \
        '["docs/IMPLEMENTATION_PLAN.md","docs/COMPARISON_BENCHMARK.md"]' \
        "accepted" \
        "Evaluation proof should not trigger policy-state without repeated operational pressure." \
        "Keep policy-state gated until traces show repeated lineage pressure" \
        "Benchmark pressure is not the same as payment release evidence" \
        "Collect more benchmark cases before changing policy-state gates" \
        "Synthetic wins can create false confidence" \
        "eval_noise_${i}" "pass" "Evaluation gate unchanged"
      ;;
    4)
      store_episode "$i" "noise-payment-obsolete-${i}" "$TARGET_WORKSPACE" "retry-hardening" \
        "Older retry note suggested quick cleanup, then was superseded by release-only failure evidence." \
        '["src/retry/service.rs","tests/retry_timeout.rs"]' \
        "ignored" \
        "Obsolete cleanup-first retry advice should not beat current release-only flake evidence." \
        "Supersede cleanup-first retry advice with release-only flake reproduction" \
        "The active prompt references release-only CI, not generic retry hardening" \
        "Keep release-only seed and logs as the active retry follow-up" \
        "Obsolete retry cleanup advice can mask current flake reproduction" \
        "obsolete_retry_noise_${i}" "fail" "Obsolete note intentionally noisy"
      ;;
    *)
      store_episode "$i" "noise-personal-workflow-${i}" "personal-agent-workflows" "personal-continuity" \
        "Personal workflow memory mentioned preferring concise implementation reports." \
        '[]' \
        "accepted" \
        "Personal agent preferences should not leak into payment incident guidance." \
        "Keep non-repo preferences separate from coding incident decisions" \
        "Generic user preference is lower priority than task-local failure evidence" \
        "Preserve personal continuity in non-repo scope" \
        "Personal preference leakage can dilute incident guidance" \
        "personal_noise_${i}" "pass" "Non-repo continuity trace"
      ;;
  esac
}

echo "Running adversarial long-memory stress benchmark..."
echo "DB_URL=${DB_URL}"
echo "noise_events=${NOISE_EVENTS}"

store_target_seed
run_probe 0 "after-target-seed"

MILESTONE_ONE=$(( NOISE_EVENTS / 3 ))
MILESTONE_TWO=$(( (NOISE_EVENTS * 2) / 3 ))

for ((i = 1; i <= NOISE_EVENTS; i++)); do
  store_noise_event "$i"
  if (( i == MILESTONE_ONE )); then
    run_probe "$i" "after-one-third-noise"
  elif (( i == MILESTONE_TWO )); then
    run_probe "$i" "after-two-thirds-noise"
  elif (( i == NOISE_EVENTS )); then
    run_probe "$i" "after-all-noise"
  fi
done

events_json="$(jq -s '.' "$EVENTS_JSONL")"
probes_json="$(jq -s '.' "$PROBES_JSONL")"

db_counts_json="$(sqlite3 -json "$DB_PATH" "
SELECT
  (SELECT COUNT(*) FROM episodes) AS stored_episodes,
  (SELECT COUNT(DISTINCT scope_key) FROM episodes) AS distinct_workspaces,
  (SELECT COUNT(*) FROM claims) AS claims,
  (SELECT COUNT(*) FROM episode_artifacts) AS artifacts;
")"

stress_analysis="$(jq -n \
  --argjson probes "$probes_json" \
  --argjson noise_events "$NOISE_EVENTS" \
  '
  def text($p):
    [
      $p.task_focus,
      (($p.decisions // []) | map(.statement // "") | join(" ")),
      (($p.open_loops // []) | map(.statement // "") | join(" ")),
      (($p.risks // []) | map(.statement // "") | join(" ")),
      (($p.next_directions // []) | map(.statement // "") | join(" "))
    ] | map(. // "" | ascii_downcase) | join(" ");
  def has($s): text(.) | contains($s);
  def hit_ratio($p):
    ["release-only", "seed", "log", "cleanup", "evidence"] as $terms
    | ($terms | map(select((text($p) | contains(.)))) | length) / ($terms | length);
  def scope_leak($p):
    (text($p) | (contains("context_id") or contains("policy-state") or contains("personal workflow") or contains("lint cleanup")));
  def row($p): {
    milestone: $p.milestone,
    label: $p.label,
    expected_evidence_hit_ratio: hit_ratio($p),
    scope_leakage_detected: scope_leak($p),
    top_decision: ($p.decisions[0].statement // null),
    top_open_loop: ($p.open_loops[0].statement // null),
    top_next_direction: ($p.next_directions[0].statement // null),
    uncertainty_count: (($p.uncertainties // []) | length),
    verdict: (
      if (hit_ratio($p) >= 0.6 and (scope_leak($p) | not) and (($p.next_directions[0].statement // "") | length > 0))
      then "useful"
      else "weak"
      end
    )
  };
  ($probes | map(row(.))) as $curve
  | ($curve[-1] // {}) as $final
  | {
      degradation_curve: $curve,
      stress_assertions: {
        adversarial_noise_volume_met: ($noise_events >= 12),
        all_probe_guidance_stays_useful: (($curve | map(.verdict == "useful") | all) == true),
        final_guidance_still_useful: (($final.verdict // "weak") == "useful"),
        no_scope_leakage_across_curve: (($curve | map(.scope_leakage_detected == false) | all) == true),
        final_expected_evidence_ratio_at_least_0_6: (($final.expected_evidence_hit_ratio // 0) >= 0.6)
      }
    }
  ')"

report_json="$(jq -n \
  --arg report_path "$REPORT_PATH" \
  --arg db_path "$DB_PATH" \
  --arg events_path "$EVENTS_JSONL" \
  --arg probes_path "$PROBES_JSONL" \
  --argjson noise_events "$NOISE_EVENTS" \
  --argjson events "$events_json" \
  --argjson probes "$probes_json" \
  --argjson counts "$db_counts_json" \
  --argjson stress "$stress_analysis" \
  '
  def bool_score($v): if $v == true then 1 else 0 end;
  def avg($xs): if ($xs | length) == 0 then 0 else (($xs | add) / ($xs | length)) end;
  ($counts[0] // {}) as $c
  | ($stress.stress_assertions // {}) as $assertions
  | {
      metadata: {
        scenario: "adversarial_long_memory_stress",
        report_path: $report_path,
        db_path: $db_path,
        events_path: $events_path,
        probes_path: $probes_path
      },
      stress_config: {
        noise_events: $noise_events,
        target_workspace: "workspace://stress-payments-release",
        target_task_hint: "ci-release-flake",
        expected_evidence: ["release-only", "seed", "log", "cleanup", "evidence"]
      },
      storage_totals: $c,
      event_summary: {
        total_events: ($events | length),
        distinct_workspaces: (($events | map(.workspace_locator) | unique) | length)
      },
      degradation_curve: $stress.degradation_curve,
      stress_assertions: $assertions,
      stress_score: avg($assertions | to_entries | map(bool_score(.value))),
      adversarial_stress_pass: (($assertions | to_entries | map(.value == true) | all) == true)
    }
  ')"

printf '%s\n' "$report_json" > "$REPORT_PATH"

echo
echo "Adversarial long-memory stress report:"
echo "  $REPORT_PATH"
jq '{
  adversarial_stress_pass,
  stress_score,
  stress_assertions,
  degradation_curve
}' "$REPORT_PATH"

if [[ "$(jq -r '.adversarial_stress_pass' "$REPORT_PATH")" != "true" ]]; then
  echo "adversarial long-memory stress benchmark failed" >&2
  exit 1
fi
