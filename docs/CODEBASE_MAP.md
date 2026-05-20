# Codebase Map (Rust Workspace Traversal)

Status: navigation-only document.
Authority: non-authoritative. Scope status is controlled by `docs/ARCHITECTURE_STATUS.md`.

## 1) Active implementation surfaces

### Core runtime and cognition
- Workspace manifest: `Cargo.toml`
- Runtime entry: `crates/adesh-daemon/src/main.rs`
- Cognition core: `crates/adesh-daemon/src/cognition.rs`
- Host CLI wrapper: `crates/adesh-daemon/src/host_cli.rs`
- Connector event adapter: `crates/adesh-daemon/src/connector_adapter.rs`
- MCP stdio bridge: `crates/adesh-daemon/src/mcp_stdio.rs`

### Host wrappers
- Gemini wrapper: `crates/adesh-daemon/src/gemini_wrapper.rs`
- Qwen wrapper: `crates/adesh-daemon/src/qwen_wrapper.rs`
- OpenCode wrapper: `crates/adesh-daemon/src/opencode_wrapper.rs`
- Shared wrapper utilities: `crates/adesh-daemon/src/host_wrapper_common.rs`

### Storage and schema
- Storage trait: `crates/adesh-core/src/ports/storage.rs`
- SQLite implementation: `crates/adesh-storage-sqlite/src/storage.rs`
- Migrations: `crates/adesh-storage-sqlite/migrations/*.sql`

## 2) Active test surfaces

- Cognitive proof: `crates/adesh-daemon/tests/cognitive_proof.rs`
- Host CLI flows: `crates/adesh-daemon/tests/host_cli_flows.rs`
- Wrapper flows:
  - `crates/adesh-daemon/tests/gemini_wrapper_flows.rs`
  - `crates/adesh-daemon/tests/qwen_wrapper_flows.rs`
  - `crates/adesh-daemon/tests/opencode_wrapper_flows.rs`
- Connector behavior: connector adapter unit tests and `scripts/connector_event_smoke.sh`

## 3) Deferred surfaces still present in code

These modules exist but are not current phase drivers:
- HTTP/UI/control-plane path:
  - `crates/adesh-daemon/src/http/*`
- legacy kernel/governance stub path:
  - `crates/adesh-daemon/src/kernel.rs`
  - `crates/adesh-daemon/tests/governance_loop.rs`

Treat these as deferred unless a change is required for compileability or shared plumbing.

## 4) Edit routing by goal

- improve continuity quality (retrieval/ranking/promotion):
  - `crates/adesh-daemon/src/cognition.rs`
- improve host ingestion ergonomics:
  - `crates/adesh-daemon/src/host_cli.rs`
  - `crates/adesh-daemon/src/*_wrapper.rs`
- improve supervisory trace capture:
  - `crates/adesh-daemon/src/connector_adapter.rs`
  - storage trait and sqlite implementation
- schema evolution:
  - migration files + storage trait + sqlite storage

## 5) Quick validation commands

```bash
cargo test --workspace
./scripts/connector_event_smoke.sh
./scripts/supervisory_trace_simulation.sh --sessions 20
./scripts/supervisory_trace_complex_simulation.sh
./scripts/cognitive_eval_harness.sh
```
