use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::StorageError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEnqueueInput {
    pub job_id: Option<String>,
    pub job_type: String,
    pub payload: serde_json::Value,
    pub dedupe_key: Option<String>,
    pub run_after: Option<DateTime<Utc>>,
    pub max_attempts: i64,
    pub sensitivity_s: i64,
    pub taint_s: i64,
    pub provenance_refs: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub job_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub run_after: Option<DateTime<Utc>>,
    pub leased_until: Option<DateTime<Utc>>,
    pub lease_owner: Option<String>,
    pub lease_epoch: i64,
    pub status: String,
    pub attempt_count: i64,
    pub max_attempts: i64,
    pub job_type: String,
    pub dedupe_key: Option<String>,
    pub payload: serde_json::Value,
    pub sensitivity_s: i64,
    pub taint_s: i64,
    pub provenance_refs: serde_json::Value,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobLeaseFilter {
    pub job_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobFailInput {
    pub job_id: String,
    pub worker_id: String,
    pub lease_epoch: i64,
    pub error_code: String,
    pub error_message: String,
    pub retry_after: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobCancelInput {
    pub job_id: String,
}

#[async_trait]
pub trait JobQueueProvider: Send + Sync {
    async fn health(&self) -> Result<(), StorageError>;
    async fn enqueue_job(&self, input: JobEnqueueInput) -> Result<JobRecord, StorageError>;
    async fn lease_jobs(
        &self,
        worker_id: &str,
        limit: u32,
        lease_duration_ms: i64,
        filter: Option<JobLeaseFilter>,
    ) -> Result<Vec<JobRecord>, StorageError>;
    async fn ack_job(
        &self,
        job_id: &str,
        worker_id: &str,
        lease_epoch: i64,
    ) -> Result<JobRecord, StorageError>;
    async fn fail_job(&self, input: JobFailInput) -> Result<JobRecord, StorageError>;
    async fn cancel_job(&self, input: JobCancelInput) -> Result<JobRecord, StorageError>;
    async fn get_job(&self, job_id: &str) -> Result<JobRecord, StorageError>;
}
