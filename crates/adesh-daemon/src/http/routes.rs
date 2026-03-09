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
    CapabilityActivationRequest, CapabilitySnapshotMintRequest, CapabilitySnapshotMintResponse,
    CapabilitySnapshotResponse, CompiledSliceResponse, CurrentVersionsResponse,
    GateDecisionResponse, HealthResponse, Meta, ReasoningOutputResponse, ReplayRequest,
    ReplayResponse, RequestEnvelope, ReviewDecisionRequest, ReviewDecisionResponse,
    ReviewItemDetail, ReviewItemSummary, SchemaEntryResponse, SchemaRegisterRequest,
    SyscallResponse,
};
use adesh_core::{
    AppError,
    action_schemas::{ValidationErrorKind, validate_instance_against_schema},
    ports::storage::{
        ApprovalConsumeInput, CapabilityActivationReviewInput, CapabilitySnapshotMintInput,
        EventAppendInput, ReasoningOutputInput, ReplayCreateInput, ReviewDecisionInput,
        SchemaRegisterInput, StorageProvider, SyscallStatusUpdateInput,
    },
};

use super::AppState;
use crate::kernel::{KernelOutcome, approval_item_input, compile_and_verify_stub};

fn emit_event(
    sender: &tokio::sync::broadcast::Sender<String>,
    event_type: &str,
    request_id: &str,
    operation_id: Option<&str>,
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
        "isolation_id": isolation_id,
        "audit_trace_id": audit_trace_id,
        "data": data,
    });
    if let Ok(payload) = serde_json::to_string(&envelope) {
        let _ = sender.send(payload);
    }
}

async fn execute_permitted_syscalls(
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

        let result = state
            .tools
            .execute_syscall(
                &syscall.syscall_id,
                &syscall.tool_name,
                &syscall.action_name,
                &syscall.args_schema_ref,
                syscall.result_schema_ref.as_deref(),
                &syscall.args,
            )
            .await?;
        if let Some(result_schema_ref) = syscall.result_schema_ref.as_deref() {
            let result_schema = state.storage.get_schema_entry(result_schema_ref).await?;
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
    }

    Ok(())
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
    let queue = "degraded";
    let status = if storage == "ok" && model_provider == "ok" && tool_provider == "ok" {
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

pub async fn submit_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RequestEnvelope>,
) -> impl IntoResponse {
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
                match state
                    .storage
                    .resolve_action_descriptor(
                        &operation.pinned_capability_snapshot_version,
                        "email",
                        "send",
                    )
                    .await
                {
                    Ok(descriptor) => Some(descriptor),
                    Err(error) => {
                        let body = AppError::Storage(error).into_response_body(request.request_id);
                        return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
                    }
                }
            } else {
                None
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
                })
                .await
            {
                Ok(output) => output,
                Err(error) => {
                    let body = AppError::Storage(error).into_response_body(request.request_id);
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
                }
            };

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

pub async fn decide_approval(
    State(state): State<AppState>,
    Path(approval_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ApprovalDecisionRequest>,
) -> impl IntoResponse {
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

pub async fn replay_audit_trace(
    State(state): State<AppState>,
    Path(audit_trace_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ReplayRequest>,
) -> impl IntoResponse {
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
