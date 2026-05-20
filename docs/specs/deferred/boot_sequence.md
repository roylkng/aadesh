# Configuration, Profiles, and Boot Sequence Spec v0.1
Adesh OS

This document specifies how Adesh OS is configured, how runtime **profiles** are selected (desktop dev, raspberry pi, server), and the deterministic **boot sequence** that brings up the kernel, storage, tool integrations, and control plane.

It defines:
- configuration sources and precedence
- immutable vs mutable config
- profile composition and override rules
- boot order and failure handling
- safe defaults for security and governance
- hot-reload semantics (what can change at runtime)
- how pluggable backends (DB/queue/cache/model/tool) are swapped without changing kernel logic

This is algorithmic logic. Not implementation code.

---

## 0) Core principles

1. **Local-first but production-grade**
- The daemon must run on Linux/macOS workstations and Raspberry Pi.
- The same architecture must support server deployments with minimal changes.

2. **No hidden defaults**
- Every meaningful behavior must be expressible via config, with secure defaults.

3. **Immutable vs mutable**
- Some configuration is immutable at runtime (kernel safety posture).
- Some is mutable (capabilities enabled/disabled) but must be governed and audited.

4. **Profiles are compositions**
- Profiles define a coherent set of backend choices and budgets.
- Profiles can be overridden explicitly.

---

## 1) Configuration sources and precedence

Configuration can come from:
1. CLI flags (highest precedence)
2. Environment variables
3. Config file (TOML/YAML/JSON)
4. Built-in profile defaults (lowest precedence)

Precedence rule:
- For each field, choose the highest-precedence source that defines it.
- Unknown config keys are rejected (fail fast).

---

## 2) Configuration domains

### 2.1 Kernel governance configuration (mostly immutable)
- `policy_mode`: default/strict/lenient (default strict where risk exists)
- negative memory defaults (never_store, never_act, do_not_assume)
- max gate policies:
  - approval thresholds
  - OOB required gates
  - refuse categories
- replay permissions

Mutability:
- changes require R4 (self-modification class) with OOB by default.

### 2.2 Runtime budgets (mutable with constraints)
- default token budgets
- per-block budgets
- timeouts (model/tool/storage)
- concurrency limits (model calls, syscalls)

Mutability:
- R3 by default (can change safety posture)

### 2.3 Storage configuration
- storage backend: sqlite|postgres
- blob store: fs|s3|other
- retention policies:
  - idempotency keys
  - experience log compaction

Mutability:
- changes require restart unless explicitly hot-swappable
- switching DB backend mid-flight is not supported in v0.1 (migration required)

### 2.4 Queue configuration
- job queue backend: db_lease|redis|kafka|other
- worker counts and lease durations

Mutability:
- safe to change worker counts at runtime
- backend swap requires restart

### 2.5 ModelProvider configuration
- provider: local_ollama|vllm|openai|anthropic|vertex|other
- model_id mapping by gate (optional)
- deterministic sampling flags
- streaming enabled
- retry policies

Mutability:
- can change model_id mapping with R3 approval
- provider swap requires restart unless you can prove compatibility

### 2.6 MCP configuration
- MCP Client servers list (stdio/http)
- MCP Host enablement and bind address
- trust classes per MCP server/tool
- tool allowlist/denylist

Mutability:
- enabling/disabling tools is governed via capabilities flow, not config edits
- adding MCP server endpoints requires restart unless dynamic discovery enabled

### 2.7 Control plane configuration
- bind address for HTTP/WS
- TLS options (if non-localhost)
- auth mode for Root Owner (local token, OS keychain, etc.)

Mutability:
- bind changes require restart

---

## 3) Profiles

Profiles are named bundles of defaults.

### 3.1 Mandatory profiles (initial)
1. `workstation_dev`
2. `raspi_local`
3. `server_single_node`
4. `server_multi_agent` (optional later)

### 3.2 Profile composition fields
A profile must specify:
- storage backend and recommended settings
- queue backend and worker counts
- default budgets and concurrency
- model provider defaults
- MCP defaults (host/client)
- observability mode

### 3.3 Override rules
- Any field can be overridden via higher-precedence config sources.
- Overrides must be audited if they change governance posture.

---

## 4) Boot sequence (deterministic order)

