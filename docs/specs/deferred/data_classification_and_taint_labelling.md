# Data Classification and Taint Labeling Spec v0.1
Adesh OS

This document specifies deterministic rules for:
- assigning **Data Sensitivity (S0–S4)** labels to ingested artifacts and events
- assigning and propagating **Taint (S0–S4)** through working memory and derived outputs
- how classification integrates with governance, compilation, verification, IPC, and sanitization
- what is forbidden to store (negative memory) and how to drop/redact at ingestion

This spec intentionally avoids “majority truth” or subjective beliefs. Sensitivity is about **harm from disclosure** and **risk from untrusted influence**.

This is algorithmic logic. Not implementation code.

---

## 0) Core principles

1. **Conservative by default**
- When uncertain, classify higher sensitivity and higher taint.

2. **Sensitivity and taint are different**
- Sensitivity measures harm if disclosed.
- Taint measures risk that reasoning/output is influenced by sensitive or untrusted content.

3. **No laundering**
- Taint does not automatically drop when text is summarized or shortened.

4. **Explicit sanitization**
- Lowering sensitivity/taint for an output requires an explicit sanitization step and verification.

5. **Negative memory enforcement at ingestion**
- Never-store categories must be dropped/redacted before persistence where applicable.

---

## 1) Definitions

### 1.1 Sensitivity levels (S)
- **S0 Public**: explicitly public or intended for public release
- **S1 Internal/Routine**: routine personal/work info with low harm if leaked
- **S2 Confidential**: private drafts, internal project details, non-public plans, private conversations
- **S3 Restricted**: PII, credentials, financials, internal secrets, access tokens, private identifiers
- **S4 Critical/Regulated**: auth secrets, identity-level keys, governance/kernel configs, mass-impact destructive details, regulated datasets, root-owner authentication materials

### 1.2 Taint levels (T)
Taint is represented using the same numeric scale 0..4 for convenience:
- **T0**: safe influence (public/system-only)
- **T1**: mild influence from internal context
- **T2**: influence from confidential context
- **T3**: influence from restricted data (PII/secrets/financials)
- **T4**: influence from critical/regulated or untrusted injection that could subvert system behavior

Interpretation:
- Taint is the maximum “influence ceiling” of the operation’s reasoning.
- If any part of working memory includes S3 data, the operation taint becomes at least T3.

---

## 2) Classification targets

Adesh OS must label:

1. **Attachments** (file/doc/email/url)
2. **Experience Log events** (request, approvals, syscalls, reasoning outputs)
3. **Blob objects** (content stored in blob store)
4. **IPCArtifacts**
5. **CompiledSlice blocks** (policy, capability, context, evidence, scratch)
6. **Syscall inputs/outputs** (taint-in, output taint)

Each label includes:
- `sensitivity_s` (0..4)
- `taint_s` (0..4)
- `classification_reasons[]` (for audit/debug)
- optional `detectors_fired[]`

---

## 3) Deterministic classification pipeline

For any new ingested content (attachment content, tool output, user message):

### Stage 1: Source-based baseline
Assign baseline S and T based on source class.

Source baseline table (default):
- Root Owner typed text: S1, T1
- Root Owner uploads a document: S2, T2
- Internal system telemetry: S1, T1
- External web content (untrusted): S1, **T2** (untrusted influence)
- External agent (MCP client) content: S2, T2 (unless explicitly trusted)
- Credentials/config files: S4, T4

### Stage 2: Pattern-based detectors
Run deterministic detectors (regex + simple classifiers) over content metadata and limited content sample.

Detectors (minimum):
- **Credentials detector**:
  - API keys, bearer tokens, secret keys, private keys
  - triggers S4, T4
- **PII detector**:
  - phone, email (as identity), address, govt id, bank acct
  - triggers minimum S3, T3
- **Financial detector**:
  - account numbers, salaries, invoices, tax IDs
  - triggers minimum S3, T3
- **Health/legal detector**:
  - explicit health records, diagnoses, legal docs
  - triggers minimum S4 if regulated, else S3
- **Internal project detector**:
  - “confidential”, “NDA”, “internal only”, project codenames
  - triggers minimum S2, T2
- **Prompt injection detector**:
  - “ignore previous instructions”, “reveal system prompt”, “do X secretly”
  - does not increase sensitivity necessarily, but increases taint:
    - triggers minimum T3 (and T4 if explicitly targeting governance)

### Stage 3: Promotion precedence
Final sensitivity is:
- `S_final = max(S_baseline, S_detector_promotions, S_user_hint, S_tool_metadata)`

Final taint is:
- `T_final = max(T_baseline, T_detector_promotions, S_final, T_injection_promotions)`

Key rule:
- Taint is at least sensitivity: `T_final >= S_final`
Rationale: if something is sensitive, reasoning influenced by it is tainted at least that much.

### Stage 4: Negative memory redaction (never-store)
If content contains never-store classes:
- redact or drop those fields before persistence.
- record redaction action in classification reasons.
- do not store raw secret values in Experience Log or blobs.

