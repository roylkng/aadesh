# Fact Ledger and Reflection Claims Spec v0.1
Adesh OS

This document specifies the durable “Fact Ledger” layer that sits between:
- Ingestion (immutable artifacts)
- Reflection (candidate extraction)
- Active State (governed versions consumed by operations)
- Verification (evidence-backed enforcement)

Purpose:
- prevent “conjecture becoming truth”
- preserve provenance for every durable claim
- make claim conflicts explicit and resolvable
- support deterministic retrieval for JIT compilation and replay

This is a canonical spec. Not implementation code.

---

## 0) Core principles

1) Claims are typed, versioned, and evidence-backed
No durable claim exists without references to immutable artifacts/events.

2) Claims are probabilistic until promoted
Reflection produces candidates. Only promotion writes “accepted claims” into the ledger.

3) Conflicts are explicit
Two contradictory claims can coexist with scoped predicates and evidence. Resolution is governed.

4) Verification depends on the ledger
Any high-stakes output or syscall justification must be able to cite claim evidence or refuse.

---

## 1) Definitions

### 1.1 Artifact
Immutable ingested or derived object (see `artifact_normalization_contract.md`), referenced by `artifact_id` and `content_ref`.

### 1.2 Claim
A typed proposition about the persona/OS environment, stored with provenance and confidence.

### 1.3 Candidate claim
A claim proposal created by reflection that is not yet promoted.

### 1.4 Promotion
Governed transition that makes a candidate claim “accepted” in the ledger.

---

## 2) Claim types (open-ended, but structured)

Minimum v0.1 types:

- `fact`: stable factual assertions (name, role, definitions). Must be conservative.
- `preference`: choices (style, scheduling preferences, formatting).
- `boundary`: “never act”, “do not assume”, disclosure rules.
- `relationship`: audience graph related facts (who is who, group membership) as claims before graph patch.
- `procedure`: repeatable workflows (tax filing steps, weekly review routine).
- `capability_assumption`: constraints about tools/models/environment (“cannot access X”, “only runs offline”)

Each claim must declare:
- `claim_type` (string)
- `claim_key` (string namespace)
- `claim_value` (JSON)
- `context_predicates` (JSON)
- `time_bounds` (optional)

---

## 3) Claim record schema (logical)

A claim record must include:

### Identity
- `claim_id` (stable)
- `claim_type`
- `claim_key`

### Content
- `value_json`
- `context_predicates_json` (may be empty)
- `time_start`, `time_end` (optional)

### Evidence and provenance
- `evidence_refs[]` where each ref points to:
  - `artifact_id` and optional locator (page/segment/message_id)
  - or `experience_event_ref`
- `evidence_quality`:
  - source reliability tier
  - recency score
  - conflict count
  - user_confirmed flag

### Lifecycle and status
- `status`: `candidate|accepted|deprecated|rejected`
- `created_at`
- `updated_at`
- `created_by`: `reflection|owner`
- `promotion_ref` (review/approval ids if applicable)

### Confidence
- `confidence` float [0..1]
- `confidence_reason_codes[]`

---

## 4) Candidate extraction (reflection output contract)

Reflection workers may produce candidate claims only.

Rules:
- Candidate generation is asynchronous.
- Candidate claims must be idempotent per `(claim_key, value_hash, context_hash, time_bounds, evidence_set_hash)`.
- Candidate claims must never directly mutate:
  - Active State versions
  - Audience Graph versions
  - Capability snapshots

Candidate claims enter:
- review queue if they are sensitive/high impact
- or auto-promote only if strictly low-risk (formatting preferences) under policy.

---

## 5) Promotion and governance rules

### 5.1 Promotion gates
Promotion to `accepted` requires:

- For `boundary` claims: review queue approval, often OOB if it changes safety posture.
- For `relationship` claims that affect disclosure: review queue + Audience Graph patch flow.
- For `fact` claims: conservative promotion policy; require high evidence quality.
- For `preference` claims: may auto-promote if low-risk and repeatedly observed.

### 5.2 Promotion gate table

Minimum deterministic policy:

| claim_type | min_gate | requires_review | requires_oob |
|---|---:|---|---|
| `preference` | 0 | no for low-risk repeated observations; yes otherwise | no |
| `fact` | 1 | yes at gate >= 2 or when downstream impact is high-stakes | no by default |
| `boundary` | 2 | yes | yes when it changes safety posture or actuator permissions |
| `relationship` | 2 | yes | yes when it changes disclosure scope or audience trust |
| `procedure` | 2 | yes when it can influence R2+ actions | no by default |
| `capability_assumption` | 2 | yes | yes when it changes safety or execution constraints |

If a claim changes disclosure, execution safety, or approval posture, the stricter row applies.

### 5.3 promotion_ref format

`promotion_ref` must be one of:
- `review_item:<id>`
- `approval:<id>`
- `owner_event:<event_ref>`

No other `promotion_ref` forms are valid in v0.1.

### 5.4 Conflict resolution (deterministic)
If two accepted claims conflict at runtime, the compiler resolves using:

1) Applicability: context predicate match
2) Constraint severity: boundaries dominate preferences unless explicit exception
3) Evidence quality tier: user-confirmed > repeated observation > single artifact
4) Specificity: more specific context beats less specific
5) Recency: tie-break only within same tier and specificity

(This matches your v1.5 tie-breaker logic.)

### 5.5 Deprecation, not deletion
Claims are deprecated, not deleted, except under explicit R4 deletion with tombstones (see retention spec).

---

## 6) JIT compiler consumption rules

The compiler must assemble persona slices by querying accepted claims:
- filtered by operation gate (R/S thresholds)
- filtered by audience ceiling and scope
- filtered by context predicates and time bounds
- with conflict resolution applied deterministically

For any claim injected into the CompiledSlice:
- include pointers to evidence refs (hard provenance)
- include confidence and quality fields

---

## 7) Verification requirements

Verification must be able to enforce:
- “If this output asserts X as fact, show claim evidence or downgrade to conjecture.”

Policy:
- Facts in high-stakes operations must be:
  - either directly evidenced by accepted claims
  - or refused / require user confirmation

---

## 8) Storage schema delta (logical)
You may implement the ledger as:
- `claims` table
- `claim_evidence` join table (optional)
- `claim_conflicts` (optional)
Or as JSON in a single table, but must support:
- lookup by claim_key + predicates
- status filtering
- conflict detection

---

## 9) Minimum acceptance tests

1) Conjecture cannot auto-promote to accepted fact without evidence.
2) Conflicting accepted claims resolve deterministically with the stated hierarchy.
3) High-stakes outputs that claim unsupported facts are downgraded or refused.
4) Claim promotion is auditable and versioned.
