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

OUTPUT_DIR=""
KEEP_ARTIFACTS=0

usage() {
  cat <<'EOF'
Usage:
  cognitive_eval_harness.sh [options]

Options:
  --output-dir <path>   Directory for DB and JSON artifacts (default: /tmp/adesh-eval-<timestamp>)
  --keep-artifacts      Keep all per-task baseline/treatment JSON files
  -h, --help            Show this help

This harness seeds two workspace-like contexts with prior episodes, then compares:
  baseline  = empty memory DB
  treatment = seeded memory DB

It prints aggregate metrics and writes:
  <output-dir>/report.json
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --keep-artifacts)
      KEEP_ARTIFACTS=1
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

if [[ -z "$OUTPUT_DIR" ]]; then
  OUTPUT_DIR="/tmp/adesh-eval-$(date +%Y%m%d-%H%M%S)"
fi
mkdir -p "$OUTPUT_DIR"

CARGO_TARGET_DIR="${ADESH_CARGO_TARGET_DIR:-/tmp/adesh-cargo-target}"

BASELINE_DB="${OUTPUT_DIR}/baseline.db"
TREATMENT_DB="${OUTPUT_DIR}/treatment.db"
rm -f "$BASELINE_DB" "$TREATMENT_DB"

baseline_url="sqlite://${BASELINE_DB}?mode=rwc"
treatment_url="sqlite://${TREATMENT_DB}?mode=rwc"

run_store() {
  local db_url="$1"
  shift
  ADESH_DATABASE_URL="$db_url" CARGO_TARGET_DIR="$CARGO_TARGET_DIR" cargo run -q -p adesh-daemon -- host store "$@" >/dev/null
}

run_prepare() {
  local db_url="$1"
  shift
  ADESH_DATABASE_URL="$db_url" CARGO_TARGET_DIR="$CARGO_TARGET_DIR" cargo run -q -p adesh-daemon -- host prepare "$@"
}

run_cognitive() {
  local db_url="$1"
  shift
  ADESH_DATABASE_URL="$db_url" CARGO_TARGET_DIR="$CARGO_TARGET_DIR" cargo run -q -p adesh-daemon -- cognitive "$@"
}

