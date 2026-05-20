# Wedge v0 Local Runbook (Cognitive Continuity)

This runbook validates the active wedge v0 path:

host task -> `prepare_task_context` -> host work -> `store_work_episode` -> next task with better continuity

Scope:
- coding-agent continuity and personalization
- local-first memory
- CLI and thin host wrappers
- no workflow/interface/UI expansion

## 1) Prerequisites

- Rust toolchain
- `jq`
- optional host CLI (`qwen` or `gemini`)

## 2) Fast local sanity checks

```bash
cargo test --workspace
./.codex/skills/adesh-spec-guard/scripts/check_spec_drift.sh .
```

## 3) Before-task context call

```bash
ADESH_DATABASE_URL='sqlite:///tmp/adesh-session.db?mode=rwc' \
cargo run -p adesh-daemon -- host prepare \
  --task "What should I do next to safely finish upload retry reliability?" \
  --task-hint retry-hardening \
  --file src/upload/upload_worker.rs
```

Expected:
- compact decisions/preferences/open loops/risk flags
- likely next directions grounded in evidence refs

## 4) After-task memory writeback

Use direct host store:

```bash
ADESH_DATABASE_URL='sqlite:///tmp/adesh-session.db?mode=rwc' \
cargo run -p adesh-daemon -- host store \
  --task "Harden upload retry under flaky network conditions" \
  --summary "Kept dedupe in service boundary; timeout benchmark still pending." \
  --task-hint retry-hardening \
  --decision "Keep duplicate protection in UploadService::Avoid transport-layer coupling" \
  --unresolved "Run timeout benchmark under packet loss" \
  --risk "Without timeout benchmark, reliability claims remain weak" \
  --test "pass::retry_backoff_unit::Backoff envelope remains within policy bounds"
```

Or low-friction capture wrapper:

```bash
ADESH_DATABASE_URL='sqlite:///tmp/adesh-session.db?mode=rwc' \
./scripts/session_learning_capture.sh \
  --task "Harden upload retry under flaky network conditions" \
  --summary "Kept dedupe in service boundary; timeout benchmark still pending." \
  --task-hint retry-hardening \
  --decision "Keep duplicate protection in UploadService::Avoid transport-layer coupling" \
  --unresolved "Run timeout benchmark under packet loss" \
  --risk "Without timeout benchmark, reliability claims remain weak"
```

## 5) Continuous background learning during coding

Start watcher:

```bash
ADESH_DATABASE_URL='sqlite:///tmp/adesh-session.db?mode=rwc' \
./scripts/session_learning_ctl.sh start \
  --task "Track my active coding session and capture continuity memory." \
  --task-hint session-watcher
```

Check/stop:

```bash
./scripts/session_learning_ctl.sh status --task-hint session-watcher
./scripts/session_learning_ctl.sh stop --task-hint session-watcher
```

Inspect recent episodes:

```bash
./scripts/session_learning_recent.sh --db-path /tmp/adesh-session.db --limit 10 --task-hint session-watcher
```

## 6) Host wrapper usage

Qwen:

```bash
ADESH_DATABASE_URL='sqlite:///tmp/adesh-session.db?mode=rwc' \
./scripts/qwen_with_aadesh.sh prompt \
  --task "Review retry reliability gaps and propose next actions." \
  --task-hint retry-hardening
```

Qwen Code alias:

```bash
ADESH_DATABASE_URL='sqlite:///tmp/adesh-session.db?mode=rwc' \
./scripts/qwen_code_with_aadesh.sh prompt \
  --task "Review retry reliability gaps and propose next actions." \
  --task-hint retry-hardening
```

Gemini:

```bash
ADESH_DATABASE_URL='sqlite:///tmp/adesh-session.db?mode=rwc' \
./scripts/gemini_with_aadesh.sh prompt \
  --task "Review retry reliability gaps and propose next actions." \
  --task-hint retry-hardening
```

OpenCode:

```bash
ADESH_DATABASE_URL='sqlite:///tmp/adesh-session.db?mode=rwc' \
./scripts/opencode_with_aadesh.sh prompt \
  --task "Review retry reliability gaps and propose next actions." \
  --task-hint retry-hardening
```

## 7) Wedge proof benchmark

Run the baseline-vs-treatment harness:

```bash
./scripts/cognitive_eval_harness.sh
```

Harness output:
- `report.json` under `/tmp/adesh-eval-<timestamp>/`
- mean score improvement
- decision/open-loop/preference recall deltas
- next-direction acceptance
- false-memory rate
- threshold pass/fail flags

