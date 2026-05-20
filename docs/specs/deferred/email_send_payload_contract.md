# Email Send Payload Contract Spec v0.1
Adesh OS

This document defines the canonical payload shape and normalization rules for the email send actuator used by the deferred email execution slice. It remains canonical for that slice, but it is not part of the active cognitive-sidecar proof path.

This is a behavioral contract for:
- approval diff payloads
- editable payload validation
- syscall argument normalization
- deterministic send previews

This is algorithmic logic. Not implementation code.

---

## 0) Scope

This contract applies to:
- `approval_oob_spec.md` diff-mode approvals for email send
- `tool_provider_port_contract.md` executions where `tool=email` and `action=send`
- any REST or WS payload that surfaces a pending email send proposal

It does not define provider-specific SMTP/API details.

The v0 wedge permits provider configuration to supply the sender identity out of band.

---

## 1) Canonical payload

The canonical email send payload is:

```json
{
  "to": ["string", "..."],
  "cc": ["string", "..."],
  "bcc": ["string", "..."],
  "subject": "string",
  "body": "string"
}
```

Rules:
- `to`, `cc`, and `bcc` are always present.
- `subject` and `body` are always present.
- Unknown fields are forbidden.
- `to` must contain at least one recipient.

The canonical payload does not include `from`.
`from` is provided by the configured email adapter identity and is not editable through diff approval in v0.

---

## 2) Normalization rules

Normalization must occur before:
- approval consumption persists a permitted syscall
- any email send syscall is executed

Rules:
1. recipient strings are trimmed
2. recipient strings are lowercased
3. empty recipient strings are invalid
4. duplicate recipients inside the same field are removed deterministically, preserving first occurrence
5. `subject` is trimmed
6. `body` is trimmed
7. empty `subject` is invalid
8. empty `body` is invalid

No additional rewriting is allowed at this layer.

Examples:
- `" User@Example.com "` -> `"user@example.com"`
- `["a@example.com", "A@example.com"]` -> `["a@example.com"]`

---

## 2.1 Sender identity binding

For the v0 email wedge:
- the sender address is configured at the tool adapter layer
- the sender address is not part of `modified_payload`
- approval diff mode governs destination and content fields only

If the configured sender identity is missing or invalid:
- the email tool adapter must fail closed
- the daemon should refuse to boot if the configured email backend requires a sender identity and it is invalid

---

## 3) Diff-mode approval payload

For email send approvals, `diff_payload` must contain:

```json
{
  "kind": "email_send_payload",
  "tool_id": "email",
  "action": "send",
  "args_schema_ref": "schema:sha256:...",
  "result_schema_ref": "schema:sha256:...",
  "before": null,
  "after": {
    "to": [],
    "cc": [],
    "bcc": [],
    "subject": "",
    "body": ""
  },
  "current_args": {
    "to": [],
    "cc": [],
    "bcc": [],
    "subject": "",
    "body": ""
  },
  "editable_payload_schema": {
    "type": "object",
    "required": ["to", "cc", "bcc", "subject", "body"]
  }
}
```

Rules:
- `tool_id` and `action` identify the capability action that will execute if approved.
- `args_schema_ref` must be the pinned action args schema ref used for validation.
- `result_schema_ref` should be the pinned action result schema ref when available.
- `after` and `current_args` must reflect the same proposed payload before user edits.
- `editable_payload_schema` must expose only the editable fields above.
- if a deterministic diff cannot be produced, the send must be denied in the v0 wedge.

---

## 4) Invalid input behavior

If `modified_payload` fails this contract:
- return `400 INVALID_INPUT`
- include structured validation details per `error_remediation.md`
- do not create a syscall
- do not change approval status
- do not change operation state

---

## 5) Audit requirements

The normalized payload, not the raw client-submitted variant, is the authoritative syscall args payload.

Audit requirements:
- store the normalized payload in the persisted syscall pre-image
- record approval consumption in the audit timeline
- any UI preview may show redacted values, but the canonical stored args must remain deterministic

Execution result requirements for email send:
- include the provider kind in `output_json`
- include the authoritative configured sender address in `output_json`
- include recipient count in `output_json`
- include whether transport-level idempotency is supported
