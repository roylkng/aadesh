````md id="z5q1p0"
# Sanitization Subsystem (Certified Sanitizers, Sensitivity Reduction Proofs) Spec v0.1
Adesh OS

This document specifies the **Sanitization subsystem** used to safely reduce disclosure risk. It defines:
- what a sanitizer is (as a capability)
- when sanitization is required (by governance/verification)
- artifact types produced (`sanitized_view`)
- how sensitivity and taint may (and may not) be reduced
- how verification confirms sanitization success (proof obligations)
- certified vs non-certified sanitizers
- audit and replay requirements

This is algorithmic logic. Not implementation code.

---

## 0) Core principles

1. **No implicit sanitization**
- Summarization does not count as sanitization.
- Sanitization must be an explicit syscall or a dedicated internal tool.

2. **Sanitization is evidence-based**
- Verification must be able to justify why sensitivity/taint was reduced.

3. **Certified sanitizers are required to reduce taint**
- Default: sanitization may reduce sensitivity labels, but taint remains unless a certified sanitizer is used and verified.

4. **Sanitization is scoped**
- Sanitization must declare the target audience ceiling and allowed scopes.
- “Sanitize for public” is different from “sanitize for vendor.”

---

## 1) Sanitizer capability descriptor

Sanitizers are registered as tools with:
- `target.kind = sanitizer`
- `risk_floor_r`: at least R2 (it can change what content is shared)
- `diff_supported`: true if sanitization policy can be previewed
- `certification_level`:
  - `none` (non-certified)
  - `certified` (trusted to reduce taint under policy)
  - `regulated` (stronger, optional future)
- `supported_policies[]`:
  - `redact_pii`
  - `remove_credentials`
  - `generalize_numbers`
  - `topic_filter`
  - `scope_filter`
- `schema_ref` for sanitizer actions

---

## 2) Sanitization syscall contract

### 2.1 Canonical action: `sanitize`
Syscall intent fields:
- `source_handles[]`: artifacts/events/blobs to sanitize
- `policy`:
  - redaction rules
  - scope rules
  - sensitivity ceiling target
- `target`:
  - `artifact_kind = sanitized_view`
  - `target_ceiling_s`
  - `target_scopes[]`

Example shape (conceptual):
```json
{
  "target": { "kind": "sanitizer", "name": "default_sanitizer" },
  "action": "sanitize",
  "args": {
    "source_handles": [{ "handle": "artifact:123", "handle_type": "artifact_id" }],
    "target_ceiling_s": 1,
    "target_scopes": ["work:status_updates"],
    "policy": {
      "remove": ["pii", "credentials", "internal_project_names"],
      "generalize": ["numbers", "dates"],
      "allow_topics": ["high_level_summary"]
    }
  }
}
````

### 2.2 Output

Sanitizer must produce:

* a new blob content_ref (sanitized content)
* an `IPCArtifact` of kind `sanitized_view` referencing that blob
* a **SanitizationReport** (structured proof) persisted and referenced

---

## 3) SanitizationReport (proof obligations)

SanitizationReport is mandatory. It allows verification and audits.

Minimum fields:

* `report_id`
* `sanitizer_tool` and version
* `source_handles` and hashes
* `target_ceiling_s` and `target_scopes`
* `rules_applied[]` (stable names)
* `detectors_run[]` and results:

  * pii detector counts
  * credentials detector counts
  * sensitive topic detector counts
* `redactions_summary`:

  * count of redactions by category
  * sample-free (do not include raw redacted content)
* `residual_risk`:

  * `low|medium|high`
  * reasons
* `recommended_labels`:

  * `recommended_sensitivity_s`
  * `recommended_taint_s` (only if certified)
* `verification_notes` (optional)

The report must not include raw sensitive text.

---

## 4) When sanitization is required

Verification must require sanitization when:

* outbound syscall target ceiling < operation taint
* IPC artifact scope tags do not match audience scopes
* content includes PII/credentials but target audience is external/public
* any publish/public action would leak non-public data

Sanitization is recommended when:

* gate >= 3 and external communications are proposed
* the model uses inline_text handles with sensitivity >= S2

---

## 5) Sensitivity and taint reduction rules

### 5.1 Sensitivity reduction

After sanitization, sensitivity may be lowered if:

* SanitizationReport shows detectors for prohibited categories are zero in output
* verification confirms scope alignment
* the sanitizer policy explicitly removed high-sensitivity elements

Default rule:

* `S_sanitized <= target_ceiling_s`
* but never below S1 unless explicitly marked public and verified.

### 5.2 Taint reduction (strict)

Taint reduction is more dangerous.

Default:

* `T_sanitized = max(T_source)` (no reduction)

Taint may be reduced only if:

* sanitizer has `certification_level=certified`
* sanitizer provides evidence that output is independent of restricted details beyond allowed abstractions
* verification confirms:

  * no restricted tokens remain
  * no high-risk identifiers remain
  * no “leak by inference” patterns detectable (heuristic)

Even then:

* taint can only be reduced down to `target_ceiling_s`, not below.

If sanitizer is non-certified:

* it may reduce sensitivity but not taint.

---

## 6) Verification of sanitization success

Verification must:

1. Validate SanitizationReport schema and hashes
2. Re-run OS-side detectors on sanitized output (bounded sampling) to confirm:

   * no credentials
   * no PII beyond allowed policy
   * no forbidden topics if filtered
3. Check that `recommended_sensitivity_s` and `recommended_taint_s` are justified by detectors
4. Confirm artifact scope tags:

   * `audience_scope_tag.allowed_scopes` is a subset of target scopes
   * `max_disclosure_s` <= target_ceiling_s

If verification fails:

* deny outbound syscall with `taint_laundering_risk` or `verification_failed`
* remediation: adjust sanitization policy, ask user, refuse

---

## 7) Interaction with IPC artifacts

Sanitized output must be represented as:

* `IPCArtifact.kind = sanitized_view`
* with:

  * `sensitivity_s` and `taint_s` labels after verification
  * `audience_scope_tag` matching target scopes and ceiling
  * provenance refs linking to source artifacts and sanitization report

Outbound syscalls to lower ceilings must reference only sanitized_view artifacts.

---

## 8) Audit and replay requirements

### 8.1 Audit anchors

AuditTrace for operations involving sanitization must reference:

* sanitizer syscall envelope
* sanitization report
* sanitized_view IPC artifact id
* any outbound syscalls that consumed it

### 8.2 Replay behavior

In dry_run replay:

* sanitization may be simulated only if:

  * sanitized artifact and report exist
  * replay uses stored artifacts
    Otherwise, replay reports that sanitization would be required and blocks outbound.

In full replay:

* sanitizer syscalls may execute, but must be audited and approved if gate requires.

---

## 9) Minimum test cases (must pass)

1. Public publish from S3 sources:

* must require sanitization and deny if absent.

2. Non-certified sanitizer:

* may not reduce taint, only sensitivity. Outbound may still be denied if taint exceeds ceiling.

3. Certified sanitizer:

* may reduce taint if report + verification confirm. Outbound permitted only then.

4. Detector confirmation:

* output still contains PII -> sanitization fails and outbound blocked.

5. Scope tags:

* sanitized_view tagged work scope cannot be sent to public edge.

```
```
