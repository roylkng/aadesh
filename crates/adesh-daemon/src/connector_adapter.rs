use adesh_contracts::{
    ConnectorEventKind, ConnectorEventRequest, ConnectorEventResponse, PrepareTaskContextRequest,
    StoreWorkEpisodeRequest,
};
use adesh_core::ports::storage::{
    InterventionContextInput, InterventionOutcomeInput, StorageProvider,
};
use anyhow::bail;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::cognition;

fn normalize_trace_component(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        trimmed
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' || ch == '/' {
                    ch
                } else {
                    '_'
                }
            })
            .collect(),
    )
}

fn push_unique_artifact_ref(target: &mut Vec<String>, value: String) {
    if !target.iter().any(|existing| existing == &value) {
        target.push(value);
    }
}

fn append_supervisory_trace_refs(artifact_refs: &mut Vec<String>, request: &ConnectorEventRequest) {
    if let Some(value) = request
        .host_agent_id
        .as_deref()
        .and_then(normalize_trace_component)
    {
        push_unique_artifact_ref(artifact_refs, format!("trace://host-agent-id/{value}"));
    }
    if let Some(value) = request
        .host_agent_kind
        .as_deref()
        .and_then(normalize_trace_component)
    {
        push_unique_artifact_ref(artifact_refs, format!("trace://host-agent-kind/{value}"));
    }
    if let Some(value) = request
        .host_model
        .as_deref()
        .and_then(normalize_trace_component)
    {
        push_unique_artifact_ref(artifact_refs, format!("trace://host-model/{value}"));
    }
    if let Some(value) = request
        .context_id
        .as_deref()
        .and_then(normalize_trace_component)
    {
        push_unique_artifact_ref(artifact_refs, format!("trace://context-id/{value}"));
    }
    if let Some(value) = request
        .selected_next_direction
        .as_deref()
        .and_then(normalize_trace_component)
    {
        push_unique_artifact_ref(
            artifact_refs,
            format!("trace://selected-next-direction/{value}"),
        );
    }
    if let Some(value) = request
        .outcome
        .as_deref()
        .and_then(normalize_trace_component)
    {
        push_unique_artifact_ref(artifact_refs, format!("trace://outcome/{value}"));
    }
    if let Some(value) = request
        .correction_summary
        .as_deref()
        .and_then(normalize_trace_component)
    {
        push_unique_artifact_ref(artifact_refs, format!("trace://correction/{value}"));
    }
}

