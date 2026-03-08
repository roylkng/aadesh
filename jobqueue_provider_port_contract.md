```md id="t7k3m9"
# JobQueue Port Contract Spec v0.1
Adesh OS

This document defines the **JobQueue** port contract used for asynchronous work (reflection loop, compaction, periodic health checks). It specifies:
- job record model
- enqueue semantics
- lease/ack/fail semantics (at-least-once delivery)
- retry and backoff rules
- idempotency and deduping
- observability requirements

This is interface and logic documentation. Not implementation code.

---

## 0) Core principles

1. **At-least-once delivery**
Jobs may be delivered more than once. Workers must be idempotent.

2. **Lease-based concurrency**
Only one worker may process a job at a time (via leases).

3. **Durable state**
Job state must be persisted. No in-memory queues as the only source of truth.

4. **Backoff and dead-lettering**
Failed jobs must backoff and eventually move to a terminal state or dead-letter.

---

## 1) Job model (logical)

A job must have:

### 1.1 Identity
- `job_id` (unique)
- `job_type` (string enum-like)
- `dedupe_key` (optional, used to coalesce duplicates)

### 1.2 Payload
- `payload_json` (structured)
- `sensitivity_s` (0..4)
- `taint_s` (0..4)
- `provenance_refs[]` (event refs, operation ids)

### 1.3 Scheduling
- `created_at`
- `run_after` (timestamp, for delayed retries)
- `priority` (int, optional)

### 1.4 Leasing
- `lease_owner` (worker_id)
- `leased_until` (timestamp)
- `lease_epoch` (optional)

### 1.5 Attempt tracking
- `attempt_count`
- `max_attempts`
- `last_error_code`
- `last_error_message` (redacted)

### 1.6 State
- `status`: `pending|leased|completed|failed|dead_lettered|cancelled`

---

## 2) Required job types (minimum)

- `reflection.process_events`
- `reflection.compact_candidates` (optional)
- `retention.gc` (optional)
- `capabilities.refresh` (optional)
- `integrity.check` (optional)

The system is open-ended; job_type is not a fixed taxonomy, but these are required initially.

---

## 3) Port methods (conceptual interface)

### 3.1 enqueue_job
Inputs:
- `job_type`
- `payload_json`
- optional `dedupe_key`
- `run_after` optional
- `max_attempts` default per job_type
- `sensitivity_s`, `taint_s`, `provenance_refs`

Semantics:
- persists job record in `pending` state
- if `dedupe_key` provided and an identical pending job exists:
  - either return existing job_id (preferred)
  - or coalesce by updating run_after to earliest
- never drop silently

Errors:
- InvalidInput if payload invalid or too large
- Db/Io errors otherwise

### 3.2 lease_jobs
Inputs:
- `worker_id`
- `limit`
- `lease_duration_ms`
- optional filters (job_type)
Output:
- list of leased jobs (job_id + payload + metadata)

Semantics:
- atomically select runnable jobs where:
  - status = pending
  - run_after <= now
  - leased_until is null or expired
- set:
  - lease_owner = worker_id
  - leased_until = now + lease_duration
  - status = leased
  - lease_epoch++ (if supported)

### 3.3 renew_lease
Inputs:
- job_id, worker_id, lease_epoch, lease_duration_ms
Semantics:
- extend leased_until only if still owned by worker and epoch matches
- otherwise return Conflict

### 3.4 ack_job
Inputs:
- job_id, worker_id, lease_epoch
Semantics:
- mark job completed
- clear lease fields
- persist completion timestamp
- append experience event (optional) or at least log/metric

### 3.5 fail_job
Inputs:
- job_id, worker_id, lease_epoch
- error_code, error_message (redacted)
- retry_after_ms (optional override)
Semantics:
- increment attempt_count
- if attempt_count >= max_attempts:
  - mark dead_lettered or failed (policy)
- else:
  - set status pending
  - set run_after = now + backoff(attempt_count) or retry_after_ms
  - clear lease fields

### 3.6 cancel_job (optional)
Used for user-requested cancellations.

---

## 4) Backoff policy (deterministic)

Default backoff function:
- exponential with jitter disabled (deterministic) for reproducibility, or bounded jitter if desired but must be recorded.

Example deterministic:
- `delay_ms = min(base_ms * 2^(attempt_count-1), max_ms)`

Job_type may override base and max.

---

## 5) Idempotency and dedupe

### 5.1 Worker idempotency requirement
Workers must be idempotent because:
- jobs may be redelivered after lease expiry
- ack may fail after execution and job may be retried

Each job payload should include stable refs (event_refs, operation_id) that allow the worker to detect it has already applied the outcome.

### 5.2 dedupe_key semantics
If `dedupe_key` is used:
- it must be unique for “same intended work”
- JobQueue should not create duplicates when dedupe_key matches and job is pending/leased
- If prior job completed, new job may be enqueued.

---

## 6) Sensitivity and taint handling

Jobs carry sensitivity/taint labels:
- payload_json must not include forbidden secrets
- workers must treat job payload as tainted per label
- logs must redact sensitive fields

Reflection jobs often carry S2+ and must be handled accordingly.

---

## 7) Observability requirements

JobQueue must emit:
- `jobs_enqueued_total` (by type)
- `jobs_leased_total`
- `jobs_completed_total`
- `jobs_failed_total`
- `jobs_dead_lettered_total`
- `job_lease_conflicts_total`

Each job processing span/log must include:
- `job_id`
- `job_type`
- provenance refs
- operation_id if present

---

## 8) Minimum acceptance tests (must pass)

1. Lease exclusivity:
- two workers lease concurrently; no job leased twice.

2. Lease expiry recovery:
- worker A leases, crashes; worker B leases after expiry.

3. At-least-once:
- ack fails; job reappears; worker idempotently handles.

4. Backoff:
- fail_job increments attempt_count and schedules run_after deterministically.

5. Dedupe:
- enqueue same dedupe_key twice; only one pending job exists.

```
