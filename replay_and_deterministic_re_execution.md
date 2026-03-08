Goal understood: continue the documentation set with the next highest-leverage spec that eliminates ambiguity for the coding agent.

Next doc should be **Replay and Deterministic Re-Execution** because:

* you already expose `/v1/audit/{audit_trace_id}/replay`
* determinism, pinning, and “dry_run vs full” are correctness-critical
* it forces you to specify exactly what is re-used vs recomputed, and how actuators are stubbed

Here is the repo-ready doc.

````md id="r8p4k1"
# Replay and Deterministic Re-Execution Spec v0.1
Adesh OS

This document specifies the **Replay subsystem** for Adesh OS. It defines:
- what “replay” means in Adesh OS
- replay modes: `dry_run` vs `full`
- deterministic reconstruction using pinned versions and stored artifacts
- tool execution stubbing and syscall simulation rules
- required storage anchors and failure behavior
- replay output artifacts and audit linkage

This is algorithmic logic. Not implementation code.

---

## 0) Goals and non-goals

### Goals
1. **Deterministic accountability**
- Given an `audit_trace_id`, the OS can reconstruct what happened and why.

2. **Safe dry-run**
- Replay in `dry_run` must never perform external side effects.
- It must still run governance, compilation, verification, and syscall gating.

3. **Actionability**
- Replay must produce a structured report describing:
  - which decisions were deterministic
  - which decisions depended on model randomness
  - which syscalls would be permitted/denied today and under the pinned state

### Non-goals
- Replay does not guarantee identical model text generation unless the underlying model is deterministic and supports deterministic sampling or recorded outputs are used.
- Replay does not bypass approvals. Full replay is still subject to gates.

---

## 1) Replay entry point