calc_metrics() {
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
      def has_kw(items; kw):
        if (kw | length) == 0 then false
        else [items[]?.statement | ascii_downcase | contains(kw | ascii_downcase)] | any
        end;

      def unsupported_count(items):
        [items[]? | select((.evidence_refs | length) == 0)] | length;

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
      | (
          (($r.relevant_decisions | length) // 0)
          + (($r.applicable_preferences | length) // 0)
          + (($r.open_loops | length) // 0)
          + (($r.risk_flags | length) // 0)
          + (($r.likely_next_directions | length) // 0)
        ) as $surface_count
      | {
          decision_hit: $decision_hit,
          open_loop_hit: $open_hit,
          preference_hit: $preference_hit,
          direction_hit: $direction_hit,
          no_unsupported_memory: ($unsupported == 0),
          unsupported_count: $unsupported,
          surface_count: $surface_count,
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

echo "Seeding treatment memory..."

# Workspace A: coding-service
run_store "$treatment_url" \
  --workspace-kind task_space \
  --workspace-locator "workspace://coding-service" \
  --task "Stabilize upload retry path for payment ingestion" \
  --task-hint retry-hardening \
  --summary "Moved duplicate protection to UploadService boundary and kept retry transport separated. Timeout-path coverage still missing." \
  --file src/upload/upload_worker.rs \
  --file src/upload/upload_service.rs \
  --decision "Keep duplicate protection in UploadService::Retry transport and dedupe boundary should remain separated" \
  --unresolved "Timeout-path coverage is still missing in upload retry tests" \
  --preference "Prefer explicit service-layer isolation over hidden helper indirection in reliability-sensitive paths" \
  --risk "Missing timeout-path coverage can hide duplicate-processing regressions" \
  --test "fail::upload_retry_timeout_path::Timeout path still fails under packet loss"

run_store "$treatment_url" \
  --workspace-kind task_space \
  --workspace-locator "workspace://coding-service" \
  --task "Tune retry backoff behavior under load" \
  --task-hint retry-hardening \
  --summary "Confirmed bounded retries with exponential backoff. Need benchmark for timeout behavior under degraded network." \
  --file src/upload/retry_policy.rs \
  --decision "Use bounded retries with exponential backoff for upload transport failures" \
  --unresolved "Need benchmark proving timeout behavior under degraded network conditions" \
  --preference "Prefer explicit service-layer isolation over hidden helper indirection in reliability-sensitive paths" \
  --risk "Without benchmark evidence the retry path can appear stable while failing in high-latency conditions" \
  --test "pass::retry_backoff_unit::Backoff envelope remains within policy bounds"

run_store "$treatment_url" \
  --workspace-kind task_space \
  --workspace-locator "workspace://coding-service" \
  --task "Prove the coding-sidecar wedge in real host usage" \
  --task-hint wedge-proof \
  --summary "Defined baseline-vs-treatment benchmark as next critical step; local cleanup should not outrank proof validation." \
  --file docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md \
  --decision "The wedge is not proven until baseline-vs-treatment evaluation is executed on real tasks" \
  --unresolved "Evaluation harness run still pending on real multi-episode tasks" \
  --risk "Without benchmark execution, quality improvements remain unproven"

# Workspace B: writing-studio (non-repo style task space)
run_store "$treatment_url" \
  --workspace-kind task_space \
  --workspace-locator "workspace://writing-studio" \
  --task "Improve pricing article quality" \
  --task-hint article-polish \
  --summary "Team feedback said opening stayed abstract; concrete example should appear before strategic framing." \
  --file docs/pricing_article.md \
  --decision "Open with one concrete customer example before conceptual framing" \
  --unresolved "Need readability check after intro rewrite" \
  --preference "Keep sections short with checklist-style bullets for actionability" \
  --risk "Abstract opening can reduce comprehension and trust"

run_store "$treatment_url" \
  --workspace-kind task_space \
  --workspace-locator "workspace://writing-studio" \
  --task "Refine article structure" \
  --task-hint article-polish \
  --summary "Validated that checklist pacing improves readability in internal review." \
  --file docs/pricing_article.md \
  --preference "Keep sections short with checklist-style bullets for actionability" \
  --test "pass::readability_internal_review::Readers completed task extraction faster with checklist format"

run_store "$treatment_url" \
  --workspace-kind task_space \
  --workspace-locator "workspace://writing-studio" \
  --task "Ship final pricing article revision" \
  --task-hint article-polish \
  --summary "Before final publish, team wants A/B readability confirmation of the intro change and checklist pacing." \
  --unresolved "Need A/B readability confirmation before final publish" \
  --risk "Skipping final readability validation may reintroduce abstraction-heavy opening"

TASKS_FILE="${OUTPUT_DIR}/tasks.tsv"
cat >"$TASKS_FILE" <<'EOF'
t01|workspace://coding-service|retry-hardening|What should I tackle first to safely finish upload retry reliability?|duplicate protection|timeout-path coverage|service-layer isolation|timeout behavior
t02|workspace://coding-service|retry-hardening|I need the next concrete step for retry hardening under flaky network.|bounded retries|benchmark proving timeout behavior|service-layer isolation|benchmark
t03|workspace://coding-service|wedge-proof|What should I work on next to validate the cognitive-sidecar wedge in real use?|not proven until baseline-vs-treatment evaluation|evaluation harness run still pending||baseline-vs-treatment
t04|workspace://coding-service|retry-hardening|Help me prioritize between refactor polish and reliability proof tasks right now.|bounded retries|timeout-path coverage|service-layer isolation|timeout behavior
t05|workspace://coding-service|wedge-proof|Where is the highest risk if we keep polishing internals without validation?|baseline-vs-treatment evaluation|evaluation harness run still pending||evaluation harness
t06|workspace://coding-service|retry-hardening|Give me continuity context before touching upload retry code again.|duplicate protection|timeout-path coverage|service-layer isolation|timeout behavior
t07|workspace://writing-studio|article-polish|What should I do next to improve the pricing article draft?|concrete customer example|readability check|checklist-style bullets|readability
t08|workspace://writing-studio|article-polish|I want to finish the intro quickly. What matters most first?|concrete customer example|A/B readability confirmation|checklist-style bullets|A/B readability
t09|workspace://writing-studio|article-polish|How should I reduce confusion risk before final publish?|concrete customer example|A/B readability confirmation|checklist-style bullets|readability
t10|workspace://writing-studio|article-polish|Give me continuity guidance for the next article revision pass.|concrete customer example|readability check|checklist-style bullets|checklist
t11|workspace://writing-studio|article-polish|What open loop matters more than small wording cleanup right now?|concrete customer example|A/B readability confirmation|checklist-style bullets|A/B readability
t12|workspace://writing-studio|article-polish|What is the most defensible next direction before we ship?|concrete customer example|A/B readability confirmation|checklist-style bullets|readability
EOF

rows_file="${OUTPUT_DIR}/rows.jsonl"
rm -f "$rows_file"

echo "Running baseline vs treatment prepares..."
while IFS='|' read -r task_id workspace_locator task_hint task_prompt decision_kw open_kw preference_kw direction_kw; do
  baseline_path="${OUTPUT_DIR}/${task_id}_baseline.json"
  treatment_path="${OUTPUT_DIR}/${task_id}_treatment.json"

  run_prepare "$baseline_url" \
    --workspace-kind task_space \
    --workspace-locator "$workspace_locator" \
    --task-hint "$task_hint" \
    --task "$task_prompt" \
    >"$baseline_path"

  run_prepare "$treatment_url" \
    --workspace-kind task_space \
    --workspace-locator "$workspace_locator" \
    --task-hint "$task_hint" \
    --task "$task_prompt" \
    >"$treatment_path"

  baseline_metrics="$(calc_metrics "$baseline_path" "$decision_kw" "$open_kw" "$preference_kw" "$direction_kw")"
  treatment_metrics="$(calc_metrics "$treatment_path" "$decision_kw" "$open_kw" "$preference_kw" "$direction_kw")"

  jq -n \
    --arg task_id "$task_id" \
    --arg workspace_locator "$workspace_locator" \
    --arg task_hint "$task_hint" \
    --arg task_prompt "$task_prompt" \
    --arg decision_kw "$decision_kw" \
    --arg open_kw "$open_kw" \
    --arg preference_kw "$preference_kw" \
    --arg direction_kw "$direction_kw" \
    --argjson baseline "$baseline_metrics" \
    --argjson treatment "$treatment_metrics" \
    '{
      task_id: $task_id,
      workspace_locator: $workspace_locator,
      task_hint: $task_hint,
      task_prompt: $task_prompt,
      expected: {
        decision_kw: $decision_kw,
        open_loop_kw: $open_kw,
        preference_kw: $preference_kw,
        direction_kw: $direction_kw
      },
      baseline: $baseline,
      treatment: $treatment,
      score_delta: ($treatment.score - $baseline.score)
    }' >>"$rows_file"

  if [[ "$KEEP_ARTIFACTS" -eq 0 ]]; then
    rm -f "$baseline_path" "$treatment_path"
  fi
done <"$TASKS_FILE"

report_path="${OUTPUT_DIR}/report.json"

jq -s '
  . as $rows
  | (length) as $n
  | {
      tasks_total: $n,
      baseline_mean_score: (($rows | map(.baseline.score) | add) / $n),
      treatment_mean_score: (($rows | map(.treatment.score) | add) / $n),
      mean_score_improvement: ((($rows | map(.treatment.score) | add) - ($rows | map(.baseline.score) | add)) / $n),
      baseline_decision_recall_pct: (100 * (($rows | map(if .baseline.decision_hit then 1 else 0 end) | add) / $n)),
      treatment_decision_recall_pct: (100 * (($rows | map(if .treatment.decision_hit then 1 else 0 end) | add) / $n)),
      baseline_open_loop_recall_pct: (100 * (($rows | map(if .baseline.open_loop_hit then 1 else 0 end) | add) / $n)),
      treatment_open_loop_recall_pct: (100 * (($rows | map(if .treatment.open_loop_hit then 1 else 0 end) | add) / $n)),
      baseline_preference_recall_pct: (100 * (($rows | map(if .baseline.preference_hit then 1 else 0 end) | add) / $n)),
      treatment_preference_recall_pct: (100 * (($rows | map(if .treatment.preference_hit then 1 else 0 end) | add) / $n)),
      treatment_next_direction_acceptance_pct: (100 * (($rows | map(if .treatment.direction_hit then 1 else 0 end) | add) / $n)),
      treatment_false_memory_rate_pct: (
        100 * (
          (($rows | map(.treatment.unsupported_count) | add) as $unsupported_total
          | (($rows | map(.treatment.surface_count) | add) // 0) as $surface_total
          | if $surface_total == 0 then 0 else ($unsupported_total / $surface_total) end
          )
        )
      ),
      thresholds: {
        mean_score_improvement_ge_1: true,
        decision_recall_improvement_ge_30pp: true,
        open_loop_recall_improvement_ge_30pp: true,
        next_direction_acceptance_ge_50: true,
        false_memory_rate_lt_10: true
      },
      pass: {
        mean_score_improvement_ge_1: (((($rows | map(.treatment.score) | add) - ($rows | map(.baseline.score) | add)) / $n) >= 1),
        decision_recall_improvement_ge_30pp: (
          (100 * (($rows | map(if .treatment.decision_hit then 1 else 0 end) | add) / $n))
          - (100 * (($rows | map(if .baseline.decision_hit then 1 else 0 end) | add) / $n))
          >= 30
        ),
        open_loop_recall_improvement_ge_30pp: (
          (100 * (($rows | map(if .treatment.open_loop_hit then 1 else 0 end) | add) / $n))
          - (100 * (($rows | map(if .baseline.open_loop_hit then 1 else 0 end) | add) / $n))
          >= 30
        ),
        next_direction_acceptance_ge_50: (
          (100 * (($rows | map(if .treatment.direction_hit then 1 else 0 end) | add) / $n)) >= 50
        ),
        false_memory_rate_lt_10: (
          (
            100 * (
              (($rows | map(.treatment.unsupported_count) | add) as $unsupported_total
              | (($rows | map(.treatment.surface_count) | add) // 0) as $surface_total
              | if $surface_total == 0 then 0 else ($unsupported_total / $surface_total) end
              )
            )
          ) < 10
        )
      },
      rows: $rows
    }
  ' "$rows_file" >"$report_path"

echo "Wrote report: $report_path"

EVAL_DB="${OUTPUT_DIR}/eval.db"
eval_url="sqlite://${EVAL_DB}?mode=rwc"

run_id="eval-$(date +%Y%m%d-%H%M%S)"
run_started_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)

baseline_summary=$(jq -r '"\(.tasks_total) tasks, mean=\(.baseline_mean_score)"' "$report_path")
treatment_summary=$(jq -r '"\(.tasks_total) tasks, mean=\(.treatment_mean_score), improvement=\(.mean_score_improvement)"' "$report_path")
judge_summary=$(jq -r 'if (.pass | to_entries | map(.value) | all) then "promote" elif (.pass | to_entries | map(.value) | any) then "mixed" else "reject" end' "$report_path")

failure_tags=$(jq -r '[.pass | to_entries[] | select(.value == false) | .key] | join(",")' "$report_path")
promotion_decision=$(jq -r 'if .pass | to_entries | map(.value) | all then "promote" elif .pass | to_entries | map(.value) | any then "hold" else "reject" end' "$report_path")

run_completed_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)

eval_run_json=$(jq -n \
  --arg run_id "$run_id" \
  --arg eval_name "cognitive-eval" \
  --arg run_started_at "$run_started_at" \
  --arg run_completed_at "$run_completed_at" \
  --arg baseline_summary "$baseline_summary" \
  --arg treatment_summary "$treatment_summary" \
  --arg judge_summary "$judge_summary" \
  --arg failure_tags "$failure_tags" \
  --arg promotion_decision "$promotion_decision" \
  '{
    run_id: $run_id,
    eval_name: $eval_name,
    eval_version: "1.0.0",
    run_started_at: $run_started_at,
    run_completed_at: $run_completed_at,
    baseline_summary: $baseline_summary,
    treatment_summary: $treatment_summary,
    judge_summary: $judge_summary,
    failure_tags: $failure_tags,
    promotion_decision: $promotion_decision
  }')

stored_run=$(run_cognitive "$eval_url" store-eval-run --json "$eval_run_json")
stored_run_id=$(echo "$stored_run" | jq -r '.run_id')
echo "Stored eval run: ${stored_run_id}"

artifact_json=$(jq -n \
  --arg run_id "$run_id" \
  --arg report_path "$report_path" \
  '{
    artifact_id: "",
    run_id: $run_id,
    artifact_kind: "output",
    file_path: $report_path,
    mime_type: "application/json"
  }')

artifact_json=$(echo "$artifact_json" | jq --arg persisted_run_id "$stored_run_id" '.run_id = $persisted_run_id')

run_cognitive "$eval_url" store-eval-artifact --json "$artifact_json" >/dev/null
echo "Stored eval artifact: $report_path"

echo ""
jq '{
  tasks_total,
  baseline_mean_score,
  treatment_mean_score,
  mean_score_improvement,
  baseline_decision_recall_pct,
  treatment_decision_recall_pct,
  baseline_open_loop_recall_pct,
  treatment_open_loop_recall_pct,
  baseline_preference_recall_pct,
  treatment_preference_recall_pct,
  treatment_next_direction_acceptance_pct,
  treatment_false_memory_rate_pct,
  pass
}' "$report_path"
