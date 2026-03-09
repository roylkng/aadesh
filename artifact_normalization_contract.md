# Artifact Normalization Contract Spec v0.1
Adesh OS

This document defines the canonical artifact representations produced by ingestion. It is the interface between:
- the Ingestor (parsing/extraction)
- StorageProvider/BlobStore (persistence)
- Reflection loop (downstream extraction)

This is a canonical spec. Not implementation code.

---

## 0) Artifact kinds

The ingestion system may produce these artifact kinds:

- `source_binary` (original file bytes)
- `source_text` (original text payload)
- `derived_text` (extracted plain text)
- `derived_segment` (page/section segments)
- `conversation_transcript_raw` (raw JSON export)
- `conversation_message_stream` (normalized message stream)
- `tool_trace` (optional)
- `sanitization_report` (from sanitizer, not ingestor)

Every artifact has:
- `artifact_id`
- `kind`
- `content_ref`
- `meta` (see below)

---

## 1) ArtifactMeta (required)

Each artifact meta must include:

### 1.1 Provenance
- `source_type`: text|file|folder|conversation|url
- `source_locator`: path/url/logical id (redacted if sensitive)
- `source_fingerprint`: stable hash of locator+size+mtime or equivalent
- `parent_artifact_id` (for derived artifacts)
- `provenance_refs[]` (Experience event refs and artifact ids)

### 1.2 Time
- `created_at` (ingest time)
- `observed_at` (original time if known, e.g., message timestamp range)
- `time_bounds` (optional: start/end for transcripts or segments)

### 1.3 Classification
- `sensitivity_s` (0..4)
- `taint_s` (0..4)
- `classification_reasons[]` (stable codes)

### 1.4 Size and type
- `content_type`
- `size_bytes`
- `checksum_sha256`

### 1.5 Conversation metadata (if applicable)
- `participants[]` (speaker ids)
- `channel` (optional)
- `message_count`
- `role_schema` (mapping of roles)

---

## 2) Canonical conversation_message_stream format

`conversation_message_stream` content (JSON) must be:

```json
{
  "schema_version": "0.1",
  "messages": [
    {
      "msg_id": "string",
      "ts": "rfc3339|string",
      "speaker": "string",
      "role": "user|assistant|system|tool|other",
      "text": "string",
      "attachments": [{ "artifact_id": "string" }]
    }
  ]
}
```

Rules:

* Preserve the original ordering exactly as imported.
* Do not summarize or rewrite text.
* If timestamps are missing, use ingest ordering with synthetic ids.

---

## 3) Canonical derived_segment format

`derived_segment` content (JSON) must be:

```json
{
  "schema_version": "0.1",
  "source_artifact_id": "string",
  "segments": [
    {
      "segment_id": "string",
      "locator": { "page": 12, "offset": 0 },
      "text": "string"
    }
  ]
}
```

Rules:

* Segments inherit sensitivity and taint from source unless classifiers promote them higher.
* Segmenting is optional at ingest; if done, must preserve provenance to source.

---

## 4) Dedupe keys

Artifact dedupe key:

* `artifact_dedupe_key = hash(kind + content_ref + source_fingerprint + parent_artifact_id)`

If dedupe enabled:
- do not create duplicates for same dedupe key.
- if `dedupe=true`, `artifact_dedupe_key` must be non-null on every persisted artifact row.
- if `dedupe=false`, the dedupe key may be null and duplicate artifact rows are allowed.

---

## 5) Negative memory constraints

The normalizer must:

* detect credentials/secret tokens
* redact before persistence if possible
* otherwise refuse to store the content and mark item failed

Derived artifacts must not reintroduce forbidden secrets.

---

## 6) Extraction and adapter policy

### 6.1 Large-file extraction at v0.1
- v0.1 must always persist the original source artifact first.
- Text extraction from PDFs, EPUBs, DOCX, or HTML is optional but deterministic when enabled.
- If extraction is deferred, the system must still persist the original source artifact and record that no derived text artifact was produced.

### 6.2 Conversation adapter support
Implementations may support multiple transcript import formats, but each adapter must deterministically map input into `conversation_message_stream`.

At minimum, an adapter must define:
- supported input format identifier
- how speaker ids are derived
- how roles are mapped into `user|assistant|system|tool|other`
- how timestamps are preserved or synthesized
- how attachments are represented as artifact references

---

## 7) Minimum acceptance tests

1. Conversation import produces both raw transcript and normalized message stream artifacts.
2. Derived segments preserve source linkage via parent_artifact_id.
3. Dedupe prevents duplicate artifacts under retries.
4. Forbidden secrets are not persisted in raw form.
