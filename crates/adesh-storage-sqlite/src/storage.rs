use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool, sqlite::SqlitePoolOptions};
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

use adesh_contracts::{
    ApprovalDecisionResponse, ApprovalItemDetail, ApprovalItemSummary,
    CapabilitySnapshotMintResponse, CapabilitySnapshotResponse, CompiledSliceResponse,
    CurrentVersionsResponse, GateDecisionResponse, OperationResponse, ReasoningOutputResponse,
    ReplayResponse, RequestAcceptedResponse, RequestEnvelope, ReviewDecisionResponse,
    ReviewItemDetail, ReviewItemSummary, SchemaEntryResponse, SyscallResponse,
};
use adesh_core::{
    StorageError,
    action_schemas::{
        ValidationErrorKind, bootstrap_capability_snapshot, bootstrap_schema_registry_entries,
        normalize_args_for_action, resolve_action_descriptor_from_snapshot,
        validate_instance_against_schema,
    },
    ports::storage::{
        ApprovalConsumeInput, ApprovalItemInput, AuditTraceRecord, CapabilityActivationReviewInput,
        CapabilitySnapshotMintInput, CompiledSliceInput, GateDecisionInput, LeaseAcquisition,
        OperationLease, ReasoningOutputInput, ReplayCreateInput, ReviewDecisionInput,
        SchemaRegisterInput, StorageProvider, SyscallStatusUpdateInput,
    },
};

#[derive(Default)]
struct FaultInjection {
    fail_audit_trace_writes: AtomicBool,
}

pub struct SqliteStorage {
    pool: SqlitePool,
    faults: FaultInjection,
}

impl SqliteStorage {
    pub async fn connect(database_url: &str) -> Result<Self, StorageError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        Ok(Self {
            pool,
            faults: FaultInjection::default(),
        })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn inject_fail_audit_trace_writes_for_tests(&self, enabled: bool) {
        self.faults
            .fail_audit_trace_writes
            .store(enabled, Ordering::SeqCst);
    }

    async fn load_current_versions(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    ) -> Result<(String, String, String), StorageError> {
        let rows = sqlx::query("SELECT version_kind, version_id FROM current_versions")
            .fetch_all(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let mut active = None;
        let mut audience = None;
        let mut capability = None;

        for row in rows {
            match row.get::<String, _>("version_kind").as_str() {
                "active_state" => active = Some(row.get("version_id")),
                "audience_graph" => audience = Some(row.get("version_id")),
                "capability_snapshot" => capability = Some(row.get("version_id")),
                _ => {}
            }
        }

        match (active, audience, capability) {
            (Some(active), Some(audience), Some(capability)) => Ok((active, audience, capability)),
            _ => Err(StorageError::Corruption(
                "missing bootstrap current_versions rows".to_string(),
            )),
        }
    }

    fn derive_operation_goal(request: &RequestEnvelope) -> Value {
        let summary = request
            .intent_anchor
            .as_ref()
            .and_then(|anchor| anchor.goal.clone())
            .filter(|goal| !goal.trim().is_empty())
            .unwrap_or_else(|| request.input.content.chars().take(240).collect::<String>());

        json!({
            "summary": summary,
        })
    }

    fn parse_timeline(raw: &str) -> Result<Vec<Value>, StorageError> {
        serde_json::from_str(raw).map_err(|err| StorageError::Corruption(err.to_string()))
    }

    fn parse_syscall_row(row: sqlx::sqlite::SqliteRow) -> Result<SyscallResponse, StorageError> {
        Ok(SyscallResponse {
            syscall_id: row.get("syscall_id"),
            operation_id: row.get("operation_id"),
            approval_id: row.get::<Option<String>, _>("approval_id"),
            tool_name: row.get("tool_name"),
            action_name: row.get("action_name"),
            args_schema_ref: row.get("args_schema_ref"),
            result_schema_ref: row.get::<Option<String>, _>("result_schema_ref"),
            status: row.get("status"),
            args: serde_json::from_str(&row.get::<String, _>("args_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            result_ref: row.get::<Option<String>, _>("result_ref"),
            audit_trace_id: row.get("audit_trace_id"),
            created_at: Self::parse_rfc3339(row.get::<String, _>("created_at").as_str())?,
            updated_at: Self::parse_rfc3339(row.get::<String, _>("updated_at").as_str())?,
        })
    }

    fn parse_review_item_summary(
        row: sqlx::sqlite::SqliteRow,
    ) -> Result<ReviewItemSummary, StorageError> {
        Ok(ReviewItemSummary {
            item_id: row.get("item_id"),
            status: row.get("status"),
            source: row.get("source"),
            target_domain: row.get("target_domain"),
            risk_r_estimate: row.get("risk_r_estimate"),
            sensitivity_s_estimate: row.get("sensitivity_s_estimate"),
            requires_oob: row.get::<i64, _>("requires_oob") != 0,
            created_at: Self::parse_rfc3339(row.get::<String, _>("created_at").as_str())?,
        })
    }

    fn parse_review_item_detail(
        row: sqlx::sqlite::SqliteRow,
    ) -> Result<ReviewItemDetail, StorageError> {
        Ok(ReviewItemDetail {
            item_id: row.get("item_id"),
            status: row.get("status"),
            source: row.get("source"),
            target_domain: row.get("target_domain"),
            risk_r_estimate: row.get("risk_r_estimate"),
            sensitivity_s_estimate: row.get("sensitivity_s_estimate"),
            requires_oob: row.get::<i64, _>("requires_oob") != 0,
            created_at: Self::parse_rfc3339(row.get::<String, _>("created_at").as_str())?,
            proposal: serde_json::from_str(&row.get::<String, _>("proposal_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            evidence: serde_json::from_str(&row.get::<String, _>("evidence_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            impact: serde_json::from_str(&row.get::<String, _>("impact_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            base_version: row.get("base_version"),
        })
    }

    fn parse_rfc3339(ts: &str) -> Result<DateTime<Utc>, StorageError> {
        DateTime::parse_from_rfc3339(ts)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|err| StorageError::Corruption(format!("invalid stored timestamp: {err}")))
    }

    fn canonical_json_string(value: &Value) -> Result<String, StorageError> {
        match value {
            Value::Null => Ok("null".to_string()),
            Value::Bool(v) => Ok(v.to_string()),
            Value::Number(v) => Ok(v.to_string()),
            Value::String(v) => {
                serde_json::to_string(v).map_err(|err| StorageError::Unavailable(err.to_string()))
            }
            Value::Array(values) => {
                let mut rendered = String::from("[");
                for (idx, item) in values.iter().enumerate() {
                    if idx > 0 {
                        rendered.push(',');
                    }
                    rendered.push_str(&Self::canonical_json_string(item)?);
                }
                rendered.push(']');
                Ok(rendered)
            }
            Value::Object(map) => {
                let mut keys = map.keys().cloned().collect::<Vec<_>>();
                keys.sort();
                let mut rendered = String::from("{");
                for (idx, key) in keys.iter().enumerate() {
                    if idx > 0 {
                        rendered.push(',');
                    }
                    rendered.push_str(
                        &serde_json::to_string(key)
                            .map_err(|err| StorageError::Unavailable(err.to_string()))?,
                    );
                    rendered.push(':');
                    rendered.push_str(&Self::canonical_json_string(&map[key])?);
                }
                rendered.push('}');
                Ok(rendered)
            }
        }
    }

    fn sha256_hex(input: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    async fn ensure_bootstrap_capability_snapshot(&self) -> Result<(), StorageError> {
        let version = "cap:bootstrap";
        let payload = bootstrap_capability_snapshot(version);
        let payload_json = serde_json::to_string(&payload)
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        sqlx::query(
            "INSERT OR IGNORE INTO capability_snapshots (
                capability_snapshot_version, created_at, parent_version, content_hash, json_payload, notes
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(version)
        .bind(Utc::now().to_rfc3339())
        .bind(Option::<String>::None)
        .bind("capability-snapshot-bootstrap-v0_1")
        .bind(payload_json)
        .bind(Some("bootstrap capability snapshot".to_string()))
        .execute(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        Ok(())
    }

    async fn ensure_bootstrap_schema_entries(&self) -> Result<(), StorageError> {
        for (_schema_ref, entry) in bootstrap_schema_registry_entries() {
            let schema_ref = entry
                .get("schema_ref")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    StorageError::Corruption(
                        "bootstrap schema entry missing schema_ref".to_string(),
                    )
                })?;
            let schema_kind = entry
                .get("schema_kind")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    StorageError::Corruption(
                        "bootstrap schema entry missing schema_kind".to_string(),
                    )
                })?;
            let name = entry.get("name").and_then(Value::as_str).ok_or_else(|| {
                StorageError::Corruption("bootstrap schema entry missing name".to_string())
            })?;
            let semver = entry.get("semver").and_then(Value::as_str).ok_or_else(|| {
                StorageError::Corruption("bootstrap schema entry missing semver".to_string())
            })?;
            let content_hash = entry
                .get("content_hash")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    StorageError::Corruption(
                        "bootstrap schema entry missing content_hash".to_string(),
                    )
                })?;
            let status = entry.get("status").and_then(Value::as_str).ok_or_else(|| {
                StorageError::Corruption("bootstrap schema entry missing status".to_string())
            })?;
            let compatibility = entry
                .get("compatibility")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    StorageError::Corruption(
                        "bootstrap schema entry missing compatibility".to_string(),
                    )
                })?;
            let payload = entry.get("payload_json").cloned().ok_or_else(|| {
                StorageError::Corruption("bootstrap schema entry missing payload_json".to_string())
            })?;

            sqlx::query(
                "INSERT OR IGNORE INTO schema_registry_entries (
                    schema_ref, schema_kind, name, semver, content_hash, created_at, status, compatibility, payload_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )
            .bind(schema_ref)
            .bind(schema_kind)
            .bind(name)
            .bind(semver)
            .bind(content_hash)
            .bind(Utc::now().to_rfc3339())
            .bind(status)
            .bind(compatibility)
            .bind(
                serde_json::to_string(&payload)
                    .map_err(|err| StorageError::Unavailable(err.to_string()))?,
            )
            .execute(&self.pool)
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        }

        Ok(())
    }
}

#[async_trait]
impl StorageProvider for SqliteStorage {
    async fn migrate(&self) -> Result<(), StorageError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        self.ensure_bootstrap_capability_snapshot().await?;
        self.ensure_bootstrap_schema_entries().await
    }

