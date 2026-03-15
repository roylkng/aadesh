use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use adesh_core::{
    StorageError,
    ports::job_queue::{
        JobCancelInput, JobEnqueueInput, JobFailInput, JobLeaseFilter, JobQueueProvider, JobRecord,
    },
};

pub struct SqliteJobQueue {
    pool: SqlitePool,
}

impl SqliteJobQueue {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn parse_rfc3339(ts: &str) -> Result<DateTime<Utc>, StorageError> {
        DateTime::parse_from_rfc3339(ts)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|err| StorageError::Corruption(err.to_string()))
    }

    fn parse_job(row: sqlx::sqlite::SqliteRow) -> Result<JobRecord, StorageError> {
        Ok(JobRecord {
            job_id: row.get("job_id"),
            created_at: Self::parse_rfc3339(row.get::<String, _>("created_at").as_str())?,
            updated_at: Self::parse_rfc3339(row.get::<String, _>("updated_at").as_str())?,
            run_after: row
                .get::<Option<String>, _>("run_after")
                .map(|value| Self::parse_rfc3339(&value))
                .transpose()?,
            leased_until: row
                .get::<Option<String>, _>("leased_until")
                .map(|value| Self::parse_rfc3339(&value))
                .transpose()?,
            lease_owner: row.get("lease_owner"),
            lease_epoch: row.get("lease_epoch"),
            status: row.get("status"),
            attempt_count: row.get("attempt_count"),
            max_attempts: row.get("max_attempts"),
            job_type: row.get("job_type"),
            dedupe_key: row.get("dedupe_key"),
            payload: serde_json::from_str(&row.get::<String, _>("payload_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            sensitivity_s: row.get("sensitivity_s"),
            taint_s: row.get("taint_s"),
            provenance_refs: serde_json::from_str(&row.get::<String, _>("provenance_refs_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            last_error_code: row.get("last_error_code"),
            last_error_message: row.get("last_error_message"),
            completed_at: row
                .get::<Option<String>, _>("completed_at")
                .map(|value| Self::parse_rfc3339(&value))
                .transpose()?,
        })
    }

    async fn get_job_from_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        job_id: &str,
    ) -> Result<JobRecord, StorageError> {
        let row = sqlx::query(
            "SELECT job_id, created_at, updated_at, run_after, leased_until, lease_owner, lease_epoch,
                    status, attempt_count, max_attempts, job_type, dedupe_key, payload_json,
                    sensitivity_s, taint_s, provenance_refs_json, last_error_code,
                    last_error_message, completed_at
             FROM jobs
             WHERE job_id = ?1",
        )
        .bind(job_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .ok_or_else(|| StorageError::NotFound(format!("job {job_id}")))?;

        Self::parse_job(row)
    }
}

#[async_trait]
impl JobQueueProvider for SqliteJobQueue {
    async fn health(&self) -> Result<(), StorageError> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM jobs")
            .fetch_one(&self.pool)
            .await
            .map(|_| ())
            .map_err(|err| StorageError::Unavailable(err.to_string()))
    }

    async fn enqueue_job(&self, input: JobEnqueueInput) -> Result<JobRecord, StorageError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        if let Some(dedupe_key) = input.dedupe_key.as_deref() {
            if let Some(existing_id) = sqlx::query_scalar::<_, String>(
                "SELECT job_id FROM jobs
                 WHERE dedupe_key = ?1 AND status IN ('pending', 'leased')
                 ORDER BY created_at ASC
                 LIMIT 1",
            )
            .bind(dedupe_key)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            {
                let record = Self::get_job_from_tx(&mut tx, &existing_id).await?;
                tx.commit()
                    .await
                    .map_err(|err| StorageError::Unavailable(err.to_string()))?;
                return Ok(record);
            }
        }

        let now = Utc::now();
        let now_rfc3339 = now.to_rfc3339();
        let job_id = input
            .job_id
            .unwrap_or_else(|| format!("job:{}", Uuid::new_v4()));
        sqlx::query(
            "INSERT INTO jobs (
                job_id, created_at, updated_at, run_after, leased_until, lease_owner, lease_epoch,
                status, attempt_count, max_attempts, job_type, dedupe_key, payload_json,
                sensitivity_s, taint_s, provenance_refs_json, last_error_code,
                last_error_message, completed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
        )
        .bind(&job_id)
        .bind(&now_rfc3339)
        .bind(&now_rfc3339)
        .bind(input.run_after.map(|value| value.to_rfc3339()))
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(0_i64)
        .bind("pending")
        .bind(0_i64)
        .bind(input.max_attempts)
        .bind(&input.job_type)
        .bind(&input.dedupe_key)
        .bind(
            serde_json::to_string(&input.payload)
                .map_err(|err| StorageError::InvalidInput(err.to_string()))?,
        )
        .bind(input.sensitivity_s)
        .bind(input.taint_s)
        .bind(
            serde_json::to_string(&input.provenance_refs)
                .map_err(|err| StorageError::InvalidInput(err.to_string()))?,
        )
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .bind(Option::<String>::None)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Conflict(err.to_string()))?;

        let record = Self::get_job_from_tx(&mut tx, &job_id).await?;
        tx.commit()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        Ok(record)
    }

    async fn lease_jobs(
        &self,
        worker_id: &str,
        limit: u32,
        lease_duration_ms: i64,
        filter: Option<JobLeaseFilter>,
    ) -> Result<Vec<JobRecord>, StorageError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        let now = Utc::now();
        let now_rfc3339 = now.to_rfc3339();
        let leased_until = (now + Duration::milliseconds(lease_duration_ms)).to_rfc3339();

        let query = if filter
            .as_ref()
            .and_then(|value| value.job_type.as_ref())
            .is_some()
        {
            "SELECT job_id FROM jobs
             WHERE status = 'pending'
               AND (run_after IS NULL OR run_after <= ?1)
               AND job_type = ?2
             ORDER BY created_at ASC
             LIMIT ?3"
        } else {
            "SELECT job_id FROM jobs
             WHERE status = 'pending'
               AND (run_after IS NULL OR run_after <= ?1)
             ORDER BY created_at ASC
             LIMIT ?2"
        };

        let ids = if let Some(job_type) = filter.and_then(|value| value.job_type) {
            sqlx::query_scalar::<_, String>(query)
                .bind(&now_rfc3339)
                .bind(job_type)
                .bind(i64::from(limit))
                .fetch_all(tx.as_mut())
                .await
                .map_err(|err| StorageError::Unavailable(err.to_string()))?
        } else {
            sqlx::query_scalar::<_, String>(query)
                .bind(&now_rfc3339)
                .bind(i64::from(limit))
                .fetch_all(tx.as_mut())
                .await
                .map_err(|err| StorageError::Unavailable(err.to_string()))?
        };

        let mut leased = Vec::new();
        for job_id in ids {
            let rows = sqlx::query(
                "UPDATE jobs
                 SET updated_at = ?2,
                     leased_until = ?3,
                     lease_owner = ?4,
                     lease_epoch = lease_epoch + 1,
                     status = 'leased'
                 WHERE job_id = ?1 AND status = 'pending'",
            )
            .bind(&job_id)
            .bind(&now_rfc3339)
            .bind(&leased_until)
            .bind(worker_id)
            .execute(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            .rows_affected();

            if rows == 0 {
                continue;
            }
            leased.push(Self::get_job_from_tx(&mut tx, &job_id).await?);
        }

        tx.commit()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        Ok(leased)
    }

    async fn ack_job(
        &self,
        job_id: &str,
        worker_id: &str,
        lease_epoch: i64,
    ) -> Result<JobRecord, StorageError> {
        let now = Utc::now().to_rfc3339();
        let rows = sqlx::query(
            "UPDATE jobs
             SET updated_at = ?4,
                 leased_until = NULL,
                 lease_owner = NULL,
                 status = 'completed',
                 completed_at = ?4
             WHERE job_id = ?1
               AND lease_owner = ?2
               AND lease_epoch = ?3
               AND status = 'leased'",
        )
        .bind(job_id)
        .bind(worker_id)
        .bind(lease_epoch)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .rows_affected();

        if rows == 0 {
            return Err(StorageError::Conflict(format!(
                "job {job_id} is not leased by {worker_id} at epoch {lease_epoch}"
            )));
        }

        self.get_job(job_id).await
    }

    async fn fail_job(&self, input: JobFailInput) -> Result<JobRecord, StorageError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        let current = Self::get_job_from_tx(&mut tx, &input.job_id).await?;
        if current.status != "leased"
            || current.lease_owner.as_deref() != Some(input.worker_id.as_str())
            || current.lease_epoch != input.lease_epoch
        {
            return Err(StorageError::Conflict(format!(
                "job {} is not leased by {} at epoch {}",
                input.job_id, input.worker_id, input.lease_epoch
            )));
        }

        let next_attempt = current.attempt_count + 1;
        let terminal = next_attempt >= current.max_attempts;
        let now = Utc::now();
        let next_run_after = if terminal {
            None
        } else if let Some(explicit) = input.retry_after {
            Some(explicit)
        } else {
            let delay_ms = (1_i64 << (next_attempt - 1)).saturating_mul(1_000);
            Some(now + Duration::milliseconds(delay_ms.min(60_000)))
        };
        let status = if terminal { "dead_lettered" } else { "pending" };
        let completed_at = if terminal {
            Some(now.to_rfc3339())
        } else {
            None
        };

        sqlx::query(
            "UPDATE jobs
             SET updated_at = ?2,
                 leased_until = NULL,
                 lease_owner = NULL,
                 status = ?3,
                 attempt_count = ?4,
                 run_after = ?5,
                 last_error_code = ?6,
                 last_error_message = ?7,
                 completed_at = ?8
             WHERE job_id = ?1",
        )
        .bind(&input.job_id)
        .bind(now.to_rfc3339())
        .bind(status)
        .bind(next_attempt)
        .bind(next_run_after.map(|value| value.to_rfc3339()))
        .bind(&input.error_code)
        .bind(&input.error_message)
        .bind(completed_at)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let record = Self::get_job_from_tx(&mut tx, &input.job_id).await?;
        tx.commit()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        Ok(record)
    }

    async fn cancel_job(&self, input: JobCancelInput) -> Result<JobRecord, StorageError> {
        let now = Utc::now().to_rfc3339();
        let rows = sqlx::query(
            "UPDATE jobs
             SET updated_at = ?2,
                 leased_until = NULL,
                 lease_owner = NULL,
                 status = 'cancelled',
                 completed_at = ?2
             WHERE job_id = ?1 AND status IN ('pending', 'leased')",
        )
        .bind(&input.job_id)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .rows_affected();

        if rows == 0 {
            return self.get_job(&input.job_id).await;
        }

        self.get_job(&input.job_id).await
    }

    async fn get_job(&self, job_id: &str) -> Result<JobRecord, StorageError> {
        let row = sqlx::query(
            "SELECT job_id, created_at, updated_at, run_after, leased_until, lease_owner, lease_epoch,
                    status, attempt_count, max_attempts, job_type, dedupe_key, payload_json,
                    sensitivity_s, taint_s, provenance_refs_json, last_error_code,
                    last_error_message, completed_at
             FROM jobs
             WHERE job_id = ?1",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .ok_or_else(|| StorageError::NotFound(format!("job {job_id}")))?;

        Self::parse_job(row)
    }
}
