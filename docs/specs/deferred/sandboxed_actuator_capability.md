# Sandboxed Actuator Capability Spec v0.1
Adesh OS

This document specifies a capability class for actuators that execute in a sandbox (VM/container) rather than directly on the host or against external APIs.

Purpose:
- support “computer-use” style agents safely
- reduce blast radius of untrusted tool actions
- provide deterministic logs and replay anchors for machine-side effects

This is a canonical spec. Not implementation code.

---

## 0) Core principles

1) Actuators are syscalls
All effects happen via persisted syscall envelopes.

2) Sandbox is a capability attribute
An actuator declares its execution environment:
- external API
- host-local
- sandboxed (VM/container/session)

3) Sandbox does not bypass governance
Sandboxed actions are still gated by max(R,S) and approvals.

4) Sandbox produces auditable traces
Sandbox actions must produce:
- stdout/stderr logs
- file diffs or artifact outputs
- deterministic “what changed” summaries

---

## 1) Capability descriptor additions

Every actuator capability must declare:

- `execution_class`:
  - `external_api`
  - `host_local`
  - `sandboxed`

If `sandboxed`, also declare:
- `sandbox_profile_id`
- `filesystem_policy` (read-only, ephemeral, mounted dirs)
- `network_policy` (none, allowlist, full)
- `resource_budgets` (cpu/mem/time)
- `artifact_capture_policy` (which outputs are persisted)

---

## 2) Sandboxed syscall envelope requirements

SyscallEnvelope for sandboxed actuators must include:
- `sandbox_session_id` (created on demand)
- `inputs` as artifact refs only (no raw secret strings)
- `expected_outputs` (optional) to aid verification
- `capture` config: what to persist

---

## 3) Sandbox lifecycle (logical)

OS manages sandbox sessions as resources:

- create session (governed if expensive)
- run commands/actions
- capture outputs to BlobStore
- destroy session

Isolation requirements:
- default network disabled unless explicitly allowed
- filesystem is ephemeral unless explicitly mounted
- secrets never written into logs

---

## 4) Verification rules specific to sandbox

Verification must:
- ensure sandbox policy matches operation gate
- refuse if requested action would exceed sandbox budgets
- enforce that sensitive inputs are passed via artifacts, not inline text
- require diffs for high-stakes sandbox actions:
  - file changes
  - system changes
  - outbound network calls

---

## 5) Audit and replay

Sandbox execution must produce replay anchors:
- syscall pre-image
- sandbox policy snapshot
- captured stdout/stderr
- artifact outputs
- change diffs

Replay:
- dry_run never executes sandbox actions
- full replay may execute only with approvals consistent with gate

---

## 6) Minimum acceptance tests

1) Sandboxed actuator cannot access host filesystem outside mounted dirs.
2) Network is blocked by default.
3) Outputs are captured as artifacts and referenced in AuditTrace.
4) High-stakes sandbox actions require diff approval.