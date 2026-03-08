# Review Queue and Control Plane Workflows Spec v0.1
Adesh OS

This document specifies the **Review Queue** workflow used to govern sensitive state mutations proposed by reflection or external events. It defines:
- what enters the review queue and why
- how review items are represented (diffs, evidence, impact)
- owner decision flows: approve, reject, edit
- atomicity and state version minting
- auditing and idempotency
- how the UI integrates (REST + WS events)

This is algorithmic logic. Not implementation code.

---

## 0) Core principles

1. **Write-gates are real**
Sensitive changes never auto-apply. They become review items.

2. **Owner is the only decider**
Only Root Owner can approve/deny/edit review items.

3. **Diffs must be inspectable**
Every item must show:
- what will change
- why it is proposed (evidence)
- risk/sensitivity impact

4. **Decisions are auditable**
Every decision produces:
- Experience Log event
- AuditTrace entry (or dedicated review audit trace)
- New Active State / Audience Graph / Capability Snapshot version if applied

---

## 1) What goes into the review queue

Review items are created for any proposed mutation that meets one or more:
- affects actuator behavior at R2+
- changes boundaries (never_act, do_not_assume, disclosure rules)
- changes audience graph nodes/edges/scopes or ceilings
- changes identity/security facts (owner, auth config)
- changes budgets or governance posture
- changes sanitization certification or policy
- any mutation touching S2+ data domains
- any deletion requests beyond ephemeral session states

Reflection loop must classify candidates as:
- Auto-accept (low risk)
- Review required
- Forbidden/drop

Only “Review required” becomes review queue items.

---

## 2) ReviewItem data model (logical)

A ReviewItem must contain:

### 2.1 Metadata
- `item_id`
- `created_at`
- `status`: `pending|resolved`
- `source`: `reflection|owner_action|external_agent|system`
- `target_domain`: `active_state|audience_graph|capability_registry|policy_profile`
- `risk_r_estimate` (0..4)
- `sensitivity_s_estimate` (0..4)
- `requires_oob` (bool) if R4 class

### 2.2 Proposed change (diff)
- `change_type`: `add|update|deprecate|delete`
- `target_path`: canonical path (e.g., `active_state.primitives.preference.meeting_time`)
- `before` (optional)
- `after` (proposed)
- `editable_fields[]` for owner edits
- `constraints`:
  - cannot raise ceilings without OOB
  - cannot add wildcard scopes without explicit confirmation

### 2.3 Evidence pack
- `evidence_refs[]` (Experience Log refs, IPC refs)
- `snippets[]` (bounded excerpts)
- `reasoning_summary` (short explanation, not authoritative)
- `confidence` (0..1)
- `conflicts_with[]` (existing items/primitives)

### 2.4 Impact analysis
- predicted effect on gates:
  - may increase/decrease R/S for common actions
- predicted effect on disclosure scopes
- predicted effect on compilation (more/less omissions)

---

## 3) Control plane endpoints (REST)

### 3.1 List pending items
- `GET /v1/review-queue?status=pending`
Returns:
- items summary list with item_id, type, domain, created_at, risk/sensitivity

### 3.2 Get item details
- `GET /v1/review-queue/{item_id}`
Returns full ReviewItem including evidence pack and diff.

### 3.3 Decide item
- `POST /v1/review-queue/{item_id}/decide`
Body:
```json
{
  "decision": "approve|reject|edit",
  "edited_payload": { ... }, 
  "idempotency_key": "string",
  "oob": { "challenge_id": "string|null" }
}
````

Rules:

* Approve applies proposed change as-is.
* Reject marks it rejected and stores reason.
* Edit applies edited_payload instead of proposed `after`.

### 3.4 OOB for R4 decisions

If `requires_oob=true`:

* owner must complete OOB challenge bound to item_id
* single-use consumption semantics identical to approvals.

---

## 4) Decision semantics (deterministic)

### 4.1 Approve

In a single atomic transaction:

1. Lock item
2. Verify item is pending
3. If requires_oob:

   * verify and consume OOB challenge bound to item_id
4. Validate `after` payload against domain schema
5. Apply change:

   * mint new version for the target domain:

     * new `active_state_version` OR `audience_graph_version` OR `capability_snapshot_version`
6. Mark review item resolved with decision=approve and store new_version id
7. Append Experience Log event `review_decision`
8. Emit WS `review_queue_update` + `audit_update`

### 4.2 Reject

Atomic:

* mark resolved=reject
* append Experience Log event
* no new version minted

### 4.3 Edit

Atomic:

* validate edited payload
* apply edited payload as the change
* mint new version
* mark resolved=edit with new_version id
* append Experience Log event

### 4.4 Conflict handling

If base version changed since item created:

* if change is mergeable, apply using merge rules
* otherwise return `409 CONFLICT` and require refresh/rebase:

  * create a new review item with updated before/after

---

## 5) Validation rules for edits

Owner edits must be validated against:

* domain schema (e.g., primitive schema, edge schema)
* negative memory (cannot store forbidden fields)
* governance constraints:

  * cannot lower safety requirements below baseline policy without explicit self-mod flow
  * cannot create wildcard disclosures without explicit confirmation and OOB where required

If edit increases risk:

* may require OOB or a second confirmation.

---

## 6) Auditing requirements

Every review decision must create:

* Experience Log event:

  * includes item_id, decision, hashes of before/after, new_version id if any
* AuditTrace:

  * either:

    * create a dedicated audit trace per review decision
    * or append to a global “state changes” audit log
* WS events:

  * `review_queue_update` with item_id and action

No silent state mutation.

---

## 7) Idempotency and retries

The decide endpoint must support `Idempotency-Key`:

* same key returns same decision response
* no double-minting of versions

If the transaction fails before commit:

* allow retry with same key.

---

## 8) UI integration requirements

UI must provide:

* diff view for before/after
* evidence view with provenance refs
* edit mode constrained by editable_fields
* OOB prompt when required

WS events:

* `review_queue_update` for new items and resolutions

---

## 9) Minimum test cases (must pass)

1. R4 item requires OOB:

* decision without OOB fails
* decision with OOB succeeds once, cannot be replayed

2. Edit payload validation:

* attempt to insert forbidden secret -> rejected

3. Idempotency:

* repeat approve call with same key -> no duplicate version minted

4. Conflict:

* base version changed -> returns conflict and creates new item if configured

5. Audit:

* every decision yields an experience event and references new version if applied