pub async fn handle_connector_event<S: StorageProvider + ?Sized>(
    storage: &S,
    request: ConnectorEventRequest,
) -> anyhow::Result<ConnectorEventResponse> {
    if request.connector_id.trim().is_empty() {
        bail!("connector_id may not be empty");
    }
    if request.connector_kind.trim().is_empty() {
        bail!("connector_kind may not be empty");
    }

    let mut warnings = Vec::new();
    let event_kind = request.event_kind.clone();

    match event_kind {
        ConnectorEventKind::TaskStart => {
            let prepare_request = PrepareTaskContextRequest {
                workspace: request.workspace.clone(),
                task_prompt: request.task_prompt.clone(),
                files_in_focus: request.files_in_focus.clone(),
                task_hint: request.task_hint.clone(),
            };
            let prepare_context = cognition::prepare_task_context(storage, prepare_request).await?;

            let scope_type = "workspace".to_string();
            let scope_key = prepare_context
                .workspace_resolution
                .resolved_scope_key
                .clone();
            let context_id =
                if let Some(first_direction) = prepare_context.likely_next_directions.first() {
                    let surfaced_directions_json =
                        serde_json::to_string(&prepare_context.likely_next_directions).ok();
                    let ctx_input = InterventionContextInput {
                        context_id: String::new(),
                        scope_type: scope_type.clone(),
                        scope_key: scope_key.clone(),
                        task_prompt: request.task_prompt.clone(),
                        prepared_at: request.started_at.unwrap_or_else(Utc::now),
                        host_agent_id: request.host_agent_id.clone(),
                        host_agent_kind: request.host_agent_kind.clone(),
                        host_model: request.host_model.clone(),
                        selected_direction: Some(first_direction.statement.clone()),
                        selected_direction_rank: Some(0),
                        surfaced_directions_json,
                    };
                    match storage.store_intervention_context(ctx_input).await {
                        Ok(context) => Some(context.context_id),
                        Err(err) => {
                            warnings.push(format!("failed to store intervention context: {err}"));
                            None
                        }
                    }
                } else {
                    None
                };

            Ok(ConnectorEventResponse {
                connector_id: request.connector_id,
                connector_kind: request.connector_kind,
                connector_version: request.connector_version,
                session_id: request.session_id,
                event_kind: ConnectorEventKind::TaskStart,
                handled_as: "prepare_task_context".to_string(),
                context_id,
                prepare_context: Some(prepare_context),
                stored_episode: None,
                warnings,
            })
        }
        kind @ (ConnectorEventKind::TaskCheckpoint | ConnectorEventKind::TaskEnd) => {
            let summary = match request.summary.as_ref().map(|s| s.trim()) {
                Some(text) if !text.is_empty() => text.to_string(),
                _ => {
                    warnings.push(
                        "No summary provided by connector; stored fallback summary".to_string(),
                    );
                    format!(
                        "Connector {} emitted {:?} without explicit summary for task: {}",
                        request.connector_id,
                        kind,
                        request.task_prompt.trim()
                    )
                }
            };

            let files_touched = if request.files_touched.is_empty()
                && !request.files_in_focus.is_empty()
            {
                warnings.push("files_touched was empty; fell back to files_in_focus".to_string());
                request.files_in_focus.clone()
            } else {
                request.files_touched.clone()
            };
            let mut artifact_refs = request.artifact_refs.clone();
            append_supervisory_trace_refs(&mut artifact_refs, &request);

            let store_request = StoreWorkEpisodeRequest {
                workspace: request.workspace.clone(),
                task_prompt: request.task_prompt.clone(),
                summary,
                files_touched,
                tests: request.tests.clone(),
                decisions: request.decisions.clone(),
                unresolved_items: request.unresolved_items.clone(),
                observed_preferences: request.observed_preferences.clone(),
                risk_signals: request.risk_signals.clone(),
                issue_refs: request.issue_refs.clone(),
                artifact_refs,
                task_hint: request.task_hint.clone(),
                started_at: request.started_at,
                ended_at: request.ended_at,
            };
            let stored_episode = cognition::store_work_episode(storage, store_request).await?;

            if let Some(selected_direction) = request.selected_next_direction.clone() {
                let raw_response = request
                    .outcome
                    .clone()
                    .unwrap_or_else(|| "ignored".to_string());
                let selected_response = match raw_response.as_str() {
                    "accepted" | "ignored" | "modified" => raw_response,
                    _ => "ignored".to_string(),
                };

                let valid_context_id = if let Some(ctx_id) = &request.context_id {
                    if storage.get_intervention_context(ctx_id).await.is_ok() {
                        Some(ctx_id.clone())
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Learnability is about valid linkage/provenance, not only positive outcomes.
                // Accepted/ignored/modified outcomes should all be eligible when linked.
                let learn_from = valid_context_id.is_some();

                let idempotency_key =
                    if request.context_id.is_some() && request.selected_next_direction.is_some() {
                        let event_type_str = match kind {
                            ConnectorEventKind::TaskStart => "TaskStart",
                            ConnectorEventKind::TaskCheckpoint => "TaskCheckpoint",
                            ConnectorEventKind::TaskEnd => "TaskEnd",
                        };
                        let mut hasher = Sha256::new();
                        hasher.update(format!(
                            "{}/{}/{}/{}",
                            request.context_id.as_ref().unwrap(),
                            request.selected_next_direction.as_ref().unwrap(),
                            selected_response,
                            event_type_str,
                        ));
                        Some(BASE64_STANDARD.encode(hasher.finalize()))
                    } else {
                        None
                    };

                let outcome_input = InterventionOutcomeInput {
                    intervention_id: String::new(),
                    episode_id: Some(stored_episode.episode_id.clone()),
                    surfaced_direction: selected_direction,
                    context_ref: valid_context_id,
                    surfaced_at: request.ended_at.unwrap_or_else(Utc::now),
                    selected_response,
                    modified_payload: None,
                    outcome_ref: None,
                    correction_summary: request.correction_summary.clone(),
                    learn_from_this: learn_from,
                    idempotency_key,
                };
                if let Err(err) = storage.store_intervention_outcome(outcome_input).await {
                    warnings.push(format!("failed to store intervention outcome: {err}"));
                }
            }

            Ok(ConnectorEventResponse {
                connector_id: request.connector_id,
                connector_kind: request.connector_kind,
                connector_version: request.connector_version,
                session_id: request.session_id,
                event_kind: kind,
                handled_as: "store_work_episode".to_string(),
                context_id: None,
                prepare_context: None,
                stored_episode: Some(stored_episode),
                warnings,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use adesh_contracts::{
        ConnectorEventKind, ConnectorEventRequest, PrepareTaskContextRequest,
        StoreWorkEpisodeRequest, WorkEpisodeDecision, WorkspaceDescriptor,
    };
    use adesh_core::ports::storage::StorageProvider;
    use adesh_storage_sqlite::SqliteStorage;
    use chrono::{Duration, TimeZone, Utc};

    use super::handle_connector_event;
    use crate::cognition;

    async fn new_storage() -> SqliteStorage {
        let storage = SqliteStorage::connect("sqlite::memory:").await.unwrap();
        StorageProvider::migrate(&storage).await.unwrap();
        storage
    }

    fn workspace(locator: &str) -> WorkspaceDescriptor {
        WorkspaceDescriptor {
            kind: "task_space".to_string(),
            locator: Some(locator.to_string()),
            cwd: None,
            branch: None,
            external_ref: None,
        }
    }

    #[tokio::test]
    async fn connector_task_start_maps_to_prepare() {
        let storage = new_storage().await;
        let response = handle_connector_event(
            &storage,
            ConnectorEventRequest {
                connector_id: "codex-vscode".to_string(),
                connector_kind: "chat_extension".to_string(),
                connector_version: Some("0.1.0".to_string()),
                session_id: Some("sess-1".to_string()),
                host_agent_id: None,
                host_agent_kind: None,
                host_model: None,
                context_id: None,
                selected_next_direction: None,
                outcome: None,
                correction_summary: None,
                event_kind: ConnectorEventKind::TaskStart,
                workspace: workspace("workspace://connector-smoke"),
                task_prompt: "What should I do next?".to_string(),
                files_in_focus: vec!["src/main.rs".to_string()],
                task_hint: Some("smoke".to_string()),
                summary: None,
                files_touched: Vec::new(),
                tests: Vec::new(),
                decisions: Vec::new(),
                unresolved_items: Vec::new(),
                observed_preferences: Vec::new(),
                risk_signals: Vec::new(),
                issue_refs: Vec::new(),
                artifact_refs: Vec::new(),
                started_at: None,
                ended_at: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(response.handled_as, "prepare_task_context");
        assert!(response.prepare_context.is_some());
        assert!(response.stored_episode.is_none());
    }

    #[tokio::test]
    async fn connector_task_start_stores_resolved_scope_and_direction_snapshot() {
        let storage = new_storage().await;
        let workspace = workspace("workspace://connector-scope");
        let prepared_at = Utc.with_ymd_and_hms(2026, 5, 1, 9, 30, 0).unwrap();
        cognition::store_work_episode(
            &storage,
            StoreWorkEpisodeRequest {
                workspace: workspace.clone(),
                task_prompt: "Retry hardening".to_string(),
                summary: "Identified a timeout coverage gap".to_string(),
                files_touched: vec!["src/retry.rs".to_string()],
                tests: Vec::new(),
                decisions: Vec::new(),
                unresolved_items: vec![
                    "Run timeout coverage before changing retry layer".to_string(),
                ],
                observed_preferences: Vec::new(),
                risk_signals: Vec::new(),
                issue_refs: Vec::new(),
                artifact_refs: Vec::new(),
                task_hint: Some("retry_hardening".to_string()),
                started_at: Some(prepared_at - Duration::hours(1)),
                ended_at: Some(prepared_at - Duration::minutes(30)),
            },
        )
        .await
        .unwrap();

        let response = handle_connector_event(
            &storage,
            ConnectorEventRequest {
                connector_id: "codex-vscode".to_string(),
                connector_kind: "chat_extension".to_string(),
                connector_version: Some("0.1.0".to_string()),
                session_id: Some("sess-scope".to_string()),
                host_agent_id: Some("agent-scope".to_string()),
                host_agent_kind: Some("codex-extension".to_string()),
                host_model: Some("gpt-test".to_string()),
                context_id: None,
                selected_next_direction: None,
                outcome: None,
                correction_summary: None,
                event_kind: ConnectorEventKind::TaskStart,
                workspace: workspace.clone(),
                task_prompt: "What should I focus on next for retry hardening?".to_string(),
                files_in_focus: vec!["src/retry.rs".to_string()],
                task_hint: Some("retry_hardening".to_string()),
                summary: None,
                files_touched: Vec::new(),
                tests: Vec::new(),
                decisions: Vec::new(),
                unresolved_items: Vec::new(),
                observed_preferences: Vec::new(),
                risk_signals: Vec::new(),
                issue_refs: Vec::new(),
                artifact_refs: Vec::new(),
                started_at: Some(prepared_at),
                ended_at: None,
            },
        )
        .await
        .unwrap();

        let context_id = response.context_id.expect("expected stored context");
        let context = storage.get_intervention_context(&context_id).await.unwrap();
        let expected_scope = cognition::resolve_workspace(&workspace).resolved_scope_key;
        assert_eq!(context.scope_key, expected_scope);
        assert_eq!(context.prepared_at, prepared_at);
        assert_eq!(context.selected_direction_rank, Some(0));
        assert!(
            context
                .surfaced_directions_json
                .as_deref()
                .unwrap_or_default()
                .contains("timeout coverage")
        );
    }

    #[tokio::test]
    async fn connector_task_end_maps_to_store_with_fallback_summary() {
        let storage = new_storage().await;
        let response = handle_connector_event(
            &storage,
            ConnectorEventRequest {
                connector_id: "codex-vscode".to_string(),
                connector_kind: "chat_extension".to_string(),
                connector_version: Some("0.1.0".to_string()),
                session_id: Some("sess-2".to_string()),
                host_agent_id: None,
                host_agent_kind: None,
                host_model: None,
                context_id: None,
                selected_next_direction: None,
                outcome: None,
                correction_summary: None,
                event_kind: ConnectorEventKind::TaskEnd,
                workspace: workspace("workspace://connector-smoke"),
                task_prompt: "Finalize retry hardening".to_string(),
                files_in_focus: vec!["src/retry.rs".to_string()],
                task_hint: Some("retry".to_string()),
                summary: None,
                files_touched: Vec::new(),
                tests: Vec::new(),
                decisions: Vec::new(),
                unresolved_items: vec!["Need timeout benchmark".to_string()],
                observed_preferences: Vec::new(),
                risk_signals: Vec::new(),
                issue_refs: Vec::new(),
                artifact_refs: Vec::new(),
                started_at: None,
                ended_at: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(response.handled_as, "store_work_episode");
        assert!(response.prepare_context.is_none());
        assert!(response.stored_episode.is_some());
        assert!(!response.warnings.is_empty());
    }

    #[tokio::test]
    async fn connector_task_end_persists_supervisory_trace_refs() {
        let storage = new_storage().await;
        let response = handle_connector_event(
            &storage,
            ConnectorEventRequest {
                connector_id: "codex-vscode".to_string(),
                connector_kind: "chat_extension".to_string(),
                connector_version: Some("0.1.0".to_string()),
                session_id: Some("sess-3".to_string()),
                host_agent_id: Some("agent-42".to_string()),
                host_agent_kind: Some("codex-extension".to_string()),
                host_model: Some("gpt-5.4".to_string()),
                context_id: Some("ctx-123".to_string()),
                selected_next_direction: Some("run validation harness".to_string()),
                outcome: Some("accepted".to_string()),
                correction_summary: Some("tightened assertions".to_string()),
                event_kind: ConnectorEventKind::TaskEnd,
                workspace: workspace("workspace://connector-smoke"),
                task_prompt: "Validate wedge behavior".to_string(),
                files_in_focus: vec!["src/ranking.rs".to_string()],
                task_hint: Some("validation".to_string()),
                summary: Some("Validated ranking flow".to_string()),
                files_touched: vec!["src/ranking.rs".to_string()],
                tests: Vec::new(),
                decisions: Vec::new(),
                unresolved_items: Vec::new(),
                observed_preferences: Vec::new(),
                risk_signals: Vec::new(),
                issue_refs: Vec::new(),
                artifact_refs: Vec::new(),
                started_at: None,
                ended_at: None,
            },
        )
        .await
        .unwrap();

        let episode = response.stored_episode.expect("expected stored episode");
        assert!(
            episode
                .artifact_refs
                .iter()
                .any(|value| value.starts_with("trace://host-agent-id/agent-42"))
        );
        assert!(episode.artifact_refs.iter().any(|value| {
            value.starts_with("trace://selected-next-direction/run_validation_harness")
        }));
        assert!(
            episode
                .artifact_refs
                .iter()
                .any(|value| value.starts_with("trace://outcome/accepted"))
        );
    }

    #[tokio::test]
    async fn unlinked_invalid_context_stays_non_learnable() {
        let storage = new_storage().await;
        // TaskEnd with an accepted outcome but invalid context_id
        let response = handle_connector_event(
            &storage,
            ConnectorEventRequest {
                connector_id: "test".to_string(),
                connector_kind: "test".to_string(),
                connector_version: None,
                session_id: None,
                host_agent_id: None,
                host_agent_kind: None,
                host_model: None,
                context_id: Some("invalid-context-id".to_string()),
                selected_next_direction: Some("do thing".to_string()),
                outcome: Some("accepted".to_string()),
                correction_summary: None,
                event_kind: ConnectorEventKind::TaskEnd,
                workspace: workspace("workspace://test"),
                task_prompt: "test task".to_string(),
                files_in_focus: Vec::new(),
                task_hint: None,
                summary: Some("test summary".to_string()),
                files_touched: Vec::new(),
                tests: Vec::new(),
                decisions: Vec::new(),
                unresolved_items: Vec::new(),
                observed_preferences: Vec::new(),
                risk_signals: Vec::new(),
                issue_refs: Vec::new(),
                artifact_refs: Vec::new(),
                started_at: None,
                ended_at: None,
            },
        )
        .await
        .unwrap();

        let episode = response.stored_episode.unwrap();
        use adesh_core::ports::storage::InterventionOutcomeQuery;
        let outcomes = storage
            .list_intervention_outcomes(InterventionOutcomeQuery {
                episode_id: Some(episode.episode_id),
                context_ref: None,
                learn_from_this: None,
                selected_response: None,
            })
            .await
            .unwrap();

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].learn_from_this, false); // Must be false because context is invalid
    }

    #[tokio::test]
    async fn linked_ignored_outcome_is_learnable() {
        let storage = new_storage().await;
        use adesh_core::ports::storage::InterventionContextInput;
        let context = storage
            .store_intervention_context(InterventionContextInput {
                context_id: String::new(),
                scope_type: "workspace".to_string(),
                scope_key: "workspace://test".to_string(),
                task_prompt: "test task".to_string(),
                prepared_at: chrono::Utc::now(),
                host_agent_id: Some("agent-1".to_string()),
                host_agent_kind: None,
                host_model: None,
                selected_direction: Some("do thing".to_string()),
                selected_direction_rank: Some(0),
                surfaced_directions_json: None,
            })
            .await
            .unwrap();

        let end = handle_connector_event(
            &storage,
            ConnectorEventRequest {
                connector_id: "test".to_string(),
                connector_kind: "test".to_string(),
                connector_version: None,
                session_id: None,
                host_agent_id: Some("agent-1".to_string()),
                host_agent_kind: None,
                host_model: None,
                context_id: Some(context.context_id),
                selected_next_direction: Some("do thing".to_string()),
                outcome: Some("ignored".to_string()),
                correction_summary: None,
                event_kind: ConnectorEventKind::TaskEnd,
                workspace: workspace("workspace://test"),
                task_prompt: "test task".to_string(),
                files_in_focus: Vec::new(),
                task_hint: None,
                summary: Some("test summary".to_string()),
                files_touched: Vec::new(),
                tests: Vec::new(),
                decisions: Vec::new(),
                unresolved_items: Vec::new(),
                observed_preferences: Vec::new(),
                risk_signals: Vec::new(),
                issue_refs: Vec::new(),
                artifact_refs: Vec::new(),
                started_at: None,
                ended_at: None,
            },
        )
        .await
        .unwrap();

        let episode = end.stored_episode.unwrap();
        use adesh_core::ports::storage::InterventionOutcomeQuery;
        let outcomes = storage
            .list_intervention_outcomes(InterventionOutcomeQuery {
                episode_id: Some(episode.episode_id),
                context_ref: None,
                learn_from_this: None,
                selected_response: None,
            })
            .await
            .unwrap();

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].selected_response, "ignored");
        assert!(outcomes[0].learn_from_this);
    }

    #[tokio::test]
    async fn direct_storage_never_learns_from_unlinked_outcome() {
        let storage = new_storage().await;
        use adesh_core::ports::storage::InterventionOutcomeInput;

        let outcome = storage
            .store_intervention_outcome(InterventionOutcomeInput {
                intervention_id: String::new(),
                episode_id: Some("ep-unlinked".to_string()),
                surfaced_direction: "do thing".to_string(),
                context_ref: None,
                surfaced_at: Utc::now(),
                selected_response: "accepted".to_string(),
                modified_payload: None,
                outcome_ref: None,
                correction_summary: None,
                learn_from_this: true,
                idempotency_key: Some("unlinked-learnability".to_string()),
            })
            .await
            .unwrap();

        assert!(!outcome.learn_from_this);
        assert!(outcome.context_ref.is_none());
    }

    #[tokio::test]
    async fn unlinked_outcome_hash_keeps_distinct_episodes() {
        let storage = new_storage().await;
        use adesh_core::ports::storage::{InterventionOutcomeInput, InterventionOutcomeQuery};

        for episode_id in ["ep-unlinked-a", "ep-unlinked-b"] {
            storage
                .store_intervention_outcome(InterventionOutcomeInput {
                    intervention_id: String::new(),
                    episode_id: Some(episode_id.to_string()),
                    surfaced_direction: "repeat same suggestion".to_string(),
                    context_ref: None,
                    surfaced_at: Utc::now(),
                    selected_response: "ignored".to_string(),
                    modified_payload: None,
                    outcome_ref: None,
                    correction_summary: None,
                    learn_from_this: false,
                    idempotency_key: None,
                })
                .await
                .unwrap();
        }

        let outcomes = storage
            .list_intervention_outcomes(InterventionOutcomeQuery {
                episode_id: None,
                context_ref: None,
                learn_from_this: None,
                selected_response: Some("ignored".to_string()),
            })
            .await
            .unwrap();
        assert_eq!(outcomes.len(), 2);
    }

    #[tokio::test]
    async fn context_retry_does_not_create_duplicate_logical_entries() {
        let storage = new_storage().await;

        use adesh_core::ports::storage::InterventionContextInput;

        let input = InterventionContextInput {
            context_id: String::new(),
            scope_type: "workspace".to_string(),
            scope_key: "workspace://test-dup".to_string(),
            task_prompt: "do something deterministic".to_string(),
            prepared_at: chrono::Utc::now(),
            host_agent_id: Some("agent-1".to_string()),
            host_agent_kind: None,
            host_model: None,
            selected_direction: Some("test dir".to_string()),
            selected_direction_rank: Some(0),
            surfaced_directions_json: None,
        };

        let c1 = storage
            .store_intervention_context(input.clone())
            .await
            .unwrap();
        let ctx_id_1 = c1.context_id;

        let mut input2 = input.clone();
        input2.context_id = String::new(); // Retry without ID

        let c2 = storage.store_intervention_context(input2).await.unwrap();
        let ctx_id_2 = c2.context_id;

        assert_eq!(ctx_id_1, ctx_id_2);

        let contexts = storage
            .find_intervention_contexts("workspace", "workspace://test-dup", 10)
            .await
            .unwrap();
        assert_eq!(contexts.len(), 1);
    }

    #[tokio::test]
    async fn duplicate_outcome_replay_returns_same_logical_record() {
        let storage = new_storage().await;

        use adesh_core::ports::storage::InterventionOutcomeInput;

        let input = InterventionOutcomeInput {
            intervention_id: String::new(),
            episode_id: Some("ep-1".to_string()),
            surfaced_direction: "do things".to_string(),
            context_ref: None,
            surfaced_at: chrono::Utc::now(),
            selected_response: "accepted".to_string(),
            modified_payload: None,
            outcome_ref: None,
            correction_summary: None,
            learn_from_this: false,
            idempotency_key: Some("fixed-outcome-replay-key".to_string()),
        };

        let out1 = storage
            .store_intervention_outcome(input.clone())
            .await
            .unwrap();

        let mut input2 = input.clone();
        input2.intervention_id = String::new(); // host retry
        input2.surfaced_direction = "changed during replay".to_string();
        input2.selected_response = "ignored".to_string();

        let out2 = storage.store_intervention_outcome(input2).await.unwrap();

        assert_eq!(out1.intervention_id, out2.intervention_id);
        assert_eq!(out2.surfaced_direction, "do things");
        assert_eq!(out2.selected_response, "accepted");
    }

    #[tokio::test]
    async fn duplicate_context_replay_returns_stored_record() {
        let storage = new_storage().await;
        use adesh_core::ports::storage::InterventionContextInput;

        let input = InterventionContextInput {
            context_id: "ctx-fixed-replay".to_string(),
            scope_type: "workspace".to_string(),
            scope_key: "workspace://test-fixed".to_string(),
            task_prompt: "original prompt".to_string(),
            prepared_at: Utc::now(),
            host_agent_id: Some("agent-1".to_string()),
            host_agent_kind: None,
            host_model: None,
            selected_direction: Some("original direction".to_string()),
            selected_direction_rank: Some(0),
            surfaced_directions_json: Some("[{\"statement\":\"original direction\"}]".to_string()),
        };

        let first = storage
            .store_intervention_context(input.clone())
            .await
            .unwrap();
        let mut replay = input.clone();
        replay.task_prompt = "changed prompt".to_string();
        replay.selected_direction = Some("changed direction".to_string());
        replay.selected_direction_rank = Some(2);

        let second = storage.store_intervention_context(replay).await.unwrap();
        assert_eq!(first.context_id, second.context_id);
        assert_eq!(second.task_prompt, "original prompt");
        assert_eq!(
            second.selected_direction.as_deref(),
            Some("original direction")
        );
        assert_eq!(second.selected_direction_rank, Some(0));
    }

    #[tokio::test]
    async fn connector_accepted_outcome_feeds_later_ranking() {
        let storage = new_storage().await;
        let workspace = workspace("workspace://connector-learning");

        cognition::store_work_episode(
            &storage,
            StoreWorkEpisodeRequest {
                workspace: workspace.clone(),
                task_prompt: "Earlier retry hardening decision".to_string(),
                summary: "Kept explicit retry state in service".to_string(),
                files_touched: vec!["src/retry.rs".to_string()],
                tests: Vec::new(),
                decisions: vec![WorkEpisodeDecision {
                    decision: "Keep retry state explicit in service layer".to_string(),
                    rationale: Some("Improves failure-path auditability".to_string()),
                }],
                unresolved_items: Vec::new(),
                observed_preferences: Vec::new(),
                risk_signals: Vec::new(),
                issue_refs: Vec::new(),
                artifact_refs: Vec::new(),
                task_hint: None,
                started_at: None,
                ended_at: Some(Utc::now() - Duration::hours(6)),
            },
        )
        .await
        .unwrap();
        cognition::store_work_episode(
            &storage,
            StoreWorkEpisodeRequest {
                workspace: workspace.clone(),
                task_prompt: "Later retry style note".to_string(),
                summary: "Added simple sleep retry note".to_string(),
                files_touched: vec!["src/retry.rs".to_string()],
                tests: Vec::new(),
                decisions: vec![WorkEpisodeDecision {
                    decision: "Use simple sleep retry loop".to_string(),
                    rationale: Some("Shorter code".to_string()),
                }],
                unresolved_items: Vec::new(),
                observed_preferences: Vec::new(),
                risk_signals: Vec::new(),
                issue_refs: Vec::new(),
                artifact_refs: Vec::new(),
                task_hint: None,
                started_at: None,
                ended_at: Some(Utc::now()),
            },
        )
        .await
        .unwrap();

        let start = handle_connector_event(
            &storage,
            ConnectorEventRequest {
                connector_id: "codex-vscode".to_string(),
                connector_kind: "chat_extension".to_string(),
                connector_version: Some("0.1.0".to_string()),
                session_id: Some("sess-learning".to_string()),
                host_agent_id: Some("agent-learning".to_string()),
                host_agent_kind: Some("codex-extension".to_string()),
                host_model: Some("gpt-test".to_string()),
                context_id: None,
                selected_next_direction: None,
                outcome: None,
                correction_summary: None,
                event_kind: ConnectorEventKind::TaskStart,
                workspace: workspace.clone(),
                task_prompt: "Which retry decision should we carry forward?".to_string(),
                files_in_focus: vec!["src/retry.rs".to_string()],
                task_hint: None,
                summary: None,
                files_touched: Vec::new(),
                tests: Vec::new(),
                decisions: Vec::new(),
                unresolved_items: Vec::new(),
                observed_preferences: Vec::new(),
                risk_signals: Vec::new(),
                issue_refs: Vec::new(),
                artifact_refs: Vec::new(),
                started_at: None,
                ended_at: None,
            },
        )
        .await
        .unwrap();
        let context_id = start.context_id.expect("expected connector context");

        handle_connector_event(
            &storage,
            ConnectorEventRequest {
                connector_id: "codex-vscode".to_string(),
                connector_kind: "chat_extension".to_string(),
                connector_version: Some("0.1.0".to_string()),
                session_id: Some("sess-learning".to_string()),
                host_agent_id: Some("agent-learning".to_string()),
                host_agent_kind: Some("codex-extension".to_string()),
                host_model: Some("gpt-test".to_string()),
                context_id: Some(context_id),
                selected_next_direction: Some(
                    "Keep retry state explicit in service layer".to_string(),
                ),
                outcome: Some("accepted".to_string()),
                correction_summary: Some(
                    "This matched the desired auditability standard".to_string(),
                ),
                event_kind: ConnectorEventKind::TaskEnd,
                workspace: workspace.clone(),
                task_prompt: "Which retry decision should we carry forward?".to_string(),
                files_in_focus: vec!["src/retry.rs".to_string()],
                task_hint: None,
                summary: Some("Accepted explicit retry state guidance".to_string()),
                files_touched: vec!["src/retry.rs".to_string()],
                tests: Vec::new(),
                decisions: Vec::new(),
                unresolved_items: Vec::new(),
                observed_preferences: Vec::new(),
                risk_signals: Vec::new(),
                issue_refs: Vec::new(),
                artifact_refs: Vec::new(),
                started_at: None,
                ended_at: None,
            },
        )
        .await
        .unwrap();

        let response = cognition::prepare_task_context(
            &storage,
            PrepareTaskContextRequest {
                workspace,
                task_prompt: "Which retry decision should we carry forward?".to_string(),
                files_in_focus: vec!["src/retry.rs".to_string()],
                task_hint: None,
            },
        )
        .await
        .unwrap();

        let explicit_idx = response
            .relevant_decisions
            .iter()
            .position(|item| item.statement.contains("explicit in service layer"))
            .expect("expected explicit retry decision");
        let simple_idx = response
            .relevant_decisions
            .iter()
            .position(|item| item.statement.contains("simple sleep retry"))
            .expect("expected simple retry decision");
        assert!(
            explicit_idx < simple_idx,
            "connector-produced accepted outcome should promote matching decision"
        );
        assert!(
            response
                .likely_next_directions
                .iter()
                .any(|item| item.basis.contains("accepted")),
            "later guidance should expose accepted intervention evidence"
        );
    }

    #[tokio::test]
    async fn eval_run_persists_and_fetches() {
        let storage = new_storage().await;

        use adesh_core::ports::storage::EvalRunInput;

        let input = EvalRunInput {
            run_id: String::new(),
            eval_name: "test-eval".to_string(),
            eval_version: Some("1.0.0".to_string()),
            run_started_at: chrono::Utc::now(),
            run_completed_at: Some(chrono::Utc::now()),
            baseline_summary: Some("baseline test".to_string()),
            treatment_summary: Some("treatment test".to_string()),
            judge_summary: Some("judge test".to_string()),
            failure_tags: Some("tag1,tag2".to_string()),
            promotion_decision: Some("promote".to_string()),
            idempotency_key: None,
        };

        let run = storage.store_eval_run(input.clone()).await.unwrap();

        let fetched = storage.get_eval_run(&run.run_id).await.unwrap();

        assert_eq!(fetched.run_id, run.run_id);
        assert_eq!(fetched.eval_name, "test-eval");
        assert_eq!(fetched.promotion_decision, Some("promote".to_string()));
    }

    #[tokio::test]
    async fn eval_run_idempotent_replay_returns_same_record() {
        let storage = new_storage().await;

        use adesh_core::ports::storage::EvalRunInput;

        let input = EvalRunInput {
            run_id: String::new(),
            eval_name: "dup-eval".to_string(),
            eval_version: Some("1.0.0".to_string()),
            run_started_at: chrono::Utc::now(),
            run_completed_at: None,
            baseline_summary: Some("baseline".to_string()),
            treatment_summary: None,
            judge_summary: None,
            failure_tags: None,
            promotion_decision: None,
            idempotency_key: Some("test-key-123".to_string()),
        };

        let run1 = storage.store_eval_run(input.clone()).await.unwrap();

        let run2 = storage.store_eval_run(input).await.unwrap();

        assert_eq!(run1.run_id, run2.run_id);
    }

    #[tokio::test]
    async fn eval_run_list_filters_by_promotion() {
        let storage = new_storage().await;

        use adesh_core::ports::storage::EvalRunInput;

        for i in 0..3 {
            let input = EvalRunInput {
                run_id: String::new(),
                eval_name: "filter-test".to_string(),
                eval_version: Some("1.0.0".to_string()),
                run_started_at: chrono::Utc::now(),
                run_completed_at: None,
                baseline_summary: None,
                treatment_summary: None,
                judge_summary: None,
                failure_tags: None,
                promotion_decision: if i < 2 {
                    Some("promote".to_string())
                } else {
                    Some("reject".to_string())
                },
                idempotency_key: None,
            };
            storage.store_eval_run(input).await.unwrap();
        }

        use adesh_core::ports::storage::EvalRunQuery;
        let promote_runs = storage
            .list_eval_runs(EvalRunQuery {
                eval_name: None,
                eval_version: None,
                promotion_decision: Some("promote".to_string()),
            })
            .await
            .unwrap();

        assert_eq!(promote_runs.len(), 2);
    }

    #[tokio::test]
    async fn eval_artifact_links_to_run() {
        let storage = new_storage().await;

        use adesh_core::ports::storage::{EvalArtifactInput, EvalRunInput};

        let run_input = EvalRunInput {
            run_id: String::new(),
            eval_name: "artifact-test".to_string(),
            eval_version: None,
            run_started_at: chrono::Utc::now(),
            run_completed_at: None,
            baseline_summary: None,
            treatment_summary: None,
            judge_summary: None,
            failure_tags: None,
            promotion_decision: None,
            idempotency_key: None,
        };

        let run = storage.store_eval_run(run_input).await.unwrap();

        let artifact = EvalArtifactInput {
            artifact_id: String::new(),
            run_id: run.run_id.clone(),
            artifact_kind: "transcript".to_string(),
            file_path: "/tmp/transcript.txt".to_string(),
            mime_type: Some("text/plain".to_string()),
        };

        let _stored = storage.store_eval_artifact(artifact).await.unwrap();

        let artifacts = storage.list_eval_artifacts(&run.run_id).await.unwrap();

        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].artifact_kind, "transcript");
    }
}
