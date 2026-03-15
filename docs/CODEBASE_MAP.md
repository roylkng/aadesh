# Codebase Map (Rust Workspace Traversal)

Status: Navigation-only document.
Authority: Non-authoritative. Behavioral truth remains in canonical specs.

This map is for fast code traversal by humans and coding agents.

## 1) Workspace entry points

- Workspace manifest: `Cargo.toml`
- Runtime binary entry: `crates/adesh-daemon/src/main.rs`
- App wiring (router + providers): `crates/adesh-daemon/src/http/mod.rs`
- Cognitive proof service: `crates/adesh-daemon/src/cognition.rs`
- Host-friendly cognition wrapper: `crates/adesh-daemon/src/host_cli.rs`
- Gemini CLI integration wrapper: `crates/adesh-daemon/src/gemini_wrapper.rs`
- Qwen CLI integration wrapper: `crates/adesh-daemon/src/qwen_wrapper.rs`

## 2) Crate responsibilities

### `crates/adesh-contracts`
- Shared API contracts and response envelopes used by HTTP/storage layers.
- Start: `crates/adesh-contracts/src/lib.rs`

### `crates/adesh-core`
- Core config/error types.
- Port traits for storage/model/tool providers.
- Action/schema normalization and validation helpers.
- Starts:
  - `crates/adesh-core/src/lib.rs`
  - `crates/adesh-core/src/config.rs`
  - `crates/adesh-core/src/error.rs`
  - `crates/adesh-core/src/ports/storage.rs`
  - `crates/adesh-core/src/ports/model.rs`
  - `crates/adesh-core/src/ports/tool.rs`
  - `crates/adesh-core/src/action_schemas.rs`

### `crates/adesh-storage-sqlite`
- SQLite implementation of `StorageProvider`.
- Includes migrations and bootstrap seed logic.
- Starts:
  - `crates/adesh-storage-sqlite/src/lib.rs`
  - `crates/adesh-storage-sqlite/src/storage.rs`
  - `crates/adesh-storage-sqlite/migrations/*.sql`

### `crates/adesh-daemon`
- HTTP/WS surface, request flow orchestration, kernel stub, provider adapters, and tests.
- Also hosts the current cognitive-sidecar proof path and CLI entrypoint.
- Starts:
  - `crates/adesh-daemon/src/main.rs`
  - `crates/adesh-daemon/src/cognition.rs`
  - `crates/adesh-daemon/src/host_cli.rs`
  - `crates/adesh-daemon/src/http/mod.rs`
  - `crates/adesh-daemon/src/http/routes.rs`
  - `crates/adesh-daemon/src/http/ws.rs`
  - `crates/adesh-daemon/src/http/auth.rs`
  - `crates/adesh-daemon/src/http/ui.rs`
  - `crates/adesh-daemon/src/kernel.rs`
  - `crates/adesh-daemon/src/modeling.rs`
  - `crates/adesh-daemon/src/tooling.rs`

## 3) Runtime call path (high-level)

1. `main.rs`
2. either `cognitive::*` in `cognition.rs` or `http::app(...)` in `http/mod.rs`
   or `host_cli::*` in `host_cli.rs`
3. route handlers in `http/routes.rs`
4. governance/compile stub in `kernel.rs`
5. model provider in `modeling.rs`
6. tool provider in `tooling.rs`
7. persistence in `adesh-storage-sqlite::storage.rs`
8. WS notifications via broadcast sender in HTTP state

## 4) Where to edit by concern

### Add or change endpoint behavior
- Router wiring: `crates/adesh-daemon/src/http/mod.rs`
- Handler logic: `crates/adesh-daemon/src/http/routes.rs`
- Auth policy: `crates/adesh-daemon/src/http/auth.rs`
- WS behavior: `crates/adesh-daemon/src/http/ws.rs`

### Add or change cognitive proof behavior
- CLI entry and dispatch: `crates/adesh-daemon/src/main.rs`
- Host-facing wrapper parsing and workspace auto-detection: `crates/adesh-daemon/src/host_cli.rs`
- Workspace resolution, promotion, retrieval, ranking: `crates/adesh-daemon/src/cognition.rs`
- Storage contract: `crates/adesh-core/src/ports/storage.rs`
- SQLite implementation: `crates/adesh-storage-sqlite/src/storage.rs`
- DB schema: `crates/adesh-storage-sqlite/migrations/0010_cognition_memory.sql`

### Add or change storage behavior
- Trait contract: `crates/adesh-core/src/ports/storage.rs`
- Implementation: `crates/adesh-storage-sqlite/src/storage.rs`
- DB schema: `crates/adesh-storage-sqlite/migrations/*.sql`

### Add or change model integration
- Trait: `crates/adesh-core/src/ports/model.rs`
- Implementations: `crates/adesh-daemon/src/modeling.rs`

### Add or change tool/action execution
- Trait: `crates/adesh-core/src/ports/tool.rs`
- Implementations/routing: `crates/adesh-daemon/src/tooling.rs`
- Action schema helpers: `crates/adesh-core/src/action_schemas.rs`
- Bootstrap action/schema files: `registry/bootstrap/**`

### Add or change governance/verification stub behavior
- `crates/adesh-daemon/src/kernel.rs`
- (Ensure canonical specs are updated first when behavior changes)

## 5) Test map

- HTTP/auth/smoke: `crates/adesh-daemon/tests/smoke.rs`
- Idempotency: `crates/adesh-daemon/tests/request_idempotency.rs`
- Lease exclusivity: `crates/adesh-daemon/tests/lease_exclusivity.rs`
- Audit fail-closed: `crates/adesh-daemon/tests/audit_fail_closed.rs`
- Governance loop: `crates/adesh-daemon/tests/governance_loop.rs`
- Approval consumption and diff flow: `crates/adesh-daemon/tests/approval_consumption.rs`
- Replay dry run: `crates/adesh-daemon/tests/replay_dry_run.rs`
- WS events: `crates/adesh-daemon/tests/ws_events.rs`
- Registry/capability mutation: `crates/adesh-daemon/tests/registry_mutation.rs`
- Wedge extension coverage (manual artifacts, OOB, wedge metrics, capability/diff fallback): `crates/adesh-daemon/tests/wedge_extensions.rs`
- Cognitive proof slice: `crates/adesh-daemon/tests/cognitive_proof.rs`
- Host-wrapper flows: `crates/adesh-daemon/tests/host_cli_flows.rs`
- Gemini-wrapper flows: `crates/adesh-daemon/tests/gemini_wrapper_flows.rs`
- Qwen-wrapper flows: `crates/adesh-daemon/tests/qwen_wrapper_flows.rs`

## 6) Quick commands

```bash
cargo test --workspace
./.codex/skills/adesh-spec-guard/scripts/check_spec_drift.sh
./scripts/demo_start.sh
ADESH_ROOT_OWNER_TOKEN=demo-root-owner-token ./scripts/demo_smoke.sh
```
