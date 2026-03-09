# Ingestion Pipeline Spec v0.1
Adesh OS

This document specifies the ingestion pipeline for Adesh OS. It covers:
- ingesting large documents (books, PDFs, folders)
- ingesting long conversation histories (agent logs, chat transcripts)
- deterministic normalization into immutable artifacts
- classification (S) and taint (T) labeling at ingest
- job scheduling, backpressure, and idempotency
- how ingestion feeds the reflection loop without blocking the synchronous execution path

This is a canonical spec. Not implementation code.

---

## 0) Core principles

1. Immutable archive first
All ingestion results in immutable artifacts stored as:
- Experience Log events
- Blob objects (content_ref)
- Artifact metadata rows

2. Async and resumable
Ingestion is always asynchronous. Large inputs must be processed via JobQueue.

3. Deterministic normalization
Normalization must not depend on LLM output. LLMs may be used only in reflection later.

4. Conservative labeling
When uncertain, label higher sensitivity and taint per `data_classification_and_taint_labelling.md`.

5. Fail closed on audit-critical persistence
If required writes fail (Experience event, blob metadata), ingestion fails and reports reason.

---

## 1) Ingestion outcomes

Every ingestion run must produce:

A) Immutable artifacts
- `artifact_id`
- `content_ref` (BlobStore pointer)
- `artifact_meta`:
  - source_type
  - timestamps
  - author/participants if known
  - sensitivity_s, taint_s
  - provenance_refs

B) Experience events
- `kind=ingest_job_created`
- `kind=ingest_artifact_added` (one per artifact)
- `kind=ingest_job_completed` or `kind=ingest_job_failed`

C) Optional derived artifacts (non-authoritative)
- extracted plain text from PDFs
- page-level segments
- structured tables
These are still artifacts, but must be tagged as derived and inherit taint.

---

## 2) Ingestion types

Supported source types (v0.1):

### 2.1 Text payload
- Raw text pasted by Root Owner
- or JSON transcript payloads

### 2.2 File upload
- PDF, EPUB, TXT, DOCX, MD, HTML
- Images are allowed but OCR is optional

### 2.3 Folder ingest
- Recursively ingest a directory
- Must record file paths as metadata (do not treat as secrets)

### 2.4 Conversation import
- Exported chat logs with roles and timestamps
- Agent logs or tool traces
- Must preserve ordering and participant identities as metadata

### 2.5 URL ingest (optional in v0.1)
- Fetching web pages is allowed only if explicitly enabled
- Treat as untrusted content: taint >= T2 by default

---

## 3) Control plane API (Root Owner only)

All endpoints are Root Owner-only HTTP control plane.

### 3.1 Create ingestion job
`POST /v1/ingest/jobs`

Body (conceptual):
- `sources[]` where each source has:
  - `type`: text|file|folder|conversation|url
  - `payload`: inline text or file handle or path or url
  - `metadata`: optional tags and timestamps
- `options`:
  - `dedupe`: true/false (default true)
  - `max_artifacts`: limit
  - `chunking`: none|page|fixed_tokens (default none at ingestion; chunking is reflection stage)
  - `classification_mode`: conservative|normal (default conservative)

Headers:
- `Idempotency-Key` supported and required for large ingests recommended

Response:
- `job_id`
- initial counters

### 3.2 Get job status
`GET /v1/ingest/jobs/{job_id}`

Returns:
- status: pending|running|completed|failed|cancelled
- counters:
  - artifacts_total
  - artifacts_succeeded
  - artifacts_failed
  - bytes_ingested
  - s_distribution (counts per S0..S4)
  - t_distribution (counts per T0..T4)
- errors (bounded, redacted)

### 3.3 Cancel job
`POST /v1/ingest/jobs/{job_id}/cancel`

---

## 4) Job model and backpressure

Ingestion is implemented as JobQueue jobs:
- `ingest.run_job` (root job)
- `ingest.process_item` (per file / per transcript segment)
- `ingest.finalize_job`

### 4.1 Backpressure rules
System must enforce:
- max concurrent ingest items (config)
- max bytes per minute (config)
- max blob write bandwidth (config)

When limits reached:
- enqueue remaining items with delayed `run_after`

### 4.2 Resumability
Each ingest item must be idempotent:
- identify item by `(job_id, item_key)`
- item_key examples:
  - file path + mtime + size + hash
  - transcript segment id

If worker crashes:
- lease expires and another worker reprocesses
- dedupe prevents duplicate artifacts

---

## 5) Dedupe and idempotency

### 5.1 Content dedupe
BlobStore is content-addressed:
- same content => same content_ref

Artifacts must dedupe at metadata layer too:
- If `dedupe=true`, do not create duplicate artifact rows for the same `(source_fingerprint, content_ref)`.

### 5.2 Idempotency key
If the same Idempotency-Key is used for `POST /v1/ingest/jobs`, response must be identical:
- same job_id returned
- job state unchanged

If `dedupe=true`:
- every persisted artifact row must carry a non-null dedupe key
- dedupe decisions must be based on that persisted key, not only blob-address equality

If `dedupe=false`:
- dedupe key may be null
- duplicate artifact rows are permitted

---

## 6) Normalization rules

Normalization creates canonical artifact payloads. It must preserve provenance and ordering.

### 6.1 For books and large documents
- Store original file as blob artifact (binary)
- Optionally store extracted text as a derived artifact
- If extraction is performed:
  - each page or section can be an artifact (derived)
  - must link back to source artifact via provenance_refs

### 6.2 For chat / conversation history
- Store original transcript JSON as blob artifact
- Store a normalized “message stream” artifact:
  - array of messages with role, timestamp, speaker id, text
- Preserve ordering as given
- Never rewrite content semantically

### 6.3 No LLM in normalization
Normalization is parsing/extraction only.
LLM use is confined to reflection.

---

## 7) Classification at ingestion (S/T)

For each artifact created:
- run classification pipeline per `data_classification_and_taint_labelling.md`
- record:
  - sensitivity_s
  - taint_s
  - reason codes
- enforce negative memory:
  - redact/don’t store forbidden secrets (credentials) if detected

For URL content:
- taint must be at least T2 by default.

---

## 8) Persistence sequence (storage-first)

For each artifact:
1. Write blob to BlobStore and obtain content_ref
2. Persist blob metadata (`blob_objects`)
3. Persist artifact metadata row
4. Append Experience event `ingest_artifact_added` referencing artifact_id + content_ref

If any step fails:
- mark item failed
- do not emit success WS event

---

## 9) Emitted WS events

Ingestion emits WS events after commit:
- `ingest_job_created`
- `ingest_progress` (bounded frequency)
- `ingest_artifact_added` (optional)
- `ingest_job_completed` or `ingest_job_failed`

---

## 10) How ingestion feeds reflection

Ingestion does not mutate Active State.
Instead, ingestion completion enqueues reflection jobs:
- `reflection.process_events` targeting the newly created ingest events/artifacts

Reflection then:
- extracts candidate primitives
- creates review items as needed
- mints new versions only through governed paths

---

## 11) Minimum acceptance tests

1. Large book ingest creates:
- source artifact
- optional extracted text artifact
- correct provenance refs
- conservative S/T labels

2. Conversation ingest preserves ordering and roles
3. Ingest is resumable under worker crash
4. Dedupe prevents duplicate artifact rows
5. Ingest never blocks synchronous request handling
6. Negative memory prevents storing credentials in raw form
