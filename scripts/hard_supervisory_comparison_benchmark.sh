#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: ./scripts/hard_supervisory_comparison_benchmark.sh [options]

Runs the harder Aadesh validation layer:
1. consumes or creates an external comparison report
2. runs multi-week noisy supervisory usage simulation
3. runs deep supervisory guidance probes
4. produces a judge-style report separating recall, next-direction quality,
   outcome-history behavior, noisy temporal behavior, and supervisory trace value

Options:
  --comparison-report PATH  Existing external comparison report to judge.
  --output-dir DIR          Directory for artifacts/reports. Default: /tmp/adesh-hard-supervisory-<run_id>.
  --days N                  Simulated multi-week days. Default: 14.
  --sessions N              Deep benchmark linked sessions. Default: 12.
  --stress-events N         Adversarial noise events. Default: 36.
  --data-profile PROFILE    standard or production. Default: production.
  --judge-mode MODE         local or lmstudio. Default: local.
  -h, --help                Show this help.

Environment:
  ADESH_DAEMON_ROOT         Override repo root.
  ADESH_CARGO_TARGET_DIR    Cargo target dir. Default: /tmp/adesh-cargo-target.
  HARD_JUDGE_CHAT_URL       LM Studio chat URL for --judge-mode lmstudio.
                            Default: http://127.0.0.1:1234/api/v1/chat.
  HARD_JUDGE_MODEL          Judge model for --judge-mode lmstudio.
                            Default: qwen/qwen3.6-27b.
USAGE
}

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required" >&2
  exit 1
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required" >&2
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
COMPARISON_REPORT=""
OUTPUT_DIR=""
DAYS=14
SESSIONS=12
STRESS_EVENTS=36
DATA_PROFILE="production"
JUDGE_MODE="local"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --comparison-report)
      COMPARISON_REPORT="${2:-}"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="${2:-}"
      shift 2
      ;;
    --days)
      DAYS="${2:-}"
      shift 2
      ;;
    --sessions)
      SESSIONS="${2:-}"
      shift 2
      ;;
    --stress-events)
      STRESS_EVENTS="${2:-}"
      shift 2
      ;;
    --data-profile)
      DATA_PROFILE="${2:-}"
      shift 2
      ;;
    --judge-mode)
      JUDGE_MODE="${2:-}"
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
  echo "--days must be an integer >= 14" >&2
  exit 1
fi
if ! [[ "$SESSIONS" =~ ^[0-9]+$ ]] || [[ "$SESSIONS" -lt 12 ]]; then
  echo "--sessions must be an integer >= 12" >&2
  exit 1
fi
if ! [[ "$STRESS_EVENTS" =~ ^[0-9]+$ ]] || [[ "$STRESS_EVENTS" -lt 12 ]]; then
  echo "--stress-events must be an integer >= 12" >&2
  exit 1
fi
if [[ "$DATA_PROFILE" != "standard" && "$DATA_PROFILE" != "production" ]]; then
  echo "--data-profile must be standard or production" >&2
  exit 1
fi
if [[ "$JUDGE_MODE" != "local" && "$JUDGE_MODE" != "lmstudio" ]]; then
  echo "--judge-mode must be local or lmstudio" >&2
  exit 1
fi

OUTPUT_DIR="${OUTPUT_DIR:-/tmp/adesh-hard-supervisory-${RUN_ID}}"
mkdir -p "$OUTPUT_DIR"

export ADESH_CARGO_TARGET_DIR="${ADESH_CARGO_TARGET_DIR:-/tmp/adesh-cargo-target}"

if [[ -n "$COMPARISON_REPORT" && ! -f "$COMPARISON_REPORT" ]]; then
  echo "--comparison-report does not exist: $COMPARISON_REPORT" >&2
  exit 1
fi

if [[ -z "$COMPARISON_REPORT" ]]; then
  echo "No comparison report supplied; running internal baseline/Aadesh comparison..."
  "${ADESH_ROOT}/scripts/external_memory_comparison_harness.sh" \
    --output-dir "${OUTPUT_DIR}/external_comparison" >/dev/null
  COMPARISON_REPORT="${OUTPUT_DIR}/external_comparison/comparison_report.json"
fi

MULTIDAY_DIR="${OUTPUT_DIR}/multiday"
DEEP_DIR="${OUTPUT_DIR}/deep"
STRESS_DIR="${OUTPUT_DIR}/adversarial_stress"
REPORT_PATH="${OUTPUT_DIR}/hard_supervisory_comparison_report.json"
LLM_JUDGE_RAW_PATH="${OUTPUT_DIR}/lmstudio_judge_raw.json"
LLM_JUDGE_TEXT_PATH="${OUTPUT_DIR}/lmstudio_judge.txt"