Adesh OS boot must follow this order:

### Step 1: Load config
- Resolve config from sources and precedence.
- Validate against config schema.
- If invalid: fail fast before starting any servers.

### Step 2: Initialize observability
- Logging, metrics, tracing initialized first.
- Attach build/version info.

### Step 3: Initialize storage backend
- Connect to SQLite/Postgres.
- Run migrations (if enabled).
- Validate required tables exist.
- If storage unavailable: fail fast. The OS cannot run without storage.

### Step 4: Initialize blob store
- Ensure blob root exists (fs) or bucket reachable (s3).
- If blob store unavailable: fail fast if the OS is configured to require it.

### Step 5: Load/initialize bootstrap state
- Ensure Root Owner node exists.
- Ensure initial audience graph version exists.
- Ensure initial active_state_version exists.
- Ensure initial capability snapshot exists (may be empty).
If any bootstrap element missing:
- create it and append Experience Log bootstrap event
- mint versions and store

### Step 6: Initialize job queue and workers (reflection)
- Start job queue backend.
- Start reflection workers (can be 0 in minimal runs, but the queue should exist).
- Workers must not begin processing until Step 8 completes (avoid races).

### Step 7: Initialize MCP Client (tool discovery)
- Connect to configured MCP servers.
- Discover tools and schemas.
- Create capability snapshot version.
- Mark unavailable servers as degraded.
- If all required tools are missing:
  - continue boot but capability snapshot indicates degraded state
  - do not fail unless policy demands specific tools

### Step 8: Initialize Scheduler/Runner
- Start scheduler loop.
- Scheduler begins leasing runnable operations.

### Step 9: Start HTTP/WS control plane
- Bind on configured address.
- Expose REST endpoints.
- Start WS event bus.

### Step 10: Start MCP Host (integration plane)
- If enabled, start MCP Host.
- Map external clients to agent_client audience nodes.

### Step 11: Mark system ready
- Emit readiness metric and log
- `GET /v1/health` returns ok or degraded

---

## 5) Failure handling policy during boot

### 5.1 Fail-fast components (hard dependencies)
If any of these fail:
- config validation
- storage initialization
- bootstrap state initialization

Then boot must abort.

### 5.2 Degradable components (soft dependencies)
If any of these fail:
- model provider initialization
- MCP discovery
- optional MCP host
- reflection workers

Then boot may continue but:
- health is `degraded`
- capabilities reflect the degraded tool availability
- scheduler may still accept R0/R1 local operations depending on policy

### 5.3 Degradation reporting
System must:
- log degraded reason codes
- expose them in `/v1/health`
- emit metrics `*_up` gauges accordingly

---

## 6) Hot reload and runtime mutation rules

### 6.1 Hot-reload allowed
- concurrency limits (within safe bounds)
- model_id mapping (R3 gated)
- enabling/disabling tools (governed via capabilities endpoints)
- adding audience graph nodes/edges/scopes (governed via graph patch endpoints)
- budgets (R3 gated)

### 6.2 Hot-reload forbidden (restart required)
- switching storage backend
- switching queue backend
- changing HTTP bind address
- changing core governance kernel posture (default requires R4 with OOB and restart)

### 6.3 Audit requirements for hot changes
Any hot change that affects:
- gates
- ceilings
- tool enablement
- budgets
must be recorded as:
- Experience Log event
- AuditTrace anchor (if tied to an operation)
- capability snapshot or graph version mint

---

## 7) Pluggability contract (swap without kernel change)

Backends must be behind strict interfaces:
- StorageProvider
- BlobStore
- JobQueue
- ModelProvider
- ToolProvider (MCP Client/Host adapters)

Rules:
- Kernel never imports vendor-specific code.
- Provider implementations must satisfy storage semantics and transaction boundary specs.

---

## 8) Minimum test cases (must pass)

1. Boot on raspi_local profile with no MCP tools:
- system starts degraded, control plane reachable, no crashes.

2. Boot with missing storage:
- fail fast, no partial servers started.

3. Boot creates Root Owner bootstrap node:
- audience graph version minted and persisted.

4. Hot tool enablement:
- mint new capability snapshot version and emit WS capability_update.

5. Restart with same DB:
- retains versions and continues from prior state.
