# Wedge v0 Local Runbook (Draft and Send)

This runbook validates the wedge v0 hot path:

request -> operation -> gate -> compile -> model -> verify -> approval -> syscall -> audit

Scope:
- Root Owner HTTP/WS only
- email draft and send only
- no workflow/interface runtime expansion

## 1) Prerequisites

- Rust toolchain
- `jq`
- LM Studio running on `http://127.0.0.1:1234` (optional; fake model backend also works)

## 2) Start daemon (safe default)

This keeps actuator execution non-destructive by using fake providers.

```bash
ADESH_ROOT_OWNER_TOKEN=test-token \
ADESH_BIND_ADDR=127.0.0.1:7777 \
ADESH_MODEL_PROVIDER_BACKEND=lm_studio \
ADESH_MODEL_PROVIDER_BASE_URL=http://127.0.0.1:1234 \
ADESH_MODEL_PROVIDER_MODEL=qwen3.5-27b \
ADESH_MODEL_PROVIDER_TIMEOUT_SECONDS=180 \
ADESH_EMAIL_PROVIDER_BACKEND=fake \
ADESH_WEBHOOK_PROVIDER_BACKEND=fake \
cargo run -p adesh-daemon
```

Notes:
- To avoid any real side effects, keep `ADESH_EMAIL_PROVIDER_BACKEND=fake`.
- HTTP binds to localhost by default.
- For larger local models, increase `ADESH_MODEL_PROVIDER_TIMEOUT_SECONDS` if first-token latency is high.

## 3) Run smoke flow (non-destructive by default)

From another shell:

```bash
ADESH_ROOT_OWNER_TOKEN=test-token \
BASE_URL=http://127.0.0.1:7777 \
./scripts/wedge_local_smoke.sh
```

Default behavior:
- submits send request
- verifies operation + pending approval
- stops before approval consumption

To execute full path through syscall (only safe with fake provider):

```bash
ADESH_ROOT_OWNER_TOKEN=test-token \
BASE_URL=http://127.0.0.1:7777 \
SMOKE_APPROVE_SEND=1 \
./scripts/wedge_local_smoke.sh
```

## 4) Expected outcomes

- Request creation returns `201` with operation and audit ids.
- Send request enters `awaiting_approval` unless blocked by policy.
- Approval consumption creates syscall pre-image and then executes.
- Audit trace contains timeline entries for gate/compile/reasoning/approval/syscall.

## 5) Fast troubleshooting

- `401/403`:
  - ensure `Authorization: Bearer <ADESH_ROOT_OWNER_TOKEN>` is correct
- `blocked` with `send_capability_unavailable`:
  - capability snapshot does not expose `email/send`
- `blocked` with `diff_unavailable_for_send`:
  - capability snapshot action has `diff_supported=false`
- model errors:
  - confirm LM Studio base URL/model values

## 6) Regression checks

Run full automated suite:

```bash
cargo test --workspace
```

Run spec drift guard:

```bash
bash .codex/skills/adesh-spec-guard/scripts/check_spec_drift.sh .
```
