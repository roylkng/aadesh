use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use adesh_contracts::{
    ApiSuccess, ApprovalDecisionRequest, ApprovalItemDetail, ApprovalItemSummary,
    AuditTraceResponse, CapabilityActivationRequest, CapabilitySnapshotMintRequest,
    CapabilitySnapshotMintResponse, CapabilitySnapshotResponse, CompiledSliceResponse,
    CurrentVersionsResponse, GateDecisionResponse, HealthResponse, IngestJobCreateRequest,
    IngestJobResponse, InterfaceInstanceCreateRequest, InterfaceInstanceResponse,
    InterfaceSpecRegisterRequest, InterfaceSpecResponse, InterfaceSpecSummary,
    ManualArtifactCreateRequest, ManualArtifactResponse, Meta, OobStartResponse, OobVerifyRequest,
    OobVerifyResponse, ReasoningOutputResponse, ReplayRequest, ReplayResponse, RequestEnvelope,
    RequestStatusResponse, ReviewDecisionRequest, ReviewDecisionResponse, ReviewItemDetail,
    ReviewItemSummary, SchemaEntryResponse, SchemaRegisterRequest, SyscallResponse,
    WedgeMetricsResponse, WorkflowInstanceCreateRequest, WorkflowInstanceResponse,
    WorkflowSpecRegisterRequest, WorkflowSpecResponse, WorkflowSpecSummary,
};
use adesh_core::{
    AppError, StorageError,
    action_schemas::{ValidationErrorKind, validate_instance_against_schema},
    ports::{
        job_queue::{JobCancelInput, JobEnqueueInput},
        storage::{
            ApprovalConsumeInput, CapabilityActivationReviewInput, CapabilitySnapshotMintInput,
            EventAppendInput, IngestJobCreateInput, IngestJobStatusUpdateInput, IngestOptionsInput,
            IngestSourceInput, InterfaceInstanceCreateInput, InterfaceSpecQuery,
            InterfaceSpecRegisterInput, ManualArtifactCreateInput, OobStartInput, OobVerifyInput,
            ReasoningOutputInput, ReplayCreateInput, ReviewDecisionInput, SchemaRegisterInput,
            StorageProvider, SyscallStatusUpdateInput, WorkflowInstanceCreateInput,
            WorkflowInstanceStateUpdateInput, WorkflowSpecQuery, WorkflowSpecRegisterInput,
        },
    },
};

use super::AppState;
use crate::kernel::{KernelOutcome, approval_item_input, compile_and_verify_stub};

const EXECUTION_LEASE_MS: i64 = 30_000;

#[derive(Debug, serde::Deserialize, Default)]
pub struct SpecListQuery {
    name: Option<String>,
    tag: Option<String>,
    author: Option<String>,
    limit: Option<u32>,
}

fn emit_event(
    sender: &tokio::sync::broadcast::Sender<String>,
    event_type: &str,
    request_id: &str,
    operation_id: Option<&str>,
    isolation_id: Option<&str>,
    audit_trace_id: Option<&str>,
    data: Value,
) {
    emit_extended_event(
        sender,
        event_type,
        request_id,
        operation_id,
        None,
        None,
        None,
        isolation_id,
        audit_trace_id,
        data,
    );
}

fn emit_extended_event(
    sender: &tokio::sync::broadcast::Sender<String>,
    event_type: &str,
    request_id: &str,
    operation_id: Option<&str>,
    workflow_instance_id: Option<&str>,
    step_id: Option<&str>,
    interface_instance_id: Option<&str>,
    isolation_id: Option<&str>,
    audit_trace_id: Option<&str>,
    data: Value,
) {
    let envelope = json!({
        "event_id": Uuid::new_v4().to_string(),
        "ts": Utc::now(),
        "type": event_type,
        "request_id": request_id,
        "operation_id": operation_id,
        "workflow_instance_id": workflow_instance_id,
        "step_id": step_id,
        "interface_instance_id": interface_instance_id,
        "isolation_id": isolation_id,
        "audit_trace_id": audit_trace_id,
        "data": data,
    });
    if let Ok(payload) = serde_json::to_string(&envelope) {
        let _ = sender.send(payload);
    }
}

fn rate_limit_key(headers: &HeaderMap, suffix: &str) -> String {
    let principal = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("anonymous");
    format!("{principal}:{suffix}")
}

fn enforce_rate_limit(
    state: &AppState,
    headers: &HeaderMap,
    suffix: &str,
) -> Result<(), axum::response::Response> {
    let key = rate_limit_key(headers, suffix);
    if state.rate_limiter.allow(
        &key,
        state.config.rate_limit_max_requests,
        state.config.rate_limit_window_seconds,
    ) {
        return Ok(());
    }

    let body = AppError::RateLimited.into_response_body(Uuid::new_v4().to_string());
    Err((StatusCode::TOO_MANY_REQUESTS, Json(body)).into_response())
}

fn validate_ingest_request(request: &IngestJobCreateRequest) -> Result<(), AppError> {
    if request.sources.is_empty() {
        return Err(AppError::BadRequest(
            "ingest job requires at least one source".to_string(),
        ));
    }
    if request.options.max_artifacts <= 0 {
        return Err(AppError::BadRequest(
            "ingest options.max_artifacts must be positive".to_string(),
        ));
    }

    for source in &request.sources {
        match source.r#type.as_str() {
            "text" | "file" | "folder" | "conversation" | "url" => {}
            other => {
                return Err(AppError::BadRequest(format!(
                    "unsupported ingest source type `{other}`"
                )));
            }
        }
    }

    match request.options.chunking.as_str() {
        "none" | "page" | "fixed_tokens" => {}
        other => {
            return Err(AppError::BadRequest(format!(
                "unsupported ingest chunking mode `{other}`"
            )));
        }
    }

    match request.options.classification_mode.as_str() {
        "conservative" | "normal" => {}
        other => {
            return Err(AppError::BadRequest(format!(
                "unsupported classification mode `{other}`"
            )));
        }
    }

    Ok(())
}

async fn execute_permitted_syscalls(
    state: &AppState,
    request_id: &str,
    operation_id: &str,
    audit_trace_id: &str,
    syscall_ids: &[String],
) -> Result<(), adesh_core::StorageError> {
    let runner_id = format!("executor:{request_id}");
    let lease = state
        .storage
        .try_acquire_operation_lease(operation_id, &runner_id, EXECUTION_LEASE_MS)
        .await?;
    if !lease.acquired {
        return Ok(());
    }

    let execution_result = execute_permitted_syscalls_under_lease(
        state,
        request_id,
        operation_id,
        audit_trace_id,
        syscall_ids,
    )
    .await;

    let release_result = state
        .storage
        .release_operation_lease(
            operation_id,
            &runner_id,
            lease.lease_epoch.unwrap_or_default(),
        )
        .await;

    execution_result?;
    release_result?;
    Ok(())
}

