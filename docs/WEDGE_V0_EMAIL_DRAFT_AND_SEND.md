# Deferred Wedge Brief: Personal Draft and Send (Email Only)

Status: deferred legacy wedge brief. Retained for reference only. This is not the active product proof.

Note:
- the active proof is now `docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md`
- this email wedge is no longer the implementation driver
- keep this document only as a retained record of the earlier execution-oriented slice

## 1) Target user
- Single Root Owner, day-to-day personal use.
- Wants fast drafting plus safe, governed sending.

Environment:
- Adesh OS localhost daemon.
- Root Owner HTTP/WS only.

## 2) Job to be done
“Turn my messy intent and provided context into a high-quality email draft, then send it safely with explicit approval and an audit trail.”

Examples:
- Reply to a long thread with correct tone and constraints.
- Draft a follow-up email based on notes I paste.
- Summarize an attached document and write a response.

## 3) v0 product posture
Draft-first, execute-last.

- Drafting is fast and streaming (R0/R1).
- Sending is a side effect and always staged then approved with a diff (R3).
- No background autonomy.

## 4) Allowed actions in v0 (strict)
### 4.1 Drafting (R0/R1)
- Draft email subject + body.
- Rewrite, shorten, change tone, format.
- Summarize user-provided text into an email.
- Produce multiple variants.

No approvals in R0/R1.

### 4.2 Context inputs (manual only)
Allowed inputs are explicit, user-provided artifacts:
- pasted text
- uploaded files (PDF/TXT/MD/DOCX)
- user-pasted email thread text

Hard rule:
- No “index everything.”
- No mailbox sync.
- No URL crawling.

The system may store these inputs as immutable artifacts for provenance, but does not require full ingestion jobs in v0.

### 4.3 Send flow (R3, diff approval)
- Prepare a send syscall payload:
  - to, cc, bcc, subject, body
- Show a deterministic diff (final payload).
- Require explicit approval to execute send syscall.

If deterministic diff cannot be produced, sending is denied in v0.

## 5) Forbidden in v0 (hard exclusions)
These are explicitly out of wedge.

- No WhatsApp/Telegram/Slack/Teams adapters.
- No local CLI execution (Gemini CLI etc.).
- No calendar actions.
- No file operations (rename/move/delete).
- No sandboxed actuators.
- No reflection automation that silently alters behavior.
- No automatic claim promotion beyond explicitly confirmed trivial formatting preferences.
- No audience graph editing UI.
- No `WorkflowSpec` runtime orchestration layer.
- No `InterfaceSpec`/`InterfaceInstance` composition runtime beyond the current fixed local UI shell.

## 6) Audience model for v0
v0 has only one audience: Root Owner.

Outbound recipients are treated as destination fields for a send syscall, not as “audiences” that can query the OS. Default deny still applies to any external agent connections, but v0 does not build that surface.

Privacy rule:
- Draft content must never include restricted artifacts unless explicitly provided in the same request or explicitly selected by the user.

## 7) Required hot path components (minimal)
The wedge must exercise only the kernel physics required to draft and send:

- request → operation → gate → compile → model → verify → approval → syscall → audit
- StorageProvider:
  - idempotency
  - operation state + leases
  - approval items + OOB scaffolding (OOB not required unless you treat sending as R4, which v0 does not)
  - audit trace anchors
- ModelProvider + structured ReasoningOutput
- Verification Core:
  - schema enforcement
  - “high-stakes facts require evidence” rule applies only to factual assertions explicitly grounded in provided artifacts
  - ordinary drafting does not require accepted claims from the Fact Ledger; provided artifacts are sufficient grounding input in v0
- ToolProvider:
  - email_send actuator behind diff approval

Explicitly not required for v0:
- ingestion jobs
- fact ledger automation
- reflection loop
- sandbox execution
- external agent MCP host surface

## 8) Approval budget KPI (product constraint)
- Drafting flows: approvals per session = 0.
- Sending: approvals per send = 1 (diff approve).
- If approvals occur during drafting, treat as a regression.

## 9) Success metrics after 30 days (measurable)
Productivity:
- Time-to-draft p50 < 2 minutes.
- Acceptance rate: >60% of drafts used with minimal edits.
- Normalized edit distance median < 0.25.

Trust:
- “Send regret rate” < 2% (user says “this should not have been sent”).
- Provenance coverage metric applies only when the user supplied artifacts:
  - For artifact-grounded summaries used in drafts: provenance coverage > 90%.
  - Pure stylistic drafting excluded from provenance metric.

Operational:
- 0 duplicate sends under retries (idempotency).
- 0 audit-fail-open events.

## 10) Scope enforcement rule
Any PR that adds:
- new channels (WhatsApp/Telegram/etc.)
- CLI execution
- calendar actions
- file operations
- ingestion pipelines beyond manual artifact attach
- sandboxed execution
- reflection auto-learning
must be rejected unless a new wedge doc is written and explicitly approved.

This wedge brief is the v0 scope lock.