    async fn health(&self) -> Result<(), StorageError> {
        sqlx::query_scalar::<_, i64>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map(|_| ())
            .map_err(|err| StorageError::Unavailable(err.to_string()))
    }

    async fn append_event(
        &self,
        input: adesh_core::ports::storage::EventAppendInput,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO experience_events (
                event_ref, created_at, source_class, author, audience_id, sensitivity_s, taint_s, kind, content_ref, json_payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(&input.event_ref)
        .bind(input.created_at.to_rfc3339())
        .bind(&input.source_class)
        .bind(&input.author)
        .bind(&input.audience_id)
        .bind(input.sensitivity_s)
        .bind(input.taint_s)
        .bind(&input.kind)
        .bind(&input.content_ref)
        .bind(
            serde_json::to_string(&input.json_payload)
                .map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .execute(&self.pool)
        .await
        .map_err(|err| StorageError::Conflict(err.to_string()))?;
        Ok(())
    }

    async fn create_operation_bundle(
        &self,
        request: &RequestEnvelope,
        idempotency_key: Option<&str>,
    ) -> Result<RequestAcceptedResponse, StorageError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        if let Some(key) = idempotency_key {
            if let Some(existing) = sqlx::query_scalar::<_, String>(
                "SELECT response_json FROM idempotency_keys WHERE endpoint_scope = ?1 AND idempotency_key = ?2",
            )
            .bind("/v1/requests")
            .bind(key)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            {
                let parsed = serde_json::from_str::<RequestAcceptedResponse>(&existing)
                    .map_err(|err| StorageError::Corruption(err.to_string()))?;
                tx.commit()
                    .await
                    .map_err(|err| StorageError::Unavailable(err.to_string()))?;
                return Ok(parsed);
            }
        }

        let now = Utc::now();
        let now_rfc3339 = now.to_rfc3339();
        let request_event_ref = format!("event:request:{}", request.request_id);
        let operation_id = format!("op:{}", Uuid::new_v4());
        let isolation_id = format!("iso:{}", Uuid::new_v4());
        let audit_trace_id = format!("audit:{}", Uuid::new_v4());
        let transition_id = format!("transition:{}", Uuid::new_v4());

        let (active_state_version, audience_graph_version, capability_snapshot_version) =
            self.load_current_versions(&mut tx).await?;

        let operation_goal = Self::derive_operation_goal(request);
        let budgets = serde_json::to_value(&request.constraints.budgets)
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        let attachments = serde_json::to_value(&request.input.attachments)
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        let pinned = json!({
            "active_state_version": active_state_version,
            "capability_snapshot_version": capability_snapshot_version,
            "audience_graph_version": audience_graph_version,
        });

        sqlx::query(
            "INSERT INTO experience_events (
                event_ref, created_at, source_class, author, audience_id, sensitivity_s, taint_s, kind, content_ref, json_payload
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(&request_event_ref)
        .bind(&now_rfc3339)
        .bind("control_plane")
        .bind(&request.requesting_principal.principal_id)
        .bind(&request.requesting_audience_id)
        .bind(0_i64)
        .bind(0_i64)
        .bind("request")
        .bind(Option::<String>::None)
        .bind(
            serde_json::to_string(request).map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        sqlx::query(
            "INSERT INTO operations (
                operation_id, parent_request_id, isolation_id, created_at, updated_at, state, state_reason,
                requesting_audience_id, pinned_active_state_version, pinned_capability_snapshot_version,
                pinned_audience_graph_version, budgets_json, operation_goal_json, ipc_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )
        .bind(&operation_id)
        .bind(&request.request_id)
        .bind(&isolation_id)
        .bind(&now_rfc3339)
        .bind(&now_rfc3339)
        .bind("created")
        .bind(Option::<String>::None)
        .bind(&request.requesting_audience_id)
        .bind(pinned["active_state_version"].as_str().unwrap())
        .bind(pinned["capability_snapshot_version"].as_str().unwrap())
        .bind(pinned["audience_graph_version"].as_str().unwrap())
        .bind(serde_json::to_string(&budgets).map_err(|err| StorageError::Unavailable(err.to_string()))?)
        .bind(serde_json::to_string(&operation_goal).map_err(|err| StorageError::Unavailable(err.to_string()))?)
        .bind(Option::<String>::None)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        sqlx::query(
            "INSERT INTO operation_transitions (
                transition_id, operation_id, ts, from_state, to_state, reason, audit_trace_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(&transition_id)
        .bind(&operation_id)
        .bind(&now_rfc3339)
        .bind(Option::<String>::None)
        .bind("created")
        .bind(Option::<String>::None)
        .bind(&audit_trace_id)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        if self.faults.fail_audit_trace_writes.load(Ordering::SeqCst) {
            return Err(StorageError::Unavailable(
                "fault injection: audit trace write failed".to_string(),
            ));
        }

        sqlx::query(
            "INSERT INTO audit_traces (
                audit_trace_id, created_at, request_id, operation_id, isolation_id, pinned_json, summary_json, timeline_json, attachments_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(&audit_trace_id)
        .bind(&now_rfc3339)
        .bind(&request.request_id)
        .bind(&operation_id)
        .bind(&isolation_id)
        .bind(serde_json::to_string(&pinned).map_err(|err| StorageError::Unavailable(err.to_string()))?)
        .bind(
            serde_json::to_string(&json!({"request_event_ref": request_event_ref, "state": "created"}))
                .map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .bind(
            serde_json::to_string(&json!([
                {"type": "request_accepted", "ts": now_rfc3339, "event_ref": request_event_ref, "transition_id": transition_id}
            ]))
            .map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .bind(serde_json::to_string(&attachments).map_err(|err| StorageError::Unavailable(err.to_string()))?)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let response = RequestAcceptedResponse {
            request_id: request.request_id.clone(),
            operation_ids: vec![operation_id.clone()],
            primary_operation_id: operation_id.clone(),
            audit_trace_ids: vec![audit_trace_id.clone()],
        };

        if let Some(key) = idempotency_key {
            let response_json = serde_json::to_string(&response)
                .map_err(|err| StorageError::Unavailable(err.to_string()))?;
            sqlx::query(
                "INSERT INTO idempotency_keys (
                    endpoint_scope, idempotency_key, request_id, response_json, response_hash, created_at, expires_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind("/v1/requests")
            .bind(key)
            .bind(&request.request_id)
            .bind(&response_json)
            .bind(&response_json)
            .bind(&now_rfc3339)
            .bind(Option::<String>::None)
            .execute(tx.as_mut())
            .await
            .map_err(|err| StorageError::Conflict(err.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        Ok(response)
    }

    async fn get_operation(&self, operation_id: &str) -> Result<OperationResponse, StorageError> {
        let row = sqlx::query(
            "SELECT o.operation_id, o.parent_request_id, o.isolation_id, o.state, o.state_reason,
                    o.requesting_audience_id, o.pinned_active_state_version, o.pinned_capability_snapshot_version,
                    o.pinned_audience_graph_version, o.budgets_json, o.operation_goal_json, o.created_at, o.updated_at,
                    a.audit_trace_id
             FROM operations o
             INNER JOIN audit_traces a ON a.operation_id = o.operation_id
             WHERE o.operation_id = ?1",
        )
        .bind(operation_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .ok_or_else(|| StorageError::NotFound(format!("operation {operation_id}")))?;

        let created_at = Self::parse_rfc3339(row.get::<String, _>("created_at").as_str())?;
        let updated_at = Self::parse_rfc3339(row.get::<String, _>("updated_at").as_str())?;

        Ok(OperationResponse {
            operation_id: row.get("operation_id"),
            request_id: row.get("parent_request_id"),
            isolation_id: row.get("isolation_id"),
            state: row.get("state"),
            state_reason: row.get("state_reason"),
            requesting_audience_id: row.get("requesting_audience_id"),
            audit_trace_id: row.get("audit_trace_id"),
            pinned_active_state_version: row.get("pinned_active_state_version"),
            pinned_capability_snapshot_version: row.get("pinned_capability_snapshot_version"),
            pinned_audience_graph_version: row.get("pinned_audience_graph_version"),
            budgets: serde_json::from_str(&row.get::<String, _>("budgets_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            operation_goal: serde_json::from_str(&row.get::<String, _>("operation_goal_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            created_at,
            updated_at,
        })
    }

    async fn update_operation_state(
        &self,
        operation_id: &str,
        new_state: &str,
        reason: Option<&str>,
        audit_trace_id: &str,
    ) -> Result<(), StorageError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        let now = Utc::now().to_rfc3339();
        let current = sqlx::query("SELECT state FROM operations WHERE operation_id = ?1")
            .bind(operation_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            .ok_or_else(|| StorageError::NotFound(format!("operation {operation_id}")))?;
        let from_state: String = current.get("state");

        sqlx::query(
            "UPDATE operations SET state = ?2, state_reason = ?3, updated_at = ?4 WHERE operation_id = ?1",
        )
        .bind(operation_id)
        .bind(new_state)
        .bind(reason)
        .bind(&now)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        sqlx::query(
            "INSERT INTO operation_transitions (
                transition_id, operation_id, ts, from_state, to_state, reason, audit_trace_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(format!("transition:{}", Uuid::new_v4()))
        .bind(operation_id)
        .bind(&now)
        .bind(from_state)
        .bind(new_state)
        .bind(reason)
        .bind(audit_trace_id)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        tx.commit()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        Ok(())
    }

    async fn put_gate_decision(
        &self,
        input: GateDecisionInput,
    ) -> Result<GateDecisionResponse, StorageError> {
        let gate_decision_id = format!("gate:{}", Uuid::new_v4());
        let evaluated_at = Utc::now();
        let payload = json!({
            "risk_r": input.risk_r,
            "sensitivity_s": input.sensitivity_s,
            "max_gate": input.max_gate,
            "approval_mode": input.approval_mode,
        });

        sqlx::query(
            "INSERT INTO gate_decisions (
                gate_decision_id, operation_id, isolation_id, evaluated_at, active_state_version,
                capability_snapshot_version, audience_graph_version, risk_r, sensitivity_s, max_gate,
                approval_mode, requesting_audience_id, scopes_allowed_json, scopes_denied_json,
                sensitivity_ceiling_s, predicates_json, constraints_json, json_payload, audit_trace_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
        )
        .bind(&gate_decision_id)
        .bind(&input.operation_id)
        .bind(&input.isolation_id)
        .bind(evaluated_at.to_rfc3339())
        .bind(&input.active_state_version)
        .bind(&input.capability_snapshot_version)
        .bind(&input.audience_graph_version)
        .bind(input.risk_r)
        .bind(input.sensitivity_s)
        .bind(input.max_gate)
        .bind(&input.approval_mode)
        .bind(&input.requesting_audience_id)
        .bind(serde_json::to_string(&input.scopes_allowed).map_err(|err| StorageError::Unavailable(err.to_string()))?)
        .bind(serde_json::to_string(&input.scopes_denied).map_err(|err| StorageError::Unavailable(err.to_string()))?)
        .bind(input.sensitivity_ceiling_s)
        .bind(serde_json::to_string(&input.predicates).map_err(|err| StorageError::Unavailable(err.to_string()))?)
        .bind(serde_json::to_string(&input.constraints).map_err(|err| StorageError::Unavailable(err.to_string()))?)
        .bind(serde_json::to_string(&payload).map_err(|err| StorageError::Unavailable(err.to_string()))?)
        .bind(&input.audit_trace_id)
        .execute(&self.pool)
        .await
        .map_err(|err| StorageError::Conflict(err.to_string()))?;

        Ok(GateDecisionResponse {
            gate_decision_id,
            operation_id: input.operation_id,
            isolation_id: input.isolation_id,
            evaluated_at,
            active_state_version: input.active_state_version,
            capability_snapshot_version: input.capability_snapshot_version,
            audience_graph_version: input.audience_graph_version,
            risk_r: input.risk_r,
            sensitivity_s: input.sensitivity_s,
            max_gate: input.max_gate,
            approval_mode: input.approval_mode,
            requesting_audience_id: input.requesting_audience_id,
            scopes_allowed: input.scopes_allowed,
            scopes_denied: input.scopes_denied,
            sensitivity_ceiling_s: input.sensitivity_ceiling_s,
            predicates: input.predicates,
            constraints: input.constraints,
            audit_trace_id: input.audit_trace_id,
        })
    }

    async fn get_gate_decision(
        &self,
        operation_id: &str,
    ) -> Result<GateDecisionResponse, StorageError> {
        let row = sqlx::query("SELECT * FROM gate_decisions WHERE operation_id = ?1")
            .bind(operation_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            .ok_or_else(|| StorageError::NotFound(format!("gate decision for {operation_id}")))?;

        Ok(GateDecisionResponse {
            gate_decision_id: row.get("gate_decision_id"),
            operation_id: row.get("operation_id"),
            isolation_id: row.get("isolation_id"),
            evaluated_at: Self::parse_rfc3339(row.get::<String, _>("evaluated_at").as_str())?,
            active_state_version: row.get("active_state_version"),
            capability_snapshot_version: row.get("capability_snapshot_version"),
            audience_graph_version: row.get("audience_graph_version"),
            risk_r: row.get("risk_r"),
            sensitivity_s: row.get("sensitivity_s"),
            max_gate: row.get("max_gate"),
            approval_mode: row.get("approval_mode"),
            requesting_audience_id: row.get("requesting_audience_id"),
            scopes_allowed: serde_json::from_str(&row.get::<String, _>("scopes_allowed_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            scopes_denied: serde_json::from_str(&row.get::<String, _>("scopes_denied_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            sensitivity_ceiling_s: row.get("sensitivity_ceiling_s"),
            predicates: serde_json::from_str(&row.get::<String, _>("predicates_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            constraints: serde_json::from_str(&row.get::<String, _>("constraints_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            audit_trace_id: row.get("audit_trace_id"),
        })
    }

    async fn put_compiled_slice(
        &self,
        input: CompiledSliceInput,
    ) -> Result<CompiledSliceResponse, StorageError> {
        let compiled_slice_id = format!("slice:{}", Uuid::new_v4());
        let compiled_at = Utc::now();
        let payload = json!({
            "approval_mode": input.approval_mode,
            "operation_max_taint_s": input.operation_max_taint_s,
        });
        sqlx::query(
            "INSERT INTO compiled_slices (
                compiled_slice_id, operation_id, isolation_id, compiled_at, active_state_version,
                capability_snapshot_version, audience_graph_version, risk_r, sensitivity_s, max_gate,
                approval_mode, operation_max_taint_s, did_omit, omissions_json, provenance_summary_json,
                intent_anchor_json, blocks_json, json_payload, audit_trace_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
        )
        .bind(&compiled_slice_id)
        .bind(&input.operation_id)
        .bind(&input.isolation_id)
        .bind(compiled_at.to_rfc3339())
        .bind(&input.active_state_version)
        .bind(&input.capability_snapshot_version)
        .bind(&input.audience_graph_version)
        .bind(input.risk_r)
        .bind(input.sensitivity_s)
        .bind(input.max_gate)
        .bind(&input.approval_mode)
        .bind(input.operation_max_taint_s)
        .bind(if input.did_omit { 1_i64 } else { 0_i64 })
        .bind(serde_json::to_string(&input.omissions).map_err(|err| StorageError::Unavailable(err.to_string()))?)
        .bind(serde_json::to_string(&input.provenance_summary).map_err(|err| StorageError::Unavailable(err.to_string()))?)
        .bind(serde_json::to_string(&input.intent_anchor).map_err(|err| StorageError::Unavailable(err.to_string()))?)
        .bind(serde_json::to_string(&input.blocks).map_err(|err| StorageError::Unavailable(err.to_string()))?)
        .bind(serde_json::to_string(&payload).map_err(|err| StorageError::Unavailable(err.to_string()))?)
        .bind(&input.audit_trace_id)
        .execute(&self.pool)
        .await
        .map_err(|err| StorageError::Conflict(err.to_string()))?;

        Ok(CompiledSliceResponse {
            compiled_slice_id,
            operation_id: input.operation_id,
            isolation_id: input.isolation_id,
            compiled_at,
            active_state_version: input.active_state_version,
            capability_snapshot_version: input.capability_snapshot_version,
            audience_graph_version: input.audience_graph_version,
            risk_r: input.risk_r,
            sensitivity_s: input.sensitivity_s,
            max_gate: input.max_gate,
            approval_mode: input.approval_mode,
            operation_max_taint_s: input.operation_max_taint_s,
            did_omit: input.did_omit,
            omissions: input.omissions,
            provenance_summary: input.provenance_summary,
            intent_anchor: input.intent_anchor,
            blocks: input.blocks,
            audit_trace_id: input.audit_trace_id,
        })
    }

    async fn get_compiled_slice(
        &self,
        operation_id: &str,
    ) -> Result<CompiledSliceResponse, StorageError> {
        let row = sqlx::query("SELECT * FROM compiled_slices WHERE operation_id = ?1")
            .bind(operation_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            .ok_or_else(|| StorageError::NotFound(format!("compiled slice for {operation_id}")))?;

        Ok(CompiledSliceResponse {
            compiled_slice_id: row.get("compiled_slice_id"),
            operation_id: row.get("operation_id"),
            isolation_id: row.get("isolation_id"),
            compiled_at: Self::parse_rfc3339(row.get::<String, _>("compiled_at").as_str())?,
            active_state_version: row.get("active_state_version"),
            capability_snapshot_version: row.get("capability_snapshot_version"),
            audience_graph_version: row.get("audience_graph_version"),
            risk_r: row.get("risk_r"),
            sensitivity_s: row.get("sensitivity_s"),
            max_gate: row.get("max_gate"),
            approval_mode: row.get("approval_mode"),
            operation_max_taint_s: row.get("operation_max_taint_s"),
            did_omit: row.get::<i64, _>("did_omit") != 0,
            omissions: serde_json::from_str(&row.get::<String, _>("omissions_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            provenance_summary: serde_json::from_str(
                &row.get::<String, _>("provenance_summary_json"),
            )
            .map_err(|err| StorageError::Corruption(err.to_string()))?,
            intent_anchor: serde_json::from_str(&row.get::<String, _>("intent_anchor_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            blocks: serde_json::from_str(&row.get::<String, _>("blocks_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            audit_trace_id: row.get("audit_trace_id"),
        })
    }

    async fn put_reasoning_output(
        &self,
        input: ReasoningOutputInput,
    ) -> Result<ReasoningOutputResponse, StorageError> {
        let event_ref = format!("event:reasoning_output:{}", Uuid::new_v4());
        let created_at = Utc::now();
        let payload = json!({
            "operation_id": input.operation_id,
            "isolation_id": input.isolation_id,
            "model_id": input.model_id,
            "provider_trace_id": input.provider_trace_id,
            "reasoning_output": input.reasoning_output,
        });

        sqlx::query(
            "INSERT INTO experience_events (
                event_ref, created_at, source_class, author, audience_id, sensitivity_s, taint_s, kind, content_ref, json_payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(&event_ref)
        .bind(created_at.to_rfc3339())
        .bind("model_provider")
        .bind(&input.model_id)
        .bind("root_owner")
        .bind(0_i64)
        .bind(0_i64)
        .bind("reasoning_output")
        .bind(Option::<String>::None)
        .bind(
            serde_json::to_string(&payload)
                .map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .execute(&self.pool)
        .await
        .map_err(|err| StorageError::Conflict(err.to_string()))?;

        Ok(ReasoningOutputResponse {
            event_ref,
            operation_id: input.operation_id,
            model_id: input.model_id,
            provider_trace_id: input.provider_trace_id,
            reasoning_output: input.reasoning_output,
        })
    }

    async fn get_reasoning_output(
        &self,
        operation_id: &str,
    ) -> Result<ReasoningOutputResponse, StorageError> {
        let row = sqlx::query(
            "SELECT event_ref, json_payload
             FROM experience_events
             WHERE kind = 'reasoning_output'
             ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .into_iter()
        .find(|row| {
            serde_json::from_str::<Value>(&row.get::<String, _>("json_payload"))
                .ok()
                .and_then(|payload| {
                    payload
                        .get("operation_id")
                        .and_then(Value::as_str)
                        .map(|value| value == operation_id)
                })
                .unwrap_or(false)
        })
        .ok_or_else(|| StorageError::NotFound(format!("reasoning output for {operation_id}")))?;

        let payload: Value = serde_json::from_str(&row.get::<String, _>("json_payload"))
            .map_err(|err| StorageError::Corruption(err.to_string()))?;

        Ok(ReasoningOutputResponse {
            event_ref: row.get("event_ref"),
            operation_id: payload
                .get("operation_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            model_id: payload
                .get("model_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            provider_trace_id: payload
                .get("provider_trace_id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            reasoning_output: payload
                .get("reasoning_output")
                .cloned()
                .unwrap_or_else(|| json!({})),
        })
    }

    async fn get_current_versions(&self) -> Result<CurrentVersionsResponse, StorageError> {
        let rows = sqlx::query("SELECT version_kind, version_id FROM current_versions")
            .fetch_all(&self.pool)
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let mut active_state_version = None;
        let mut audience_graph_version = None;
        let mut capability_snapshot_version = None;

        for row in rows {
            match row.get::<String, _>("version_kind").as_str() {
                "active_state" => active_state_version = Some(row.get("version_id")),
                "audience_graph" => audience_graph_version = Some(row.get("version_id")),
                "capability_snapshot" => capability_snapshot_version = Some(row.get("version_id")),
                _ => {}
            }
        }

        Ok(CurrentVersionsResponse {
            active_state_version: active_state_version.ok_or_else(|| {
                StorageError::Corruption("missing current active_state_version".to_string())
            })?,
            audience_graph_version: audience_graph_version.ok_or_else(|| {
                StorageError::Corruption("missing current audience_graph_version".to_string())
            })?,
            capability_snapshot_version: capability_snapshot_version.ok_or_else(|| {
                StorageError::Corruption("missing current capability_snapshot_version".to_string())
            })?,
        })
    }

    async fn get_capability_snapshot(
        &self,
        capability_snapshot_version: &str,
    ) -> Result<CapabilitySnapshotResponse, StorageError> {
        let row = sqlx::query(
            "SELECT capability_snapshot_version, json_payload
             FROM capability_snapshots WHERE capability_snapshot_version = ?1",
        )
        .bind(capability_snapshot_version)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .ok_or_else(|| {
            StorageError::NotFound(format!("capability snapshot {capability_snapshot_version}"))
        })?;

        Ok(CapabilitySnapshotResponse {
            capability_snapshot_version: row.get("capability_snapshot_version"),
            payload: serde_json::from_str(&row.get::<String, _>("json_payload"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
        })
    }

    async fn mint_capability_snapshot(
        &self,
        input: CapabilitySnapshotMintInput,
    ) -> Result<CapabilitySnapshotMintResponse, StorageError> {
        let endpoint_scope = "/v1/capabilities/snapshots";
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = sqlx::query_scalar::<_, String>(
                "SELECT response_json FROM idempotency_keys WHERE endpoint_scope = ?1 AND idempotency_key = ?2",
            )
            .bind(endpoint_scope)
            .bind(key)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            {
                let parsed = serde_json::from_str::<CapabilitySnapshotMintResponse>(&existing)
                    .map_err(|err| StorageError::Corruption(err.to_string()))?;
                tx.commit()
                    .await
                    .map_err(|err| StorageError::Unavailable(err.to_string()))?;
                return Ok(parsed);
            }
        }

        let capabilities = input
            .snapshot_payload
            .get("capabilities")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                StorageError::InvalidInput(
                    "snapshot_payload missing capabilities array".to_string(),
                )
            })?;
        for capability in capabilities {
            for action in capability
                .get("actions")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    StorageError::InvalidInput(
                        "snapshot_payload capability missing actions array".to_string(),
                    )
                })?
            {
                let args_schema_ref = action
                    .get("args_schema_ref")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        StorageError::InvalidInput(
                            "snapshot action missing args_schema_ref".to_string(),
                        )
                    })?;
                sqlx::query_scalar::<_, String>(
                    "SELECT schema_ref FROM schema_registry_entries WHERE schema_ref = ?1",
                )
                .bind(args_schema_ref)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(|err| StorageError::Unavailable(err.to_string()))?
                .ok_or_else(|| {
                    StorageError::InvalidInput(format!(
                        "snapshot references unknown args schema_ref {args_schema_ref}"
                    ))
                })?;

                if let Some(result_schema_ref) =
                    action.get("result_schema_ref").and_then(Value::as_str)
                {
                    sqlx::query_scalar::<_, String>(
                        "SELECT schema_ref FROM schema_registry_entries WHERE schema_ref = ?1",
                    )
                    .bind(result_schema_ref)
                    .fetch_optional(tx.as_mut())
                    .await
                    .map_err(|err| StorageError::Unavailable(err.to_string()))?
                    .ok_or_else(|| {
                        StorageError::InvalidInput(format!(
                            "snapshot references unknown result schema_ref {result_schema_ref}"
                        ))
                    })?;
                }
            }
        }

        let canonical_payload = Self::canonical_json_string(&input.snapshot_payload)?;
        let hash = Self::sha256_hex(&canonical_payload);
        let capability_snapshot_version = format!("cap:sha256:{hash}");
        let created_at = Utc::now();

        sqlx::query(
            "INSERT OR IGNORE INTO capability_snapshots (
                capability_snapshot_version, created_at, parent_version, content_hash, json_payload, notes
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&capability_snapshot_version)
        .bind(created_at.to_rfc3339())
        .bind(input.base_version)
        .bind(&hash)
        .bind(&canonical_payload)
        .bind(Some("minted via control plane".to_string()))
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let response = CapabilitySnapshotMintResponse {
            capability_snapshot_version,
            created_at,
        };

        if let Some(key) = input.idempotency_key.as_deref() {
            let response_json = serde_json::to_string(&response)
                .map_err(|err| StorageError::Unavailable(err.to_string()))?;
            sqlx::query(
                "INSERT INTO idempotency_keys (
                    endpoint_scope, idempotency_key, request_id, response_json, response_hash, created_at, expires_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(endpoint_scope)
            .bind(key)
            .bind(&response.capability_snapshot_version)
            .bind(&response_json)
            .bind(&response_json)
            .bind(created_at.to_rfc3339())
            .bind(Option::<String>::None)
            .execute(tx.as_mut())
            .await
            .map_err(|err| StorageError::Conflict(err.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        Ok(response)
    }

    async fn get_schema_entry(
        &self,
        schema_ref: &str,
    ) -> Result<SchemaEntryResponse, StorageError> {
        let row = sqlx::query(
            "SELECT schema_ref, schema_kind, name, semver, content_hash, created_at, status, compatibility, payload_json
             FROM schema_registry_entries WHERE schema_ref = ?1",
        )
        .bind(schema_ref)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .ok_or_else(|| StorageError::NotFound(format!("schema entry {schema_ref}")))?;

        Ok(SchemaEntryResponse {
            schema_ref: row.get("schema_ref"),
            schema_kind: row.get("schema_kind"),
            name: row.get("name"),
            semver: row.get("semver"),
            content_hash: row.get("content_hash"),
            created_at: Self::parse_rfc3339(&row.get::<String, _>("created_at"))?,
            status: row.get("status"),
            compatibility: row.get("compatibility"),
            payload: serde_json::from_str(&row.get::<String, _>("payload_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
        })
    }

    async fn register_schema_entry(
        &self,
        input: SchemaRegisterInput,
    ) -> Result<SchemaEntryResponse, StorageError> {
        let endpoint_scope = "/v1/schema-registry/register";
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = sqlx::query_scalar::<_, String>(
                "SELECT response_json FROM idempotency_keys WHERE endpoint_scope = ?1 AND idempotency_key = ?2",
            )
            .bind(endpoint_scope)
            .bind(key)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            {
                let parsed = serde_json::from_str::<SchemaEntryResponse>(&existing)
                    .map_err(|err| StorageError::Corruption(err.to_string()))?;
                tx.commit()
                    .await
                    .map_err(|err| StorageError::Unavailable(err.to_string()))?;
                return Ok(parsed);
            }
        }

        let canonical_payload = Self::canonical_json_string(&input.schema_payload)?;
        let hash = Self::sha256_hex(&canonical_payload);
        let schema_ref = format!("schema:sha256:{hash}");
        let created_at = Utc::now();

        sqlx::query(
            "INSERT OR IGNORE INTO schema_registry_entries (
                schema_ref, schema_kind, name, semver, content_hash, created_at, status, compatibility, payload_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(&schema_ref)
        .bind(&input.schema_kind)
        .bind(&input.name)
        .bind(&input.semver)
        .bind(&hash)
        .bind(created_at.to_rfc3339())
        .bind("active")
        .bind("exact")
        .bind(&canonical_payload)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let response = SchemaEntryResponse {
            schema_ref,
            schema_kind: input.schema_kind,
            name: input.name,
            semver: input.semver,
            content_hash: hash,
            created_at,
            status: "active".to_string(),
            compatibility: "exact".to_string(),
            payload: input.schema_payload,
        };

        if let Some(key) = input.idempotency_key.as_deref() {
            let response_json = serde_json::to_string(&response)
                .map_err(|err| StorageError::Unavailable(err.to_string()))?;
            sqlx::query(
                "INSERT INTO idempotency_keys (
                    endpoint_scope, idempotency_key, request_id, response_json, response_hash, created_at, expires_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(endpoint_scope)
            .bind(key)
            .bind(&response.schema_ref)
            .bind(&response_json)
            .bind(&response_json)
            .bind(created_at.to_rfc3339())
            .bind(Option::<String>::None)
            .execute(tx.as_mut())
            .await
            .map_err(|err| StorageError::Conflict(err.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        Ok(response)
    }

    async fn create_capability_activation_review_item(
        &self,
        input: CapabilityActivationReviewInput,
    ) -> Result<ReviewItemDetail, StorageError> {
        let endpoint_scope = "/v1/capabilities/current/activate";
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = sqlx::query_scalar::<_, String>(
                "SELECT response_json FROM idempotency_keys WHERE endpoint_scope = ?1 AND idempotency_key = ?2",
            )
            .bind(endpoint_scope)
            .bind(key)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            {
                let parsed = serde_json::from_str::<ReviewItemDetail>(&existing)
                    .map_err(|err| StorageError::Corruption(err.to_string()))?;
                tx.commit()
                    .await
                    .map_err(|err| StorageError::Unavailable(err.to_string()))?;
                return Ok(parsed);
            }
        }

        let candidate = sqlx::query_scalar::<_, String>(
            "SELECT capability_snapshot_version FROM capability_snapshots WHERE capability_snapshot_version = ?1",
        )
        .bind(&input.capability_snapshot_version)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .ok_or_else(|| {
            StorageError::NotFound(format!(
                "capability snapshot {}",
                input.capability_snapshot_version
            ))
        })?;

        let current = self.load_current_versions(&mut tx).await?;
        let current_capability_version = current.2;
        if candidate == current_capability_version {
            return Err(StorageError::Conflict(format!(
                "capability snapshot {} is already active",
                input.capability_snapshot_version
            )));
        }

        let item_id = format!("review:{}", Uuid::new_v4());
        let now = Utc::now();
        let now_rfc3339 = now.to_rfc3339();
        let proposal = json!({
            "change_type": "activate_capability_snapshot",
            "capability_snapshot_version": input.capability_snapshot_version,
        });
        let evidence = json!({
            "current_capability_snapshot_version": current_capability_version,
            "candidate_capability_snapshot_version": input.capability_snapshot_version,
        });
        let impact = json!({
            "target_version_kind": "capability_snapshot",
            "base_version": current_capability_version,
            "next_version": input.capability_snapshot_version,
        });

        sqlx::query(
            "INSERT INTO review_queue_items (
                item_id, created_at, updated_at, status, source, target_domain, risk_r_estimate,
                sensitivity_s_estimate, requires_oob, proposal_json, evidence_json, impact_json,
                base_version, resolved_version
             ) VALUES (?1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )
        .bind(&item_id)
        .bind(&now_rfc3339)
        .bind("pending")
        .bind("owner_action")
        .bind("capability_registry")
        .bind(2_i64)
        .bind(0_i64)
        .bind(0_i64)
        .bind(
            serde_json::to_string(&proposal)
                .map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .bind(
            serde_json::to_string(&evidence)
                .map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .bind(
            serde_json::to_string(&impact)
                .map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .bind(&current_capability_version)
        .bind(Option::<String>::None)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Conflict(err.to_string()))?;

        sqlx::query(
            "INSERT INTO experience_events (
                event_ref, created_at, source_class, author, audience_id, sensitivity_s, taint_s, kind, content_ref, json_payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(format!("event:review-item:{}", Uuid::new_v4()))
        .bind(&now_rfc3339)
        .bind("control_plane")
        .bind("root_owner")
        .bind("root_owner")
        .bind(0_i64)
        .bind(0_i64)
        .bind("review_item_created")
        .bind(Option::<String>::None)
        .bind(
            serde_json::to_string(&json!({
                "item_id": item_id.clone(),
                "target_domain": "capability_registry",
                "proposal": proposal.clone(),
                "evidence": evidence.clone(),
                "impact": impact.clone(),
            }))
            .map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let response = ReviewItemDetail {
            item_id: item_id.clone(),
            status: "pending".to_string(),
            source: "owner_action".to_string(),
            target_domain: "capability_registry".to_string(),
            risk_r_estimate: 2,
            sensitivity_s_estimate: 0,
            requires_oob: false,
            created_at: now,
            proposal,
            evidence,
            impact,
            base_version: Some(current_capability_version),
        };

        if let Some(key) = input.idempotency_key.as_deref() {
            let response_json = serde_json::to_string(&response)
                .map_err(|err| StorageError::Unavailable(err.to_string()))?;
            sqlx::query(
                "INSERT INTO idempotency_keys (
                    endpoint_scope, idempotency_key, request_id, response_json, response_hash, created_at, expires_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(endpoint_scope)
            .bind(key)
            .bind(&item_id)
            .bind(&response_json)
            .bind(&response_json)
            .bind(&now_rfc3339)
            .bind(Option::<String>::None)
            .execute(tx.as_mut())
            .await
            .map_err(|err| StorageError::Conflict(err.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        Ok(response)
    }

    async fn list_review_items(&self) -> Result<Vec<ReviewItemSummary>, StorageError> {
        let rows = sqlx::query(
            "SELECT item_id, status, source, target_domain, risk_r_estimate, sensitivity_s_estimate, requires_oob, created_at
             FROM review_queue_items
             ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        rows.into_iter()
            .map(Self::parse_review_item_summary)
            .collect()
    }

    async fn get_review_item(&self, item_id: &str) -> Result<ReviewItemDetail, StorageError> {
        let row = sqlx::query(
            "SELECT item_id, status, source, target_domain, risk_r_estimate, sensitivity_s_estimate,
                    requires_oob, created_at, proposal_json, evidence_json, impact_json, base_version
             FROM review_queue_items
             WHERE item_id = ?1",
        )
        .bind(item_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .ok_or_else(|| StorageError::NotFound(format!("review item {item_id}")))?;

        Self::parse_review_item_detail(row)
    }

    async fn decide_review_item(
        &self,
        input: ReviewDecisionInput,
    ) -> Result<ReviewDecisionResponse, StorageError> {
        let endpoint_scope = format!("/v1/review-queue/{}/decide", input.item_id);
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = sqlx::query_scalar::<_, String>(
                "SELECT response_json FROM idempotency_keys WHERE endpoint_scope = ?1 AND idempotency_key = ?2",
            )
            .bind(&endpoint_scope)
            .bind(key)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            {
                let parsed = serde_json::from_str::<ReviewDecisionResponse>(&existing)
                    .map_err(|err| StorageError::Corruption(err.to_string()))?;
                tx.commit()
                    .await
                    .map_err(|err| StorageError::Unavailable(err.to_string()))?;
                return Ok(parsed);
            }
        }

        let row = sqlx::query(
            "SELECT item_id, status, target_domain, proposal_json, base_version
             FROM review_queue_items WHERE item_id = ?1",
        )
        .bind(&input.item_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .ok_or_else(|| StorageError::NotFound(format!("review item {}", input.item_id)))?;

        let current_status: String = row.get("status");
        if current_status != "pending" {
            return Err(StorageError::Conflict(format!(
                "review item {} is not pending",
                input.item_id
            )));
        }

        let target_domain: String = row.get("target_domain");
        if target_domain != "capability_registry" {
            return Err(StorageError::Unsupported(format!(
                "review target domain {target_domain} is not implemented"
            )));
        }

        if input.decision != "approve" && input.decision != "reject" && input.decision != "edit" {
            return Err(StorageError::InvalidInput(
                "decision must be `approve`, `reject`, or `edit`".to_string(),
            ));
        }

        let proposal: Value = serde_json::from_str(&row.get::<String, _>("proposal_json"))
            .map_err(|err| StorageError::Corruption(err.to_string()))?;
        let now = Utc::now();
        let now_rfc3339 = now.to_rfc3339();

        let applied_version = if input.decision == "reject" {
            None
        } else {
            let chosen_payload = input.edited_payload.as_ref().unwrap_or(&proposal);
            let capability_snapshot_version = chosen_payload
                .get("capability_snapshot_version")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    StorageError::InvalidInput(
                        "edited_payload missing capability_snapshot_version".to_string(),
                    )
                })?;

            sqlx::query_scalar::<_, String>(
                "SELECT capability_snapshot_version FROM capability_snapshots WHERE capability_snapshot_version = ?1",
            )
            .bind(capability_snapshot_version)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            .ok_or_else(|| {
                StorageError::InvalidInput(format!(
                    "unknown capability snapshot {capability_snapshot_version}"
                ))
            })?;

            sqlx::query(
                "UPDATE current_versions SET version_id = ?2, updated_at = ?3 WHERE version_kind = ?1",
            )
            .bind("capability_snapshot")
            .bind(capability_snapshot_version)
            .bind(&now_rfc3339)
            .execute(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

            Some(capability_snapshot_version.to_string())
        };

        sqlx::query(
            "UPDATE review_queue_items
             SET status = ?2, updated_at = ?3, resolved_version = ?4
             WHERE item_id = ?1",
        )
        .bind(&input.item_id)
        .bind("resolved")
        .bind(&now_rfc3339)
        .bind(&applied_version)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        sqlx::query(
            "INSERT INTO review_queue_decisions (
                decision_id, item_id, created_at, decision, edited_payload_json, applied_version
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(format!("review-decision:{}", Uuid::new_v4()))
        .bind(&input.item_id)
        .bind(&now_rfc3339)
        .bind(&input.decision)
        .bind(
            input
                .edited_payload
                .as_ref()
                .map(Self::canonical_json_string)
                .transpose()?,
        )
        .bind(&applied_version)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Conflict(err.to_string()))?;

        sqlx::query(
            "INSERT INTO experience_events (
                event_ref, created_at, source_class, author, audience_id, sensitivity_s, taint_s, kind, content_ref, json_payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(format!("event:review:{}", Uuid::new_v4()))
        .bind(&now_rfc3339)
        .bind("control_plane")
        .bind("root_owner")
        .bind("root_owner")
        .bind(0_i64)
        .bind(0_i64)
        .bind("review_decision")
        .bind(Option::<String>::None)
        .bind(
            serde_json::to_string(&json!({
                "item_id": input.item_id.clone(),
                "decision": input.decision.clone(),
                "target_domain": target_domain,
                "base_version": row.get::<Option<String>, _>("base_version"),
                "applied_version": applied_version.clone(),
            }))
            .map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let response = ReviewDecisionResponse {
            item_id: input.item_id.clone(),
            status: "resolved".to_string(),
            decision: input.decision.clone(),
            applied_version: applied_version.clone(),
        };

        if let Some(key) = input.idempotency_key.as_deref() {
            let response_json = serde_json::to_string(&response)
                .map_err(|err| StorageError::Unavailable(err.to_string()))?;
            sqlx::query(
                "INSERT INTO idempotency_keys (
                    endpoint_scope, idempotency_key, request_id, response_json, response_hash, created_at, expires_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(&endpoint_scope)
            .bind(key)
            .bind(&response.item_id)
            .bind(&response_json)
            .bind(&response_json)
            .bind(&now_rfc3339)
            .bind(Option::<String>::None)
            .execute(tx.as_mut())
            .await
            .map_err(|err| StorageError::Conflict(err.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        Ok(response)
    }

    async fn resolve_action_descriptor(
        &self,
        capability_snapshot_version: &str,
        tool_name: &str,
        action_name: &str,
    ) -> Result<adesh_core::action_schemas::ActionDescriptor, StorageError> {
        let snapshot = self
            .get_capability_snapshot(capability_snapshot_version)
            .await?;
        resolve_action_descriptor_from_snapshot(&snapshot.payload, tool_name, action_name)
    }

    async fn create_approval_item(
        &self,
        input: ApprovalItemInput,
    ) -> Result<ApprovalItemSummary, StorageError> {
        let approval_id = format!("approval:{}", Uuid::new_v4());
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO approval_items (
                approval_id, operation_id, created_at, updated_at, status, approval_mode,
                proposal_bundle_json, diff_payload_json, prompt, expires_at, audit_trace_id
            ) VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(&approval_id)
        .bind(&input.operation_id)
        .bind(now.to_rfc3339())
        .bind("pending")
        .bind(&input.approval_mode)
        .bind(
            serde_json::to_string(&input.proposal_bundle)
                .map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .bind(
            serde_json::to_string(&input.diff_payload)
                .map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .bind(&input.prompt)
        .bind(input.expires_at.map(|value| value.to_rfc3339()))
        .bind(&input.audit_trace_id)
        .execute(&self.pool)
        .await
        .map_err(|err| StorageError::Conflict(err.to_string()))?;

        Ok(ApprovalItemSummary {
            approval_id,
            operation_id: input.operation_id,
            approval_mode: input.approval_mode,
            prompt: input.prompt,
            diff: input.diff_payload,
            expires_at: input.expires_at,
            audit_trace_id: input.audit_trace_id,
        })
    }

    async fn get_approval_item(
        &self,
        approval_id: &str,
    ) -> Result<ApprovalItemDetail, StorageError> {
        let row = sqlx::query(
            "SELECT approval_id, operation_id, status, approval_mode, prompt, proposal_bundle_json,
                    diff_payload_json, expires_at, audit_trace_id
             FROM approval_items
             WHERE approval_id = ?1",
        )
        .bind(approval_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .ok_or_else(|| StorageError::NotFound(format!("approval {approval_id}")))?;

        Ok(ApprovalItemDetail {
            approval_id: row.get("approval_id"),
            operation_id: row.get("operation_id"),
            status: row.get("status"),
            approval_mode: row.get("approval_mode"),
            prompt: row.get("prompt"),
            proposal_bundle: serde_json::from_str(&row.get::<String, _>("proposal_bundle_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            diff: serde_json::from_str(&row.get::<String, _>("diff_payload_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            expires_at: row
                .get::<Option<String>, _>("expires_at")
                .map(|value| Self::parse_rfc3339(&value))
                .transpose()?,
            audit_trace_id: row.get("audit_trace_id"),
        })
    }

    async fn list_pending_approvals(&self) -> Result<Vec<ApprovalItemSummary>, StorageError> {
        let rows = sqlx::query(
            "SELECT approval_id, operation_id, approval_mode, prompt, diff_payload_json, expires_at, audit_trace_id
             FROM approval_items WHERE status = 'pending' ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        rows.into_iter()
            .map(|row| {
                Ok(ApprovalItemSummary {
                    approval_id: row.get("approval_id"),
                    operation_id: row.get("operation_id"),
                    approval_mode: row.get("approval_mode"),
                    prompt: row.get("prompt"),
                    diff: serde_json::from_str(&row.get::<String, _>("diff_payload_json"))
                        .map_err(|err| StorageError::Corruption(err.to_string()))?,
                    expires_at: row
                        .get::<Option<String>, _>("expires_at")
                        .map(|value| Self::parse_rfc3339(&value))
                        .transpose()?,
                    audit_trace_id: row.get("audit_trace_id"),
                })
            })
            .collect()
    }

    async fn consume_approval_atomic(
        &self,
        input: ApprovalConsumeInput,
    ) -> Result<ApprovalDecisionResponse, StorageError> {
        let endpoint_scope = format!("/v1/approvals/{}", input.approval_id);
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = sqlx::query_scalar::<_, String>(
                "SELECT response_json FROM idempotency_keys WHERE endpoint_scope = ?1 AND idempotency_key = ?2",
            )
            .bind(&endpoint_scope)
            .bind(key)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            {
                let parsed = serde_json::from_str::<ApprovalDecisionResponse>(&existing)
                    .map_err(|err| StorageError::Corruption(err.to_string()))?;
                tx.commit()
                    .await
                    .map_err(|err| StorageError::Unavailable(err.to_string()))?;
                return Ok(parsed);
            }
        }

        let now = Utc::now();
        let now_rfc3339 = now.to_rfc3339();
        let approval = sqlx::query(
            "SELECT approval_id, operation_id, status, approval_mode, proposal_bundle_json, diff_payload_json,
                    prompt, audit_trace_id
             FROM approval_items
             WHERE approval_id = ?1",
        )
        .bind(&input.approval_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .ok_or_else(|| StorageError::NotFound(format!("approval {}", input.approval_id)))?;

        let status: String = approval.get("status");
        if status != "pending" {
            return Err(StorageError::Conflict(format!(
                "approval {} is not pending",
                input.approval_id
            )));
        }

        let operation_id: String = approval.get("operation_id");
        let approval_mode: String = approval.get("approval_mode");
        let audit_trace_id: String = approval.get("audit_trace_id");
        let proposal_bundle: Value =
            serde_json::from_str(&approval.get::<String, _>("proposal_bundle_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?;
        let diff_payload: Value =
            serde_json::from_str(&approval.get::<String, _>("diff_payload_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?;

        if input.decision != "approve" && input.decision != "deny" {
            return Err(StorageError::InvalidInput(
                "decision must be `approve` or `deny`".to_string(),
            ));
        }

        if input.oob_challenge_id.is_some() && approval_mode != "oob_required" {
            return Err(StorageError::InvalidInput(
                "oob challenge is not valid for non-oob approvals".to_string(),
            ));
        }

        let operation = sqlx::query(
            "SELECT state, pinned_capability_snapshot_version FROM operations WHERE operation_id = ?1",
        )
            .bind(&operation_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            .ok_or_else(|| StorageError::NotFound(format!("operation {operation_id}")))?;
        let current_state: String = operation.get("state");
        if current_state != "awaiting_approval" {
            return Err(StorageError::Conflict(format!(
                "operation {operation_id} is not awaiting approval"
            )));
        }
        let capability_snapshot_version: String =
            operation.get("pinned_capability_snapshot_version");

        let (next_operation_state, next_approval_status, syscall_ids) = if input.decision
            == "approve"
        {
            let tool_name = diff_payload
                .get("tool_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    StorageError::Corruption("approval diff missing tool_id metadata".to_string())
                })?;
            let action_name = diff_payload
                .get("action")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    StorageError::Corruption("approval diff missing action metadata".to_string())
                })?;
            let snapshot_payload = sqlx::query_scalar::<_, String>(
                "SELECT json_payload FROM capability_snapshots WHERE capability_snapshot_version = ?1",
            )
            .bind(&capability_snapshot_version)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            .ok_or_else(|| {
                StorageError::NotFound(format!(
                    "capability snapshot {capability_snapshot_version}"
                ))
            })?;
            let snapshot_json: Value = serde_json::from_str(&snapshot_payload)
                .map_err(|err| StorageError::Corruption(err.to_string()))?;
            let descriptor =
                resolve_action_descriptor_from_snapshot(&snapshot_json, tool_name, action_name)?;
            let args_schema_ref = diff_payload
                .get("args_schema_ref")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    StorageError::Corruption(
                        "approval diff missing args_schema_ref metadata".to_string(),
                    )
                })?;
            if args_schema_ref != descriptor.args_schema_ref {
                return Err(StorageError::Corruption(format!(
                    "approval diff args_schema_ref mismatch for {tool_name}/{action_name}"
                )));
            }
            let result_schema_ref = diff_payload
                .get("result_schema_ref")
                .and_then(Value::as_str);
            if result_schema_ref != descriptor.result_schema_ref.as_deref() {
                return Err(StorageError::Corruption(format!(
                    "approval diff result_schema_ref mismatch for {tool_name}/{action_name}"
                )));
            }
            let payload = normalize_args_for_action(
                tool_name,
                action_name,
                input.modified_payload.as_ref().unwrap_or(&proposal_bundle),
            )?;
            let args_schema_payload = sqlx::query_scalar::<_, String>(
                "SELECT payload_json FROM schema_registry_entries WHERE schema_ref = ?1",
            )
            .bind(descriptor.args_schema_ref.as_str())
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            .ok_or_else(|| {
                StorageError::NotFound(format!("schema entry {}", descriptor.args_schema_ref))
            })?;
            let args_schema_json: Value = serde_json::from_str(&args_schema_payload)
                .map_err(|err| StorageError::Corruption(err.to_string()))?;
            validate_instance_against_schema(
                &args_schema_json,
                &payload,
                ValidationErrorKind::InvalidInput,
            )?;
            let syscall_id = format!("syscall:{}", Uuid::new_v4());

            sqlx::query(
                    "INSERT INTO syscalls (
                    syscall_id, operation_id, approval_id, created_at, updated_at, tool_name,
                    action_name, args_schema_ref, result_schema_ref, status, args_json, result_ref, audit_trace_id
                 ) VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                )
                .bind(&syscall_id)
                .bind(&operation_id)
                .bind(&input.approval_id)
                .bind(&now_rfc3339)
                .bind(tool_name)
                .bind(action_name)
                .bind(descriptor.args_schema_ref)
                .bind(descriptor.result_schema_ref)
                .bind("permitted")
                .bind(
                    serde_json::to_string(&payload)
                        .map_err(|err| StorageError::Unavailable(err.to_string()))?,
                )
                .bind(Option::<String>::None)
                .bind(&audit_trace_id)
                .execute(tx.as_mut())
                .await
                .map_err(|err| StorageError::Conflict(err.to_string()))?;

            (
                "running".to_string(),
                "consumed".to_string(),
                vec![syscall_id],
            )
        } else {
            ("blocked".to_string(), "denied".to_string(), Vec::new())
        };

        sqlx::query(
            "UPDATE approval_items SET status = ?2, updated_at = ?3 WHERE approval_id = ?1",
        )
        .bind(&input.approval_id)
        .bind(&next_approval_status)
        .bind(&now_rfc3339)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        sqlx::query(
            "UPDATE operations SET state = ?2, state_reason = ?3, updated_at = ?4 WHERE operation_id = ?1",
        )
        .bind(&operation_id)
        .bind(&next_operation_state)
        .bind(if input.decision == "approve" {
            "approved_send_pending_execution"
        } else {
            "approval_denied"
        })
        .bind(&now_rfc3339)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        sqlx::query(
            "INSERT INTO operation_transitions (
                transition_id, operation_id, ts, from_state, to_state, reason, audit_trace_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(format!("transition:{}", Uuid::new_v4()))
        .bind(&operation_id)
        .bind(&now_rfc3339)
        .bind("awaiting_approval")
        .bind(&next_operation_state)
        .bind(if input.decision == "approve" {
            "approval_consumed"
        } else {
            "approval_denied"
        })
        .bind(&audit_trace_id)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let mut timeline = sqlx::query_scalar::<_, String>(
            "SELECT timeline_json FROM audit_traces WHERE audit_trace_id = ?1",
        )
        .bind(&audit_trace_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .map(|value| Self::parse_timeline(&value))
        .transpose()?
        .ok_or_else(|| StorageError::Corruption(format!("missing audit trace {audit_trace_id}")))?;

        timeline.push(json!({
            "type": "approval_decision",
            "ts": now_rfc3339,
            "approval_id": input.approval_id.clone(),
            "decision": input.decision.clone(),
            "operation_state": next_operation_state.clone(),
            "syscall_ids": syscall_ids.clone(),
        }));

        sqlx::query("UPDATE audit_traces SET timeline_json = ?2 WHERE audit_trace_id = ?1")
            .bind(&audit_trace_id)
            .bind(
                serde_json::to_string(&timeline)
                    .map_err(|err| StorageError::Unavailable(err.to_string()))?,
            )
            .execute(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        sqlx::query(
            "INSERT INTO experience_events (
                event_ref, created_at, source_class, author, audience_id, sensitivity_s, taint_s, kind, content_ref, json_payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(format!("event:approval:{}", Uuid::new_v4()))
        .bind(&now_rfc3339)
        .bind("control_plane")
        .bind("root_owner")
        .bind("root_owner")
        .bind(0_i64)
        .bind(0_i64)
        .bind("approval_decision")
        .bind(Option::<String>::None)
        .bind(
            serde_json::to_string(&json!({
                "approval_id": input.approval_id.clone(),
                "operation_id": operation_id.clone(),
                "decision": input.decision.clone(),
                "syscall_ids": syscall_ids.clone(),
            }))
            .map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let response = ApprovalDecisionResponse {
            approval_id: input.approval_id,
            operation_id,
            decision: input.decision,
            status: next_approval_status,
            operation_state: next_operation_state,
            syscall_ids,
            audit_trace_id,
        };

        if let Some(key) = input.idempotency_key.as_deref() {
            let response_json = serde_json::to_string(&response)
                .map_err(|err| StorageError::Unavailable(err.to_string()))?;
            sqlx::query(
                "INSERT INTO idempotency_keys (
                    endpoint_scope, idempotency_key, request_id, response_json, response_hash, created_at, expires_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(&endpoint_scope)
            .bind(key)
            .bind(&response.approval_id)
            .bind(&response_json)
            .bind(&response_json)
            .bind(&now_rfc3339)
            .bind(Option::<String>::None)
            .execute(tx.as_mut())
            .await
            .map_err(|err| StorageError::Conflict(err.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        Ok(response)
    }

    async fn list_syscalls_by_operation(
        &self,
        operation_id: &str,
    ) -> Result<Vec<SyscallResponse>, StorageError> {
        let rows = sqlx::query(
            "SELECT syscall_id, operation_id, approval_id, tool_name, action_name, status,
                    args_schema_ref, result_schema_ref, args_json, result_ref, audit_trace_id, created_at, updated_at
             FROM syscalls
             WHERE operation_id = ?1
             ORDER BY created_at ASC",
        )
        .bind(operation_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        rows.into_iter().map(Self::parse_syscall_row).collect()
    }

    async fn get_syscall(&self, syscall_id: &str) -> Result<SyscallResponse, StorageError> {
        let row = sqlx::query(
            "SELECT syscall_id, operation_id, approval_id, tool_name, action_name, status,
                    args_schema_ref, result_schema_ref, args_json, result_ref, audit_trace_id, created_at, updated_at
             FROM syscalls
             WHERE syscall_id = ?1",
        )
        .bind(syscall_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .ok_or_else(|| StorageError::NotFound(format!("syscall {syscall_id}")))?;

        Self::parse_syscall_row(row)
    }

    async fn update_syscall_status(
        &self,
        input: SyscallStatusUpdateInput,
    ) -> Result<SyscallResponse, StorageError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        let now = Utc::now().to_rfc3339();

        let syscall = sqlx::query(
            "SELECT syscall_id, operation_id, approval_id, tool_name, action_name, status,
                    args_schema_ref, result_schema_ref, args_json, result_ref, audit_trace_id, created_at, updated_at
             FROM syscalls
             WHERE syscall_id = ?1",
        )
        .bind(&input.syscall_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .ok_or_else(|| StorageError::NotFound(format!("syscall {}", input.syscall_id)))?;

        let current_status: String = syscall.get("status");
        if current_status != "permitted" && current_status != "executing" {
            return Err(StorageError::Conflict(format!(
                "syscall {} is not executable from status {}",
                input.syscall_id, current_status
            )));
        }

        sqlx::query(
            "UPDATE syscalls SET status = ?2, result_ref = ?3, updated_at = ?4 WHERE syscall_id = ?1",
        )
        .bind(&input.syscall_id)
        .bind(&input.new_status)
        .bind(&input.result_ref)
        .bind(&now)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let row = sqlx::query(
            "SELECT syscall_id, operation_id, approval_id, tool_name, action_name, status,
                    args_schema_ref, result_schema_ref, args_json, result_ref, audit_trace_id, created_at, updated_at
             FROM syscalls
             WHERE syscall_id = ?1",
        )
        .bind(&input.syscall_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        tx.commit()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        Self::parse_syscall_row(row)
    }

    async fn get_audit_trace(
        &self,
        audit_trace_id: &str,
    ) -> Result<AuditTraceRecord, StorageError> {
        let row = sqlx::query(
            "SELECT audit_trace_id, request_id, operation_id, isolation_id, pinned_json,
                    summary_json, timeline_json, attachments_json
             FROM audit_traces WHERE audit_trace_id = ?1",
        )
        .bind(audit_trace_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .ok_or_else(|| StorageError::NotFound(format!("audit trace {audit_trace_id}")))?;

        Ok(AuditTraceRecord {
            audit_trace_id: row.get("audit_trace_id"),
            request_id: row.get("request_id"),
            operation_id: row.get("operation_id"),
            isolation_id: row.get("isolation_id"),
            pinned: serde_json::from_str(&row.get::<String, _>("pinned_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            summary: serde_json::from_str(&row.get::<String, _>("summary_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            timeline: serde_json::from_str(&row.get::<String, _>("timeline_json"))
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
            attachments: row
                .get::<Option<String>, _>("attachments_json")
                .map(|value| serde_json::from_str(&value))
                .transpose()
                .map_err(|err| StorageError::Corruption(err.to_string()))?,
        })
    }

    async fn append_audit_timeline_item(
        &self,
        audit_trace_id: &str,
        item: Value,
    ) -> Result<(), StorageError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let raw = sqlx::query_scalar::<_, String>(
            "SELECT timeline_json FROM audit_traces WHERE audit_trace_id = ?1",
        )
        .bind(audit_trace_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .ok_or_else(|| StorageError::NotFound(format!("audit trace {audit_trace_id}")))?;
        let mut timeline = Self::parse_timeline(&raw)?;
        timeline.push(item);

        sqlx::query("UPDATE audit_traces SET timeline_json = ?2 WHERE audit_trace_id = ?1")
            .bind(audit_trace_id)
            .bind(
                serde_json::to_string(&timeline)
                    .map_err(|err| StorageError::Unavailable(err.to_string()))?,
            )
            .execute(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        tx.commit()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        Ok(())
    }

    async fn create_replay_dry_run(
        &self,
        input: ReplayCreateInput,
    ) -> Result<ReplayResponse, StorageError> {
        let endpoint_scope = format!("/v1/audit/{}/replay", input.source_audit_trace_id);
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        if let Some(key) = input.idempotency_key.as_deref() {
            if let Some(existing) = sqlx::query_scalar::<_, String>(
                "SELECT response_json FROM idempotency_keys WHERE endpoint_scope = ?1 AND idempotency_key = ?2",
            )
            .bind(&endpoint_scope)
            .bind(key)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?
            {
                let parsed = serde_json::from_str::<ReplayResponse>(&existing)
                    .map_err(|err| StorageError::Corruption(err.to_string()))?;
                tx.commit()
                    .await
                    .map_err(|err| StorageError::Unavailable(err.to_string()))?;
                return Ok(parsed);
            }
        }

        if input.mode != "dry_run" || input.strategy != "stored_output" {
            return Err(StorageError::InvalidInput(
                "only dry_run + stored_output replay is implemented".to_string(),
            ));
        }

        let source = sqlx::query(
            "SELECT request_id, operation_id, isolation_id, pinned_json, summary_json, timeline_json, attachments_json
             FROM audit_traces WHERE audit_trace_id = ?1",
        )
        .bind(&input.source_audit_trace_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?
        .ok_or_else(|| StorageError::NotFound(format!("audit trace {}", input.source_audit_trace_id)))?;

        let source_operation_id: String = source.get("operation_id");

        let has_gate = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM gate_decisions WHERE operation_id = ?1",
        )
        .bind(&source_operation_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        let has_slice = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM compiled_slices WHERE operation_id = ?1",
        )
        .bind(&source_operation_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        if has_gate == 0 || has_slice == 0 {
            return Err(StorageError::Corruption(format!(
                "missing replay anchor for operation {}",
                source_operation_id
            )));
        }

        let replay_id = format!("replay:{}", Uuid::new_v4());
        let replay_operation_id = format!("replay_op:{}", Uuid::new_v4());
        let replay_isolation_id = format!("replay_iso:{}", Uuid::new_v4());
        let replay_audit_trace_id = format!("audit:{}", Uuid::new_v4());
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO operations (
                operation_id, parent_request_id, isolation_id, created_at, updated_at, state, state_reason,
                requesting_audience_id, pinned_active_state_version, pinned_capability_snapshot_version,
                pinned_audience_graph_version, budgets_json, operation_goal_json, ipc_json
            )
            SELECT ?1, ?2, ?3, ?4, ?4, 'completed', 'replay_dry_run_completed',
                   requesting_audience_id, pinned_active_state_version, pinned_capability_snapshot_version,
                   pinned_audience_graph_version, budgets_json, operation_goal_json, ipc_json
            FROM operations WHERE operation_id = ?5",
        )
        .bind(&replay_operation_id)
        .bind(format!("request:{replay_id}"))
        .bind(&replay_isolation_id)
        .bind(&now)
        .bind(&source_operation_id)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        sqlx::query(
            "INSERT INTO operation_transitions (
                transition_id, operation_id, ts, from_state, to_state, reason, audit_trace_id
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(format!("transition:{}", Uuid::new_v4()))
        .bind(&replay_operation_id)
        .bind(&now)
        .bind(Option::<String>::None)
        .bind("completed")
        .bind("replay_dry_run_completed")
        .bind(&replay_audit_trace_id)
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let timeline = json!([
            {
                "type": "replay_started",
                "ts": now,
                "source_audit_trace_id": input.source_audit_trace_id,
                "strategy": input.strategy,
                "mode": input.mode
            },
            {
                "type": "replay_completed",
                "ts": now,
                "simulated": true,
                "source_operation_id": source_operation_id
            }
        ]);

        sqlx::query(
            "INSERT INTO audit_traces (
                audit_trace_id, created_at, request_id, operation_id, isolation_id, pinned_json, summary_json, timeline_json, attachments_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(&replay_audit_trace_id)
        .bind(&now)
        .bind(format!("request:{replay_id}"))
        .bind(&replay_operation_id)
        .bind(&replay_isolation_id)
        .bind(source.get::<String, _>("pinned_json"))
        .bind(
            serde_json::to_string(&json!({
                "kind": "replay_dry_run",
                "source_audit_trace_id": input.source_audit_trace_id,
                "source_operation_id": source_operation_id,
            }))
            .map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .bind(
            serde_json::to_string(&timeline)
                .map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .bind(source.get::<Option<String>, _>("attachments_json"))
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let report_event_ref = format!("event:replay_report:{}", Uuid::new_v4());
        sqlx::query(
            "INSERT INTO experience_events (
                event_ref, created_at, source_class, author, audience_id, sensitivity_s, taint_s, kind, content_ref, json_payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        )
        .bind(&report_event_ref)
        .bind(&now)
        .bind("control_plane")
        .bind("root_owner")
        .bind("root_owner")
        .bind(0_i64)
        .bind(0_i64)
        .bind("replay_report")
        .bind(Option::<String>::None)
        .bind(
            serde_json::to_string(&json!({
                "replay_id": replay_id,
                "source_audit_trace_id": input.source_audit_trace_id,
                "source_operation_id": source_operation_id,
                "simulated": true,
            }))
            .map_err(|err| StorageError::Unavailable(err.to_string()))?,
        )
        .execute(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let response = ReplayResponse {
            replay_id,
            operation_id: replay_operation_id,
            audit_trace_id: replay_audit_trace_id,
            status: "completed".to_string(),
        };

        if let Some(key) = input.idempotency_key.as_deref() {
            let response_json = serde_json::to_string(&response)
                .map_err(|err| StorageError::Unavailable(err.to_string()))?;
            sqlx::query(
                "INSERT INTO idempotency_keys (
                    endpoint_scope, idempotency_key, request_id, response_json, response_hash, created_at, expires_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(&endpoint_scope)
            .bind(key)
            .bind(&response.replay_id)
            .bind(&response_json)
            .bind(&response_json)
            .bind(&now)
            .bind(Option::<String>::None)
            .execute(tx.as_mut())
            .await
            .map_err(|err| StorageError::Conflict(err.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        Ok(response)
    }

    async fn try_acquire_operation_lease(
        &self,
        operation_id: &str,
        runner_id: &str,
        lease_duration_ms: i64,
    ) -> Result<LeaseAcquisition, StorageError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        let now = Utc::now();
        let now_rfc3339 = now.to_rfc3339();
        let leased_until = (now + Duration::milliseconds(lease_duration_ms)).to_rfc3339();

        let existing = sqlx::query(
            "SELECT lease_owner, leased_until, lease_epoch FROM operation_leases WHERE operation_id = ?1",
        )
        .bind(operation_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        let result = if let Some(row) = existing {
            let current_owner: String = row.get("lease_owner");
            let current_until: String = row.get("leased_until");
            let current_epoch: i64 = row.get("lease_epoch");
            let current_until = Self::parse_rfc3339(&current_until)?;

            if current_owner != runner_id && current_until > now {
                LeaseAcquisition {
                    acquired: false,
                    lease_epoch: None,
                }
            } else {
                let next_epoch = if current_owner == runner_id {
                    current_epoch
                } else {
                    current_epoch + 1
                };
                sqlx::query(
                    "UPDATE operation_leases
                     SET lease_owner = ?2, leased_until = ?3, lease_epoch = ?4, last_heartbeat_at = ?5, updated_at = ?5
                     WHERE operation_id = ?1",
                )
                .bind(operation_id)
                .bind(runner_id)
                .bind(&leased_until)
                .bind(next_epoch)
                .bind(&now_rfc3339)
                .execute(tx.as_mut())
                .await
                .map_err(|err| StorageError::Unavailable(err.to_string()))?;
                LeaseAcquisition {
                    acquired: true,
                    lease_epoch: Some(next_epoch),
                }
            }
        } else {
            sqlx::query(
                "INSERT INTO operation_leases (
                    operation_id, lease_owner, leased_until, lease_epoch, last_heartbeat_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            )
            .bind(operation_id)
            .bind(runner_id)
            .bind(&leased_until)
            .bind(1_i64)
            .bind(&now_rfc3339)
            .execute(tx.as_mut())
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
            LeaseAcquisition {
                acquired: true,
                lease_epoch: Some(1),
            }
        };

        tx.commit()
            .await
            .map_err(|err| StorageError::Unavailable(err.to_string()))?;
        Ok(result)
    }

    async fn renew_operation_lease(
        &self,
        operation_id: &str,
        runner_id: &str,
        lease_epoch: i64,
        lease_duration_ms: i64,
    ) -> Result<OperationLease, StorageError> {
        let now = Utc::now();
        let leased_until = now + Duration::milliseconds(lease_duration_ms);

        let result = sqlx::query(
            "UPDATE operation_leases
             SET leased_until = ?4, last_heartbeat_at = ?3, updated_at = ?3
             WHERE operation_id = ?1 AND lease_owner = ?2 AND lease_epoch = ?5",
        )
        .bind(operation_id)
        .bind(runner_id)
        .bind(now.to_rfc3339())
        .bind(leased_until.to_rfc3339())
        .bind(lease_epoch)
        .execute(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(StorageError::Conflict(format!(
                "lease renewal rejected for {operation_id}"
            )));
        }

        Ok(OperationLease {
            operation_id: operation_id.to_string(),
            lease_owner: runner_id.to_string(),
            leased_until,
            lease_epoch,
            last_heartbeat_at: Some(now),
            updated_at: now,
        })
    }

    async fn release_operation_lease(
        &self,
        operation_id: &str,
        runner_id: &str,
        lease_epoch: i64,
    ) -> Result<(), StorageError> {
        let result = sqlx::query(
            "DELETE FROM operation_leases WHERE operation_id = ?1 AND lease_owner = ?2 AND lease_epoch = ?3",
        )
        .bind(operation_id)
        .bind(runner_id)
        .bind(lease_epoch)
        .execute(&self.pool)
        .await
        .map_err(|err| StorageError::Unavailable(err.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(StorageError::Conflict(format!(
                "lease release rejected for {operation_id}"
            )));
        }

        Ok(())
    }
}
