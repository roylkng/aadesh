# Problem Statement Reference

Non-authoritative background document. Canonical implementation behavior is defined by the current spec set, not by this framing document.

## Problem Statement: The Governed Agent OS (v2.5 Inclusive Spec)

### Objective

Design and build an operating system for an artificial being that hosts a plug-in reasoning core (LLM or other planner) and composes it with modular sensors and actuators. The OS must produce coherent, evolving behavior by maintaining an immutable experience log, a governed active state, and a learned entity model consisting of:

* **Identity Model**: continuity, commitments, long-running goals and relationships.
* **Policy Profile**: values, boundaries, disclosure rules, negative memory.
* **Operational Profile**: output formatting, verbosity, escalation patterns, tool-use habits.
* **Capability Self-Model**: what the being can perceive and do under constraints and budgets.

The OS must start from zero or any corpus size, ingest free-form experience in any format, and improve task-level alignment over time. All perception, memory reads, and actions must be mediated by deterministic governance independent of the reasoning core, using dual-axis gating (Action Risk and Data Sensitivity), audience-conditional disclosure, operation isolation, token/context budget enforcement, auditable approval workflows, explicit inter-operation data transfer (IPC) with sensitivity inheritance, taint-aware working memory, and plan-trajectory alignment verification.

The OS must run as a cross-platform user-space “kernel” on **Linux, macOS, and Raspberry Pi-class devices**, with an implementation path to mobile devices. “OS” here means a governed runtime and control plane, not a replacement for the host OS.

---

## 0. Platforms, Deployment Targets, and Diagnostics First

### Supported Targets

* Primary: Linux (developer workstation), Raspberry Pi (edge appliance), macOS (developer workstation).
* Secondary: mobile devices via a constrained runtime and remote tool delegation.

### Deployment Form Factors

* Local-first daemon/service with optional UI.
* Optional “embedded mode” as a library for constrained environments.
* Optional cloud/VPC deployment for multi-device sync (not required for correctness).

### Diagnostics and Operability (First-Class)

The OS must be diagnosable by design:

* structured logs, traces, and metrics for all syscalls and compilation steps
* audit traces that explain “why” (gates, policy hits, omissions, redactions)
* replay capability for incident analysis and regression testing

---

## 1. Bootstrap, Ownership, Identity Binding, and Versioning

### Root Owner Node

* The OS initializes with an irremovable Root Owner principal.
* The authenticated creator is cryptographically bound to Root Owner.

### Owner Session

* Privileged operations require an explicit Owner Session (strong authentication state).
* Out-of-band authorization is required for R4-class operations.

### Default-Deny Bootstrap Exception

* Default-deny applies globally except for Root Owner’s ability to access and govern their own data, still subject to high-risk controls.

### State Versioning and Deterministic Compilation

* Active state is versioned. Each update creates a new immutable version (transaction snapshot).
* JIT compilation pins and records the exact active-state version used for an operation.
* Concurrent reflection updates cannot mutate a compiled slice mid-operation.

---

## 2. Embodiment Kernel: Sensors, Actuators, Budgets, and Self-Model

### Sensors (Perception Interfaces)

The OS supports pluggable sensors producing observations.

* Examples: inbox streams, filesystem, calendars, microphones/cameras (optional), web fetch, telemetry, application logs.
* Each sensor must declare:

  * modality and observation schema
  * trust and reliability class
  * maximum sensitivity ceiling it may emit
  * default audience scoping rules
  * access permissions and audit requirements
  * rate limits and cost signals (if applicable)

### Actuators (Action Interfaces)

The OS supports pluggable actuators producing external side effects.

* Examples: send message, schedule meeting, execute code, deploy, purchase, update records.
* Each actuator must declare:

  * action schema and parameters
  * irreversibility profile
  * minimum action-risk floor (cannot be downgraded by prompting)
  * diff capability (what can be previewed)
  * required approval mode (none, confirm, diff+approve, out-of-band)
  * safety constraints and rate limits
  * observability contract (trace and outcome fields)

### Interface Standardization (MCP-First, Adapter-Safe)

