# Wedge v0 Brief: Cognitive Continuity for Coding Agents

Status: active product-scope wedge brief. This document constrains the first proof. It is not a canonical behavior override.

## 1) Wedge definition

Aadesh should provide cross-session cognitive continuity and personalization for host coding agents.

The proof is not "resume one previous session." The proof is:
- memory accumulates across multiple work episodes
- a host agent provides current task context
- Aadesh returns compact, relevant, evidence-grounded guidance
- that guidance is useful even when the host is not explicitly resuming one prior session

## 2) Why this wedge exists

Coding is the first wedge because it has:
- fragmented sessions
- explicit artifacts
- measurable quality outcomes
- repeated need for continuity
- clear value from preferences, prior decisions, and unresolved loops

## 3) Product shape for the proof

For this proof, Aadesh is:
- a callable cognitive sidecar
- local-first
- transport-agnostic at the core
- CLI-first at the edge

For this proof, Aadesh is not:
- a standalone shell
- a universal agent OS
- an embodiment-first UI product
- a generic chat memory wrapper

## 4) Minimum callable tools

Only three tools are required for the proof:

1. `store_work_episode`
2. `prepare_task_context`
3. `recall_relevant_memory`

Do not expand beyond these until the proof is working end to end.

## 5) Minimum host payload model

### 5.1 Required degraded-mode payload

```json
{
  "workspace": {
    "kind": "git|directory|conversation|task_space|unknown",
    "locator": "string|null",
    "cwd": "string|null",
    "branch": "string|null",
    "external_ref": "string|null"
  },
  "task_prompt": "string",
  "files_in_focus": ["string"],
  "task_hint": "string|null"
}
```

Required in practice:
- `task_prompt`
- some workspace locator signal, if available

The system must still function in transient mode when durable workspace identity is weak.

### 5.2 Richer optional payload

The host may also provide:
- diff summary
- tests
- tool outputs
- issue refs
- work summary
- explicit decisions
- unresolved items
- artifact refs

These improve ranking and confidence. They are not mandatory for the first proof.

## 6) Memory scopes in v0

v0 memory must be scope-aware:
- `user_global`
- `workspace`
- `task_or_workstream`
- `artifact`
- `episode`

Coding uses these scopes first, but the architecture must remain generic enough for non-repo tasks later.

## 7) Memory promotion rules

Use conservative explicit rules.

States:
- `observation`
- `episode`
- `candidate_memory`
- `confirmed_memory`
- `superseded_memory`

Thresholds:
- candidate preference: one signal
- confirmed workspace preference: two aligned signals across two episodes in one workspace
- confirmed user-global preference: three aligned signals across at least two workspaces
- confirmed explicit decision: explicit decision plus at least one evidence ref
- confirmed open loop: one explicit unresolved item until resolved or superseded
- confirmed inferred risk: two aligned signals in one workspace, or one signal plus one deterministic artifact

Model-only inference remains candidate memory until corroborated.

### 7.1 v0 definition of aligned signals

Two signals are aligned only if they share:
- the same `scope_key`
- the same memory type
- the same normalized `subject_key`
- non-contradictory normalized statement keys

Normalization in v0 is intentionally crude:
- lowercase
- trim punctuation
- collapse whitespace
- small synonym map per memory type

No semantic clustering is required for the first proof.

## 8) Strict output contract for `prepare_task_context`

Hard caps:
- max 3 relevant decisions
- max 3 applicable preferences
- max 3 open loops
- max 3 risk flags
- max 3 likely next directions
- max 3 uncertainties

Every returned item must include:
- `statement`
- `confidence`
- `evidence_refs`
- `basis`

Additional required fields:
- decisions/preferences/open loops: `scope`
- risk flags: `severity`

`likely_next_directions` must be ranked from retrieved evidence, not generated as generic free-form advice.

## 9) Ranking rule for likely next directions

Rank from:
1. confirmed open loops
2. high-severity risks
3. relevant prior decisions
4. current task prompt overlap
5. workspace or user preferences
6. evidence recency

Model reasoning may compress or order the ranked candidates, but it may not invent a direction with no evidence basis.

## 10) Smallest end-to-end proof

The proof scenario uses:
- at least 3 stored prior episodes in one coding workspace
- one new vague current task prompt
- degraded current payload, not a perfectly structured one
- compact output with evidence refs on every item

The proof passes only if the returned guidance correctly surfaces:
- what matters now
- which prior decisions matter
- what the user or workspace tends to prefer here
- what unresolved loops remain
- which next direction is most plausible

