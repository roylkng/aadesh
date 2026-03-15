# Local Demo Runbook

Purpose: run a reproducible local demo for Adesh OS control-plane behavior using safe defaults.

This runbook is product-generic. It does not change canonical behavior specs.

## Profiles

### Profile A: Deterministic local demo (recommended)
- model provider: `fake`
- tool providers: `fake`
- no external side effects

### Profile B: Live local model demo
- model provider: `lm_studio`
- model endpoint: `http://127.0.0.1:1234`
- tool providers still `fake` unless intentionally changed

## 1) Prepare configuration

```bash
cp .env.example .env
```

Adjust `.env` only if needed.

## 2) Start daemon

```bash
./scripts/demo_start.sh
```

## 3) Demo via UI

Open `http://127.0.0.1:7777`.

Suggested operator flow:
1. Submit a low-risk drafting request.
2. Show operation state and reasoning output.
3. Show pending approvals list (if any).
4. Approve one staged action and show syscall status.
5. Open audit trace and show persisted timeline.

## 4) Demo via CLI

Default non-side-effect check:

```bash
ADESH_ROOT_OWNER_TOKEN=demo-root-owner-token ./scripts/demo_smoke.sh
```

Approval/send check:

```bash
ADESH_ROOT_OWNER_TOKEN=demo-root-owner-token SMOKE_SCENARIO=send SMOKE_APPROVE_SEND=1 ./scripts/demo_smoke.sh
```

## 5) Expected outcomes

- Health endpoint returns `ok`.
- Request creates an operation and audit trace.
- Operation reaches `completed`, `awaiting_approval`, or `blocked` (fail-closed).
- If approval consumed, syscall status becomes `executed`.
- Audit trace timeline is non-empty.

## 6) Troubleshooting

- `401` or `403`: token mismatch (`Authorization: Bearer <token>`).
- `Dependency is unavailable` for model calls:
  - verify model endpoint and name
  - increase `ADESH_MODEL_PROVIDER_TIMEOUT_SECONDS` for large local models
- `blocked` state:
  - policy/capability gate denied action (expected fail-closed path)

## 7) Exit criteria for a clean demo

- At least one request lifecycle shown end-to-end.
- At least one audit trace displayed.
- No duplicate side effects under retries.
- No audit-fail-open behavior.