* Sensors and actuators must be exposable via **Model Context Protocol (MCP)**.
* The OS must provide an MCP host/client implementation.
* Non-MCP integrations may exist via adapters, but they must conform to the same syscall semantics, audit schema, taint propagation, and governance gates.

### The Ultimate Actuator: Self-Modification (R4)

Self-modification is first-class and must be treated as R4:

* modifying governance rules, risk predicates, approval policies
* changing audience graph defaults or scopes
* altering write-gates, negative memory, promotion precedence
* modifying core OS scripts, system prompts, tool permission maps
  Rules:
* reasoning core may propose self-modification but cannot apply it
* application requires Root Owner session plus OOB authorization
* diff must be shown and rollback must be available

### Budgets and Constraints

The OS enforces budgets across:

* time/latency
* monetary cost
* compute usage
* risk exposure per time window
* rate limits (per sensor/actuator)
* **token/context window allocation per operation and per memory block**

#### Token/Context Window Allocation

* Each operation has a token budget for all injected context and instructions.
* The compiler enforces deterministic packing and hard-stops on overflow.
* Governance constraints must never be truncated.
* Omissions must be recorded in audit traces.

### Capability Self-Model

The OS maintains a self-model describing:

* available sensors/actuators and their current status
* what can and cannot be inferred given sensor coverage
* uncertainty expectations based on trust/reliability
* operational constraints, budgets, and policies

---

## 3. Universal Ingestion and the Immutable Experience Log

### Experience Log

* Ingest all free-form inputs and observations into an append-only, immutable Experience Log.
* Includes:

  * documents, transcripts, chats, emails, notes, tool traces
  * sensor observations
  * user feedback signals (accept/edit/reject)
  * owner approvals and policy edits
  * runtime telemetry (latency, failures, stdout/stderr where applicable)
* Normalize into canonical events with metadata:

  * source class, timestamp (or unknown), authorship, audience, sensitivity hint, reliability hint

### Provenance and Evidence Quality

All promoted state must reference evidence in the Experience Log.
Evidence quality fields:

* source reliability score
* recency
* conflict count
* user-confirmed flag
* source class

Promotion precedence:

* user-confirmed
* repeated observation across independent events and contexts
* single high-trust artifact
* untrusted external (candidate only)

---

## 4. Learned Entity Model: Identity, Policy Profile, Operational Profile

The OS maintains an Active Entity State derived from the Experience Log, never exposing the full log to the reasoning core.

### Identity Model

* commitments ledger (promises, deadlines, obligations)
* long-running goals and threads
* stable identifiers and relationship bindings

### Policy Profile

* values and boundaries
* negative memory rules (never store, never act, do not assume, forget/expire)
* disclosure constraints (audience graph scopes)
* escalation rules for high-risk and high-sensitivity operations

### Operational Profile

* output formatting preferences (bullets, diffs, templates)
* verbosity and structure constraints
* format-switching by audience and context
* tool-use habits and escalation patterns

### Hypothesis-Driven Primitives

All derived primitives are:

* hypotheses with confidence and stability tiers
* conditional on context predicates and time bounds
* backed by hard or statistical provenance

### Opportunity-Aware Evolution

* confidence updates are driven by reinforcement opportunities and negative evidence, not wall-clock time alone
* rarely-invoked but valid preferences persist until relevant contexts occur again

---

## 5. Audience Graph and Disclosure Control

### Audience Graph

* Nodes: entities, groups, roles, channels, public, Root Owner
* Edges: relationship definitions and trust levels
* Scopes: allowed disclosure topics and sensitivity ceilings per edge
* Default deny for unknown nodes, edges, or scopes (except Root Owner bootstrap)

### Audience-Conditional Disclosure

* All memory reads, compiled slices, IPC transfers, and outputs are filtered by audience scope before being returned to reasoning core or sent externally.

---

## 6. Dual-Axis Governance Using Universal Predicates

Governance is computed via universal predicates derived from the operation, audience, data, and actuator intent.

### Axis A: Data Sensitivity (S0–S4)