echo "Running multi-week noisy supervisory simulation..."
"${ADESH_ROOT}/scripts/multiday_supervisory_usage_simulation.sh" \
  --days "$DAYS" \
  --profile "$DATA_PROFILE" \
  --output-dir "$MULTIDAY_DIR" >/dev/null

echo "Running deep supervisory guidance probes..."
"${ADESH_ROOT}/scripts/deep_supervisory_guidance_benchmark.sh" \
  --sessions "$SESSIONS" \
  --output-dir "$DEEP_DIR" >/dev/null

echo "Running adversarial long-memory stress probes..."
"${ADESH_ROOT}/scripts/adversarial_long_memory_stress_benchmark.sh" \
  --noise-events "$STRESS_EVENTS" \
  --output-dir "$STRESS_DIR" >/dev/null

MULTIDAY_REPORT="${MULTIDAY_DIR}/multiday_supervisory_usage_report.json"
DEEP_REPORT="${DEEP_DIR}/deep_supervisory_guidance_report.json"
STRESS_REPORT="${STRESS_DIR}/adversarial_long_memory_stress_report.json"

if [[ ! -f "$MULTIDAY_REPORT" || ! -f "$DEEP_REPORT" || ! -f "$STRESS_REPORT" ]]; then
  echo "expected hard benchmark sub-reports were not produced" >&2
  exit 1
fi

