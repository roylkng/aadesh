# Retention and Data Lifecycle Spec v0.1
Adesh OS

This document specifies retention, compaction, deletion, and garbage-collection policy for Adesh OS data.

## 0) Core principles

1. Auditability and replay anchors are retained by default.
2. Experience Log remains append-only; deletion is governed and auditable.
3. Compaction is additive and must preserve provenance.
4. Physical deletion is never allowed when references are still required for safety/audit.
5. Destructive lifecycle changes are governed operations.

## 1) Data categories

### 1.1 Core governance ledger
- operations + transitions
- gate decisions
- compiled slices
- syscall envelopes + denies
- approvals + OOB lifecycle metadata
- audit traces + required anchors
- active/audience/capability versions

### 1.2 Experience Log
- request, reasoning, approval, denial, replay, reflection, review events

### 1.3 Blob content
- attachments
- tool outputs
- sanitized views
- large artifacts/reports

### 1.4 Derived caches/indexes
- rebuildable indexes and runtime caches
- WS stream chunks and temporary buffers

## 2) Default retention policy

### 2.1 Long/indefinite retain
- audit traces and required anchors
- operation transitions
- version history referenced by audit
- approval/OOB status records (without secrets)

### 2.2 Medium retain (policy-configurable)
- idempotency key response cache
- large raw reasoning drafts where duplication exists

### 2.3 Short retain
- ephemeral stream chunks
- temporary staging blobs
- non-authoritative scratch artifacts

## 3) Compaction rules

### 3.1 Experience Log compaction
Compaction must write additive `compacted_summary` artifacts containing:
- covered refs
- time bounds
- inherited max sensitivity/taint labels

Original refs remain unless governed deletion occurs.

### 3.2 Blob compaction
Derived compact artifacts must:
- reference original `content_ref`
- keep conservative sensitivity/taint labels

### 3.3 Version compaction
Deprecated hypotheses may be marked deprecated but not silently removed when referenced.

## 4) Deletion workflow (governed)

### 4.1 Deletion classes
- logical delete (tombstone)
- physical delete (only when safe)

### 4.2 Governance
Deletion of audit-critical anchors is at least R4 and requires OOB.

### 4.3 Tombstones
Each delete action must persist:
- object ref
- deletion timestamp
- reason
- requester
- approval/OOB references

### 4.4 Physical deletion constraints
Physical deletion allowed only when:
- object has no required references for active operations or mandatory audit anchors
- policy explicitly allows removal

If deletion removes replay anchors, replay must fail with explicit missing-anchor reason.

## 5) Garbage collection

### 5.1 Mark-and-sweep for blobs
Mark from all reachable refs in:
- audit traces
- experience events
- IPC artifacts
- versioned state refs

Only unmarked and expired blobs are eligible for physical GC.

### 5.2 Version pruning
Allowed only under governed policy; versions referenced by audit traces are protected.

### 5.3 Idempotency GC
Never remove keys tied to non-terminal operations.

## 6) Sensitivity-aware lifecycle

1. Compaction outputs inherit max sensitivity/taint of sources.
2. Lifecycle jobs must not copy S3/S4 data into lower-class stores.
3. Deletion requests for sensitive data may be prioritized but still governed.

## 7) Replay interaction

Replay requires anchors for:
- pinned versions
- gate decision
- compiled slice
- reasoning output
- approvals/OOB refs
- syscalls/results/denies
- IPC artifacts

If any required anchor was deleted, replay must fail deterministically and cite tombstone refs.

## 8) Retention policy changes

Shortening retention for audit-critical data is governed (R4 class by default).
Changing GC cadence or non-critical retention is governed (typically R3).
All policy changes must be logged and auditable.

## 9) Minimum test cases

1. Tombstone recorded for governed deletion with approval/OOB refs.
2. GC never physically deletes blob referenced by required audit anchor.
3. Compaction preserves provenance refs and conservative S/T labels.
4. Replay fails deterministically when required anchor is deleted.
5. Idempotency GC skips keys tied to active operations.