async fn execute_permitted_syscalls_under_lease(
    state: &AppState,
    request_id: &str,
    operation_id: &str,
    audit_trace_id: &str,
    syscall_ids: &[String],
) -> Result<(), adesh_core::StorageError> {
    for syscall_id in syscall_ids {
        let syscall = state.storage.get_syscall(syscall_id).await?;
        if syscall.status != "permitted" {
            continue;
        }
        state
            .storage
            .update_syscall_status(SyscallStatusUpdateInput {
                syscall_id: syscall.syscall_id.clone(),
                new_status: "executing".to_string(),
                result_ref: syscall.result_ref.clone(),
            })
            .await?;
        let retry_attempts = state.config.syscall_retry_attempts.max(1);
        let mut last_error: Option<String> = None;
        let mut executed = false;

        for attempt in 1..=retry_attempts {
            match state
                .tools
                .execute_syscall(
                    &syscall.syscall_id,
                    &syscall.tool_name,
                    &syscall.action_name,
                    &syscall.args_schema_ref,
                    syscall.result_schema_ref.as_deref(),
                    &syscall.args,
                )
                .await
            {
                Ok(result) => {
                    if let Some(result_schema_ref) = syscall.result_schema_ref.as_deref() {
                        let result_schema =
                            state.storage.get_schema_entry(result_schema_ref).await?;
                        validate_instance_against_schema(
                            &result_schema.payload,
                            &result.output_json,
                            ValidationErrorKind::Corruption,
                        )?;
                    }

                    let result_ref = format!("event:syscall_result:{}", Uuid::new_v4());
                    let event_author = result
                        .output_json
                        .get("provider")
                        .and_then(Value::as_str)
                        .unwrap_or("tool_provider");
                    state
                        .storage
                        .append_event(EventAppendInput {
                            event_ref: result_ref.clone(),
                            created_at: result.ended_at,
                            source_class: "tool_provider".to_string(),
                            author: event_author.to_string(),
                            audience_id: "root_owner".to_string(),
                            sensitivity_s: result.sensitivity_s,
                            taint_s: result.taint_s,
                            kind: "syscall_result".to_string(),
                            content_ref: result.content_ref.clone(),
                            json_payload: json!({
                                "syscall_id": syscall.syscall_id,
                                "tool_name": syscall.tool_name,
                                "action_name": syscall.action_name,
                                "ok": result.ok,
                                "output_kind": result.output_kind,
                                "output_json": result.output_json,
                                "started_at": result.started_at,
                                "ended_at": result.ended_at,
                                "attempts_used": result.attempts_used,
                            }),
                        })
                        .await?;

                    let updated = state
                        .storage
                        .update_syscall_status(SyscallStatusUpdateInput {
                            syscall_id: syscall.syscall_id.clone(),
                            new_status: "executed".to_string(),
                            result_ref: Some(result_ref.clone()),
                        })
                        .await?;

                    state
                        .storage
                        .append_audit_timeline_item(
                            audit_trace_id,
                            json!({
                                "type": "syscall_executed",
                                "ts": Utc::now(),
                                "syscall_id": updated.syscall_id,
                                "result_ref": result_ref,
                                "status": updated.status,
                            }),
                        )
                        .await?;

                    emit_event(
                        &state.events,
                        "syscall_executed",
                        request_id,
                        Some(operation_id),
                        None,
                        Some(audit_trace_id),
                        json!({
                            "syscall_id": updated.syscall_id,
                            "tool_name": updated.tool_name,
                            "action_name": updated.action_name,
                            "status": updated.status,
                        }),
                    );
                    executed = true;
                    break;
                }
                Err(error) => {
                    last_error = Some(error.to_string());
                    if attempt < retry_attempts && matches!(error, StorageError::Unavailable(_)) {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        continue;
                    }
                }
            }
        }

        if !executed {
            state
                .storage
                .update_syscall_status(SyscallStatusUpdateInput {
                    syscall_id: syscall.syscall_id.clone(),
                    new_status: "failed".to_string(),
                    result_ref: None,
                })
                .await?;
            state
                .storage
                .update_operation_state(
                    operation_id,
                    "failed",
                    Some("syscall_execution_failed"),
                    audit_trace_id,
                )
                .await?;
            state
                .storage
                .append_audit_timeline_item(
                    audit_trace_id,
                    json!({
                        "type": "syscall_failed",
                        "ts": Utc::now(),
                        "syscall_id": syscall.syscall_id,
                        "error_class": "tool_execution_failed",
                        "message": "tool execution failed after retry",
                    }),
                )
                .await?;
            emit_event(
                &state.events,
                "syscall_denied",
                request_id,
                Some(operation_id),
                None,
                Some(audit_trace_id),
                json!({
                    "syscall_id": syscall.syscall_id,
                    "deny_class": "tool_execution_failed",
                    "violations": [],
                    "remediation": {
                        "action": "retry_or_edit_request"
                    },
                    "backend_error": last_error,
                }),
            );
        }
    }

    let operation = state.storage.get_operation(operation_id).await?;
    if operation.state == "running" {
        let syscalls = state
            .storage
            .list_syscalls_by_operation(operation_id)
            .await?;
        let all_executed =
            !syscalls.is_empty() && syscalls.iter().all(|item| item.status == "executed");
        if all_executed {
            state
                .storage
                .update_operation_state(
                    operation_id,
                    "completed",
                    Some("syscalls_executed"),
                    audit_trace_id,
                )
                .await?;
            state
                .storage
                .append_audit_timeline_item(
                    audit_trace_id,
                    json!({
                        "type": "operation_completed",
                        "ts": Utc::now(),
                        "operation_id": operation_id,
                        "reason": "syscalls_executed",
                    }),
                )
                .await?;
            emit_event(
                &state.events,
                "operation_state",
                request_id,
                Some(operation_id),
                None,
                Some(audit_trace_id),
                json!({"state": "completed", "reason": "syscalls_executed"}),
            );
        }
    }

    Ok(())
}

pub(crate) async fn recover_pending_operation_executions(
    state: &AppState,
) -> Result<usize, adesh_core::StorageError> {
    let recoverable = state
        .storage
        .list_recoverable_operation_executions()
        .await?;
    for item in &recoverable {
        let request_id = format!("recovery:{}", item.operation_id);
        execute_permitted_syscalls(
            state,
            &request_id,
            &item.operation_id,
            &item.audit_trace_id,
            &item.syscall_ids,
        )
        .await?;
    }
    Ok(recoverable.len())
}

pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let storage = match state.storage.health().await {
        Ok(()) => "ok",
        Err(_) => "degraded",
    };
    let model_provider = match state.models.health().await {
        Ok(()) => "ok",
        Err(_) => "degraded",
    };
    let tool_provider = match state.tools.health().await {
        Ok(()) => "ok",
        Err(_) => "degraded",
    };
    let queue = match state.queue.health().await {
        Ok(()) => "ok",
        Err(_) => "degraded",
    };
    let status =
        if storage == "ok" && model_provider == "ok" && tool_provider == "ok" && queue == "ok" {
            "ok"
        } else {
            "degraded"
        };

    let body = ApiSuccess {
        ok: true,
        data: HealthResponse {
            status: status.to_string(),
            version: state.config.server_version.clone(),
            storage: storage.to_string(),
            model_provider: model_provider.to_string(),
            tool_provider: tool_provider.to_string(),
            queue: queue.to_string(),
        },
        meta: Meta {
            request_id: Uuid::new_v4().to_string(),
            ts: Utc::now(),
            audit_trace_id: None,
        },
    };

    (StatusCode::OK, Json(body))
}

