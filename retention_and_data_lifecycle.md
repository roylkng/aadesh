# Retention and Data Lifecycle Spec v0.1
Adesh OS

This document specifies retention, lifecycle, compaction, and deletion policies for Adesh OS data. It defines:
- what data is stored (and why)
- default retention periods by category
- compaction and summarization strategies that preserve provenance
- deletion workflows (R4) and tombstoning
- how retention interacts with replay, audit, and compliance
- safe garbage collection rules for blobs and versions

This is algorithmic logic. Not implementation code.

---

## 0) Core principles

1. **Auditability over convenience**
Data required for audit and replay must not be deleted silently.

2. **Append-only core**
Experience Log is append-only. “Deletion” is a governed, auditable process.

3. **Provenance must survive compaction**
You may compact content, but you cannot remove the ability to trace claims to sources.

4. **No unsafe garbage collection**
Never physically delete blobs or versions unless reference tracking is complete.

5. **User-controlled, explicit destructive actions**
Any deletion beyond ephemeral cache is R4 by default and requires OOB.

---

## 1) Data categories and lifecycle expectations

### 1.1 Core ledger (must retain longest)
- Operations (latest + transitions)
- AuditTrace anchors
- GateDecision, CompiledSlice refs
- Syscall envelopes and denies
- Approval decisions and OOB lifecycle records (without secrets)
- Audience graph versions
- Active state versions

Purpose:
- accountability, replay, debugging, compliance

### 1.2 Experience Log (append-only)
- request events
- reasoning outputs (structured)
- approval and denial events
- capability changes
- review decisions
- reflection updates

Purpose:
- immutable provenance and event history

### 1.3 Blob content
- attachments
- tool outputs
- sanitized views
- large reasoning drafts
- reports

Purpose:
- replay, evidence, user retrieval

### 1.4 Derived indexes/caches (short-lived)
- embeddings or vector index (if present)
- fast lookup caches
- UI transient streaming buffers

Purpose:
- performance only; safe to rebuild

---

## 2) Default retention policy (baseline)

These are defaults. They must be configurable but require governance when shortened.

### 2.1 Retain indefinitely (or very long)
- AuditTrace objects and required anchors
- Operation transitions
- OOB challenge metadata (challenge_id, timestamps, consumed status)
- Review decisions
- Versioned state history (active_state_version, audience_graph_version, capability_snapshot_version)

### 2.2 Retain long (e.g., 180–365 days)
- Idempotency keys and stored responses
- Full reasoning drafts if they contain sensitive content (may be compacted sooner)

### 2.3 Retain medium (e.g., 30–180 days)
- Non-critical system telemetry events
- Model provider raw debug snippets (redacted)

### 2.4 Retain short (e.g., session-bound)
- Scratch block content (never persisted as authoritative)
- Token stream chunks (WS only)
- Temporary staging blobs

---

## 3) Compaction and summarization (without losing provenance)

### 3.1 Experience Log compaction
You may compact by creating new derived artifacts:
- `kind=compacted_summary`
- references a set of original event_refs

Rules:
- original event_refs remain (append-only) unless deleted via R4 process
- compacted summary is additive
- compacted summary must include:
  - list of covered refs
  - time bounds
  - sensitivity/taint labels >= max of sources

### 3.2 Blob compaction
For large blobs (e.g., long documents), create a compacted representation:
- extracted text
- summaries
- thumbnails

Rules:
- compacted derivatives must reference original content_ref
- derivatives inherit taint and sensitivity conservatively

### 3.3 Active State compaction
Active State may prune deprecated hypotheses:
- mark as deprecated, not delete
- preserve provenance refs and time bounds

Never delete user-confirmed primitives without explicit owner deletion.

---

## 4) Deletion workflow (R4, OOB, tombstones)

### 4.1 What deletion means
Deletion may be:
- logical delete (tombstone)
- physical delete (only if safe)

Default approach:
- tombstone + optional blob purge later when safe.

### 4.2 Deletion request flow
Deleting any of:
- Experience Log entries
- blobs referenced by audit
- state versions
- audience graph versions
is an R4 operation requiring:
- explicit user intent
- OOB challenge
- audit record of deletion

### 4.3 Tombstone record
When deleting an object:
- create tombstone record:
  - object_ref
  - deleted_at
  - reason
  - requested_by (root_owner)
  - OOB challenge id reference
- update audit timeline

Objects with tombstones must:
- appear as deleted in UI
- remain as references for audit (but content may be removed if physical deletion executed)

### 4.4 Physical deletion constraints
Physical deletion is allowed only if:
- object is not referenced by any active operation or required audit anchor OR
- policy explicitly permits removal and audit will record the loss of content

If physical deletion would break replay:
- replay must fail with “missing anchor due to deletion,” and that must be expected behavior.

---

## 5) Garbage collection rules (safe GC)

### 5.1 Mark-and-sweep for blobs
To delete blobs safely:
- Mark all content_refs reachable from:
  - AuditTrace anchors
  - IPCArtifacts
  - Experience Log events that reference content
  - current and historical state versions (if they reference blobs)
- Sweep any blobs older than retention not marked.

If mark set cannot be computed reliably:
- do not perform physical blob GC.

### 5.2 Version retention and pruning
Versioned state (active/audience/capability):
- keep full history by default
- optional pruning:
  - keep last N versions
  - keep versions referenced by audit traces indefinitely
Pruning must be governed and audited.

### 5.3 Idempotency key GC
- safe to delete keys older than retention
- must not delete keys for operations still in non-terminal states

---

## 6) Sensitivity-aware retention

Retention actions must respect sensitivity:
- S3/S4 content should not be copied into lower-sensitivity stores
- compaction artifacts must inherit max sensitivity and taint
- deletion requests for sensitive data may be prioritized

---

## 7) Interaction with replay

Replay requires:
- GateDecision, CompiledSlice, reasoning output, syscalls, approvals, artifacts

If retention deletes any required anchor:
- replay must fail deterministically with a “missing anchor” reason
- audit must show deletion tombstone

Therefore:
- default retention for replay anchors should be long or indefinite unless user explicitly deletes.

---

## 8) Configuration and governance of retention changes

Changing retention policies is itself governed:
- shortening retention for audit-critical data is R4
- changing GC schedules is R3

All retention config changes must be:
- stored as a configuration change event
- optionally a review queue item

---

## 9) Minimum test cases (must pass)

1. Tombstone creation:
- delete a blob -> tombstone recorded, OOB required, audit updated.

2. GC safety:
- blob referenced by audit cannot be physically deleted.

3. Compaction preserves provenance:
- compacted summary references original event refs and inherits max taint.

4. Replay after deletion:
- deleting anchor causes replay failure with explicit missing anchor reason.

5. Idempotency retention:
- GC does not delete idempotency keys for running operations.