Endpoint:
- `POST /v1/audit/{audit_trace_id}/replay`
Body:
```json
{ "mode": "dry_run|full", "override_budgets": { "token_budget": 4096 } }
````

Replay produces:

* `replay_id`
* new `audit_trace_id` for replay session
* `status` `running|completed|failed`

---

## 2) Required stored anchors (preconditions)

Replay is permitted only if the original operation stored:

* pinned versions:

  * `active_state_version`
  * `capability_snapshot_version`
  * `audience_graph_version`
* `GateDecision` ref (or persisted object)
* `CompiledSlice` ref (or persisted object)
* final reasoning output ref (structured JSON `ReasoningOutput`)
* all approvals and modified_payloads (if any)
* all syscalls and their results/denies (if any)
* all IPCArtifact refs used

If any required anchor is missing:

* replay fails closed with a structured failure reason
* audit timeline records `replay_failed_missing_anchor`

---

## 3) Replay modes and safety rules

### 3.1 dry_run mode (default safe)

* No actuator syscalls are executed.
* All actuator syscalls are treated as “simulated.”
* Sensor syscalls:

  * default: do not execute
  * may be executed only if explicitly marked `safe_replay_sensor=true` in capability descriptor and does not increase sensitivity beyond original
* Sanitizer syscalls:

  * may be simulated unless deterministic sanitizer exists

### 3.2 full mode (dangerous)

* Full mode may execute actuators only if:

  * Root Owner is present
  * required approvals and OOB are obtained again unless policy says approvals can be reused (default: must re-approve)
  * replay operation recomputes gates and does not exceed original max_gate without explicit escalation approval

Default policy:

* full replay requires explicit user approval even if original already approved.

---

## 4) Replay algorithm (deterministic steps)

Replay treats the original operation as an input and reconstructs a new “replay operation” that is clearly marked as replay.

### Step 4.1: Create replay session

* Create `replay_id`
* Create a new operation record:

  * `operation_id = replay_op:<uuid>`
  * `isolation_id = replay_iso:<uuid>`
  * `operation_goal.summary = "Replay of audit_trace_id=..."`
  * pin the original versions (no “current” versions)
* Create `AuditTrace` for replay session.

### Step 4.2: Load original anchors

Load from storage:

* original GateDecision
* original CompiledSlice
* original ReasoningOutput (structured)
* original syscall set (envelopes and results/denies)
* original approvals/OOB references

### Step 4.3: Choose replay source of truth

Replay can proceed using two strategies:

#### Strategy A: Deterministic replay from stored outputs (default)

* Use the original persisted ReasoningOutput as the model output.
* Do not call the model.

Pros:

* Maximum determinism.
  Cons:
* Does not test “what would the model say today.”

#### Strategy B: Re-run model using stored CompiledSlice (optional)

* Call ModelProvider with the original CompiledSlice.
* Requires deterministic sampling settings and model version pinning if possible.

Policy:

* Default to Strategy A unless explicitly requested.

### Step 4.4: Re-run verification

Run Verification Core on:

* GateDecision (pinned)
* CompiledSlice (pinned)
* ReasoningOutput (from Strategy A or B)

Verification outputs:

* syscall classification:

  * permitted_now
  * awaiting_approval
  * denied with SyscallDeny
* drift checks are executed again, but must be annotated as “replay verification”

Record:

* `verification_report` artifact in Experience Log or blob
* audit timeline entry

### Step 4.5: Syscall simulation/execution

For each syscall proposed/verified:

#### In dry_run

* Do not execute actuators.
* Create simulated syscall result records:

  * status `simulated`
  * include what would have been executed and why it was permitted/denied
* If syscall is denied, persist SyscallDeny as normal.

#### In full

* Execute only after approvals:

  * approvals must be acquired anew unless explicit policy allows reuse
  * OOB must be acquired anew for R4
* Execute syscalls using ToolProvider, respecting the same persistence ordering as normal execution.

### Step 4.6: Produce replay report

Generate a deterministic replay report artifact:

* differences from original:

  * gate differences (should be none if pinned)
  * verification differences (may change if rules changed; must be versioned)
  * syscall outcomes:

    * original executed vs replay simulated/denied
* note: if Strategy B used, include model output diff metrics (edit distance)

Persist report and link in replay AuditTrace.

### Step 4.7: Complete replay operation

* transition replay operation to completed/failed
* emit WS events:

  * operation_state
  * audit_update

---

## 5) Replay output artifacts (required)

Replay must persist:

1. `replay_report` (structured JSON)
2. `verification_report` (structured JSON)
3. `simulated_syscall_results` (if dry_run)
4. any `SyscallDeny` objects created during replay
5. updated `AuditTrace` with anchors

All must be referenced in replay audit trace attachments.

---

## 6) Gate and policy versioning for replay

Replay correctness depends on policy code versions.

The replay report must include:

* `kernel_version`
* `governance_ruleset_version`
* `compiler_version`
* `verification_ruleset_version`

If any of these differ from the original:

* replay must annotate results as “policy-different replay”
* it must not claim to be identical reproduction

---

## 7) Approvals during replay

### 7.1 dry_run

* approvals are never consumed
* if a syscall would require approval, report it as such

### 7.2 full

Default: require approvals again.

* Because approvals are human intent at a point in time.
* Reusing an approval is a security risk.

Exception (optional future):

* allow reuse of approvals only if:

  * same Root Owner session and short window
  * same exact syscall args hash
  * explicit “reuse approvals” policy is enabled

---

## 8) Sensitivity, taint, and IPC in replay

* Replay must respect the same taint rules:

  * simulated outputs carry the same taint
* IPC artifacts referenced must be loaded from storage and used exactly as in original
* No implicit new IPC is allowed in replay unless Strategy B re-plans and verification allows it, in which case it must be marked clearly as “divergent replay.”

---

## 9) Failure modes

Replay fails closed if:

* missing anchors
* pinned versions cannot be loaded
* stored reasoning output cannot be parsed/validated
* storage writes for replay audit fail

In all cases:

* record failure in replay audit trace
* return structured failure response

---

## 10) Minimum test cases (must pass)

1. Dry-run never executes actuators:

* create a replay for an operation that sent an email
* ensure no email actuator is called, and syscalls are marked simulated.

2. Deterministic replay from stored outputs:

* replay produces identical syscall proposals and denials as original (when using Strategy A).

3. Policy difference annotation:

* bump verification ruleset version
* replay must mark output as policy-different.

4. Full replay requires re-approval:

* try full replay without approval
* must park in awaiting_approval and not execute.

5. Missing anchor:

* delete compiled slice record
* replay fails with missing anchor reason.

```
```