jq -n \
  --slurpfile comparison "$COMPARISON_REPORT" \
  --slurpfile multiday "$MULTIDAY_REPORT" \
  --slurpfile deep "$DEEP_REPORT" \
  --slurpfile stress "$STRESS_REPORT" \
  --arg run_id "$RUN_ID" \
  --arg output_dir "$OUTPUT_DIR" \
  --arg comparison_report "$COMPARISON_REPORT" \
  --arg multiday_report "$MULTIDAY_REPORT" \
  --arg deep_report "$DEEP_REPORT" \
  --arg stress_report "$STRESS_REPORT" \
  --arg judge_mode "$JUDGE_MODE" \
  --arg data_profile "$DATA_PROFILE" \
  '
  def bool_score($v): if $v == true then 1 else 0 end;
  def avg($xs): if ($xs | length) == 0 then 0 else (($xs | add) / ($xs | length)) end;
  def true_ratio($obj):
    ($obj // {} | to_entries | map(bool_score(.value))) as $scores
    | avg($scores);
  def down($v): (($v // "") | tostring | ascii_downcase);
  def case_text($case):
    [
      $case.observed.task_focus,
      $case.observed.top_decision,
      $case.observed.top_open_loop,
      $case.observed.top_next_direction,
      $case.diagnostic
    ]
    | map(down(.))
    | join(" ");
  def expected_hit($text; $expected):
    ($expected | tostring | ascii_downcase) as $raw
    | if ($raw | startswith("not ")) then
        ($text | contains($raw[4:]) | not)
      elif ($raw | contains("/")) then
        (($raw | split("/") | map(select(length > 0))) as $terms
          | ($terms | map($text | contains(.)) | any))
      else
        ($text | contains($raw))
      end;
  def case_judgment($case):
    (case_text($case)) as $text
    | ($case.expected_evidence // []) as $expected
    | ($expected | map(select(expected_hit($text; .)))) as $hits
    | (if ($expected | length) == 0 then 0 else (($hits | length) / ($expected | length)) end) as $hit_ratio
    | {
        case_id: $case.case_id,
        assertion_passed: ($case.assertion_passed == true),
        expected_evidence_hit_ratio: $hit_ratio,
        expected_evidence_hits: $hits,
        observed_top_next_direction: ($case.observed.top_next_direction // null),
        uncertainty_count: ($case.observed.uncertainty_count // null),
        verdict: (
          if (($case.assertion_passed == true) and ($hit_ratio >= 0.5) and (($case.observed.top_next_direction // "") | length > 0))
          then "useful"
          else "weak"
          end
        ),
        diagnostic: (
          if (($case.assertion_passed == true) and ($hit_ratio >= 0.5) and (($case.observed.top_next_direction // "") | length > 0))
          then "observed output is useful under deterministic case-judge criteria"
          elif ($case.assertion_passed != true) then "lexical production assertion failed"
          elif ($hit_ratio < 0.5) then "observed output did not include enough expected evidence"
          else "observed output did not provide a next direction"
          end
        )
      };
  def case_judge_score($cases):
    if ($cases | length) == 0 then null
    else avg($cases | map(if .verdict == "useful" then 1 else 0 end))
    end;
  def system_by_name($name):
    ($comparison[0].compared_systems // []) | map(select(.system == $name))[0];
  def system_judgment($system):
    ($system.dimensions.memory_recall_quality // {}) as $mem
    | {
        system: $system.system,
        comparator_class: $system.comparator_class,
        status: $system.status,
        lexical_mean_score: ($system.mean_score // null),
        memory_recall_score: avg([
          ($system.decision_recall // $mem.decision_recall // 0),
          ($system.open_loop_recall // $mem.open_loop_recall // 0),
          ($system.preference_recall // $mem.preference_recall // 0)
        ]),
        next_direction_score: ($system.next_direction_acceptance_proxy // $system.dimensions.next_direction_quality.acceptance_proxy // 0),
        false_memory_score: (1 - ($system.false_memory_rate_proxy // 1)),
        cross_host_portability_score: ($system.dimensions.cross_host_portability.score // 0),
        outcome_trace_learning_score: ($system.dimensions.outcome_trace_learning.score // 0),
        setup_friction_score: ($system.dimensions.setup_friction.score // 0),
        hard_supervisory_relevance_score: avg([
          ($system.next_direction_acceptance_proxy // $system.dimensions.next_direction_quality.acceptance_proxy // 0),
          (1 - ($system.false_memory_rate_proxy // 1)),
          ($system.dimensions.cross_host_portability.score // 0),
          ($system.dimensions.outcome_trace_learning.score // 0)
        ])
      };

  ($comparison[0].compared_systems // []) as $systems
  | (system_by_name("aadesh")) as $aadesh
  | ($systems | map(select(.system != "aadesh" and .system != "baseline"))) as $competitors
  | ($multiday[0].probe_assertions // {}) as $multiday_probe_assertions
  | ($multiday[0].multiday_assertions // {}) as $multiday_assertions
  | (($multiday[0].production_case_report // []) | map(case_judgment(.))) as $case_judgments
  | ($deep[0].guidance_probe_assertions // {}) as $deep_probe_assertions
  | ($stress[0].stress_assertions // {}) as $stress_assertions
  | {
      run_id: $run_id,
      scenario: "hard-supervisory-comparison-v0",
      judge_mode: $judge_mode,
      data_profile: $data_profile,
      output_dir: $output_dir,
      artifacts: {
        comparison_report: $comparison_report,
        multiday_report: $multiday_report,
        deep_report: $deep_report,
        adversarial_stress_report: $stress_report
      },
      rubric: {
        memory_recall_quality: "Can the system recover decisions, open loops, and preferences?",
        next_direction_quality: "Can it turn memory into useful current-task action?",
        setup_friction: "How hard was local setup?",
        cross_host_portability: "Can it work across hosts rather than inside one runtime?",
        outcome_trace_learning: "Does it preserve accepted/ignored/modified suggestions as learnable supervisory evidence?",
        noisy_temporal_behavior: "Does it handle multi-week, sparse, resolved, and non-repo traces?",
        outcome_history_behavior: "Does accepted/modified/ignored history change what gets ranked next?",
        adversarial_long_memory_behavior: "Does useful task guidance survive noisy/confusable memory growth?"
      },
      system_judgments: ($systems | map(system_judgment(.))),
      production_case_report: ($multiday[0].production_case_report // []),
      synthetic_benchmark_quality: ($multiday[0].synthetic_benchmark_quality // null),
      adversarial_stress_summary: {
        stress_config: ($stress[0].stress_config // null),
        stress_score: ($stress[0].stress_score // null),
        stress_assertions: $stress_assertions,
        degradation_curve: ($stress[0].degradation_curve // [])
      },
      aadesh_hard_scenario_scores: {
        multiday_pass: bool_score($multiday[0].multiday_simulation_pass),
        deep_guidance_pass: bool_score($deep[0].deep_benchmark_pass),
        adversarial_stress_pass: bool_score($stress[0].adversarial_stress_pass),
        adversarial_stress_score: ($stress[0].stress_score // null),
        noisy_temporal_score: true_ratio($multiday_assertions),
        production_realism_score: (
          if $data_profile == "production" then true_ratio($multiday[0].production_assertions // {})
          else null end
        ),
        production_case_judge_score: (
          if $data_profile == "production" then case_judge_score($case_judgments)
          else null end
        ),
        outcome_history_score: true_ratio($multiday_probe_assertions),
        deep_probe_score: true_ratio($deep_probe_assertions)
      },
      production_case_judgments: $case_judgments,
      hard_assertions: {
        aadesh_present: ($aadesh != null),
        aadesh_recall_is_strong: (($aadesh.decision_recall // 0) >= 0.9 and ($aadesh.open_loop_recall // 0) >= 0.9 and ($aadesh.preference_recall // 0) >= 0.9),
        aadesh_next_direction_is_strong: (($aadesh.next_direction_acceptance_proxy // 0) >= 0.9),
        aadesh_false_memory_stays_low: (($aadesh.false_memory_rate_proxy // 1) <= 0.05),
        aadesh_has_outcome_trace_learning: (($aadesh.dimensions.outcome_trace_learning.score // 0) == 1),
        competitors_do_not_exercise_outcome_trace_learning: (
          ($competitors | length) == 0
          or (($competitors | map((.dimensions.outcome_trace_learning.score // 0) == 0) | all) == true)
        ),
        multiday_noisy_simulation_passed: ($multiday[0].multiday_simulation_pass == true),
        production_profile_passed: (
          if $data_profile == "production" then ($multiday[0].production_profile_pass == true)
          else true end
        ),
        production_case_judgments_useful: (
          if $data_profile == "production" then
            (($case_judgments | length) > 0 and (($case_judgments | map(.verdict == "useful") | all) == true))
          else true end
        ),
        adversarial_long_memory_stress_passed: ($stress[0].adversarial_stress_pass == true),
        deep_guidance_benchmark_passed: ($deep[0].deep_benchmark_pass == true),
        outcome_history_changed_guidance: (
          (($multiday_probe_assertions.payment_resolution_does_not_restart_closed_timeout_benchmark // false) == true)
          and (($multiday_probe_assertions.payment_resolution_mentions_release_or_docs // false) == true)
        ),
        phase_e_stays_gated_despite_good_metrics: (
          (($multiday_probe_assertions.phase_e_still_gated_after_weeks // false) == true)
          and (($deep_probe_assertions.eval_policy_gate_does_not_start_policy_state // false) == true)
        )
      },
      judge_summary: {
        verdict: (
          if (
            (($aadesh != null)
              and (($aadesh.decision_recall // 0) >= 0.9)
              and (($aadesh.open_loop_recall // 0) >= 0.9)
              and (($aadesh.next_direction_acceptance_proxy // 0) >= 0.9)
              and (($aadesh.false_memory_rate_proxy // 1) <= 0.05)
              and (($aadesh.dimensions.outcome_trace_learning.score // 0) == 1)
              and ($multiday[0].multiday_simulation_pass == true)
              and (if $data_profile == "production" then ($multiday[0].production_profile_pass == true) else true end)
              and (if $data_profile == "production" then (($case_judgments | length) > 0 and (($case_judgments | map(.verdict == "useful") | all) == true)) else true end)
              and ($stress[0].adversarial_stress_pass == true)
              and ($deep[0].deep_benchmark_pass == true)
            )
          ) then "pass"
          elif ($aadesh != null) then "mixed"
          else "reject"
          end
        ),
        interpretation: "Aadesh should be treated as differentiated only if it keeps strong recall and next-direction quality while also proving cross-host outcome-trace learning under noisy temporal use."
      },
      hard_wedge_pass: (
        ($aadesh != null)
        and (($aadesh.decision_recall // 0) >= 0.9)
        and (($aadesh.open_loop_recall // 0) >= 0.9)
        and (($aadesh.preference_recall // 0) >= 0.9)
        and (($aadesh.next_direction_acceptance_proxy // 0) >= 0.9)
        and (($aadesh.false_memory_rate_proxy // 1) <= 0.05)
        and (($aadesh.dimensions.outcome_trace_learning.score // 0) == 1)
        and ($multiday[0].multiday_simulation_pass == true)
        and (if $data_profile == "production" then ($multiday[0].production_profile_pass == true) else true end)
        and (if $data_profile == "production" then (($case_judgments | length) > 0 and (($case_judgments | map(.verdict == "useful") | all) == true)) else true end)
        and ($stress[0].adversarial_stress_pass == true)
        and ($deep[0].deep_benchmark_pass == true)
      )
    }
  ' > "$REPORT_PATH"

if [[ "$JUDGE_MODE" == "lmstudio" ]]; then
  if ! command -v curl >/dev/null 2>&1; then
    echo "curl is required for --judge-mode lmstudio" >&2
    exit 1
  fi
  HARD_JUDGE_CHAT_URL="${HARD_JUDGE_CHAT_URL:-http://127.0.0.1:1234/api/v1/chat}"
  HARD_JUDGE_MODEL="${HARD_JUDGE_MODEL:-qwen/qwen3.6-27b}"
  judge_payload="$(jq -n \
    --arg model "$HARD_JUDGE_MODEL" \
    --arg report "$(jq -c '{system_judgments, aadesh_hard_scenario_scores, hard_assertions, production_case_report, production_case_judgments, adversarial_stress_summary, judge_summary}' "$REPORT_PATH")" \
    '{
      model: $model,
      system_prompt: "/no_think You are judging whether Aadesh has a durable wedge over memory-only systems. Be concise and cite only facts from the JSON.",
      input: ("Review this hard benchmark JSON and return: verdict, strongest evidence, weakest evidence, next benchmark improvement. JSON: " + $report)
    }')"
  if curl -sS \
    --max-time 180 \
    -H "Content-Type: application/json" \
    -d "$judge_payload" \
    "$HARD_JUDGE_CHAT_URL" > "$LLM_JUDGE_RAW_PATH" 2>"${LLM_JUDGE_RAW_PATH}.curl.err"; then
    if jq -e '.error? != null' "$LLM_JUDGE_RAW_PATH" >/dev/null 2>&1; then
      llm_judge_status="blocked"
      llm_judge_error="$(jq -r '.error.message // (.error | tostring)' "$LLM_JUDGE_RAW_PATH")"
      printf '%s\n' "$llm_judge_error" > "$LLM_JUDGE_TEXT_PATH"
    elif jq -r '
      def nonempty_string: select(type == "string" and length > 0);
      def output_messages($items):
        ([$items[]? | select(.type == "message") | .content | nonempty_string] | join("\n")) as $message
        | if ($message | length) > 0 then $message else ([$items[]? | select(.type == "reasoning") | .content | nonempty_string] | join("\n")) end;
      if type == "array" then output_messages(.)
      elif (.output? | type) == "array" then output_messages(.output)
      else (.output | nonempty_string) // (.content | nonempty_string) // (.response | nonempty_string) // tostring
      end
    ' "$LLM_JUDGE_RAW_PATH" > "$LLM_JUDGE_TEXT_PATH"; then
      if grep -Eiq 'failed to load model|model is unloaded|invalid_request|error code:' "$LLM_JUDGE_TEXT_PATH"; then
        llm_judge_status="blocked"
        llm_judge_error="$(head -n 1 "$LLM_JUDGE_TEXT_PATH")"
      else
        llm_judge_status="run"
        llm_judge_error=""
      fi
    else
      llm_judge_status="blocked"
      llm_judge_error="LM Studio judge returned an unparseable response"
      printf '%s\n' "$llm_judge_error" > "$LLM_JUDGE_TEXT_PATH"
    fi
  else
    llm_judge_status="blocked"
    llm_judge_error="LM Studio judge endpoint unavailable: ${HARD_JUDGE_CHAT_URL}"
    printf '%s\n' "$llm_judge_error" > "$LLM_JUDGE_TEXT_PATH"
  fi
  tmp_report="${REPORT_PATH}.tmp"
  jq \
    --arg status "$llm_judge_status" \
    --arg error "$llm_judge_error" \
    --arg raw_path "$LLM_JUDGE_RAW_PATH" \
    --arg text_path "$LLM_JUDGE_TEXT_PATH" \
    --rawfile judge_text "$LLM_JUDGE_TEXT_PATH" \
    '.llm_judge = {status: $status, error: (if $error == "" then null else $error end), raw_path: $raw_path, text_path: $text_path, text: $judge_text}' \
    "$REPORT_PATH" > "$tmp_report"
  mv "$tmp_report" "$REPORT_PATH"
fi

echo "Hard supervisory comparison report:"
echo "  $REPORT_PATH"
jq '{
  hard_wedge_pass,
  judge_summary,
  hard_assertions,
  aadesh_hard_scenario_scores,
  production_case_judgments,
  adversarial_stress_summary,
  system_judgments
}' "$REPORT_PATH"

if [[ "$(jq -r '.hard_wedge_pass' "$REPORT_PATH")" != "true" ]]; then
  echo "hard supervisory comparison benchmark failed" >&2
  exit 1
fi
