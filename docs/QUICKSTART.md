# Quickstart (Local Demo)

This quickstart is for first-time users who want to run Adesh OS locally and walk through a governed request end-to-end.

## 1) Prerequisites

- Rust toolchain
- `curl`
- `jq`

Optional for live local inference:
- LM Studio on `http://127.0.0.1:1234` with a loaded model (for example `qwen3.5-27b`)

## 2) Configure environment

Create local config:

```bash
cp .env.example .env
```

Default profile in `.env.example` uses:
- `ADESH_MODEL_PROVIDER_BACKEND=fake` (deterministic local demo)
- fake email/webhook providers (non-destructive)

Switch to LM Studio by setting:

```bash
ADESH_MODEL_PROVIDER_BACKEND=lm_studio
ADESH_MODEL_PROVIDER_MODEL=qwen3.5-27b
```

## 3) Start daemon

```bash
./scripts/demo_start.sh
```

## 4) Use the UI

Open:

```text
http://127.0.0.1:7777
```

Journey:
1. Set Root Owner token (defaults to `demo-root-owner-token` if unchanged).
2. Submit a request.
3. Inspect operation state, reasoning output, and syscalls.
4. If approval is required, approve or deny.
5. Verify audit trace is populated.

## 5) Run CLI smoke checks

Non-side-effect draft flow (default):

```bash
ADESH_ROOT_OWNER_TOKEN=demo-root-owner-token ./scripts/demo_smoke.sh
```

Approval/send flow:

```bash
ADESH_ROOT_OWNER_TOKEN=demo-root-owner-token SMOKE_SCENARIO=send SMOKE_APPROVE_SEND=1 ./scripts/demo_smoke.sh
```

## 6) Regression checks

```bash
cargo test --workspace
bash .codex/skills/adesh-spec-guard/scripts/check_spec_drift.sh .
```
