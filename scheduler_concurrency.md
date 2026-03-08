# Scheduler Concurrency and Operation Runner Semantics Spec v0.1
Adesh OS

This document specifies the deterministic concurrency model for Adesh OS’s Scheduler and Operation Runner. It defines:
- how operations are leased and advanced safely
- how to prevent duplicate execution and double-advancement
- crash recovery semantics
- interaction with storage transaction boundaries
- how background reflection workers coexist without mutating in-flight pinned state
- how to handle parallel operations without cross-contamination

This is algorithmic logic. Not implementation code.

---

## 0) Core principles

1. **Single-writer per operation**
At any time, only one runner may advance a given `operation_id`.

2. **Lease-based concurrency**
Concurrency control must be enforced using leases and timeouts, not best-effort in-memory locks.

3. **Storage is the arbiter**
The DB must be able to prove which runner currently owns the right to advance an operation.

4. **Crash-safe**
A daemon crash must not allow:
- duplicated syscalls
- skipped approvals
- invisible partial state

5. **Operation isolation**
Parallel operations cannot share memory. Any exchange must go through IPCArtifacts.

---

## 1) Operation runner roles

### 1.1 Scheduler
- owns the queue of runnable operations
- acquires leases for operations
- invokes the sync execution loop stages (governance, compile, model, verify, approvals, syscalls)

### 1.2 Reflection workers
- consume jobs from JobQueue
- mint new Active State versions
- create review queue items
- must never change pinned versions of in-flight operations

---

## 2) Required storage fields (logical)

Operations must have lease metadata (whether stored in the `operations` table or a separate `operation_leases` table):

- `lease_owner` (runner_id)
- `leased_until` (timestamp)
- `lease_epoch` (monotonic int, optional but recommended)
- `last_heartbeat_at` (timestamp, optional)

These fields must be updated atomically.

---

## 3) Leasing protocol (deterministic)

### 3.1 Runner identity
Each runner instance has:
- `runner_id` stable for process lifetime (uuid)

### 3.2 Lease acquisition
To acquire a lease for `operation_id`:

Acquire only if:
- `leased_until` is NULL or < now (expired)
OR
- `lease_owner == runner_id` (renewal)

Atomic update:
- set `lease_owner = runner_id`
- set `leased_until = now + lease_duration`
- increment `lease_epoch` (if used)

If acquisition fails:
- runner must not advance the operation.

### 3.3 Lease renewal
Runner must renew periodically while processing:
- update `leased_until`
- optional `last_heartbeat_at`

If renewal fails:
- stop processing and release resources.

### 3.4 Lease release
On completion or terminal state:
- clear lease fields or allow expiry
- record transition in operation_transitions

---

## 4) Runnable operation selection

An operation is runnable if:
- state in {`created`, `running`, `blocked`} AND
- not in `awaiting_approval` AND
- not terminal (`completed|failed|cancelled`) AND
- lease is free or expired

Additionally:
- `blocked` is runnable only if its blocking condition is resolved:
  - new approval decision exists
  - required IPCArtifact exists
  - retry cooldown passed

The scheduler must enforce a deterministic priority ordering:
1. operations unblocked by a recent user action (approval/clarification)
2. operations older by created_at
3. operations with lower gate first (optional fairness rule)

---

## 5) Stage advancement and idempotent side effects

### 5.1 Stage markers
The runner must treat the sync loop as a sequence of persisted stage markers:
- created
- gate_computed
- compiled
- reasoning_done
- verified
- awaiting_approval
- executing_syscalls
- completed/failed/blocked

These stage markers can be represented as:
- `operations.state` plus structured `state_reason`
- or an internal stage field

### 5.2 Idempotent stage execution
Each stage must be idempotent under retry:
- if a crash occurs after persisting GateDecision, re-running must reuse the persisted GateDecision, not recompute unless explicitly requested.
- same for CompiledSlice, reasoning output, and syscalls.

Rule:
- if a stage artifact exists in storage and matches pinned versions, reuse it.
- if it exists but is inconsistent, treat as corruption and fail closed.

---

## 6) Crash recovery semantics

### 6.1 Runner crash mid-operation
If runner dies:
- lease expires
- another runner may acquire the lease and resume from persisted artifacts

Resume rules:
- never repeat an actuator syscall unless:
  - syscall status indicates it was not executed
  - or actuator supports idempotency via syscall_id token
  - or user explicitly approves reattempt

### 6.2 Runner crash mid-syscall
If syscall record exists but status is ambiguous:
- mark syscall as `failed` with `tool_execution_failed`
- produce SyscallDeny or remediation requiring user confirmation for retry
- do not blindly re-execute

Preferred:
- persist an `executing` status and heartbeat timestamp for syscalls too, but not required in v0.1.

---

## 7) Parallelism constraints

### 7.1 Parallel operations allowed
Multiple operations may run concurrently if:
- they are different `operation_id`s
- they do not share sensitive artifacts implicitly

### 7.2 Shared resource backpressure
Scheduler must enforce backpressure limits:
- max concurrent model calls
- max concurrent syscalls per actuator
- global budget ceilings

These limits belong to the capability registry and runtime config.

---

## 8) Interaction with approvals

### 8.1 Awaiting approval is a hard stop
If operation state is `awaiting_approval`:
- runner must not proceed
- lease may be released early to avoid idle occupation

### 8.2 Approval consumption unblocks operation
When an approval is granted:
- it must be persisted (ApprovalItem status)
- operation transition to running must be recorded
- runner selection must prioritize newly unblocked operations

---

## 9) Interaction with IPC

### 9.1 IPC gating
If an operation requires IPC artifacts:
- it must not proceed until the IPCArtifact exists in storage.

### 9.2 No implicit IPC
Runner must reject any attempt to consume artifacts not referenced in OperationSpec.ipc or newly authorized by scheduler updates.

---

## 10) Reflection coexistence and pinned state protection

Reflection workers may:
- create new `active_state_version`
- add review queue items
- propose audience graph patches

They must not:
- mutate pinned versions inside existing operation records
- mutate existing CompiledSlice or GateDecision artifacts

If reflection updates are needed for an in-flight operation:
- create a new operation (or require explicit “recompile using latest state” action)

---

## 11) Required invariants for correctness

The implementation is compliant only if:

1. Duplicate syscalls cannot execute due to concurrent runners.
2. An operation cannot advance without an active lease.
3. Every stage artifact (GateDecision, CompiledSlice, SyscallEnvelope) is persisted before any side effect.
4. Crash recovery resumes from persisted stage artifacts and never repeats irreversible side effects without explicit approval.
5. Awaiting_approval is a strict stop.
6. Reflection does not mutate pinned state of in-flight operations.

---

## 12) Minimum test cases (must pass)

1. Two runners attempt same operation:
- only one acquires lease and advances.

2. Lease expiry and recovery:
- runner A acquires lease and crashes.
- runner B acquires after expiry and resumes without duplication.

3. Crash after syscall envelope persisted, before execution:
- resume does not lose syscall record; execution behavior follows retry policy.

4. Approval unblocks:
- operation in awaiting_approval is skipped until approval committed, then prioritized.

5. Reflection updates:
- reflection mints new active_state_version; in-flight operation pinned version unchanged.

