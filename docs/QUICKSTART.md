# Quickstart (Local Runtime Bring-Up)

Purpose:
- quickly start the daemon
- run basic local checks
- validate the active continuity slice

This document is an operator bring-up guide, not a product-scope authority.
Use `docs/ARCHITECTURE_STATUS.md` for active/deferred scope.

## 1) Prerequisites

- Rust toolchain
- `curl`
- `jq`

Optional for live local inference:
- LM Studio on `http://127.0.0.1:1234` with a loaded model

## 2) Configure environment

```bash
cp .env.example .env
```

Default `.env` demo profile uses fake providers for deterministic local runs.

Optional LM Studio settings:

```bash
ADESH_MODEL_PROVIDER_BACKEND=lm_studio
ADESH_MODEL_PROVIDER_MODEL=qwen3.5-27b
```

## 3) Start daemon

```bash
./scripts/demo_start.sh
```

## 4) Validate continuity path quickly

Before-task context:

```bash
cargo run -p adesh-daemon -- host prepare \
  --task "What should I focus on next for this workspace?"
```

After-task writeback:

```bash
cargo run -p adesh-daemon -- host store \
  --task "Local quickstart smoke task" \
  --summary "Validated bring-up and wrote one episode"
```

## 5) Optional runtime UI check

Open:

```text
http://127.0.0.1:7777
```

Treat this as runtime observability only. UI is not the current product center.

## 6) Regression checks

```bash
cargo test --workspace
./.codex/skills/adesh-spec-guard/scripts/check_spec_drift.sh .
```