The proof does not require explicit “continue my previous session” phrasing.

## 11) Concrete proof test shape

Seed one workspace with multiple prior episodes, including:
- at least one explicit decision
- at least one unresolved item
- at least one preference signal
- at least one artifact-backed risk

Then call `prepare_task_context` with a vague current task prompt and assert:
- relevant prior decision is surfaced
- relevant open loop is surfaced
- relevant preference is surfaced
- top next direction is plausible and evidence-backed
- each section respects the hard caps

## 12) Evaluation harness

Use the same host agent and same model in both conditions:
- baseline: no Aadesh context
- treatment: host prepends Aadesh guidance

Minimum benchmark:
- 12 tasks
- 2 workspaces
- 6 tasks per workspace
- 3 to 5 seeded episodes per workspace

Score per task:
1. relevant prior decision surfaced
2. relevant open loop surfaced
3. relevant preference surfaced
4. plausible next direction proposed
5. unsupported memory avoided

Proof threshold:
- mean task score improvement of at least `+1.0 / 5`
- decision recall improvement of at least `30%`
- open-loop recall improvement of at least `30%`
- restatement burden reduction of at least `25%`
- false-memory rate below `10%`
- next-direction acceptance rate at least `50%`

## 13) Explicitly out of scope

For this proof, do not optimize first for:
- email draft-and-send
- approval/OOB-heavy execution
- UI-led onboarding
- workflow runtime
- interface runtime
- broad actuator ecosystems
- remote sync
- semantic/vector retrieval infrastructure

## 14) Scope enforcement

Any PR that broadens the first proof toward a shell, broad execution platform, or non-essential UI work must be rejected unless this wedge brief and `docs/IMPLEMENTATION_PLAN.md` are updated first.

## 15) CLI examples

```bash
cargo run -p adesh-daemon -- cognitive store-work-episode --json '{...}'
```

```bash
cargo run -p adesh-daemon -- cognitive prepare-task-context --json '{...}'
```

```bash
cargo run -p adesh-daemon -- cognitive recall-relevant-memory --json '{...}'
```

Thin host-facing wrapper on top of the same cognition core:

```bash
cargo run -p adesh-daemon -- host prepare \
  --task "Can you help finish the upload retry work safely?" \
  --file src/upload/upload_worker.rs \
  --task-hint upload-retry
```

```bash
cargo run -p adesh-daemon -- host store \
  --task "Refactor retry fix so duplicate guard stays in service layer" \
  --summary "Moved dedupe check into UploadService and kept retry logic explicit. Timeout-path coverage is still missing." \
  --file src/upload/upload_worker.rs \
  --file src/upload/upload_service.rs \
  --decision "Use explicit retry state handling rather than macro abstraction in this subsystem::Failure paths are easier to audit in explicit code" \
  --test "fail::upload_worker_timeout_path::Timeout path still fails in the retry worker" \
  --task-hint upload-retry
```

The wrapper reduces host payload friction only. It must not diverge from the core request semantics.

Gemini reference integration on top of the host wrapper:

```bash
cargo run -p adesh-daemon -- host gemini prompt \
  --task "Use Gemini CLI to build the wrapper component for Aadesh itself." \
  --file crates/adesh-daemon/src/host_cli.rs \
  --task-hint gemini-wrapper
```

```bash
cargo run -p adesh-daemon -- host gemini run \
  --task "Use Gemini CLI to build the wrapper component for Aadesh itself." \
  --file crates/adesh-daemon/src/host_cli.rs \
  --file README.md \
  --task-hint gemini-wrapper \
  -- --model gemini-2.5-pro
```

```bash
cargo run -p adesh-daemon -- host gemini store \
  --task "Use Gemini CLI to build the wrapper component for Aadesh itself." \
  --summary "Added the Gemini integration wrapper and validated it with a fake CLI binary." \
  --file crates/adesh-daemon/src/gemini_wrapper.rs \
  --file crates/adesh-daemon/tests/gemini_wrapper_flows.rs \
  --decision "Keep the cognition core unchanged and add a thin host-specific wrapper::Transport integration should not mutate the cognitive API" \
  --task-hint gemini-wrapper
```

Qwen reference integration on top of the same host wrapper:

```bash
cargo run -p adesh-daemon -- host qwen prompt \
  --task "Use Qwen CLI to review the wrapper component for Aadesh itself." \
  --file crates/adesh-daemon/src/host_cli.rs \
  --task-hint qwen-wrapper
```