pub async fn create_manual_artifact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ManualArtifactCreateRequest>,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_manual_artifact") {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    respond_mutation::<ManualArtifactResponse, _>(
        state
            .storage
            .create_manual_artifact(ManualArtifactCreateInput {
                filename: request.filename,
                media_type: request.media_type,
                content_base64: request.content_base64,
                sensitivity_hint: request.sensitivity_hint,
                idempotency_key,
            })
            .await,
        request_id,
    )
}

pub async fn create_ingest_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<IngestJobCreateRequest>,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_ingest_jobs") {
        return response;
    }
    if let Err(error) = validate_ingest_request(&request) {
        let request_id = Uuid::new_v4().to_string();
        let body = error.into_response_body(request_id);
        return (StatusCode::BAD_REQUEST, Json(body)).into_response();
    }

    let request_id = Uuid::new_v4().to_string();
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    match state
        .storage
        .create_ingest_job(IngestJobCreateInput {
            sources: request
                .sources
                .into_iter()
                .map(|source| IngestSourceInput {
                    source_type: source.r#type,
                    payload: source.payload,
                    metadata: source.metadata,
                })
                .collect(),
            options: IngestOptionsInput {
                dedupe: request.options.dedupe,
                max_artifacts: request.options.max_artifacts,
                chunking: request.options.chunking,
                classification_mode: request.options.classification_mode,
            },
            idempotency_key,
        })
        .await
    {
        Ok(data) => {
            if let Err(error) = state
                .queue
                .enqueue_job(JobEnqueueInput {
                    job_id: Some(data.job_id.clone()),
                    job_type: "ingest.run_job".to_string(),
                    payload: json!({
                        "job_id": data.job_id.clone(),
                    }),
                    dedupe_key: Some(format!("ingest.run_job:{}", data.job_id)),
                    run_after: None,
                    max_attempts: 5,
                    sensitivity_s: 0,
                    taint_s: 0,
                    provenance_refs: json!([format!("ingest_job:{}", data.job_id)]),
                })
                .await
            {
                let _ = state
                    .storage
                    .update_ingest_job_status(IngestJobStatusUpdateInput {
                        job_id: data.job_id.clone(),
                        status: "failed".to_string(),
                        artifacts_total: data.counters.artifacts_total,
                        artifacts_succeeded: data.counters.artifacts_succeeded,
                        artifacts_failed: data.counters.artifacts_failed,
                        bytes_ingested: data.counters.bytes_ingested,
                        error_summary: Some("job queue enqueue failed".to_string()),
                    })
                    .await;
                let body = AppError::Storage(error).into_response_body(request_id);
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
            }

            emit_extended_event(
                &state.events,
                "ingest_job_created",
                &request_id,
                None,
                None,
                None,
                None,
                None,
                None,
                json!({
                    "job_id": data.job_id.clone(),
                    "status": data.status.clone(),
                }),
            );

            (
                StatusCode::ACCEPTED,
                Json(ApiSuccess {
                    ok: true,
                    meta: Meta {
                        request_id,
                        ts: Utc::now(),
                        audit_trace_id: None,
                    },
                    data,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            (status, Json(body)).into_response()
        }
    }
}

pub async fn get_ingest_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    respond_query::<IngestJobResponse, _>(
        state.storage.get_ingest_job(&job_id).await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn cancel_ingest_job(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_ingest_cancel") {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    match state.storage.get_ingest_job(&job_id).await {
        Ok(existing) => {
            if existing.status == "cancelled" {
                return (
                    StatusCode::OK,
                    Json(ApiSuccess {
                        ok: true,
                        meta: Meta {
                            request_id,
                            ts: Utc::now(),
                            audit_trace_id: None,
                        },
                        data: existing,
                    }),
                )
                    .into_response();
            }
            if existing.status != "pending" && existing.status != "running" {
                let body = AppError::Storage(StorageError::Conflict(format!(
                    "ingest job {job_id} cannot be cancelled from status {}",
                    existing.status
                )))
                .into_response_body(request_id);
                return (StatusCode::CONFLICT, Json(body)).into_response();
            }

            let _ = state
                .queue
                .cancel_job(JobCancelInput {
                    job_id: job_id.clone(),
                })
                .await;

            match state
                .storage
                .update_ingest_job_status(IngestJobStatusUpdateInput {
                    job_id,
                    status: "cancelled".to_string(),
                    artifacts_total: existing.counters.artifacts_total,
                    artifacts_succeeded: existing.counters.artifacts_succeeded,
                    artifacts_failed: existing.counters.artifacts_failed,
                    bytes_ingested: existing.counters.bytes_ingested,
                    error_summary: existing.error_summary,
                })
                .await
            {
                Ok(data) => {
                    emit_extended_event(
                        &state.events,
                        "ingest_job_cancelled",
                        &request_id,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        json!({
                            "job_id": data.job_id.clone(),
                            "status": data.status.clone(),
                            "error_summary": data.error_summary.clone(),
                        }),
                    );

                    (
                        StatusCode::OK,
                        Json(ApiSuccess {
                            ok: true,
                            meta: Meta {
                                request_id,
                                ts: Utc::now(),
                                audit_trace_id: None,
                            },
                            data,
                        }),
                    )
                        .into_response()
                }
                Err(error) => {
                    let status = status_for_storage_error(&error);
                    let body = AppError::Storage(error).into_response_body(request_id);
                    (status, Json(body)).into_response()
                }
            }
        }
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            (status, Json(body)).into_response()
        }
    }
}

pub async fn submit_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RequestEnvelope>,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_requests") {
        return response;
    }

    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok());

    match state
        .storage
        .create_operation_bundle(&request, idempotency_key)
        .await
    {
        Ok(data) => {
            let operation = match state
                .storage
                .get_operation(&data.primary_operation_id)
                .await
            {
                Ok(operation) => operation,
                Err(error) => {
                    let body = AppError::Storage(error).into_response_body(request.request_id);
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
                }
            };

            if operation.state != "created" {
                let body = ApiSuccess {
                    ok: true,
                    meta: Meta {
                        request_id: request.request_id,
                        ts: request.received_at,
                        audit_trace_id: data.audit_trace_ids.first().cloned(),
                    },
                    data,
                };
                return (StatusCode::CREATED, Json(body)).into_response();
            }

            let requested_send = request.input.content.to_lowercase().contains("send");
            let send_descriptor = if requested_send {
                state
                    .storage
                    .resolve_action_descriptor(
                        &operation.pinned_capability_snapshot_version,
                        "email",
                        "send",
                    )
                    .await
                    .ok()
            } else {
                None
            };

            let attachment_context = {
                let mut entries = Vec::new();
                for attachment in &request.input.attachments {
                    if attachment.ref_type != "manual_artifact" {
                        continue;
                    }
                    let context = match state
                        .storage
                        .get_manual_artifact_context(&attachment.ref_id, 1_500)
                        .await
                    {
                        Ok(value) => value,
                        Err(error) => {
                            let body =
                                AppError::Storage(error).into_response_body(request.request_id);
                            return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
                        }
                    };
                    if let Some(context) = context {
                        entries.push(context);
                    }
                }
                entries
            };

            let artifacts = compile_and_verify_stub(
                &request,
                &operation.operation_id,
                &operation.isolation_id,
                &operation.audit_trace_id,
                &operation.pinned_active_state_version,
                &operation.pinned_capability_snapshot_version,
                &operation.pinned_audience_graph_version,
                send_descriptor.as_ref(),
            );

            let gate = match state
                .storage
                .put_gate_decision(artifacts.gate_decision)
                .await
            {
                Ok(gate) => gate,
                Err(error) => {
                    let body = AppError::Storage(error).into_response_body(request.request_id);
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
                }
            };
            emit_event(
                &state.events,
                "audit_update",
                &request.request_id,
                Some(&operation.operation_id),
                Some(&operation.isolation_id),
                Some(&operation.audit_trace_id),
                json!({"ref_type": "gate_decision", "ref_id": gate.gate_decision_id}),
            );

            let compiled = match state
                .storage
                .put_compiled_slice(artifacts.compiled_slice)
                .await
            {
                Ok(compiled) => compiled,
                Err(error) => {
                    let body = AppError::Storage(error).into_response_body(request.request_id);
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
                }
            };
            emit_event(
                &state.events,
                "audit_update",
                &request.request_id,
                Some(&operation.operation_id),
                Some(&operation.isolation_id),
                Some(&operation.audit_trace_id),
                json!({"ref_type": "compiled_slice", "ref_id": compiled.compiled_slice_id}),
            );

            let model_output = match state
                .models
                .generate(adesh_core::ports::model::ModelGenerateInput {
                    operation_id: operation.operation_id.clone(),
                    isolation_id: operation.isolation_id.clone(),
                    audit_trace_id: operation.audit_trace_id.clone(),
                    request_content: request.input.content.clone(),
                    attachment_count: request.input.attachments.len(),
                    attachment_context,
                })
                .await
            {
                Ok(output) => output,
                Err(error) => {
                    let body = AppError::Storage(error).into_response_body(request.request_id);
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
                }
            };

            let stream_id = format!("stream:{}", Uuid::new_v4());
            emit_event(
                &state.events,
                "reasoning_stream_start",
                &request.request_id,
                Some(&operation.operation_id),
                Some(&operation.isolation_id),
                Some(&operation.audit_trace_id),
                json!({
                    "stream_id": stream_id,
                    "channels": ["draft"],
                    "model_id": model_output.model_id,
                }),
            );
            let draft_text = model_output
                .reasoning_output
                .get("drafts")
                .and_then(Value::as_array)
                .and_then(|drafts| drafts.first())
                .and_then(|first| first.get("content"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            for (seq, chunk) in draft_text
                .as_bytes()
                .chunks(160)
                .map(|part| String::from_utf8_lossy(part).to_string())
                .enumerate()
            {
                emit_event(
                    &state.events,
                    "reasoning_stream_chunk",
                    &request.request_id,
                    Some(&operation.operation_id),
                    Some(&operation.isolation_id),
                    Some(&operation.audit_trace_id),
                    json!({
                        "stream_id": stream_id,
                        "channel": "draft",
                        "seq": seq,
                        "delta": chunk,
                        "is_final": false,
                    }),
                );
            }

            let reasoning = match state
                .storage
                .put_reasoning_output(ReasoningOutputInput {
                    operation_id: operation.operation_id.clone(),
                    isolation_id: operation.isolation_id.clone(),
                    audit_trace_id: operation.audit_trace_id.clone(),
                    model_id: model_output.model_id,
                    provider_trace_id: model_output.provider_trace_id,
                    reasoning_output: model_output.reasoning_output,
                })
                .await
            {
                Ok(reasoning) => reasoning,
                Err(error) => {
                    let body = AppError::Storage(error).into_response_body(request.request_id);
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
                }
            };
            emit_event(
                &state.events,
                "audit_update",
                &request.request_id,
                Some(&operation.operation_id),
                Some(&operation.isolation_id),
                Some(&operation.audit_trace_id),
                json!({"ref_type": "reasoning_output", "ref_id": reasoning.event_ref}),
            );
            emit_event(
                &state.events,
                "reasoning_stream_end",
                &request.request_id,
                Some(&operation.operation_id),
                Some(&operation.isolation_id),
                Some(&operation.audit_trace_id),
                json!({
                    "stream_id": stream_id,
                    "is_final": true,
                    "final_output_ref": reasoning.event_ref,
                }),
            );

            match artifacts.outcome {
                KernelOutcome::CompletedDraft => {
                    if let Err(error) = state
                        .storage
                        .update_operation_state(
                            &operation.operation_id,
                            "completed",
                            Some("draft_ready"),
                            &operation.audit_trace_id,
                        )
                        .await
                    {
                        let body =
                            AppError::Storage(error).into_response_body(request.request_id.clone());
                        return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
                    }
                    emit_event(
                        &state.events,
                        "operation_state",
                        &request.request_id,
                        Some(&operation.operation_id),
                        Some(&operation.isolation_id),
                        Some(&operation.audit_trace_id),
                        json!({"state": "completed", "reason": "draft_ready"}),
                    );
                }
                KernelOutcome::AwaitingApproval(plan) => {
                    let approval = match state
                        .storage
                        .create_approval_item(approval_item_input(
                            &operation.operation_id,
                            &operation.audit_trace_id,
                            plan,
                        ))
                        .await
                    {
                        Ok(approval) => approval,
                        Err(error) => {
                            let body =
                                AppError::Storage(error).into_response_body(request.request_id);
                            return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
                        }
                    };
                    if let Err(error) = state
                        .storage
                        .update_operation_state(
                            &operation.operation_id,
                            "awaiting_approval",
                            Some("approval_required"),
                            &operation.audit_trace_id,
                        )
                        .await
                    {
                        let body =
                            AppError::Storage(error).into_response_body(request.request_id.clone());
                        return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
                    }
                    emit_event(
                        &state.events,
                        "operation_state",
                        &request.request_id,
                        Some(&operation.operation_id),
                        Some(&operation.isolation_id),
                        Some(&operation.audit_trace_id),
                        json!({"state": "awaiting_approval", "reason": "approval_required"}),
                    );
                    emit_event(
                        &state.events,
                        "approval_required",
                        &request.request_id,
                        Some(&operation.operation_id),
                        Some(&operation.isolation_id),
                        Some(&operation.audit_trace_id),
                        json!({
                            "approval_id": approval.approval_id,
                            "approval_mode": approval.approval_mode,
                            "prompt": approval.prompt,
                            "diff": approval.diff,
                            "expires_at": approval.expires_at,
                        }),
                    );
                }
                KernelOutcome::Blocked { reason } => {
                    if let Err(error) = state
                        .storage
                        .update_operation_state(
                            &operation.operation_id,
                            "blocked",
                            Some(&reason),
                            &operation.audit_trace_id,
                        )
                        .await
                    {
                        let body =
                            AppError::Storage(error).into_response_body(request.request_id.clone());
                        return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
                    }
                    emit_event(
                        &state.events,
                        "operation_state",
                        &request.request_id,
                        Some(&operation.operation_id),
                        Some(&operation.isolation_id),
                        Some(&operation.audit_trace_id),
                        json!({"state": "blocked", "reason": reason}),
                    );
                }
            }

            let body = ApiSuccess {
                ok: true,
                meta: Meta {
                    request_id: request.request_id,
                    ts: request.received_at,
                    audit_trace_id: data.audit_trace_ids.first().cloned(),
                },
                data,
            };
            (StatusCode::CREATED, Json(body)).into_response()
        }
        Err(error) => {
            let body = AppError::Storage(error).into_response_body(request.request_id);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
        }
    }
}

pub async fn get_operation(
    State(state): State<AppState>,
    Path(operation_id): Path<String>,
) -> impl IntoResponse {
    match state.storage.get_operation(&operation_id).await {
        Ok(data) => (
            StatusCode::OK,
            Json(ApiSuccess {
                ok: true,
                meta: Meta {
                    request_id: Uuid::new_v4().to_string(),
                    ts: Utc::now(),
                    audit_trace_id: Some(data.audit_trace_id.clone()),
                },
                data,
            }),
        )
            .into_response(),
        Err(error) => {
            let status = match error {
                adesh_core::StorageError::NotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            let body = AppError::Storage(error).into_response_body(Uuid::new_v4().to_string());
            (status, Json(body)).into_response()
        }
    }
}

pub async fn get_request_status(
    State(state): State<AppState>,
    Path(request_id): Path<String>,
) -> impl IntoResponse {
    respond_query::<RequestStatusResponse, _>(
        state.storage.get_request_status(&request_id).await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn cancel_operation(
    State(state): State<AppState>,
    Path(operation_id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_cancel_operation") {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    let result = state
        .storage
        .cancel_operation(
            &operation_id,
            Some("cancelled_by_root_owner"),
            idempotency_key,
        )
        .await;

    match result {
        Ok(operation) => {
            emit_event(
                &state.events,
                "operation_state",
                &request_id,
                Some(&operation.operation_id),
                Some(&operation.isolation_id),
                Some(&operation.audit_trace_id),
                json!({"state": "cancelled", "reason": "cancelled_by_root_owner"}),
            );
            emit_event(
                &state.events,
                "audit_update",
                &request_id,
                Some(&operation.operation_id),
                Some(&operation.isolation_id),
                Some(&operation.audit_trace_id),
                json!({"ref_type": "audit_trace", "ref_id": operation.audit_trace_id}),
            );
            (
                StatusCode::OK,
                Json(ApiSuccess {
                    ok: true,
                    meta: Meta {
                        request_id,
                        ts: Utc::now(),
                        audit_trace_id: Some(operation.audit_trace_id.clone()),
                    },
                    data: operation,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            (status, Json(body)).into_response()
        }
    }
}

pub async fn get_gate_decision(
    State(state): State<AppState>,
    Path(operation_id): Path<String>,
) -> impl IntoResponse {
    respond_query::<GateDecisionResponse, _>(
        state.storage.get_gate_decision(&operation_id).await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn get_compiled_slice(
    State(state): State<AppState>,
    Path(operation_id): Path<String>,
) -> impl IntoResponse {
    respond_query::<CompiledSliceResponse, _>(
        state.storage.get_compiled_slice(&operation_id).await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn get_reasoning_output(
    State(state): State<AppState>,
    Path(operation_id): Path<String>,
) -> impl IntoResponse {
    respond_query::<ReasoningOutputResponse, _>(
        state.storage.get_reasoning_output(&operation_id).await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn list_pending_approvals(State(state): State<AppState>) -> impl IntoResponse {
    respond_query::<Vec<ApprovalItemSummary>, _>(
        state.storage.list_pending_approvals().await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn get_current_capabilities(State(state): State<AppState>) -> impl IntoResponse {
    let result = async {
        let current = state.storage.get_current_versions().await?;
        let snapshot = state
            .storage
            .get_capability_snapshot(&current.capability_snapshot_version)
            .await?;
        Ok::<(CurrentVersionsResponse, CapabilitySnapshotResponse), adesh_core::StorageError>((
            current, snapshot,
        ))
    }
    .await;

    match result {
        Ok((current, snapshot)) => (
            StatusCode::OK,
            Json(ApiSuccess {
                ok: true,
                meta: Meta {
                    request_id: Uuid::new_v4().to_string(),
                    ts: Utc::now(),
                    audit_trace_id: None,
                },
                data: json!({
                    "active_state_version": current.active_state_version,
                    "audience_graph_version": current.audience_graph_version,
                    "capability_snapshot_version": current.capability_snapshot_version,
                    "payload": snapshot.payload,
                }),
            }),
        )
            .into_response(),
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(Uuid::new_v4().to_string());
            (status, Json(body)).into_response()
        }
    }
}

pub async fn get_capability_snapshot(
    State(state): State<AppState>,
    Path(capability_snapshot_version): Path<String>,
) -> impl IntoResponse {
    respond_query::<CapabilitySnapshotResponse, _>(
        state
            .storage
            .get_capability_snapshot(&capability_snapshot_version)
            .await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn mint_capability_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CapabilitySnapshotMintRequest>,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_capability_snapshot_mint") {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    respond_mutation::<CapabilitySnapshotMintResponse, _>(
        state
            .storage
            .mint_capability_snapshot(CapabilitySnapshotMintInput {
                base_version: request.base_version,
                snapshot_payload: request.snapshot_payload,
                idempotency_key,
            })
            .await,
        request_id,
    )
}

#[derive(serde::Deserialize)]
pub struct ReviewQueueQuery {
    pub status: Option<String>,
}

pub async fn activate_current_capability_snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CapabilityActivationRequest>,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_capability_snapshot_activate")
    {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    let result = state
        .storage
        .create_capability_activation_review_item(CapabilityActivationReviewInput {
            capability_snapshot_version: request.capability_snapshot_version,
            idempotency_key,
        })
        .await;

    match result {
        Ok(data) => {
            emit_event(
                &state.events,
                "review_queue_update",
                &request_id,
                None,
                None,
                None,
                json!({
                    "item_id": data.item_id,
                    "status": data.status,
                    "action": "created",
                    "target_domain": data.target_domain,
                }),
            );
            emit_event(
                &state.events,
                "audit_update",
                &request_id,
                None,
                None,
                None,
                json!({
                    "ref_type": "review_item",
                    "ref_id": data.item_id,
                }),
            );
            (
                StatusCode::OK,
                Json(ApiSuccess::<ReviewItemDetail> {
                    ok: true,
                    meta: Meta {
                        request_id,
                        ts: Utc::now(),
                        audit_trace_id: None,
                    },
                    data,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            (status, Json(body)).into_response()
        }
    }
}

pub async fn register_schema_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SchemaRegisterRequest>,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_schema_register") {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    respond_mutation::<SchemaEntryResponse, _>(
        state
            .storage
            .register_schema_entry(SchemaRegisterInput {
                schema_kind: request.schema_kind,
                name: request.name,
                semver: request.semver,
                schema_payload: request.schema_payload,
                idempotency_key,
            })
            .await,
        request_id,
    )
}

pub async fn get_schema_entry(
    State(state): State<AppState>,
    Path(schema_ref): Path<String>,
) -> impl IntoResponse {
    respond_query::<SchemaEntryResponse, _>(
        state.storage.get_schema_entry(&schema_ref).await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn list_review_items(
    State(state): State<AppState>,
    Query(query): Query<ReviewQueueQuery>,
) -> impl IntoResponse {
    let result = state.storage.list_review_items().await.map(|items| {
        if let Some(status) = query.status.as_deref() {
            items
                .into_iter()
                .filter(|item| item.status == status)
                .collect::<Vec<_>>()
        } else {
            items
        }
    });

    respond_query::<Vec<ReviewItemSummary>, _>(result, Uuid::new_v4().to_string())
}

pub async fn get_review_item(
    State(state): State<AppState>,
    Path(item_id): Path<String>,
) -> impl IntoResponse {
    respond_query::<ReviewItemDetail, _>(
        state.storage.get_review_item(&item_id).await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn decide_review_item(
    State(state): State<AppState>,
    Path(item_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ReviewDecisionRequest>,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_review_decision") {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    let result = state
        .storage
        .decide_review_item(ReviewDecisionInput {
            item_id,
            decision: request.decision,
            edited_payload: request.edited_payload,
            idempotency_key,
        })
        .await;

    match result {
        Ok(data) => {
            emit_event(
                &state.events,
                "review_queue_update",
                &request_id,
                None,
                None,
                None,
                json!({
                    "item_id": data.item_id,
                    "status": data.status,
                    "action": "resolved",
                    "decision": data.decision,
                    "applied_version": data.applied_version,
                }),
            );
            emit_event(
                &state.events,
                "audit_update",
                &request_id,
                None,
                None,
                None,
                json!({
                    "ref_type": "review_item",
                    "ref_id": data.item_id.clone(),
                }),
            );
            if let Some(applied_version) = data.applied_version.clone() {
                emit_event(
                    &state.events,
                    "capability_update",
                    &request_id,
                    None,
                    None,
                    None,
                    json!({
                        "capability_snapshot_version": applied_version,
                        "changed": ["capability_snapshot"],
                    }),
                );
            }
            (
                StatusCode::OK,
                Json(ApiSuccess::<ReviewDecisionResponse> {
                    ok: true,
                    meta: Meta {
                        request_id,
                        ts: Utc::now(),
                        audit_trace_id: None,
                    },
                    data,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            (status, Json(body)).into_response()
        }
    }
}

pub async fn get_approval_item(
    State(state): State<AppState>,
    Path(approval_id): Path<String>,
) -> impl IntoResponse {
    respond_query::<ApprovalItemDetail, _>(
        state.storage.get_approval_item(&approval_id).await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn start_approval_oob(
    State(state): State<AppState>,
    Path(approval_id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_approval_oob_start") {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    let result = state
        .storage
        .start_oob_challenge(OobStartInput {
            approval_id,
            idempotency_key,
        })
        .await;

    match result {
        Ok(data) => {
            emit_event(
                &state.events,
                "oob_challenge_requested",
                &request_id,
                Some(&data.approval_id),
                None,
                None,
                json!({
                    "approval_id": data.approval_id,
                    "challenge_id": data.challenge_id,
                    "expires_at": data.expires_at,
                }),
            );
            (
                StatusCode::OK,
                Json(ApiSuccess::<OobStartResponse> {
                    ok: true,
                    meta: Meta {
                        request_id,
                        ts: Utc::now(),
                        audit_trace_id: None,
                    },
                    data,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            (status, Json(body)).into_response()
        }
    }
}

pub async fn verify_approval_oob(
    State(state): State<AppState>,
    Path(approval_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<OobVerifyRequest>,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_approval_oob_verify") {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    let result = state
        .storage
        .verify_oob_challenge(OobVerifyInput {
            approval_id,
            challenge_id: request.challenge_id,
            response_payload: request.response,
            idempotency_key,
        })
        .await;

    match result {
        Ok(data) => {
            emit_event(
                &state.events,
                "oob_challenge_verified",
                &request_id,
                Some(&data.approval_id),
                None,
                None,
                json!({
                    "approval_id": data.approval_id,
                    "challenge_id": data.challenge_id,
                }),
            );
            (
                StatusCode::OK,
                Json(ApiSuccess::<OobVerifyResponse> {
                    ok: true,
                    meta: Meta {
                        request_id,
                        ts: Utc::now(),
                        audit_trace_id: None,
                    },
                    data,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            (status, Json(body)).into_response()
        }
    }
}

pub async fn decide_approval(
    State(state): State<AppState>,
    Path(approval_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ApprovalDecisionRequest>,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_approval_decision") {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    let outcome = state
        .storage
        .consume_approval_atomic(ApprovalConsumeInput {
            approval_id,
            decision: request.decision,
            modified_payload: request.modified_payload,
            oob_challenge_id: request.oob.and_then(|value| value.challenge_id),
            idempotency_key,
        })
        .await;

    match outcome {
        Ok(data) => {
            if let Err(error) = execute_permitted_syscalls(
                &state,
                &request_id,
                &data.operation_id,
                &data.audit_trace_id,
                &data.syscall_ids,
            )
            .await
            {
                let status = status_for_storage_error(&error);
                let body = AppError::Storage(error).into_response_body(request_id);
                return (status, Json(body)).into_response();
            }

            emit_event(
                &state.events,
                "audit_update",
                &request_id,
                Some(&data.operation_id),
                None,
                Some(&data.audit_trace_id),
                json!({"ref_type": "approval_decision", "approval_id": data.approval_id.clone(), "decision": data.decision.clone()}),
            );
            emit_event(
                &state.events,
                "operation_state",
                &request_id,
                Some(&data.operation_id),
                None,
                Some(&data.audit_trace_id),
                json!({"state": data.operation_state.clone(), "approval_id": data.approval_id.clone()}),
            );
            emit_event(
                &state.events,
                "audit_update",
                &request_id,
                Some(&data.operation_id),
                None,
                Some(&data.audit_trace_id),
                json!({"ref_type": "syscalls", "syscall_ids": data.syscall_ids.clone()}),
            );
            emit_event(
                &state.events,
                if data.decision == "approve" {
                    "approval_granted"
                } else {
                    "approval_denied"
                },
                &request_id,
                Some(&data.operation_id),
                None,
                Some(&data.audit_trace_id),
                json!({
                    "approval_id": data.approval_id.clone(),
                    "decision": data.decision.clone(),
                    "next_state": data.operation_state.clone(),
                }),
            );

            (
                StatusCode::OK,
                Json(ApiSuccess {
                    ok: true,
                    meta: Meta {
                        request_id,
                        ts: Utc::now(),
                        audit_trace_id: Some(data.audit_trace_id.clone()),
                    },
                    data,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            (status, Json(body)).into_response()
        }
    }
}

pub async fn list_operation_syscalls(
    State(state): State<AppState>,
    Path(operation_id): Path<String>,
) -> impl IntoResponse {
    respond_query::<Vec<SyscallResponse>, _>(
        state
            .storage
            .list_syscalls_by_operation(&operation_id)
            .await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn get_audit_trace(
    State(state): State<AppState>,
    Path(audit_trace_id): Path<String>,
) -> impl IntoResponse {
    let result = state
        .storage
        .get_audit_trace(&audit_trace_id)
        .await
        .map(|record| AuditTraceResponse {
            audit_trace_id: record.audit_trace_id,
            request_id: record.request_id,
            operation_id: record.operation_id,
            isolation_id: record.isolation_id,
            pinned: record.pinned,
            summary: record.summary,
            timeline: record.timeline,
            attachments: record.attachments,
        });

    respond_query::<AuditTraceResponse, _>(result, Uuid::new_v4().to_string())
}

pub async fn replay_audit_trace(
    State(state): State<AppState>,
    Path(audit_trace_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ReplayRequest>,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_replay") {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    let result = state
        .storage
        .create_replay_dry_run(ReplayCreateInput {
            source_audit_trace_id: audit_trace_id.clone(),
            mode: request.mode,
            strategy: request.strategy,
            idempotency_key,
        })
        .await;

    match result {
        Ok(data) => {
            emit_event(
                &state.events,
                "audit_update",
                &request_id,
                Some(&data.operation_id),
                None,
                Some(&data.audit_trace_id),
                json!({
                    "ref_type": "replay_dry_run",
                    "replay_id": data.replay_id,
                    "source_audit_trace_id": audit_trace_id,
                }),
            );

            (
                StatusCode::OK,
                Json(ApiSuccess::<ReplayResponse> {
                    ok: true,
                    meta: Meta {
                        request_id,
                        ts: Utc::now(),
                        audit_trace_id: Some(data.audit_trace_id.clone()),
                    },
                    data,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            (status, Json(body)).into_response()
        }
    }
}

pub async fn register_workflow_spec(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<WorkflowSpecRegisterRequest>,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_workflow_specs") {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    respond_mutation::<WorkflowSpecResponse, _>(
        state
            .storage
            .register_workflow_spec(WorkflowSpecRegisterInput {
                name: request.name,
                description: request.description,
                tags: request.tags,
                spec_payload: request.spec,
                idempotency_key,
            })
            .await,
        request_id,
    )
}

pub async fn get_workflow_spec(
    State(state): State<AppState>,
    Path(workflow_ref): Path<String>,
) -> impl IntoResponse {
    respond_query::<WorkflowSpecResponse, _>(
        state.storage.get_workflow_spec(&workflow_ref).await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn list_workflow_specs(
    State(state): State<AppState>,
    Query(query): Query<SpecListQuery>,
) -> impl IntoResponse {
    respond_query::<Vec<WorkflowSpecSummary>, _>(
        state
            .storage
            .find_workflow_specs(WorkflowSpecQuery {
                name: query.name,
                tag: query.tag,
                author: query.author,
                limit: query.limit,
            })
            .await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn create_workflow_instance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<WorkflowInstanceCreateRequest>,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_workflow_instances") {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    match state
        .storage
        .create_workflow_instance(WorkflowInstanceCreateInput {
            workflow_ref: request.workflow_ref,
            parent_request_id: request.request_context.parent_request_id,
            parent_operation_id: request.request_context.operation_id,
            inputs: request.inputs,
            idempotency_key,
        })
        .await
    {
        Ok(data) => {
            emit_extended_event(
                &state.events,
                "workflow_instance_state",
                &request_id,
                data.parent_operation_id.as_deref(),
                Some(&data.workflow_instance_id),
                None,
                None,
                None,
                None,
                json!({"state": data.state, "reason": data.state_reason}),
            );
            for step in &data.step_states {
                emit_extended_event(
                    &state.events,
                    "workflow_step_state",
                    &request_id,
                    step.operation_id.as_deref(),
                    Some(&data.workflow_instance_id),
                    Some(&step.step_id),
                    None,
                    None,
                    None,
                    json!({
                        "step_id": step.step_id,
                        "step_type": step.step_type,
                        "state": step.state,
                        "attempt": step.attempt,
                    }),
                );
            }
            (
                StatusCode::CREATED,
                Json(ApiSuccess {
                    ok: true,
                    meta: Meta {
                        request_id,
                        ts: Utc::now(),
                        audit_trace_id: None,
                    },
                    data,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            (status, Json(body)).into_response()
        }
    }
}

pub async fn get_workflow_instance(
    State(state): State<AppState>,
    Path(workflow_instance_id): Path<String>,
) -> impl IntoResponse {
    respond_query::<WorkflowInstanceResponse, _>(
        state
            .storage
            .get_workflow_instance(&workflow_instance_id)
            .await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn cancel_workflow_instance(
    State(state): State<AppState>,
    Path(workflow_instance_id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_workflow_instance_cancel") {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    let current = match state
        .storage
        .get_workflow_instance(&workflow_instance_id)
        .await
    {
        Ok(current) => current,
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            return (status, Json(body)).into_response();
        }
    };
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    match state
        .storage
        .update_workflow_instance_state(WorkflowInstanceStateUpdateInput {
            workflow_instance_id,
            expected_state: Some(current.state),
            new_state: "cancelled".to_string(),
            reason: Some("cancelled_by_root_owner".to_string()),
            idempotency_key,
        })
        .await
    {
        Ok(data) => {
            emit_extended_event(
                &state.events,
                "workflow_instance_state",
                &request_id,
                data.parent_operation_id.as_deref(),
                Some(&data.workflow_instance_id),
                None,
                None,
                None,
                None,
                json!({"state": data.state, "reason": data.state_reason}),
            );
            (
                StatusCode::OK,
                Json(ApiSuccess {
                    ok: true,
                    meta: Meta {
                        request_id,
                        ts: Utc::now(),
                        audit_trace_id: None,
                    },
                    data,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            (status, Json(body)).into_response()
        }
    }
}

pub async fn register_interface_spec(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<InterfaceSpecRegisterRequest>,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_interface_specs") {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    respond_mutation::<InterfaceSpecResponse, _>(
        state
            .storage
            .register_interface_spec(InterfaceSpecRegisterInput {
                name: request.name,
                description: request.description,
                tags: request.tags,
                spec_payload: request.spec,
                idempotency_key,
            })
            .await,
        request_id,
    )
}

pub async fn get_interface_spec(
    State(state): State<AppState>,
    Path(interface_ref): Path<String>,
) -> impl IntoResponse {
    respond_query::<InterfaceSpecResponse, _>(
        state.storage.get_interface_spec(&interface_ref).await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn list_interface_specs(
    State(state): State<AppState>,
    Query(query): Query<SpecListQuery>,
) -> impl IntoResponse {
    respond_query::<Vec<InterfaceSpecSummary>, _>(
        state
            .storage
            .find_interface_specs(InterfaceSpecQuery {
                name: query.name,
                tag: query.tag,
                author: query.author,
                limit: query.limit,
            })
            .await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn create_interface_instance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<InterfaceInstanceCreateRequest>,
) -> impl IntoResponse {
    if let Err(response) = enforce_rate_limit(&state, &headers, "post_interface_instances") {
        return response;
    }

    let request_id = Uuid::new_v4().to_string();
    let idempotency_key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);

    if request.viewer.audience_id != "root_owner" {
        let body = AppError::BadRequest("viewer.audience_id must be `root_owner`".to_string())
            .into_response_body(request_id);
        return (StatusCode::BAD_REQUEST, Json(body)).into_response();
    }

    let interface_spec = match state
        .storage
        .get_interface_spec(&request.interface_ref)
        .await
    {
        Ok(spec) => spec,
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            return (status, Json(body)).into_response();
        }
    };

    let blocks = interface_spec
        .payload
        .get("blocks")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let bindings = interface_spec
        .payload
        .get("bindings")
        .cloned()
        .unwrap_or_else(|| json!([]));

    let (
        operation_id,
        workflow_instance_id,
        pinned_active_state_version,
        pinned_capability_snapshot_version,
        pinned_audience_graph_version,
        gate_summary,
        taint_summary,
    ) = if let Some(operation_id) = request.operation_id.clone() {
        let operation = match state.storage.get_operation(&operation_id).await {
            Ok(operation) => operation,
            Err(error) => {
                let status = status_for_storage_error(&error);
                let body = AppError::Storage(error).into_response_body(request_id);
                return (status, Json(body)).into_response();
            }
        };
        let gate = match state.storage.get_gate_decision(&operation_id).await {
            Ok(gate) => gate,
            Err(error) => {
                let status = status_for_storage_error(&error);
                let body = AppError::Storage(error).into_response_body(request_id);
                return (status, Json(body)).into_response();
            }
        };
        let compiled = match state.storage.get_compiled_slice(&operation_id).await {
            Ok(compiled) => compiled,
            Err(error) => {
                let status = status_for_storage_error(&error);
                let body = AppError::Storage(error).into_response_body(request_id);
                return (status, Json(body)).into_response();
            }
        };
        (
            Some(operation_id),
            None,
            operation.pinned_active_state_version,
            operation.pinned_capability_snapshot_version,
            operation.pinned_audience_graph_version,
            json!({
                "risk_r": gate.risk_r,
                "sensitivity_s": gate.sensitivity_s,
                "max_gate": gate.max_gate,
                "approval_mode": gate.approval_mode,
            }),
            json!({
                "operation_max_taint_s": compiled.operation_max_taint_s,
            }),
        )
    } else {
        let workflow_instance_id = match request.workflow_instance_id.clone() {
            Some(value) => value,
            None => {
                let body = AppError::BadRequest(
                    "exactly one of operation_id or workflow_instance_id must be set".to_string(),
                )
                .into_response_body(request_id);
                return (StatusCode::BAD_REQUEST, Json(body)).into_response();
            }
        };
        let workflow = match state
            .storage
            .get_workflow_instance(&workflow_instance_id)
            .await
        {
            Ok(workflow) => workflow,
            Err(error) => {
                let status = status_for_storage_error(&error);
                let body = AppError::Storage(error).into_response_body(request_id);
                return (status, Json(body)).into_response();
            }
        };
        let parent_operation_id = match workflow.parent_operation_id.clone() {
            Some(value) => value,
            None => {
                let body = AppError::BadRequest(
                    "workflow-backed interface instances require parent_operation_id".to_string(),
                )
                .into_response_body(request_id);
                return (StatusCode::BAD_REQUEST, Json(body)).into_response();
            }
        };
        let gate = match state.storage.get_gate_decision(&parent_operation_id).await {
            Ok(gate) => gate,
            Err(error) => {
                let status = status_for_storage_error(&error);
                let body = AppError::Storage(error).into_response_body(request_id);
                return (status, Json(body)).into_response();
            }
        };
        let compiled = match state.storage.get_compiled_slice(&parent_operation_id).await {
            Ok(compiled) => compiled,
            Err(error) => {
                let status = status_for_storage_error(&error);
                let body = AppError::Storage(error).into_response_body(request_id);
                return (status, Json(body)).into_response();
            }
        };
        (
            None,
            Some(workflow_instance_id),
            workflow.pinned_active_state_version,
            workflow.pinned_capability_snapshot_version,
            workflow.pinned_audience_graph_version,
            json!({
                "risk_r": gate.risk_r,
                "sensitivity_s": gate.sensitivity_s,
                "max_gate": gate.max_gate,
                "approval_mode": gate.approval_mode,
            }),
            json!({
                "operation_max_taint_s": compiled.operation_max_taint_s,
            }),
        )
    };

    match state
        .storage
        .create_interface_instance(InterfaceInstanceCreateInput {
            interface_ref: request.interface_ref,
            operation_id,
            workflow_instance_id,
            viewer_audience_id: request.viewer.audience_id,
            pinned_active_state_version,
            pinned_capability_snapshot_version,
            pinned_audience_graph_version,
            gate_summary,
            blocks,
            bindings,
            taint_summary,
            idempotency_key,
        })
        .await
    {
        Ok(data) => {
            emit_extended_event(
                &state.events,
                "interface_instance_ready",
                &request_id,
                data.operation_id.as_deref(),
                data.workflow_instance_id.as_deref(),
                None,
                Some(&data.interface_instance_id),
                None,
                None,
                json!({
                    "interface_instance_id": data.interface_instance_id,
                    "interface_ref": data.interface_ref,
                    "operation_id": data.operation_id,
                    "workflow_instance_id": data.workflow_instance_id,
                    "state": data.state,
                }),
            );
            (
                StatusCode::CREATED,
                Json(ApiSuccess {
                    ok: true,
                    meta: Meta {
                        request_id,
                        ts: Utc::now(),
                        audit_trace_id: None,
                    },
                    data,
                }),
            )
                .into_response()
        }
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            (status, Json(body)).into_response()
        }
    }
}

pub async fn get_interface_instance(
    State(state): State<AppState>,
    Path(interface_instance_id): Path<String>,
) -> impl IntoResponse {
    respond_query::<InterfaceInstanceResponse, _>(
        state
            .storage
            .get_interface_instance(&interface_instance_id)
            .await,
        Uuid::new_v4().to_string(),
    )
}

pub async fn get_wedge_metrics(State(state): State<AppState>) -> impl IntoResponse {
    respond_query::<WedgeMetricsResponse, _>(
        state.storage.get_wedge_metrics().await,
        Uuid::new_v4().to_string(),
    )
}

fn respond_query<T, E>(result: Result<T, E>, request_id: String) -> axum::response::Response
where
    T: serde::Serialize,
    E: Into<adesh_core::StorageError>,
{
    match result.map_err(Into::into) {
        Ok(data) => (
            StatusCode::OK,
            Json(ApiSuccess {
                ok: true,
                meta: Meta {
                    request_id,
                    ts: Utc::now(),
                    audit_trace_id: None,
                },
                data,
            }),
        )
            .into_response(),
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            (status, Json(body)).into_response()
        }
    }
}

fn respond_mutation<T, E>(result: Result<T, E>, request_id: String) -> axum::response::Response
where
    T: serde::Serialize,
    E: Into<adesh_core::StorageError>,
{
    match result.map_err(Into::into) {
        Ok(data) => (
            StatusCode::OK,
            Json(ApiSuccess {
                ok: true,
                meta: Meta {
                    request_id,
                    ts: Utc::now(),
                    audit_trace_id: None,
                },
                data,
            }),
        )
            .into_response(),
        Err(error) => {
            let status = status_for_storage_error(&error);
            let body = AppError::Storage(error).into_response_body(request_id);
            (status, Json(body)).into_response()
        }
    }
}

fn status_for_storage_error(error: &adesh_core::StorageError) -> StatusCode {
    match error {
        adesh_core::StorageError::InvalidInput(_) => StatusCode::BAD_REQUEST,
        adesh_core::StorageError::NotFound(_) => StatusCode::NOT_FOUND,
        adesh_core::StorageError::Conflict(_) => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
