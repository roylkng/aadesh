# Adesh OS Spec Index and Reading Order (v0.1)

This repository contains the production-grade specifications for Adesh OS, a governed agent operating system with:
- syscall-based tool execution
- max(R,S) governance with approvals and OOB authorization
- taint-aware working memory and explicit IPC between operations
- audience-conditional disclosure via an Audience Graph
- immutable provenance and replayable audit trails
- asynchronous reflection loop for governed persona/OS enrichment

This document is the entry point. It lists what exists, what each spec controls, and the recommended reading order using the **actual filenames in this repo**.

Repository organization:
- Root-level spec files are canonical unless explicitly marked otherwise.
- `reference/` contains non-authoritative summaries, sketches, and planning artifacts.
- `archive/` contains retained legacy material for historical context only.

---

## 1) System overview

Adesh OS runs a strict execution pipeline:

1. Accept Root Owner request via HTTP control plane.
2. Decompose into operations (processes) with isolation boundaries.
3. Compute governance gates (risk R, sensitivity S, max_gate).
4. Compile a minimal, taint-labeled persona/memory slice (JIT compilation).
5. Call an LLM via ModelProvider boundary (structured output only).
6. Verify plan trajectory alignment, schemas, audience scopes, and taint laundering constraints.
7. Park gated actions into approvals (confirm/diff/OOB) with atomic consumption.
8. Execute permitted syscalls via ToolProvider with persisted pre-images and audit anchors.
9. Persist everything into an immutable Experience Log and an inspectable AuditTrace.
10. Enrich state asynchronously via Reflection Loop with review queue write-gates.
11. Support deterministic replay (dry_run/full) from stored anchors.

---

## 2) Key invariants (non-negotiable)

- All side effects occur only via persisted `SyscallEnvelope` records.
- max_gate = max(Action Risk R, Data Sensitivity S) governs every syscall and output.
- Default deny for unknown audience nodes/edges/scopes.
- Operation isolation is strict. Cross-operation data requires explicit IPCArtifacts.
- Working memory is taint-aware. No taint laundering into lower-sensitivity outputs.
- Audit never fails open. Missing audit anchors is a hard failure.
- OOB authorization is approval-bound and single-use. Never elevates sessions globally.
- Schemas are immutable and content-addressed. Operations pin capability snapshots (and schemas).
- Reflection produces new versions, never mutates in-flight pinned state.

---

## 3) Reading order (recommended)

### A) Core execution physics
1. `kernel_execution_loop.md`  
   Deterministic sync execution algorithm and operation state machine.

2. `governance_kernel_logic.md`  
   Governance kernel logic: R/S/max_gate, predicates, approval modes, denials.

3. `jit_compiler.md`  
   JIT compilation: block packing, provenance, conflicts, taint computation.

4. `verification_core_ruleset.md`  
   Verification: plan trajectory alignment, taint laundering detection, schema enforcement, anti-retry.

---

### B) Persistence, safety, and concurrency
5. `storage_semantics_txn.md`  
   Atomicity rules, write ordering, replay anchors, fail-closed behavior.

6. `approval_oob_spec.md`  
   Confirm/diff/OOB flows, approve-with-edits, single-use OOB, atomic consumption.

7. `operation_decomposition_ipc.md`  
   Operation splitting rules and explicit IPC artifacts with sensitivity inheritance.

8. `scheduler_concurrency.md`  
   Leasing, crash recovery, stage idempotence, preventing duplicate execution.

---

### C) Integration planes and embodiment
9. `capability_mcp.md`  
   Capability self-model, snapshot pinning, MCP discovery, enable/disable gating.

10. `sandboxed_actuator_capability.md`
   Sandboxed actuator capability class, sandbox policy descriptors, containment, and replay anchors.

11. `mcp_host_surface_contract_spec.md`  
   MCP Host tool surface for external agents, audience-scoped outputs, default deny.

12. `websocket_events_contract.md`  
   WS event envelope, streaming chunks, ordering expectations, persistence vs ephemeral.

13. `control_plane_api_spec.md`  
   Root Owner HTTP control plane API (REST + WS integration).

14. `adaptive_interface.md`
   Adaptive interface layers, persona policy model, and safe UI evolution boundaries.

15. `ui_theme.md`
   Signal District design token system and visual interaction rules for the control plane UI.

16. `email_send_payload_contract.md`
   Canonical email send payload shape, normalization, and diff-edit rules for the v0 wedge.

---

### D) Security, privacy, classification, and governance extensions
17. `audience_graph_and_disclosure_policy.md`  
   Audience graph model, scopes, ceilings, outbound audience resolution, default deny.

18. `data_classification_and_taint_labelling.md`  
   Sensitivity and taint labeling, propagation, laundering prevention.

19. `sanitization_subsystem.md`  
   Sanitized_view artifacts, sanitization reports, certified sanitizers, verification rules.

20. `error_remediation.md`  
   Error taxonomy, constraint ids, remediation payloads, bounded retry rules.