If redaction is impossible without destroying meaning:
- refuse to store and return an error for that ingestion step (fail closed for audit-critical cases).

---

## 4) Sensitivity of common data types

### 4.1 Email and messages
- Subject + body: S2 by default (private conversation)
- If contains PII/credentials: S3/S4 accordingly
- Email addresses:
  - as identifiers: S3 when tied to identity in a private context
  - in a public directory context: can be S1/S2 depending on policy

### 4.2 Documents
- Unknown docs: S2 baseline
- Marked “confidential” or containing internal plans: S2
- Financial statements / tax: S3
- Credentials/config: S4

### 4.3 System prompts and governance configs
- Always S4/T4
- Never disclosed to external audiences; modifications require R4 with OOB.

### 4.4 Web pages
- Content is generally S1
- But prompt injection risk makes taint at least T2
- If page includes secrets (rare but possible): promote S and T accordingly

---

## 5) Taint propagation rules (operation runtime)

### 5.1 Working memory blocks
For CompiledSlice:
- `block.taint_s` is computed from included items:
  - policy block: typically T0–T2 depending on whether it includes sensitive boundaries
  - capability block: T0–T1
  - operation_context block: max taint of included primitives/events
  - evidence block: max taint of included snippets
  - scratch block: starts low, increases during operation as reasoning references sensitive data

### 5.2 Operation taint
- `T_operation = max(block.taint_s)` exactly (compiler invariant)

### 5.3 Tool syscall taint-in
For a syscall:
- `T_in = max(T_operation, max(taint(data_handles referenced)))`
unless the syscall is explicitly restricted to sanitized artifacts:
- if all handles are `sanitized_view` artifacts whose taint <= ceiling, then `T_in` can be reduced to that max.

### 5.4 Derived outputs
Any derived artifact (draft, summary, IPC artifact, tool output) inherits:
- `T_out = max(T_influences)`
- `S_out` is at least the sensitivity of what it reveals, but never below `min(S_inputs)` unless sanitized and verified.

No automatic S/T reduction for summaries.

---

## 6) Taint-aware memory and laundering prevention

### 6.1 Taint permanence within an operation
Once an operation ingests S3/S4 data into any working memory block, the operation is tainted at that level for the remainder of the operation lifetime.

This prevents “read secret then later produce public output” laundering.

### 6.2 Output ceiling enforcement
Before any outbound syscall:
- verify `T_out <= audience_ceiling_s`
- if violated:
  - require sanitization step
  - or deny

### 6.3 IPC inheritance
IPCArtifact carries `S_artifact` and `T_artifact`.
Receiver operation must:
- treat those as sensitivity sources
- set its operation taint at least `T_artifact` once compiled/consumed.

---

## 7) Sanitization and sensitivity reduction policy (interface-level)

This spec does not define sanitizer implementation, but defines when reduction is allowed.

### 7.1 When sensitivity reduction is permitted
Only if:
- an explicit sanitizer syscall produced a `sanitized_view` artifact
- verification confirms:
  - removed sensitive fields
  - replaced identifiers with abstractions
  - no secret tokens/PII remain
- the sanitized output is tagged with:
  - reduced `S_sanitized`
  - reduced `T_sanitized` only if policy allows and verification is satisfied

Default conservative rule:
- Sanitization may reduce **sensitivity** but taint remains at the max influence unless the sanitizer is “certified” and scope-limited.

### 7.2 Certification (future)
A sanitizer tool can be marked “certified” in capability registry.
Only certified sanitizers may reduce taint level.

---

## 8) Storage labeling rules

### 8.1 Experience Log
Each event must store:
- `sensitivity_s`, `taint_s`
- reasons (as metadata)
- avoid raw secrets

### 8.2 Blob objects
Blob metadata must store:
- sensitivity and taint labels
- provenance refs
- checksum

### 8.3 IPCArtifacts
Must store:
- sensitivity and taint labels
- audience scope tags

---

## 9) Deterministic reason codes

Classification reasons must be structured. Use stable codes such as:
- `baseline::owner_text`
- `baseline::attachment_unknown`
- `detector::pii_email`
- `detector::credential_token`
- `detector::prompt_injection`
- `detector::financial_statement`
- `policy::never_store_redacted`
- `promotion::tool_metadata`
- `promotion::user_hint`

These codes enable audit and debugging.

---

## 10) Minimum test cases (must pass)

1. Credentials detection:
- input contains bearer token -> S4/T4 and redaction.

2. PII detection:
- input contains phone/address -> S3/T3.

3. Prompt injection taint:
- web page includes “ignore instructions” -> taint promoted to at least T3.

4. No laundering:
- summarize S3 doc -> output artifact remains tainted (T3) unless sanitized.

5. IPC inheritance:
- IPCArtifact T3 consumed -> receiver operation tainted at least T3.

6. Negative memory:
- never-store secrets are not persisted in raw form in logs/blobs.