* S0 Public
* S1 Internal/Routine
* S2 Confidential
* S3 Restricted
* S4 Regulated/Critical

### Axis B: Action Risk (R0–R4)

* R0 Passive/Transformative
* R1 Low-Stakes Generative
* R2 Medium-Stakes Active
* R3 High-Stakes Execution
* R4 Critical/Irreversible (includes self-modification)

### Universal Risk Predicates

Examples:

* sends_information_to_third_party
* has_external_side_effect
* touches_money_or_accounts
* touches_identity_or_security
* touches_health_or_legal
* is_irreversible_or_mass_impact
* reveals_sensitive_data
* depends_on_unconfirmed_hypotheses
* audience_is_unknown_or_out_of_scope
* requires_access_to_raw_log
* attempts_self_modification

### Per-Operation Decomposition and Gating

* Decompose each request into one or more operations.
* Compute (R, S) per operation, not per message.
* Gate per operation is max(R, S).
* Multi-operation requests must not downgrade gates via bundling.

---

## 7. JIT Entity Compilation, Working Memory, Taint, and Token Packing

### JIT Compilation

For each operation, the OS compiles a minimal “entity slice”:

* relevant identity/policy/operational primitives (filtered by audience, confidence, gate)
* capability self-model slice (available sensors/actuators, budgets, constraints)
* approved evidence snippets (as needed)
  Reasoning core never receives the full experience log.

### Working Memory Blocks

OS-managed memory blocks with explicit token budgets:

* Policy Block (non-truncatable)
* Capability Block
* Operation Context Block
* Evidence Block
* Scratch Block (ephemeral, low-trust, auto-expiring)

### Taint-Aware Memory Tracking (Cognitive Integrity)

* Any memory block that ingests Sx data becomes tainted at Sx for the lifespan of the operation.
* Derived artifacts inherit the maximum taint of their inputs unless they pass an explicit, audited sanitization syscall.
* Outputs must satisfy both Audience Graph ceilings and output sensitivity limits. This prevents laundering S3/S4 influence into S0 outputs.

### Token Packing and Truncation Rules

Deterministic packing within budgets. Priority order:

1. governance constraints (gates, negative memory, refusal rules)
2. capability self-model essentials (available sensors/actuators, budgets, permissions)
3. high-severity policy/identity constraints relevant to the operation
4. operational profile constraints relevant to the audience and context
5. minimal evidence required for grounding
6. optional supporting context (only if budget remains)
   If overflow:

* omit lowest-priority items first
* record omissions
* never truncate governance constraints

### Deterministic Conflict Resolution

When primitives conflict, resolve in strict order:

1. applicability (predicate match)
2. constraint severity (policy profile and negative memory dominate)
3. explicit user-confirmed exceptions that match context may override broader non-safety boundaries
4. evidence quality tier (user-confirmed > repeated obs > single artifact)
5. predicate specificity
6. recency
   If unresolved and gate ≥ 2, require clarification rather than guessing.

---

## 8. Operation Isolation and Inter-Process Communication (IPC)

### Operation Isolation

* Each operation has isolated compilation and generation contexts.
* High-sensitivity contexts cannot contaminate lower-sensitivity generations unless explicitly linked and gated.

### IPC and Explicit Piping

* OS supports explicit, gated IPC (“piping”) between operations.
* IPC transfers only explicit serialized artifacts (not implicit context windows).
* Piped artifacts carry sensitivity labels and provenance pointers.
* Receiving operation inherits maximum sensitivity of piped artifacts and recompiles under that gate.
* IPC must respect Audience Graph scopes and is recorded in Experience Log.

---

## 9. OS Execution Model: Operations, Syscalls, Scheduling, Preemption

### Operations as Processes

* Each operation is a process with lifecycle states:

  * created, compiled, awaiting_approval, running, blocked, completed, failed, cancelled

### Syscalls (Mediated)

* Sensor reads, memory reads, IPC transfers, and actuator invocations are syscalls.
* All syscalls pass governance checks (max(R, S), audience scopes, budgets, taint rules).
* Syscalls emit structured traces into Experience Log.