21. `schema_registry_and_versioning.md`  
   Schema storage, hashing, pinning via capability snapshots, upgrade rules.

22. `schema_based_tools_and_actions.md`
   Generic schema-defined tool/action model, action-level args/result schemas, canonicalization, and idempotent syscall behavior.

23. `artifact_normalization_contract.md`
   Canonical artifact shapes produced by ingestion and downstream deterministic normalization.

24. `fact_ledger_and_reflection_claims.md`
   Durable claim ledger, evidence-backed promotion, conflict handling, and compiler/verification consumption rules.

---

### E) Evolution and long-term operability
25. `ingestion_pipeline_spec.md`
   Asynchronous ingest jobs, deterministic normalization flow, artifact persistence, and reflection handoff.

26. `reflection_and_persona.md`  
   Async enrichment, hypothesis lifecycle, opportunity-aware decay, write-gates.

27. `review_queue_and_control_plane.md`  
   Review items, diffs, approve/reject/edit, OOB for R4 state changes, UI workflows.

28. `replay_and_deterministic_re_execution.md`  
   Replay modes (dry_run/full), deterministic anchors, divergence reporting.

29. `version_diff_and_merge.md`  
   Canonical diffs, safe merges, conflict surfacing across versions.

30. `retention_and_data_lifecycle.md`  
   Retention, compaction, deletion (R4), tombstones, safe GC rules.

---

### F) Platform ops, telemetry, and security validation
31. `observability_audit.md`  
   Logs, traces, metrics, KRIs, redaction rules, audit correlation.

32. `test_and_kri.md`  
   Red-team suite, pass/fail criteria, KRI thresholds.

33. `threat_model_spec.md`  
   Threat model: assets, boundaries, adversaries, mitigations, residual risk.

34. `boot_sequence.md`  
   Config precedence, profiles, deterministic boot order, degradation behavior, hot reload rules.

---

### G) Contracts and port boundaries
35. `api_batch_1.md`  
36. `api_batch_2.md`  
37. `api_batch_3.md`  
   Batch contracts: Boot & Route, Compilation & Governance, Execution & Audit.

38. `storage_provider_port_contract.md`  
   StorageProvider method-level contract, atomic operations, leases, idempotency.

39. `blobstore_provider_port_contract.md`  
   BlobStore contract: content refs, integrity, metadata, retention safety.

40. `jobqueue_provider_port_contract.md`  
   JobQueue contract: enqueue/lease/ack/fail, backoff, dedupe.

41. `tool_provider_port_contract.md`  
   ToolProvider syscall execution contract, idempotency, structured results.

42. `model_provider_port_contract.md`  
   ModelProvider contract: generation, streaming, retries, validation.

43. `model_output_contract.md`  
   ReasoningOutput schema and LLM translation boundary (canonical LLM I/O).

---

## 4) Supporting and meta docs
- `stack.md`  
  Tech stack strategy and swapability principles.
- `storage_schema.md`  
  Storage schema notes (DDL-level).
- `docs/REPO_ORGANIZATION.md`  
  Repository layout, placement, and contribution hygiene contract.
- `registry/README.md`  
  Bootstrap artifact layout for schema and capability snapshot initialization.
- `reference/provider_interfaces_summary.md`  
  Consolidated interface overview (if kept in sync with port contracts).
- `archive/api_spec_legacy.md`  
  Legacy/earlier API spec (keep only if intentionally retained).
- `reference/contract_summaries.md` and `reference/rust_contract_summaries.md`  
  Contract summaries and code-facing notes.
- `reference/problem_statement.md`  
  Problem statement (AgentOS framing).
- `README.md`  
  Repo entry.
- `reference/implementation_backlog.md`  
  Task list / planning artifacts.
- `reference/code_skeleton_reference.md`  
  Implementation skeleton notes (non-authoritative versus specs).

---

## 5) Implementation guidance

- Treat each spec as a law. Do not improvise behavior.
- If two specs appear to conflict, resolve in this order:
  1) Invariants in this index
  2) `storage_semantics_txn.md`
  3) `governance_kernel_logic.md` + `verification_core_ruleset.md`
  4) API specs (`control_plane_api_spec.md`, `mcp_host_surface_contract_spec.md`)
  5) Port contracts

- Any new behavior should land as a spec update first unless it is a pure refactor.

---

## 6) Terminology quick map

- **Operation**: isolated process with its own memory slice, state, and syscalls.
- **Syscall**: the only way to interact with sensors/actuators/sanitizers.
- **GateDecision**: computed R/S/max_gate + scopes and ceilings.
- **CompiledSlice**: token-budgeted blocks injected into ModelProvider.
- **Taint**: influence ceiling of sensitive/untrusted inputs.
- **Audience Graph**: disclosure policy graph with default deny and ceilings.
- **OOB**: out-of-band authorization for R4 actions, single-use, approval-bound.
- **Review Queue**: governed write-gate for sensitive state mutations.
