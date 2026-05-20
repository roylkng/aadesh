# Adesh OS Tech Stack (stack.md)

This document locks a **production-grade** technology strategy for Adesh OS across its full journey:
- Local-first daemon (Linux/macOS/Raspberry Pi)
- Optional server/enterprise deployment (single-tenant / VPC)
- Ecosystem integration (MCP tools + agent-to-agent bridges)

It avoids “POC thinking.” Implementation will be phased, but the **architecture and interfaces assume production constraints**: security, auditability, portability, and long-term maintainability.

Adesh OS “OS” means a governed runtime and control plane in user space, not a replacement for the host OS.

---

## 1) Product Topology and Surfaces

### Primary Control Plane (Admin + UX)
- **Localhost HTTP API** (REST) for CRUD and commands
- **WebSockets (or SSE)** for real-time events:
  - operation state transitions
  - approval requests (awaiting_approval)
  - syscall denials + remediation options
  - audit trace streaming

Rationale:
- Control-plane workflows are stateful (diff rendering, review queues, graph editing) and map naturally to HTTP/WS.
- This avoids forcing admin UI patterns into tool-call protocols.

### Integration Plane (Tool + Agent Ecosystem)
- **MCP Client** as the default mechanism to connect sensors and actuators
- **MCP Host** as a secondary bridge so external agent runtimes (OpenClaw, Claude Desktop, etc.) can delegate tasks to Adesh OS

Rationale:
- MCP is the de facto standard for tool integration.
- MCP is not the primary protocol for admin UI and lifecycle management.

---

## 2) Core Implementation Language and Runtime

### Core daemon (kernel) language
**Rust (recommended default)**  
- Async runtime: **Tokio**
- HTTP server: **axum**
- WebSockets: axum WS or `tokio-tungstenite`

Why Rust:
- memory safety and deterministic performance characteristics
- excellent ARM support (Raspberry Pi)
- ideal for security-critical enforcement code (governance + verification kernels)

### Alternative
**Go** remains a viable option if iteration speed becomes the primary constraint.

---

## 3) Architecture Rule: Kernel Ports and Swappable Providers

Adesh OS is built on a **ports-and-adapters** model:
- The kernel owns the **Batch 1–3 contracts** and core invariants.
- All external dependencies are behind **provider interfaces** so we can swap:
  - databases
  - queues
  - caches
  - blob storage
  - LLM backends
  - tool protocols/adapters

### Kernel Ports (provider interfaces)
These are the required provider categories:

1. **StorageProvider**
   - Experience Log (append-only)
   - Active State (versioned)
   - Audience Graph, hypotheses ledger, review queue
   - operations table + lifecycle transitions
   - audit trace persistence and replay pointers

2. **JobQueue**
   - enqueue/lease/ack/fail with backoff
   - at-least-once semantics
   - supports horizontal worker scaling in server profiles

3. **BlobStore**
   - content refs for attachments, tool outputs, IPC artifacts
   - sensitivity + taint metadata

4. **ModelProvider**
   - reasoning core adapter with strict structured output
   - supports local and cloud backends without kernel changes

5. **ToolProvider**
   - syscall execution against sensors/actuators
   - MCP-first, adapter-safe

6. **AuthProvider**
   - OwnerSession issuance/validation
   - OOB challenge/verify for R4 operations

7. **ObservabilityProvider**
   - structured logs, metrics, traces
   - correlation IDs always present: request_id, operation_id, syscall_id, audit_trace_id

Rule:
- If it is not expressible through these ports and the Batch contracts, it does not exist in the kernel.

---

## 4) Persistence Strategy (Production-Grade Journey)

Adesh OS supports multiple **deployment profiles** that select different provider implementations while keeping kernel semantics identical.

### Profile A: Local-first (default)
Goal: single-host, low ops, runs on Raspberry Pi and workstations.
- **SQLite (WAL)** as the default StorageProvider
- **Filesystem BlobStore** (optionally content-addressed), encrypted via OS volume or app-layer encryption
- **SQLite-backed JobQueue**
- Graphs (Audience Graph + primitive relations) stored as relational tables (nodes/edges/scopes)

Why:
- minimal operational burden
- strong portability
- deterministic behavior under constrained resources

