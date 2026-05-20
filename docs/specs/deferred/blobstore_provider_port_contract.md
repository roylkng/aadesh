# BlobStore Port Contract Spec v0.1
Adesh OS

This document defines the **BlobStore** port contract for storing and retrieving binary/text payloads (attachments, tool outputs, compiled artifacts, sanitized views). It specifies:
- content addressing and metadata requirements
- sensitivity/taint labeling at rest
- integrity verification (checksums)
- atomic write semantics
- retention and safe deletion hooks
- interaction with IPCArtifacts, AuditTrace, and Replay

This is interface and logic documentation. Not implementation code.

---

## 0) Core principles

1. **Content-addressed by default**
Blob references should be content-addressed (hash-based) to support dedupe and integrity.

2. **Metadata is mandatory**
Every blob must have metadata including sensitivity/taint labels and provenance refs.

3. **Integrity checks are required**
Reads must verify checksums when feasible.

4. **No secrets in paths**
Blob references must not embed secrets.

---

## 1) Blob identity and references

### 1.1 ContentRef format
Canonical:
- `sha256:<hex>`

Optional namespace:
- `blob:sha256:<hex>`

### 1.2 BlobMeta (required fields)
- `content_type` (optional)
- `size_bytes`
- `checksum_sha256`
- `sensitivity_s` (0..4)
- `taint_s` (0..4)
- `provenance_refs[]` (event_ref, syscall_id, artifact_id)
- `created_at`
- `tags[]` (optional; e.g., artifact_kind)

### 1.3 Immutability
A blob referenced by `content_ref` is immutable. Any change yields a new content_ref.

---

## 2) Port methods (conceptual interface)

### 2.1 put_bytes
Inputs:
- bytes (binary)
- BlobMeta (may omit checksum; provider computes)
Outputs:
- `content_ref`

Semantics:
- compute checksum, derive content_ref
- write content and metadata atomically
- if content_ref already exists:
  - verify checksum and metadata compatibility
  - return existing content_ref (dedupe)
- must not partially write (no orphaned meta without content)

Errors:
- InvalidInput if meta invalid or size too large
- Io/Db for backend failures

### 2.2 get_bytes
Inputs:
- content_ref
Outputs:
- bytes
Semantics:
- fetch content
- verify checksum matches metadata when configured
- if mismatch: Corruption

### 2.3 head
Inputs:
- content_ref
Outputs:
- BlobMeta

### 2.4 exists
Optional convenience method.

### 2.5 delete (governed, optional in v0.1)
Deletion is dangerous and usually R4.
If implemented:
- requires explicit governance approval at caller layer
- must record deletion tombstone and audit event
- may be logical delete rather than physical delete

---

## 3) Atomicity and integrity semantics

### 3.1 Atomic write
put_bytes must be atomic in effect:
- either both content and meta are durable, or neither is visible.

### 3.2 Integrity verification
- `head` returns checksum
- `get_bytes` verifies checksum (configurable but recommended)
- on mismatch: return Corruption and alert

---

## 4) Sensitivity and taint handling

### 4.1 Label at write time
BlobStore must store the sensitivity/taint labels provided by classification pipeline.
It must not silently downgrade labels.

### 4.2 Redaction requirement
BlobStore must never be used to store forbidden categories (negative memory):
- credentials
- auth secrets
- SSNs
unless explicitly allowed under a secure vault subsystem (not in v0.1)

If caller attempts:
- reject with InvalidInput and include redaction guidance

---

## 5) Provenance and audit linkage

### 5.1 Provenance refs
Every blob must have at least one provenance ref:
- event_ref that produced it
- syscall_id that produced it
- artifact_id that references it

### 5.2 AuditTrace references
Any content_ref referenced in AuditTrace must be retrievable during retention window.

---

## 6) Interaction with IPCArtifacts

IPCArtifacts reference BlobStore content_ref.
Rules:
- IPCArtifact sensitivity/taint must be consistent with blob meta or stricter.
- IPCArtifact must not reference a blob lacking metadata.

---

## 7) Replay requirements

Replay relies on stored blobs:
- reasoning outputs
- syscall outputs
- sanitized views
BlobStore must support reads for the retention window.

In dry_run replay:
- no new blobs are required if replay uses stored artifacts
- if a blob is missing: replay fails with missing anchor

---

## 8) Retention and compaction hooks (optional)

### 8.1 Mark-and-sweep GC (future)
BlobStore may support:
- list blobs by age
- delete unreferenced blobs older than retention

But deletion must be careful:
- reference tracking must include AuditTrace, IPCArtifacts, Experience Log content_refs

If reference tracking is incomplete:
- do not implement physical deletion; use logical deletion with tombstones.

---

## 9) Minimum acceptance tests (must pass)

1. Dedupe:
- put_bytes same content twice -> same content_ref

2. Integrity:
- corrupt stored bytes -> get_bytes fails with Corruption

3. Metadata:
- head returns correct sensitivity/taint labels

4. Atomic write:
- simulate crash mid-write -> no orphaned meta or partial visibility

5. Provenance:
- put_bytes requires provenance_refs; missing -> InvalidInput

