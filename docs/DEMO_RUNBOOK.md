# Local Demo Runbook

Purpose: run a reproducible local demo of the current Aadesh runtime and continuity substrate.

This runbook is for demos and troubleshooting. It is not a scope-definition document.
Use `docs/ARCHITECTURE_STATUS.md` and `docs/IMPLEMENTATION_PLAN.md` for scope and priorities.

## Profiles

### Profile A: deterministic local demo (recommended)
- model provider: `fake`
- tool providers: `fake`
- no external side effects

### Profile B: live local model demo
- model provider: `lm_studio`
- endpoint: `http://127.0.0.1:1234`
- tool providers remain fake unless intentionally changed

## 1) Prepare configuration

```bash
cp .env.example .env
export ADESH_CARGO_TARGET_DIR=/tmp/adesh-cargo-target
```

## 2) Start daemon

```bash
./scripts/demo_start.sh
```

## 3) Continuity demo (recommended)

Before-task:

```bash
cargo run -p adesh-daemon -- host prepare \
  --task "What matters most right now for this task?" \
  --task-hint demo-continuity
```

After-task:

```bash
cargo run -p adesh-daemon -- host store \
  --task "Demo continuity task" \
  --summary "Captured decisions, outcomes, and open loops" \
  --task-hint demo-continuity
```

Connector event demo:

```bash
./scripts/connector_event_smoke.sh
```

Standard supervisory trace simulation:

```bash
./scripts/supervisory_trace_simulation.sh --sessions 20
```

This produces a deterministic report for two-workspace trace linkage, learnability, and
accepted/ignored/modified outcome coverage without requiring a live host model.

Complex realism simulation:

```bash
./scripts/supervisory_trace_complex_simulation.sh
```

This adds sparse payloads, conflicting/stale memory, duplicate replay, and one controlled
unlearnable stale-context event.

## 4) Optional control-plane/UI demo

Open `http://127.0.0.1:7777` and run a request lifecycle.

Note:
- control-plane flows are present in the repo, but broad governed-OS expansion is deferred in current product direction.

## 5) Expected outcomes

- daemon health is `ok`
- prepare/store lifecycle succeeds
- stored episode is queryable in follow-up prepare
- no audit-fail-open behavior

## 6) Troubleshooting

- `401`/`403`: token mismatch (`Authorization: Bearer <token>`)
- model dependency unavailable:
  - verify endpoint/model
  - raise timeout for local large models
- blocked state:
  - expected fail-closed behavior on denied actions

## 7) Exit criteria

- one full before-task + after-task continuity loop completed
- connector smoke completed
- no duplicate side effects under retries
