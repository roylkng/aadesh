# Connector Integration v0 (One-Click Host Ecosystem Strategy)

Status: implementation guidance for host integrations.
Authority: non-canonical; behavior remains defined by active specs and Rust contracts.

## 1) Why connector-first

Aadesh should not hard-code to one host (Codex, Gemini CLI, Qwen CLI, etc.).

Use:
- one cognition core
- one normalized connector event contract
- many thin connectors

This allows onboarding many host agents without changing cognition logic.

## 2) Normalized connector lifecycle

Connector events:
- `task_start`
- `task_checkpoint`
- `task_end`

Important:
- these are Aadesh connector abstraction events
- they are not claimed as native lifecycle callbacks provided by VS Code

Mapping:
- `task_start` calls `prepare_task_context`
- `task_checkpoint` and `task_end` call `store_work_episode`

This gives one reusable integration pattern for:
- chat extensions
- terminal CLIs
- IDE plugins
- app SDKs

## 3) Connector event contract (daemon side)

CLI surface:

```bash
cargo run -p adesh-daemon -- host connector --json '<connector_event_payload>'
```

MCP surface:
- tool name: `adesh.connector_event`
- same payload semantics as the CLI connector event contract

Response shape:
- `handled_as = prepare_task_context` with `prepare_context`
- or `handled_as = store_work_episode` with `stored_episode`
- optional warnings for degraded payloads

See `ConnectorEventRequest` and `ConnectorEventResponse` in `crates/adesh-contracts/src/lib.rs`.

## 4) Minimum host payload

Required:
- `connector_id`
- `connector_kind`
- `event_kind`
- `workspace`
- `task_prompt`

Strongly recommended for better quality:
- `task_hint`
- `files_in_focus` (for `task_start`)
- `summary`, `decisions`, `unresolved_items`, `risk_signals`, `tests` (for checkpoint/end)

Optional supervisory trace fields (recommended for future agent-governor work):
- `host_agent_id`
- `host_agent_kind`
- `host_model`
- `context_id`
- `selected_next_direction`
- `outcome`
- `correction_summary`

These remain optional in v0 and do not change current cognition output shape. They are stored as
trace artifact references so future supervisory logic can learn from agent behavior over time.

## 5) Degraded-mode behavior

If connector payload is sparse:
- `task_end/checkpoint` without summary is still stored with fallback summary
- `files_touched` can fall back to `files_in_focus`
- warnings are returned in connector response

This keeps ingestion robust across heterogeneous host clients.

## 6) One-click product path

For VS Code/chat-extension style hosts, one-click onboarding should do:
1. register MCP/CLI connector once at global host settings level
2. in a chat participant/request handler, map request flow to connector events
3. via extension tools or MCP tools, invoke `adesh.connector_event` with normalized payloads
4. auto-send `task_start` and `task_end/task_checkpoint` from extension-defined checkpoints
5. keep user-facing UX to one enable toggle

No per-repo manual command should be required in steady state.

## 7) Local validation

```bash
./scripts/connector_event_smoke.sh
```

This smoke validates normalized connector mapping without host-specific wrappers.

Standard deterministic supervisory trace simulation:

```bash
./scripts/supervisory_trace_simulation.sh --sessions 20
```

This is the repeatable test for real-use mechanics. It uses the public connector event path,
seeds two workspaces, captures the `context_id` returned by `task_start`, writes
`accepted|ignored|modified` outcomes on `task_end`, and reports linked/learnable trace quality.
Use `--sessions 50` when checking the linked-outcome volume portion of the post-Phase-D gate.

Complex deterministic supervisory trace simulation:

```bash
./scripts/supervisory_trace_complex_simulation.sh
```

This is the stronger realism test. It covers multiple workspaces and workstreams, stale/conflicting
memory, sparse host payloads, failing and passing evidence, ignored/modified directions, duplicate
event replay, and one intentionally stale context that must persist as observability data but remain
excluded from learning.

Real-host supervisory trace observability run:

```bash
./scripts/supervisory_trace_real_runs.sh
```

This optional run invokes Qwen CLI per episode, persists connector trace fields, and emits a JSON
report with accepted-vs-ignored direction outcomes plus field signal/noise assessment. It is useful
for live-host evidence, but it is not the deterministic standard test.