### Profile B: Server / Single-tenant (enterprise-ready)
Goal: multi-device, multi-operator, centralized audit, scalable workers.
- **PostgreSQL** as StorageProvider (JSONB for flexible payloads)
- **PostgreSQL-backed JobQueue** (jobs table + leasing) or external queue if needed
- **Object storage** BlobStore (S3-compatible)
- optional cache (Redis) for hot paths, never as source of truth

### Profile C: Graph-accelerated (optional)
Goal: advanced graph traversal analytics while staying within Postgres.
- **Apache AGE on PostgreSQL** as an optional graph acceleration layer
- Not a safety dependency: the Audience Graph and core governance remain representable relationally.

Principle:
- Graph acceleration is optional. Governance correctness must not depend on a complex extension.

---

## 5) Graph Requirements Without a Dedicated Graph DB

Adesh OS has two graph-shaped datasets:
- **Audience Graph** (safety-critical, small, frequently consulted)
- **Primitive relationship graph** (contradictions, exceptions, provenance links)

Production approach:
- Model graphs as relational tables in all profiles:
  - nodes, edges, scopes, indices
- In server profiles, optionally provide:
  - AGE-powered traversal queries for analytics/debug
  - without making governance dependent on AGE availability

---

## 6) Queue and Eventing Strategy (No Micro-Infra Assumptions)

Adesh OS has two event domains:
- **Synchronous execution loop** (user-facing)
- **Asynchronous reflection loop** (background)

Production approach:
- Use a **durable JobQueue provider** with leasing and retries.
- Start with DB-backed queues (SQLite/Postgres) because they are portable and auditable.
- Introduce external queues (Redis/Kafka) only when the deployment profile requires distributed throughput.

Control plane events:
- WebSockets/SSE for UI and operator experience.
- All state transitions remain persisted (Experience Log + audit traces).

---

## 7) Observability (Production Baseline)

Minimum production baseline across all profiles:
- structured logs (JSON)
- metrics endpoint
- trace correlation via:
  - request_id
  - operation_id
  - syscall_id
  - audit_trace_id

Recommended stack (Rust):
- `tracing` + `tracing-subscriber` (JSON logs)
- Prometheus exporter (`metrics` + exporter)
- optional OpenTelemetry exporter in server profiles

Auditability:
- AuditTrace is a first-class artifact and must be replay-friendly using pinned versions.

---

## 8) Tool Integration Strategy (MCP-First)

### Sensors and Actuators
- MCP is the preferred protocol for tool integration.
- Non-MCP tools are supported via adapters but must conform to syscall semantics:
  - risk floors
  - sensitivity ceilings
  - taint propagation
  - policy-aware denials

### Agent-to-Agent Bridge
- MCP Host exposes OS functions to external agents.
- External agents map to `audience_id` nodes and are subject to Audience Graph and default-deny.

---

## 9) Reasoning Core Strategy (Provider-Agnostic)

ModelProvider must support:
- local models (Ollama, vLLM)
- cloud models (OpenAI/Anthropic/etc.)
- strict structured output constraints (plan + syscall proposals)

Implementation options:
- native Rust API clients for chosen providers
- optional sidecar adapter (LiteLLM) in some deployment profiles, but not required as a dependency

Principle:
- kernel never hardcodes a single LLM vendor.

---

## 10) Security and Integrity Requirements That Shape the Stack

The tech stack must support these invariants:
- deterministic governance independent of reasoning core
- taint-aware working memory and outputs
- operation isolation and IPC sensitivity inheritance
- policy-aware syscall denials (anti-retry trap)
- R4 self-modification requires OwnerSession + OOB
- encrypted persistence at rest
- replayable audit traces

This is why:
- a memory-safe kernel language (Rust) is preferred
- embedded/local-first storage (SQLite) is supported
- server-grade storage (Postgres) is supported
- MCP is used for tools but not for admin UI

---

## 11) Packaging and Distribution (Production Mindset)

### Local-first packaging
- single daemon binary (`agentosd`) per target architecture:
  - x86_64 Linux/macOS
  - ARM64 for Raspberry Pi
- UI served as static assets by daemon (no separate runtime required)
- optional companion CLI

### Server packaging
- container images for daemon and workers
- Postgres and object store as managed dependencies
- hardened auth and OOB integration