```bash
cargo run -p adesh-daemon -- host qwen run \
  --task "Use Qwen CLI to review the wrapper component for Aadesh itself." \
  --file crates/adesh-daemon/src/host_cli.rs \
  --task-hint qwen-wrapper \
  -- --model qwen3-coder-plus
```

Qwen Code alias (same wrapper behavior):

```bash
cargo run -p adesh-daemon -- host qwen-code run \
  --task "Use Qwen Code CLI to review the wrapper component for Aadesh itself." \
  --file crates/adesh-daemon/src/host_cli.rs \
  --task-hint qwen-wrapper \
  -- --model qwen3-coder-plus
```

If the Qwen binary is not on `PATH`, set `ADESH_QWEN_BIN` to the executable path before running the wrapper.

OpenCode reference integration on top of the same host wrapper:

```bash
cargo run -p adesh-daemon -- host opencode run \
  --task "Use OpenCode CLI to build the wrapper component for Aadesh itself." \
  --file crates/adesh-daemon/src/host_cli.rs \
  --task-hint opencode-wrapper \
  -- --model opencode-large
```

If the OpenCode binary is not on `PATH`, set `ADESH_OPENCODE_BIN` to the executable path before running the wrapper.

Minimal MCP bridge on top of the same cognition core:

```bash
cargo run -p adesh-daemon -- host mcp-stdio
```

The active MCP bridge profile exposes only:
- `adesh.prepare_task_context`
- `adesh.store_work_episode`
- `adesh.recall_relevant_memory`
- `adesh.connector_event`

Smoke test the MCP bridge:

```bash
./scripts/mcp_cognition_smoke.sh
```

Generic connector event adapter (host-agnostic):

```bash
cargo run -p adesh-daemon -- host connector --json '{...}'
```

Event mapping:
- `task_start` => `prepare_task_context`
- `task_checkpoint` => `store_work_episode`
- `task_end` => `store_work_episode`

These are connector abstraction events, not claimed native lifecycle callbacks of any specific host.

Optional connector trace fields may be provided for future supervisory analysis:
`host_agent_id`, `host_agent_kind`, `host_model`, `context_id`,
`selected_next_direction`, `outcome`, `correction_summary`.

Connector smoke:

```bash
./scripts/connector_event_smoke.sh
```

Background coding-session watcher (auto-store episodes):

```bash
./scripts/session_learning_ctl.sh start \
  --task "Track my active Aadesh coding session and capture useful continuity memory." \
  --task-hint session-watcher
```

Check watcher status or stop it:

```bash
./scripts/session_learning_ctl.sh status --task-hint session-watcher
./scripts/session_learning_ctl.sh stop --task-hint session-watcher
```

Inspect latest watcher-captured episodes:

```bash
./scripts/session_learning_recent.sh --db-path /path/to/adesh.db --limit 10 --task-hint session-watcher
```

Capture richer post-task memory with low host friction:

```bash
./scripts/session_learning_capture.sh \
  --task "Harden retry path for upload worker" \
  --summary "Kept dedupe in service boundary; timeout benchmark still pending." \
  --task-hint retry-hardening \
  --decision "Keep duplicate protection in UploadService::Avoid transport-layer coupling" \
  --unresolved "Run timeout benchmark under packet loss" \
  --risk "Without timeout benchmark, reliability claims remain weak" \
  --test "pass::retry_backoff_unit::Backoff envelope remains within policy bounds"
```

Run local wedge evaluation harness:

```bash
./scripts/cognitive_eval_harness.sh
```

The harness outputs `/tmp/adesh-eval-<timestamp>/report.json` and checks:
- mean score improvement
- decision and open-loop recall improvement
- next-direction acceptance
- false-memory rate

Multi-repo Codex-extension usage (shared Aadesh DB, per-repo sessions):

```bash
export ADESH_DAEMON_ROOT="/home/rajan/Desktop/work/aadesh"
export ADESH_DATABASE_URL="sqlite:///home/rajan/.aadesh/cognition.db?mode=rwc"

"$ADESH_DAEMON_ROOT/scripts/session_learning_prepare.sh" \
  --task "What should I focus on next for this task?" \
  --task-hint codex-main \
  --auto-files

"$ADESH_DAEMON_ROOT/scripts/session_learning_ctl.sh" start \
  --task "Track this Codex coding session and capture continuity memory." \
  --task-hint codex-main

"$ADESH_DAEMON_ROOT/scripts/session_learning_capture.sh" \
  --task "Task I just finished" \
  --summary "What changed and why" \
  --task-hint codex-main \
  --decision "Key decision::Rationale"
```