#### Policy-Aware Syscall Rejections (Anti-Retry Trap)

Blocked syscalls must return structured rejection payloads:

* violated constraint identifiers (policy, scope, gate)
* triggering fields/data classes and sensitivity
* whether retry is permitted and under what modifications
* permitted remediation paths (ask user, sanitize, alternate actuator, require OOB, refuse)
  This prevents infinite retries and hallucinated substitutions.

### Scheduling and Cancellation

* Provide cancellation semantics (owner-cancel and system-cancel), timeouts, and auditable preemption rules.

### Deterministic Execution Under Concurrency

* Each operation pins:

  * active-state version
  * capability snapshot
  * token budgets
* No mid-operation slice mutation.

---

## 10. Core Separation: Reasoning, Governance, Verification

### Reasoning Core

* Generates plans, drafts, and candidate actions.
* Operates only on compiled slices and syscall results.

### Governance Kernel

* Computes risk/sensitivity, enforces audience scopes, budgets, taint, isolation, approvals.
* Mediates all syscalls and context injection.

### Verification Core

Validates candidate plans and outputs against:

* gate requirements (confirm/diff/OOB)
* audience scope and disclosure ceilings
* negative memory constraints
* self-modification constraints
* token budget compliance and omission rules
* IPC sensitivity inheritance and taint rules

#### Plan-Trajectory Alignment (Intent Drift Defense)

* Each operation has an **Intent Anchor** representing the Root Owner’s original objective, constraints, and forbidden outcomes.
* Verification continuously checks that intermediate steps and evolving subgoals remain aligned with the Intent Anchor.
* Any semantic drift that increases scope, risk, or sensitivity requires explicit user confirmation or is refused.

Verification must be deterministic-first, with optional model-based checks as enhancements.

---

## 11. Security Threat Model and Required Invariants

### Threat Model

* prompt injection from untrusted content
* memory poisoning via interactions or crafted artifacts
* tool misuse and privilege escalation
* data exfiltration via outputs and cross-operation contamination
* self-modification attacks targeting governance and policy
* plan drift attacks over multi-step trajectories

### Required Invariants

* untrusted content cannot directly promote into core state
* promotions are governed, auditable, revertible
* default deny enforced in code
* isolation and taint enforced at compilation and runtime
* self-modification always R4 with Owner Session + OOB
* negative memory rules absolute unless Root Owner modifies via R4
* IPC explicit, gated, audited, sensitivity-inheriting

### Continuous Security and Red-Teaming

* adversarial prompt suites, poisoning simulations, regression tests for gates/IPC/taint/isolation, audit replay.

---

## 12. Execution Loop vs Reflection Loop

### Synchronous Execution Loop (Low-Latency)

Includes only:

* decomposition
* per-operation compilation (read-only, pinned)
* governed reasoning and output generation
* governed syscalls with confirmation/diff/OOB
* append experience records and traces

### Asynchronous Reflection Loop

* hypothesis extraction, evidence scoring, conflict linking
* opportunity-aware evolution
* promotion candidates and owner review queue
* consolidation and compaction

Owner confirmations/corrections may apply synchronously.

---

## Non-Goals

* not consciousness or subjective inner experience
* no inference of protected traits or diagnoses
* no historical accuracy guarantee beyond artifacts
* no untrusted rewrite of core state without governance
* no commitment to a specific DB architecture until primitives are modeled

---

## Evaluation Metrics

### Alignment and Usability

* acceptance rate, normalized edit distance, turns-to-accept (per scenario family)

### Safety and Correctness

* overreach rate
* unjustified drift rate (target 0)
* true evolution rate
* consistency under perturbation
* provenance coverage (>95% hard, >90% statistical)
* privacy/policy leakage under red-team
* operation isolation failure rate
* IPC policy violation rate
* self-modification violations blocked rate
* plan-trajectory drift incidents detected rate

### Performance and Resource Control

* P95 latency
* reflection backlog health
* token budget compliance (governance never truncated)
* syscall failure and retry effectiveness