External memory comparison harness:

```bash
./scripts/external_memory_comparison_harness.sh --include-external-stubs
```

This runs the same comparison contract for baseline and Aadesh, while reserving explicit import slots for memd, Knowns, OpenMemory, and Hermes results. See `docs/COMPARISON_BENCHMARK.md`.

## 8) Exit criteria

Treat wedge as proven for this iteration only when:
- treatment beats baseline on recall and overall score
- next-direction acceptance is strong enough to guide real work
- false-memory remains low
- host can run before-task + after-task loop with low friction

## 8.1 Live usage readiness checklist

Before broadening scope, ensure all items below are true:
- `cargo test --workspace` is green on your machine
- connector smoke and eval harness both pass:
  - `./scripts/connector_event_smoke.sh`
  - `./scripts/cognitive_eval_harness.sh`
- at least one real host loop is exercised end-to-end:
  - before-task `host prepare`
  - after-task `host store` or `host connector task_end`
- linked intervention outcomes are being written (not only unlinked traces)
- at least one workspace shows repeated cross-session usefulness without explicit “resume session” prompting

Recommended local stability setting for script-driven runs:

```bash
export ADESH_CARGO_TARGET_DIR=/tmp/adesh-cargo-target
```

This avoids intermittent local cargo lock-path issues in some environments.

## 9) Codex extension multi-repo pattern

Use one shared DB and run commands from each repo terminal used by Codex:

```bash
export ADESH_DAEMON_ROOT="/home/rajan/Desktop/work/aadesh"
export ADESH_DATABASE_URL="sqlite:///home/rajan/.aadesh/cognition.db?mode=rwc"
```

Before task:

```bash
"$ADESH_DAEMON_ROOT/scripts/session_learning_prepare.sh" \
  --task "What should I focus on next for this task?" \
  --task-hint codex-main \
  --auto-files
```

Start/stop watcher in that repo:

```bash
"$ADESH_DAEMON_ROOT/scripts/session_learning_ctl.sh" start \
  --task "Track this Codex coding session and capture continuity memory." \
  --task-hint codex-main

"$ADESH_DAEMON_ROOT/scripts/session_learning_ctl.sh" stop --task-hint codex-main
```

After task:

```bash
"$ADESH_DAEMON_ROOT/scripts/session_learning_capture.sh" \
  --task "Task I just finished" \
  --summary "What changed and why" \
  --task-hint codex-main \
  --decision "Key decision::Rationale" \
  --unresolved "Open loop if any"
```

## 10) Connector adapter path (for host implementers)

Use one normalized connector event command:

```bash
cargo run -p adesh-daemon -- host connector --json '<connector_event_payload>'
```

Mapping:
- `task_start` returns `prepare_context`
- `task_checkpoint` and `task_end` persist episodes

Note:
- these are Aadesh connector abstraction events
- host integrations (for example VS Code chat participants/tools) choose when to emit them
- optional supervisory trace metadata can be included:
  `host_agent_id`, `host_agent_kind`, `host_model`, `context_id`,
  `selected_next_direction`, `outcome`, `correction_summary`

Quick smoke:

```bash
./scripts/connector_event_smoke.sh
```

Standard deterministic supervisory trace simulation:

```bash
./scripts/supervisory_trace_simulation.sh --sessions 20
```

This simulation is the local repeatable test for post-Phase-D real-use mechanics. It does not call
a live model. It seeds two workspaces, runs completed connector sessions through `task_start` and
`task_end`, verifies returned `context_id` linkage, and reports learnable intervention outcomes.
Use `--sessions 50` to exercise the linked-outcome volume requirement before doing the real
two-week observation window.

Complex realism simulation:

```bash
./scripts/supervisory_trace_complex_simulation.sh
```

Use this when validating cognitive quality under less ideal conditions. It adds overlapping
workstreams, stale/conflicting memory, sparse payloads, failing/passing evidence, duplicate replay,
and one controlled unlearnable stale-context event.

## 11) Phase E trigger check (keep gated by default)

Do not start Phase E unless repeated operational evidence appears.

Use this quick threshold check:
- 3+ unresolved policy-lineage reconstruction incidents
- 5+ rollback/supersession trace cases in 14 days that are hard to query
- 3+ ranking explanation failures needing explicit policy version lineage
- 10+ policy comparison requests in 14 days requiring manual reconstruction

If not met, keep focus on host adoption + observational data quality.
