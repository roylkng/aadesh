use std::collections::{BTreeMap, BTreeSet, HashSet};

use adesh_contracts::{
    MemoryClaimRecord, NextDirectionItem, PrepareTaskContextRequest, PrepareTaskContextResponse,
    RecallRelevantMemoryItem, RecallRelevantMemoryRequest, RecallRelevantMemoryResponse,
    RiskFlagItem, ScopedGuidanceItem, StoreWorkEpisodeRequest, WorkEpisodeDecision,
    WorkEpisodeResponse, WorkEpisodeTestResult, WorkspaceDescriptor, WorkspaceResolutionResponse,
};
use adesh_core::{
    AppError,
    ports::storage::{
        MemoryClaimEvidenceInput, MemoryClaimQuery, MemoryClaimUpsertInput, SearchDocumentQuery,
        SearchDocumentUpsertInput, StorageProvider, WorkEpisodeDecisionInput, WorkEpisodeListQuery,
        WorkEpisodeStoreInput, WorkEpisodeTestResultInput,
    },
};
use chrono::Utc;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const MAX_GUIDANCE_ITEMS: usize = 3;

#[derive(Debug, Clone)]
struct RankingContext<'a> {
    query_tokens: &'a [String],
    task_scope_key: Option<&'a str>,
    files_in_focus: &'a [String],
    intent_mode: PromptIntentMode,
    now: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptIntentMode {
    ValidationProof,
    Implementation,
    Cleanup,
    Debugging,
    Research,
    General,
}

#[derive(Debug, Clone)]
struct RankedClaim {
    score: i64,
    claim: MemoryClaimRecord,
}

pub async fn store_work_episode<S: StorageProvider + ?Sized>(
    storage: &S,
    request: StoreWorkEpisodeRequest,
) -> Result<WorkEpisodeResponse, AppError> {
    if request.task_prompt.trim().is_empty() {
        return Err(AppError::BadRequest("task_prompt is required".to_string()));
    }
    if request.summary.trim().is_empty() {
        return Err(AppError::BadRequest("summary is required".to_string()));
    }

    let workspace_resolution = resolve_workspace(&request.workspace);
    let task_scope_key = derive_task_scope_key(
        request.task_hint.as_deref(),
        &request.issue_refs,
        &request.task_prompt,
        &request.files_touched,
        request.workspace.branch.as_deref(),
    );
    let started_at = request.started_at.unwrap_or_else(Utc::now);
    let ended_at = request.ended_at.unwrap_or_else(Utc::now);
    let response = storage
        .store_work_episode(WorkEpisodeStoreInput {
            workspace: request.workspace.clone(),
            workspace_resolution: workspace_resolution.clone(),
            task_scope_key: task_scope_key.clone(),
            task_prompt: request.task_prompt.clone(),
            summary: request.summary.clone(),
            files_touched: request.files_touched.clone(),
            tests: request
                .tests
                .iter()
                .map(|item| WorkEpisodeTestResultInput {
                    name: item.name.clone(),
                    status: item.status.clone(),
                    summary: item.summary.clone(),
                })
                .collect(),
            decisions: request
                .decisions
                .iter()
                .map(|item| WorkEpisodeDecisionInput {
                    decision: item.decision.clone(),
                    rationale: item.rationale.clone(),
                })
                .collect(),
            unresolved_items: request.unresolved_items.clone(),
            observed_preferences: request.observed_preferences.clone(),
            risk_signals: request.risk_signals.clone(),
            issue_refs: request.issue_refs.clone(),
            artifact_refs: request.artifact_refs.clone(),
            started_at,
            ended_at,
        })
        .await?;

    let mut evidence = vec![MemoryClaimEvidenceInput {
        evidence_ref: response.event_ref.clone(),
        evidence_kind: "experience_event".to_string(),
        locator_json: None,
    }];
    for artifact_ref in &response.artifact_refs {
        evidence.push(MemoryClaimEvidenceInput {
            evidence_ref: artifact_ref.clone(),
            evidence_kind: "artifact".to_string(),
            locator_json: None,
        });
    }

    for decision in &request.decisions {
        upsert_memory_statement(
            storage,
            "decision",
            "workspace",
            &workspace_resolution.resolved_scope_key,
            decision.decision.as_str(),
            &json!({
                "statement": decision.decision,
                "rationale": decision.rationale,
            }),
            &json!({
                "branch": request.workspace.branch.clone(),
                "task_scope_key": task_scope_key.clone(),
                "files_touched": request.files_touched.clone(),
            }),
            evidence.clone(),
            true,
            &response.event_ref,
        )
        .await?;
    }

    let open_loop_scope = if task_scope_key.is_some() {
        ("task_or_workstream", task_scope_key.as_deref().unwrap())
    } else {
        (
            "workspace",
            workspace_resolution.resolved_scope_key.as_str(),
        )
    };
    resolve_stale_open_loops(
        storage,
        open_loop_scope.0,
        open_loop_scope.1,
        &request.summary,
        &request.decisions,
        &request.tests,
        &request.unresolved_items,
        &response.event_ref,
    )
    .await?;
    for item in &request.unresolved_items {
        upsert_memory_statement(
            storage,
            "open_loop",
            open_loop_scope.0,
            open_loop_scope.1,
            item,
            &json!({ "statement": item }),
            &json!({
                "branch": request.workspace.branch.clone(),
                "files_touched": request.files_touched.clone(),
            }),
            evidence.clone(),
            true,
            &response.event_ref,
        )
        .await?;
    }

    for item in &request.observed_preferences {
        let claim = upsert_memory_statement(
            storage,
            "preference",
            "workspace",
            &workspace_resolution.resolved_scope_key,
            item,
            &json!({ "statement": item }),
            &json!({
                "branch": request.workspace.branch.clone(),
                "files_touched": request.files_touched.clone(),
            }),
            evidence.clone(),
            true,
            &response.event_ref,
        )
        .await?;
        promote_user_global_preference(storage, &claim, &response.event_ref).await?;
    }

    for item in &request.risk_signals {
        upsert_memory_statement(
            storage,
            "risk",
            open_loop_scope.0,
            open_loop_scope.1,
            item,
            &json!({ "statement": item, "severity": derive_risk_severity(item) }),
            &json!({
                "branch": request.workspace.branch.clone(),
                "files_touched": request.files_touched.clone(),
            }),
            evidence.clone(),
            true,
            &response.event_ref,
        )
        .await?;
    }

    extract_candidate_memories(
        storage,
        &request,
        &workspace_resolution,
        task_scope_key.as_deref(),
        &response,
        &evidence,
    )
    .await?;

    Ok(response)
}

async fn extract_candidate_memories<S: StorageProvider + ?Sized>(
    storage: &S,
    request: &StoreWorkEpisodeRequest,
    workspace_resolution: &WorkspaceResolutionResponse,
    task_scope_key: Option<&str>,
    response: &WorkEpisodeResponse,
    evidence: &[MemoryClaimEvidenceInput],
) -> Result<(), AppError> {
    let workspace_context = json!({
        "branch": request.workspace.branch.clone(),
        "task_scope_key": task_scope_key,
        "files_touched": request.files_touched.clone(),
    });
    let open_loop_scope = if let Some(task_scope_key) = task_scope_key {
        ("task_or_workstream", task_scope_key)
    } else {
        (
            "workspace",
            workspace_resolution.resolved_scope_key.as_str(),
        )
    };

    for sentence in split_summary_sentences(&request.summary) {
        if request
            .decisions
            .iter()
            .any(|item| statements_materially_overlap(item.decision.as_str(), sentence.as_str()))
        {
            continue;
        }
        if looks_like_inferred_decision(sentence.as_str()) {
            upsert_memory_statement(
                storage,
                "decision",
                "workspace",
                &workspace_resolution.resolved_scope_key,
                sentence.as_str(),
                &json!({ "statement": sentence }),
                &workspace_context,
                evidence.to_vec(),
                false,
                &response.event_ref,
            )
            .await?;
        }
        if request
            .unresolved_items
            .iter()
            .any(|item| statements_materially_overlap(item.as_str(), sentence.as_str()))
        {
            continue;
        }
        if looks_like_inferred_open_loop(sentence.as_str()) {
            upsert_memory_statement(
                storage,
                "open_loop",
                open_loop_scope.0,
                open_loop_scope.1,
                sentence.as_str(),
                &json!({ "statement": sentence }),
                &workspace_context,
                evidence.to_vec(),
                false,
                &response.event_ref,
            )
            .await?;
        }
        if request
            .observed_preferences
            .iter()
            .any(|item| statements_materially_overlap(item.as_str(), sentence.as_str()))
        {
            continue;
        }
        if looks_like_inferred_preference(sentence.as_str()) {
            upsert_memory_statement(
                storage,
                "preference",
                "workspace",
                &workspace_resolution.resolved_scope_key,
                sentence.as_str(),
                &json!({ "statement": sentence }),
                &workspace_context,
                evidence.to_vec(),
                false,
                &response.event_ref,
            )
            .await?;
        }
    }

    for test in &request.tests {
        if test.status == "fail" {
            let statement = test
                .summary
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| format!("Failing test suggests unresolved work: {}", test.name));
            upsert_memory_statement(
                storage,
                "open_loop",
                open_loop_scope.0,
                open_loop_scope.1,
                statement.as_str(),
                &json!({ "statement": statement, "source_test": test.name }),
                &workspace_context,
                evidence.to_vec(),
                false,
                &response.event_ref,
            )
            .await?;

            if request.risk_signals.is_empty()
                && (request
                    .artifact_refs
                    .iter()
                    .any(|item| item.contains("incident") || item.contains("near-miss"))
                    || normalize_key(&request.summary).contains("risk")
                    || normalize_key(&statement).contains("duplicate")
                    || normalize_key(&statement).contains("unsafe"))
            {
                upsert_memory_statement(
                    storage,
                    "risk",
                    open_loop_scope.0,
                    open_loop_scope.1,
                    statement.as_str(),
                    &json!({
                        "statement": statement,
                        "severity": derive_risk_severity(statement.as_str())
                    }),
                    &workspace_context,
                    evidence.to_vec(),
                    false,
                    &response.event_ref,
                )
                .await?;
            }
        }
    }

    if request.risk_signals.is_empty()
        && request
            .artifact_refs
            .iter()
            .any(|item| item.contains("incident") || item.contains("near-miss"))
    {
        for sentence in split_summary_sentences(&request.summary) {
            if is_risk_like(sentence.as_str()) {
                upsert_memory_statement(
                    storage,
                    "risk",
                    open_loop_scope.0,
                    open_loop_scope.1,
                    sentence.as_str(),
                    &json!({
                        "statement": sentence,
                        "severity": derive_risk_severity(sentence.as_str())
                    }),
                    &workspace_context,
                    evidence.to_vec(),
                    false,
                    &response.event_ref,
                )
                .await?;
            }
        }
    }

    Ok(())
}

pub async fn prepare_task_context<S: StorageProvider + ?Sized>(
    storage: &S,
    request: PrepareTaskContextRequest,
) -> Result<PrepareTaskContextResponse, AppError> {
    if request.task_prompt.trim().is_empty() {
        return Err(AppError::BadRequest("task_prompt is required".to_string()));
    }

    let workspace_resolution = resolve_workspace(&request.workspace);
    let derived_task_scope = derive_task_scope_key(
        request.task_hint.as_deref(),
        &[],
        &request.task_prompt,
        &request.files_in_focus,
        request.workspace.branch.as_deref(),
    );
    let recent_episodes = storage
        .list_work_episodes(WorkEpisodeListQuery {
            scope_type: Some("workspace".to_string()),
            scope_key: Some(workspace_resolution.resolved_scope_key.clone()),
            task_scope_key: None,
            limit: Some(12),
        })
        .await?;
    let mut search_hits =
        search_workspace_documents(storage, &workspace_resolution, &request.task_prompt).await?;

    if search_hits.is_empty() {
        for file in &request.files_in_focus {
            let file_hits =
                search_workspace_documents(storage, &workspace_resolution, file).await?;
            if !file_hits.is_empty() {
                search_hits = file_hits;
                break;
            }
        }
    }

    let mut claims = storage
        .list_memory_claims(MemoryClaimQuery {
            scope_type: Some("workspace".to_string()),
            scope_key: Some(workspace_resolution.resolved_scope_key.clone()),
            statuses: vec!["accepted".to_string()],
            claim_types: Vec::new(),
            limit: Some(100),
        })
        .await?;
    let mut user_global_claims = storage
        .list_memory_claims(MemoryClaimQuery {
            scope_type: Some("user_global".to_string()),
            scope_key: Some("user:root_owner".to_string()),
            statuses: vec!["accepted".to_string()],
            claim_types: Vec::new(),
            limit: Some(100),
        })
        .await?;
    claims.append(&mut user_global_claims);
    if let Some(task_scope_key) = derived_task_scope.as_ref() {
        let mut task_claims = storage
            .list_memory_claims(MemoryClaimQuery {
                scope_type: Some("task_or_workstream".to_string()),
                scope_key: Some(task_scope_key.clone()),
                statuses: vec!["accepted".to_string()],
                claim_types: Vec::new(),
                limit: Some(100),
            })
            .await?;
        claims.append(&mut task_claims);
    }
    let claims = dedupe_claims(claims);
    let mut candidate_claims = storage
        .list_memory_claims(MemoryClaimQuery {
            scope_type: Some("workspace".to_string()),
            scope_key: Some(workspace_resolution.resolved_scope_key.clone()),
            statuses: vec!["candidate".to_string()],
            claim_types: Vec::new(),
            limit: Some(100),
        })
        .await?;
    if let Some(task_scope_key) = derived_task_scope.as_ref() {
        let mut task_candidates = storage
            .list_memory_claims(MemoryClaimQuery {
                scope_type: Some("task_or_workstream".to_string()),
                scope_key: Some(task_scope_key.clone()),
                statuses: vec!["candidate".to_string()],
                claim_types: Vec::new(),
                limit: Some(100),
            })
            .await?;
        candidate_claims.append(&mut task_candidates);
    }
    let mut global_candidates = storage
        .list_memory_claims(MemoryClaimQuery {
            scope_type: Some("user_global".to_string()),
            scope_key: Some("user:root_owner".to_string()),
            statuses: vec!["candidate".to_string()],
            claim_types: Vec::new(),
            limit: Some(100),
        })
        .await?;
    candidate_claims.append(&mut global_candidates);
    let candidate_claims = dedupe_claims(candidate_claims);

    let query_tokens = build_query_tokens(&request.task_prompt, &request.files_in_focus);
    let ranking = RankingContext {
        query_tokens: &query_tokens,
        task_scope_key: derived_task_scope.as_deref(),
        files_in_focus: &request.files_in_focus,
        intent_mode: classify_prompt_intent(&request.task_prompt),
        now: Utc::now(),
    };
    let ranked_decisions = rank_claims(&claims, "decision", &ranking);
    let ranked_preferences = rank_claims(&claims, "preference", &ranking);
    let ranked_open_loops = rank_claims(&claims, "open_loop", &ranking);
    let ranked_risks = rank_claims(&claims, "risk", &ranking);
    let ranked_candidate_decisions = rank_claims(&candidate_claims, "decision", &ranking);
    let ranked_candidate_preferences = rank_claims(&candidate_claims, "preference", &ranking);
    let ranked_candidate_open_loops = rank_claims(&candidate_claims, "open_loop", &ranking);
    let ranked_candidate_risks = rank_claims(&candidate_claims, "risk", &ranking);

    let selected_decisions = fill_with_candidate_claims(
        select_ranked_claims(&ranked_decisions, MAX_GUIDANCE_ITEMS, &[]),
        &ranked_candidate_decisions,
        MAX_GUIDANCE_ITEMS,
        &[],
    );
    let suppressed_preference_keys = selected_decisions
        .iter()
        .map(|claim| claim_statement(claim).to_string())
        .collect::<Vec<_>>();
    let selected_preferences = fill_with_candidate_claims(
        select_ranked_claims(
            &ranked_preferences,
            MAX_GUIDANCE_ITEMS,
            &suppressed_preference_keys,
        ),
        &ranked_candidate_preferences,
        MAX_GUIDANCE_ITEMS,
        &suppressed_preference_keys,
    );
    let selected_open_loops = fill_with_candidate_claims(
        select_ranked_claims(&ranked_open_loops, MAX_GUIDANCE_ITEMS, &[]),
        &ranked_candidate_open_loops,
        MAX_GUIDANCE_ITEMS,
        &[],
    );
    let selected_risks = prune_redundant_risks(
        fill_with_candidate_claims(
            select_ranked_claims(&ranked_risks, MAX_GUIDANCE_ITEMS, &[]),
            &ranked_candidate_risks,
            MAX_GUIDANCE_ITEMS,
            &[],
        ),
        &selected_open_loops,
    );

    let decisions = claims_to_scoped_items(&selected_decisions);
    let preferences = claims_to_scoped_items(&selected_preferences);
    let open_loops = claims_to_scoped_items(&selected_open_loops);
    let risks = claims_to_risk_items(&selected_risks, &open_loops);
    let likely_next_directions = build_next_directions(
        &selected_open_loops,
        &selected_risks,
        &selected_decisions,
        &selected_preferences,
    );

    let task_focus = select_task_focus(
        request.task_prompt.trim(),
        derived_task_scope.as_deref(),
        &request.files_in_focus,
        &query_tokens,
        ranking.intent_mode,
        &recent_episodes,
        &search_hits,
    );

    let context_status = if recent_episodes.is_empty() && claims.is_empty() {
        "insufficient"
    } else if search_hits.is_empty() || request.files_in_focus.is_empty() {
        "partial_but_sufficient"
    } else {
        "full"
    }
    .to_string();

    let mut uncertainties = conflict_uncertainties(
        &candidate_claims,
        &selected_decisions,
        &selected_preferences,
        &selected_open_loops,
        &selected_risks,
    );
    if selected_decisions
        .iter()
        .chain(selected_preferences.iter())
        .chain(selected_open_loops.iter())
        .chain(selected_risks.iter())
        .any(|claim| claim.status == "candidate")
    {
        uncertainties.push(
            "Some surfaced guidance is inferred candidate memory backed by limited evidence."
                .to_string(),
        );
    }
    if derived_task_scope.is_some() && request.task_hint.is_none() {
        uncertainties.push(
            "Task scope key was derived heuristically from prompt, files, and branch.".to_string(),
        );
    }
    if search_hits.is_empty() {
        uncertainties.push(
            "No lexical matches were found; guidance is based on scoped memory only.".to_string(),
        );
    }
    if request.files_in_focus.is_empty() {
        uncertainties.push("No files_in_focus were provided by the host.".to_string());
    }
    uncertainties.truncate(MAX_GUIDANCE_ITEMS);

    Ok(PrepareTaskContextResponse {
        context_status,
        workspace_resolution,
        task_focus,
        relevant_decisions: decisions,
        applicable_preferences: preferences,
        open_loops,
        risk_flags: risks,
        likely_next_directions,
        uncertainties,
    })
}

pub async fn recall_relevant_memory<S: StorageProvider + ?Sized>(
    storage: &S,
    request: RecallRelevantMemoryRequest,
) -> Result<RecallRelevantMemoryResponse, AppError> {
    if request.query.trim().is_empty() {
        return Err(AppError::BadRequest("query is required".to_string()));
    }

    let workspace_resolution = resolve_workspace(&request.workspace);
    let claims = storage
        .list_memory_claims(MemoryClaimQuery {
            scope_type: Some("workspace".to_string()),
            scope_key: Some(workspace_resolution.resolved_scope_key.clone()),
            statuses: vec!["accepted".to_string()],
            claim_types: request.memory_types.clone(),
            limit: Some(request.limit.unwrap_or(25)),
        })
        .await?;
    let query_tokens = build_query_tokens(&request.query, &[]);
    let derived_task_scope = request
        .task_hint
        .as_deref()
        .map(normalize_key)
        .map(|value| format!("task:hint:{value}"));
    let ranking = RankingContext {
        query_tokens: &query_tokens,
        task_scope_key: derived_task_scope.as_deref(),
        files_in_focus: &[],
        intent_mode: classify_prompt_intent(&request.query),
        now: Utc::now(),
    };
    let mut ranked = rank_flat_memories(&claims, &ranking);
    ranked.truncate(usize::try_from(request.limit.unwrap_or(8)).unwrap_or(8));

    let uncertainties = if ranked.is_empty() {
        vec!["No accepted scoped memories matched the current query.".to_string()]
    } else {
        Vec::new()
    };

    Ok(RecallRelevantMemoryResponse {
        workspace_resolution,
        memories: ranked,
        uncertainties,
    })
}

pub fn resolve_workspace(workspace: &WorkspaceDescriptor) -> WorkspaceResolutionResponse {
    if let Some(locator) = workspace
        .locator
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        return WorkspaceResolutionResponse {
            resolved_scope_key: format!("workspace:{}:{}", workspace.kind, locator.trim()),
            scope_type: "workspace".to_string(),
            resolution_basis: vec!["locator".to_string()],
            confidence: 0.99,
        };
    }
    if let Some(external_ref) = workspace
        .external_ref
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        return WorkspaceResolutionResponse {
            resolved_scope_key: format!("workspace:{}:{}", workspace.kind, external_ref.trim()),
            scope_type: "workspace".to_string(),
            resolution_basis: vec!["external_ref".to_string()],
            confidence: 0.96,
        };
    }
    if let Some(cwd) = workspace
        .cwd
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        let mut basis = vec!["cwd".to_string()];
        if workspace.branch.as_deref().is_some() {
            basis.push("branch".to_string());
        }
        return WorkspaceResolutionResponse {
            resolved_scope_key: format!("workspace:{}:{}", workspace.kind, cwd.trim()),
            scope_type: "workspace".to_string(),
            resolution_basis: basis,
            confidence: if workspace.branch.is_some() {
                0.9
            } else {
                0.82
            },
        };
    }

    WorkspaceResolutionResponse {
        resolved_scope_key: format!("workspace:transient:{}", workspace.kind),
        scope_type: "workspace".to_string(),
        resolution_basis: vec!["transient".to_string()],
        confidence: 0.35,
    }
}

pub fn derive_task_scope_key(
    task_hint: Option<&str>,
    issue_refs: &[String],
    task_prompt: &str,
    files: &[String],
    branch: Option<&str>,
) -> Option<String> {
    if let Some(task_hint) = task_hint.filter(|value| !value.trim().is_empty()) {
        return Some(format!("task:hint:{}", normalize_key(task_hint)));
    }
    if let Some(issue_ref) = issue_refs.iter().find(|value| !value.trim().is_empty()) {
        return Some(format!("task:issue:{}", normalize_key(issue_ref)));
    }
    if task_prompt.trim().is_empty() {
        return None;
    }

    let mut seed = String::new();
    seed.push_str(task_prompt.trim());
    seed.push('|');
    seed.push_str(branch.unwrap_or(""));
    seed.push('|');
    seed.push_str(
        &files
            .iter()
            .take(3)
            .map(|value| value.trim())
            .collect::<Vec<_>>()
            .join("|"),
    );
    Some(format!("task:derived:{}", short_hash(&seed)))
}

async fn search_workspace_documents<S: StorageProvider + ?Sized>(
    storage: &S,
    workspace_resolution: &WorkspaceResolutionResponse,
    query: &str,
) -> Result<Vec<adesh_contracts::SearchDocumentHit>, AppError> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let mut seen = BTreeSet::new();
    let mut hits = Vec::new();
    for candidate in std::iter::once(trimmed.to_string()).chain(build_query_tokens(trimmed, &[])) {
        let docs = storage
            .search_documents(SearchDocumentQuery {
                scope_type: "workspace".to_string(),
                scope_key: workspace_resolution.resolved_scope_key.clone(),
                query_text: candidate,
                limit: 8,
            })
            .await?;
        for doc in docs {
            if seen.insert(doc.doc_id.clone()) {
                hits.push(doc);
            }
        }
        if hits.len() >= 8 {
            break;
        }
    }
    Ok(hits)
}

async fn upsert_memory_statement<S: StorageProvider + ?Sized>(
    storage: &S,
    claim_type: &str,
    scope_type: &str,
    scope_key: &str,
    statement: &str,
    value_json: &Value,
    context_predicates_json: &Value,
    evidence: Vec<MemoryClaimEvidenceInput>,
    explicit_host_signal: bool,
    promotion_event_ref: &str,
) -> Result<MemoryClaimRecord, AppError> {
    let subject_key = derive_subject_key(claim_type, statement);
    let statement_key = normalize_key(statement);
    let existing = storage
        .list_memory_claims(MemoryClaimQuery {
            scope_type: Some(scope_type.to_string()),
            scope_key: Some(scope_key.to_string()),
            statuses: vec![
                "candidate".to_string(),
                "accepted".to_string(),
                "superseded".to_string(),
            ],
            claim_types: vec![claim_type.to_string()],
            limit: Some(100),
        })
        .await?;

    let mut aligned = None;
    let mut conflicting = Vec::new();
    for claim in &existing {
        let existing_statement_key = normalize_key(claim_statement(claim));
        if existing_statement_key == statement_key {
            aligned = Some(claim.clone());
            continue;
        }
        if claim.scope_type == scope_type
            && claim.scope_key == scope_key
            && statements_conflict(claim_statement(claim), statement)
        {
            conflicting.push(claim.clone());
        }
    }

    let prior_signal_count = aligned.as_ref().map(signal_count).unwrap_or(0);
    let new_signal_count = prior_signal_count + 1;
    let supporting_artifact_evidence = has_supporting_artifact_evidence(&evidence);
    let deterministic_evidence = has_strong_artifact_evidence(&evidence);
    let merged_evidence =
        merge_evidence(aligned.as_ref().map(|item| item.evidence.clone()), evidence);
    let distinct_workspace_count = if claim_type == "preference" && scope_type == "user_global" {
        distinct_workspace_scope_count(storage, statement_key.as_str()).await?
    } else {
        1
    };
    let (mut status, mut confidence) = determine_claim_state(
        claim_type,
        scope_type,
        explicit_host_signal,
        new_signal_count,
        supporting_artifact_evidence,
        deterministic_evidence,
        distinct_workspace_count,
    );
    let incoming_strength = claim_strength_components(
        status,
        explicit_host_signal,
        supporting_artifact_evidence,
        deterministic_evidence,
        new_signal_count,
    );

    let mut superseded_conflicts = Vec::new();
    let mut unresolved_conflicts = Vec::new();
    for claim in &conflicting {
        let existing_strength = claim_strength(claim);
        if status == "accepted"
            && (claim.status != "accepted" || incoming_strength >= existing_strength)
        {
            superseded_conflicts.push(claim.clone());
        } else {
            status = "candidate";
            confidence = confidence.min(0.67);
            unresolved_conflicts.push(claim.claim_id.clone());
        }
    }

    let claim = storage
        .upsert_memory_claim(MemoryClaimUpsertInput {
            claim_id: aligned.as_ref().map(|item| item.claim_id.clone()),
            claim_type: claim_type.to_string(),
            claim_key: format!("{claim_type}:{scope_key}:{subject_key}"),
            scope_type: scope_type.to_string(),
            scope_key: scope_key.to_string(),
            subject_key,
            status: status.to_string(),
            created_by: "owner".to_string(),
            confidence,
            value_json: value_json.clone(),
            context_predicates_json: context_predicates_json.clone(),
            time_start: None,
            time_end: None,
            evidence_quality_json: json!({
                "signal_count": new_signal_count,
                "explicit_host_signal": explicit_host_signal,
                "supporting_artifact_evidence": supporting_artifact_evidence,
                "deterministic_evidence": deterministic_evidence,
                "statement_key": statement_key,
                "distinct_workspace_count": distinct_workspace_count,
                "conflict_state": if unresolved_conflicts.is_empty() { "clear" } else { "pending" },
                "conflicting_claim_ids": unresolved_conflicts,
            }),
            promotion_ref: Some(format!("owner_event:{promotion_event_ref}")),
            evidence: merged_evidence,
        })
        .await?;

    for old_claim in superseded_conflicts {
        supersede_claim(
            storage,
            &old_claim,
            &claim.claim_id,
            "newer_conflicting_memory",
        )
        .await?;
    }

    storage
        .upsert_search_document(SearchDocumentUpsertInput {
            doc_id: format!("doc:claim:{}", claim.claim_id),
            scope_type: claim.scope_type.clone(),
            scope_key: claim.scope_key.clone(),
            source_type: "claim".to_string(),
            source_ref: claim.claim_id.clone(),
            title: Some(claim_type.to_string()),
            body_text: claim_statement(&claim).to_string(),
        })
        .await?;

    Ok(claim)
}

fn build_query_tokens(prompt: &str, files: &[String]) -> Vec<String> {
    let mut tokens = BTreeSet::new();
    for token in normalize_key(prompt).split('_') {
        if token.len() >= 3 {
            tokens.insert(token.to_string());
        }
    }
    for file in files {
        for token in normalize_key(file).split('_') {
            if token.len() >= 3 {
                tokens.insert(token.to_string());
            }
        }
    }
    tokens.into_iter().collect()
}

fn rank_claims(
    claims: &[MemoryClaimRecord],
    claim_type: &str,
    ranking: &RankingContext<'_>,
) -> Vec<RankedClaim> {
    let token_set = ranking.query_tokens.iter().cloned().collect::<HashSet<_>>();
    let mut ranked = claims
        .iter()
        .filter(|claim| claim.claim_type == claim_type)
        .map(|claim| {
            let statement_tokens = build_query_tokens(claim_statement(claim), &[]);
            let overlap = statement_tokens
                .iter()
                .filter(|token| token_set.contains(token.as_str()))
                .count() as i64;
            let signals = signal_count(claim) as i64;
            let task_scope_match = ranking
                .task_scope_key
                .map(|scope| claim.scope_type == "task_or_workstream" && claim.scope_key == scope)
                .unwrap_or(false);
            let file_overlap = file_overlap_count(claim, ranking.files_in_focus) as i64;
            let recency_bonus = recency_bonus(claim, ranking.now);
            let evidence_bonus = evidence_bonus(claim);
            let category_bonus = category_bonus(claim_type);
            let explicitness_bonus = explicitness_bonus(claim);
            let actionability_bonus = actionability_bonus(claim);
            let intent_bonus = intent_alignment_bonus(claim, ranking.intent_mode);
            let scope_bonus = if task_scope_match {
                28
            } else if claim.scope_type == "workspace" {
                10
            } else {
                0
            };
            let score = category_bonus
                + scope_bonus
                + overlap * 12
                + file_overlap * 18
                + signals.min(4) * 7
                + explicitness_bonus
                + evidence_bonus
                + actionability_bonus
                + intent_bonus
                + recency_bonus;
            RankedClaim {
                score,
                claim: claim.clone(),
            }
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.claim.updated_at.cmp(&a.claim.updated_at))
    });
    ranked
}

fn claims_to_scoped_items(claims: &[MemoryClaimRecord]) -> Vec<ScopedGuidanceItem> {
    claims
        .iter()
        .map(|claim| ScopedGuidanceItem {
            statement: claim_statement(claim).to_string(),
            scope: claim.scope_type.clone(),
            confidence: claim.confidence,
            evidence_refs: claim
                .evidence
                .iter()
                .map(|item| item.evidence_ref.clone())
                .collect(),
            basis: basis_from_claim(claim),
        })
        .collect()
}

fn claims_to_risk_items(
    claims: &[MemoryClaimRecord],
    open_loops: &[ScopedGuidanceItem],
) -> Vec<RiskFlagItem> {
    let mut items = claims
        .iter()
        .map(|claim| {
            let statement = claim_statement(&claim).to_string();
            let severity = claim
                .value
                .get("severity")
                .and_then(Value::as_str)
                .unwrap_or("medium")
                .to_string();
            let confidence = claim.confidence;
            let evidence_refs = claim
                .evidence
                .iter()
                .map(|item| item.evidence_ref.clone())
                .collect();
            let basis = basis_from_claim(&claim);
            RiskFlagItem {
                statement,
                severity,
                confidence,
                evidence_refs,
                basis,
            }
        })
        .collect::<Vec<_>>();

    if items.is_empty() {
        for open_loop in open_loops.iter().take(MAX_GUIDANCE_ITEMS) {
            if should_derive_risk_from_open_loop(open_loop) {
                items.push(RiskFlagItem {
                    statement: open_loop.statement.clone(),
                    severity: derive_risk_severity(open_loop.statement.as_str()).to_string(),
                    confidence: open_loop.confidence,
                    evidence_refs: open_loop.evidence_refs.clone(),
                    basis:
                        "Derived from unresolved open loop with safety or validation risk language"
                            .to_string(),
                });
            }
        }
    }

    items.truncate(MAX_GUIDANCE_ITEMS);
    items
}

fn build_next_directions(
    open_loops: &[MemoryClaimRecord],
    risks: &[MemoryClaimRecord],
    decisions: &[MemoryClaimRecord],
    preferences: &[MemoryClaimRecord],
) -> Vec<NextDirectionItem> {
    let mut items = Vec::new();
    if let Some(open_loop) = open_loops.first() {
        items.push(NextDirectionItem {
            statement: format!(
                "Start with the most actionable open item: {}",
                claim_statement(open_loop)
            ),
            confidence: open_loop.confidence,
            evidence_refs: open_loop
                .evidence
                .iter()
                .map(|item| item.evidence_ref.clone())
                .collect(),
            basis: actionable_basis(open_loop, risks),
        });
    } else if let Some(risk) = risks.first() {
        items.push(NextDirectionItem {
            statement: format!(
                "Close the concrete safety gap behind this risk: {}",
                claim_statement(risk)
            ),
            confidence: risk.confidence,
            evidence_refs: risk
                .evidence
                .iter()
                .map(|item| item.evidence_ref.clone())
                .collect(),
            basis: "Highest-ranked risk with no stronger unresolved work item".to_string(),
        });
    }

    if let Some(decision) = decisions.first() {
        items.push(NextDirectionItem {
            statement: format!(
                "Keep this constraint in place while changing the code: {}",
                claim_statement(decision)
            ),
            confidence: decision.confidence,
            evidence_refs: decision
                .evidence
                .iter()
                .map(|item| item.evidence_ref.clone())
                .collect(),
            basis: "Relevant prior decision".to_string(),
        });
    } else if let Some(preference) = preferences.first() {
        items.push(NextDirectionItem {
            statement: format!(
                "Maintain the strongest observed preference here: {}",
                claim_statement(preference)
            ),
            confidence: preference.confidence,
            evidence_refs: preference
                .evidence
                .iter()
                .map(|item| item.evidence_ref.clone())
                .collect(),
            basis: "Relevant observed preference".to_string(),
        });
    }

    if open_loops.len() > 1 {
        let secondary = &open_loops[1];
        items.push(NextDirectionItem {
            statement: format!("Then follow up on: {}", claim_statement(secondary)),
            confidence: secondary.confidence * 0.95,
            evidence_refs: secondary
                .evidence
                .iter()
                .map(|item| item.evidence_ref.clone())
                .collect(),
            basis: "Secondary ranked unresolved item".to_string(),
        });
    }

    dedupe_next_directions(items)
}

fn rank_flat_memories(
    claims: &[MemoryClaimRecord],
    ranking: &RankingContext<'_>,
) -> Vec<RecallRelevantMemoryItem> {
    let mut ranked = rank_claims(claims, "decision", ranking);
    ranked.extend(rank_claims(claims, "preference", ranking));
    ranked.extend(rank_claims(claims, "open_loop", ranking));
    ranked.extend(rank_claims(claims, "risk", ranking));
    ranked.sort_by(|a, b| b.score.cmp(&a.score));

    ranked
        .into_iter()
        .map(|item| {
            let claim = item.claim;
            let memory_type = claim.claim_type.clone();
            let statement = claim_statement(&claim).to_string();
            let scope = claim.scope_type.clone();
            let confidence = claim.confidence;
            let evidence_refs = claim
                .evidence
                .iter()
                .map(|item| item.evidence_ref.clone())
                .collect();
            let basis = basis_from_claim(&claim);
            RecallRelevantMemoryItem {
                memory_type,
                statement,
                scope,
                confidence,
                evidence_refs,
                basis,
            }
        })
        .collect()
}

fn dedupe_claims(claims: Vec<MemoryClaimRecord>) -> Vec<MemoryClaimRecord> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for claim in claims {
        if seen.insert(claim.claim_id.clone()) {
            result.push(claim);
        }
    }
    result
}

fn select_ranked_claims(
    ranked: &[RankedClaim],
    max_items: usize,
    suppressed_statements: &[String],
) -> Vec<MemoryClaimRecord> {
    let mut selected = Vec::new();
    for ranked_claim in ranked {
        if claim_is_suppressed(&selected, &ranked_claim.claim, suppressed_statements) {
            continue;
        }
        selected.push(ranked_claim.claim.clone());
        if selected.len() >= max_items {
            break;
        }
    }
    selected
}

fn fill_with_candidate_claims(
    mut accepted: Vec<MemoryClaimRecord>,
    ranked_candidates: &[RankedClaim],
    max_items: usize,
    suppressed_statements: &[String],
) -> Vec<MemoryClaimRecord> {
    if accepted.len() >= max_items {
        return accepted;
    }
    for ranked_claim in ranked_candidates {
        if claim_is_suppressed(&accepted, &ranked_claim.claim, suppressed_statements) {
            continue;
        }
        accepted.push(ranked_claim.claim.clone());
        if accepted.len() >= max_items {
            break;
        }
    }
    accepted
}

fn claim_is_suppressed(
    selected: &[MemoryClaimRecord],
    candidate: &MemoryClaimRecord,
    suppressed_statements: &[String],
) -> bool {
    let statement = claim_statement(candidate);
    if suppressed_statements.iter().any(|existing| {
        statements_materially_overlap(existing, statement)
            || statements_conflict(existing, statement)
    }) {
        return true;
    }

    selected
        .iter()
        .any(|existing| claims_redundant(existing, candidate))
}

fn claims_redundant(left: &MemoryClaimRecord, right: &MemoryClaimRecord) -> bool {
    let left_statement = claim_statement(left);
    let right_statement = claim_statement(right);

    if statements_conflict(left_statement, right_statement) {
        return true;
    }

    if left.claim_type == right.claim_type {
        if left.subject_key == right.subject_key {
            return true;
        }
        if statements_materially_overlap(left_statement, right_statement) {
            return true;
        }
        let core_overlap = core_statement_overlap(left_statement, right_statement);
        if core_overlap >= 0.5 && claims_share_primary_evidence(left, right) {
            return true;
        }
        if core_overlap >= 0.75 {
            return true;
        }
    }

    false
}

fn claims_share_primary_evidence(left: &MemoryClaimRecord, right: &MemoryClaimRecord) -> bool {
    let left_evidence = left
        .evidence
        .iter()
        .map(|item| item.evidence_ref.as_str())
        .collect::<HashSet<_>>();
    right
        .evidence
        .iter()
        .any(|item| left_evidence.contains(item.evidence_ref.as_str()))
}

fn prune_redundant_risks(
    risks: Vec<MemoryClaimRecord>,
    open_loops: &[MemoryClaimRecord],
) -> Vec<MemoryClaimRecord> {
    let mut selected = Vec::new();
    for risk in risks {
        let overlaps_open_loop = open_loops.iter().any(|open_loop| {
            let statement_overlap =
                core_statement_overlap(claim_statement(open_loop), claim_statement(&risk)) >= 0.5;
            let shared_evidence = claims_share_primary_evidence(open_loop, &risk);
            statement_overlap && shared_evidence
        });
        if overlaps_open_loop && risk.status == "candidate" {
            continue;
        }
        if selected
            .iter()
            .any(|existing| claims_redundant(existing, &risk))
        {
            continue;
        }
        selected.push(risk);
        if selected.len() >= MAX_GUIDANCE_ITEMS {
            break;
        }
    }
    selected
}

fn determine_claim_state(
    claim_type: &str,
    scope_type: &str,
    explicit_host_signal: bool,
    signal_count: u32,
    supporting_artifact_evidence: bool,
    deterministic_evidence: bool,
    distinct_workspace_count: usize,
) -> (&'static str, f64) {
    match claim_type {
        "decision" => {
            if signal_count >= 2 || (explicit_host_signal && supporting_artifact_evidence) {
                ("accepted", 0.9)
            } else {
                ("candidate", 0.68)
            }
        }
        "open_loop" => {
            if signal_count >= 2 {
                ("accepted", 0.94)
            } else if explicit_host_signal && supporting_artifact_evidence {
                ("accepted", 0.86)
            } else {
                ("candidate", 0.72)
            }
        }
        "preference" => {
            if scope_type == "user_global" {
                if distinct_workspace_count >= 2 {
                    ("accepted", 0.84)
                } else {
                    ("candidate", 0.58)
                }
            } else if signal_count >= 2 {
                ("accepted", 0.9)
            } else if explicit_host_signal && supporting_artifact_evidence {
                ("accepted", 0.82)
            } else {
                ("candidate", 0.64)
            }
        }
        "risk" => {
            if signal_count >= 2 || (explicit_host_signal && deterministic_evidence) {
                ("accepted", if signal_count > 1 { 0.9 } else { 0.84 })
            } else {
                ("candidate", 0.6)
            }
        }
        _ => ("candidate", 0.45),
    }
}

fn claim_strength_components(
    status: &str,
    explicit_host_signal: bool,
    supporting_artifact_evidence: bool,
    deterministic_evidence: bool,
    signal_count: u32,
) -> i64 {
    let mut score = 0;
    if status == "accepted" {
        score += 100;
    }
    if explicit_host_signal {
        score += 12;
    }
    if supporting_artifact_evidence {
        score += 10;
    }
    if deterministic_evidence {
        score += 10;
    }
    score + i64::from(signal_count.min(4)) * 8
}

fn claim_strength(claim: &MemoryClaimRecord) -> i64 {
    claim_strength_components(
        &claim.status,
        claim
            .evidence_quality
            .get("explicit_host_signal")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        claim
            .evidence_quality
            .get("supporting_artifact_evidence")
            .and_then(Value::as_bool)
            .unwrap_or_else(|| {
                claim.evidence.iter().any(|item| {
                    item.evidence_kind == "artifact" || !item.evidence_ref.starts_with("event:")
                })
            }),
        claim
            .evidence_quality
            .get("deterministic_evidence")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        signal_count(claim),
    )
}

fn category_bonus(claim_type: &str) -> i64 {
    match claim_type {
        "open_loop" => 24,
        "risk" => 18,
        "decision" => 14,
        "preference" => 8,
        _ => 0,
    }
}

fn explicitness_bonus(claim: &MemoryClaimRecord) -> i64 {
    let explicit = claim
        .evidence_quality
        .get("explicit_host_signal")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !explicit {
        return 0;
    }
    match claim.claim_type.as_str() {
        "open_loop" => 16,
        "decision" => 12,
        "risk" => 10,
        "preference" => 8,
        _ => 0,
    }
}

fn actionability_bonus(claim: &MemoryClaimRecord) -> i64 {
    if claim.claim_type != "open_loop" {
        return 0;
    }
    let normalized = normalize_key(claim_statement(claim));
    let mut bonus = 0;
    for needle in [
        "coverage",
        "test",
        "timeout",
        "path",
        "example",
        "intro",
        "rollback",
        "safeguard",
    ] {
        if normalized.contains(needle) {
            bonus += 4;
        }
    }
    bonus
}

fn intent_alignment_bonus(claim: &MemoryClaimRecord, intent_mode: PromptIntentMode) -> i64 {
    let normalized = normalize_key(claim_statement(claim));
    match intent_mode {
        PromptIntentMode::ValidationProof => {
            let mut bonus = 0;
            if claim.claim_type == "open_loop" {
                bonus += match_intent_terms(
                    &normalized,
                    &[
                        "validate",
                        "validation",
                        "benchmark",
                        "evaluation",
                        "prove",
                        "proof",
                        "baseline",
                        "treatment",
                        "dataset",
                        "judge",
                        "acceptance",
                        "restatement",
                        "experiment",
                        "real_use",
                        "real_task",
                        "harness",
                    ],
                    8,
                );
            }
            if claim.claim_type == "decision" {
                bonus += match_intent_terms(
                    &normalized,
                    &[
                        "validate",
                        "validation",
                        "benchmark",
                        "evaluation",
                        "prove",
                        "proof",
                        "baseline",
                        "treatment",
                        "real_use",
                        "real_task",
                        "harness",
                        "dataset",
                    ],
                    7,
                );
            }
            if claim.claim_type == "risk" {
                bonus += match_intent_terms(
                    &normalized,
                    &[
                        "prove",
                        "proof",
                        "benchmark",
                        "evaluation",
                        "real_use",
                        "usefulness",
                    ],
                    4,
                );
            }
            bonus
        }
        PromptIntentMode::Implementation => {
            if claim.claim_type == "open_loop" {
                match_intent_terms(
                    &normalized,
                    &[
                        "implement",
                        "build",
                        "wire",
                        "add",
                        "support",
                        "ship",
                        "command",
                        "route",
                        "module",
                    ],
                    5,
                )
            } else {
                0
            }
        }
        PromptIntentMode::Cleanup => {
            if claim.claim_type == "open_loop" || claim.claim_type == "decision" {
                match_intent_terms(
                    &normalized,
                    &[
                        "cleanup",
                        "polish",
                        "dedup",
                        "consolidation",
                        "normalize",
                        "refactor",
                        "cleanup_debt",
                        "phrasing",
                    ],
                    5,
                )
            } else {
                0
            }
        }
        PromptIntentMode::Debugging => {
            if claim.claim_type == "open_loop" || claim.claim_type == "risk" {
                match_intent_terms(
                    &normalized,
                    &[
                        "bug",
                        "debug",
                        "failure",
                        "fails",
                        "failing",
                        "regression",
                        "error",
                        "timeout",
                        "crash",
                    ],
                    6,
                )
            } else {
                0
            }
        }
        PromptIntentMode::Research => match_intent_terms(
            &normalized,
            &[
                "research",
                "investigate",
                "explore",
                "compare",
                "understand",
                "survey",
            ],
            4,
        ),
        PromptIntentMode::General => 0,
    }
}

fn match_intent_terms(normalized: &str, needles: &[&str], weight: i64) -> i64 {
    needles
        .iter()
        .filter(|needle| normalized.contains(**needle))
        .count() as i64
        * weight
}

fn classify_prompt_intent(prompt: &str) -> PromptIntentMode {
    let normalized = normalize_key(prompt);
    if [
        "validate",
        "validation",
        "benchmark",
        "evaluation",
        "prove",
        "proof",
        "baseline",
        "treatment",
        "real_use",
        "real_task",
        "experiment",
        "harness",
        "acceptance",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
    {
        return PromptIntentMode::ValidationProof;
    }
    if [
        "debug",
        "debugging",
        "bug",
        "fix",
        "failure",
        "error",
        "regression",
        "broken",
        "crash",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
    {
        return PromptIntentMode::Debugging;
    }
    if [
        "cleanup",
        "polish",
        "dedup",
        "consolidate",
        "normalize",
        "refactor",
        "tidy",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
    {
        return PromptIntentMode::Cleanup;
    }
    if [
        "research",
        "investigate",
        "explore",
        "compare",
        "understand",
        "study",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
    {
        return PromptIntentMode::Research;
    }
    if [
        "implement",
        "build",
        "add",
        "wire",
        "support",
        "create",
        "ship",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
    {
        return PromptIntentMode::Implementation;
    }
    PromptIntentMode::General
}

fn file_overlap_count(claim: &MemoryClaimRecord, files_in_focus: &[String]) -> usize {
    if files_in_focus.is_empty() {
        return 0;
    }
    let claim_files = claim
        .context_predicates
        .get("files_touched")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let file_set = files_in_focus
        .iter()
        .map(|item| normalize_key(item))
        .collect::<HashSet<_>>();
    claim_files
        .iter()
        .filter_map(Value::as_str)
        .map(normalize_key)
        .filter(|item| file_set.contains(item))
        .count()
}

fn evidence_bonus(claim: &MemoryClaimRecord) -> i64 {
    let mut bonus = 0;
    let mut has_test = false;
    let mut has_incident = false;
    let mut has_diff = false;
    for evidence in &claim.evidence {
        let normalized = normalize_key(&evidence.evidence_ref);
        if evidence.evidence_ref.starts_with("test:") {
            has_test = true;
        }
        if normalized.contains("incident") || normalized.contains("near_miss") {
            has_incident = true;
        }
        if evidence.evidence_ref.starts_with("diff:") {
            has_diff = true;
        }
    }
    if has_test {
        bonus += 8;
    }
    if has_incident {
        bonus += 8;
    }
    if has_diff {
        bonus += 2;
    }
    bonus
}

fn recency_bonus(claim: &MemoryClaimRecord, now: chrono::DateTime<Utc>) -> i64 {
    let age = now.signed_duration_since(claim.updated_at);
    if age < chrono::Duration::days(1) {
        8
    } else if age < chrono::Duration::days(7) {
        5
    } else if age < chrono::Duration::days(30) {
        2
    } else {
        0
    }
}

fn signal_count(claim: &MemoryClaimRecord) -> u32 {
    claim
        .evidence_quality
        .get("signal_count")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(1)
}

fn has_supporting_artifact_evidence(evidence: &[MemoryClaimEvidenceInput]) -> bool {
    evidence
        .iter()
        .any(|item| item.evidence_kind == "artifact" || !item.evidence_ref.starts_with("event:"))
}

fn has_strong_artifact_evidence(evidence: &[MemoryClaimEvidenceInput]) -> bool {
    evidence.iter().any(|item| {
        item.evidence_ref.starts_with("doc:")
            || item.evidence_ref.starts_with("issue:")
            || item.evidence_ref.starts_with("test:")
            || item.evidence_ref.starts_with("diff:")
    })
}

fn derive_subject_key(claim_type: &str, statement: &str) -> String {
    let normalized = normalize_key(statement);
    let filtered = normalized
        .split('_')
        .filter(|token| {
            !matches!(
                *token,
                "prefer"
                    | "keep"
                    | "avoid"
                    | "use"
                    | "with"
                    | "without"
                    | "not"
                    | "include"
                    | "requires"
                    | "should"
                    | "still"
                    | "need"
                    | "needs"
                    | "must"
                    | "added"
                    | "add"
                    | "remove"
                    | "removed"
            )
        })
        .collect::<Vec<_>>();
    if filtered.is_empty() {
        format!("{claim_type}:{normalized}")
    } else {
        format!("{claim_type}:{}", filtered.join("_"))
    }
}

fn claim_statement<'a>(claim: &'a MemoryClaimRecord) -> &'a str {
    claim
        .value
        .get("statement")
        .and_then(Value::as_str)
        .unwrap_or("")
}

async fn distinct_workspace_scope_count<S: StorageProvider + ?Sized>(
    storage: &S,
    statement_key: &str,
) -> Result<usize, AppError> {
    let claims = storage
        .list_memory_claims(MemoryClaimQuery {
            scope_type: Some("workspace".to_string()),
            scope_key: None,
            statuses: vec!["candidate".to_string(), "accepted".to_string()],
            claim_types: vec!["preference".to_string()],
            limit: Some(500),
        })
        .await?;
    let distinct = claims
        .into_iter()
        .filter(|claim| normalize_key(claim_statement(claim)) == statement_key)
        .map(|claim| claim.scope_key)
        .collect::<BTreeSet<_>>();
    Ok(distinct.len())
}

async fn supersede_claim<S: StorageProvider + ?Sized>(
    storage: &S,
    claim: &MemoryClaimRecord,
    superseded_by_ref: &str,
    reason: &str,
) -> Result<(), AppError> {
    let mut evidence_quality = claim.evidence_quality.clone();
    evidence_quality["superseded_by_ref"] = json!(superseded_by_ref);
    evidence_quality["supersession_reason"] = json!(reason);
    storage
        .upsert_memory_claim(MemoryClaimUpsertInput {
            claim_id: Some(claim.claim_id.clone()),
            claim_type: claim.claim_type.clone(),
            claim_key: claim.claim_key.clone(),
            scope_type: claim.scope_type.clone(),
            scope_key: claim.scope_key.clone(),
            subject_key: claim.subject_key.clone(),
            status: "superseded".to_string(),
            created_by: claim.created_by.clone(),
            confidence: claim.confidence.min(0.7),
            value_json: claim.value.clone(),
            context_predicates_json: claim.context_predicates.clone(),
            time_start: claim.time_start,
            time_end: claim.time_end,
            evidence_quality_json: evidence_quality,
            promotion_ref: claim.promotion_ref.clone(),
            evidence: claim
                .evidence
                .iter()
                .map(|item| MemoryClaimEvidenceInput {
                    evidence_ref: item.evidence_ref.clone(),
                    evidence_kind: item.evidence_kind.clone(),
                    locator_json: item.locator.clone(),
                })
                .collect(),
        })
        .await?;
    Ok(())
}

async fn promote_user_global_preference<S: StorageProvider + ?Sized>(
    storage: &S,
    claim: &MemoryClaimRecord,
    promotion_event_ref: &str,
) -> Result<(), AppError> {
    let statement = claim_statement(claim);
    let statement_key = normalize_key(statement);
    let distinct_workspace_count =
        distinct_workspace_scope_count(storage, statement_key.as_str()).await?;
    if distinct_workspace_count < 2 {
        return Ok(());
    }

    upsert_memory_statement(
        storage,
        "preference",
        "user_global",
        "user:root_owner",
        statement,
        &claim.value,
        &claim.context_predicates,
        claim
            .evidence
            .iter()
            .map(|item| MemoryClaimEvidenceInput {
                evidence_ref: item.evidence_ref.clone(),
                evidence_kind: item.evidence_kind.clone(),
                locator_json: item.locator.clone(),
            })
            .collect(),
        true,
        promotion_event_ref,
    )
    .await?;

    Ok(())
}

async fn resolve_stale_open_loops<S: StorageProvider + ?Sized>(
    storage: &S,
    scope_type: &str,
    scope_key: &str,
    summary: &str,
    decisions: &[WorkEpisodeDecision],
    tests: &[WorkEpisodeTestResult],
    current_unresolved_items: &[String],
    promotion_event_ref: &str,
) -> Result<(), AppError> {
    if !has_resolution_language(summary, decisions) {
        return Ok(());
    }

    let claims = storage
        .list_memory_claims(MemoryClaimQuery {
            scope_type: Some(scope_type.to_string()),
            scope_key: Some(scope_key.to_string()),
            statuses: vec!["candidate".to_string(), "accepted".to_string()],
            claim_types: vec!["open_loop".to_string()],
            limit: Some(100),
        })
        .await?;

    for claim in claims {
        let statement = claim_statement(&claim);
        if current_unresolved_items
            .iter()
            .any(|item| statements_materially_overlap(item, statement))
        {
            continue;
        }
        if open_loop_appears_resolved(statement, summary, decisions, tests) {
            supersede_claim(
                storage,
                &claim,
                promotion_event_ref,
                "resolved_by_newer_episode",
            )
            .await?;
        }
    }

    Ok(())
}

fn conflict_uncertainties(
    candidate_claims: &[MemoryClaimRecord],
    decisions: &[MemoryClaimRecord],
    preferences: &[MemoryClaimRecord],
    open_loops: &[MemoryClaimRecord],
    risks: &[MemoryClaimRecord],
) -> Vec<String> {
    let selected = decisions
        .iter()
        .chain(preferences.iter())
        .chain(open_loops.iter())
        .chain(risks.iter())
        .map(|claim| claim_statement(claim).to_string())
        .collect::<Vec<_>>();
    let mut uncertainties = Vec::new();
    for claim in candidate_claims {
        let conflict_state = claim
            .evidence_quality
            .get("conflict_state")
            .and_then(Value::as_str)
            .unwrap_or("clear");
        if conflict_state != "pending" {
            continue;
        }
        if selected
            .iter()
            .any(|statement| statements_conflict(statement, claim_statement(claim)))
        {
            uncertainties.push(format!(
                "Conflicting {} memory was withheld pending stronger evidence: {}",
                claim.claim_type,
                claim_statement(claim)
            ));
        }
        if uncertainties.len() >= MAX_GUIDANCE_ITEMS {
            break;
        }
    }
    uncertainties
}

fn has_resolution_language(summary: &str, decisions: &[WorkEpisodeDecision]) -> bool {
    let combined = format!(
        "{} {}",
        summary,
        decisions
            .iter()
            .map(|item| item.decision.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    );
    let normalized = normalize_key(&combined);
    [
        "fixed",
        "resolved",
        "closed",
        "added",
        "covered",
        "completed",
        "addressed",
        "implemented",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn open_loop_appears_resolved(
    statement: &str,
    summary: &str,
    decisions: &[WorkEpisodeDecision],
    tests: &[WorkEpisodeTestResult],
) -> bool {
    let combined = format!(
        "{} {} {}",
        summary,
        decisions
            .iter()
            .map(|item| item.decision.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        tests
            .iter()
            .filter(|item| item.status == "pass")
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    );
    let loop_tokens = build_query_tokens(statement, &[]);
    let combined_tokens = build_query_tokens(&combined, &[]);
    let token_set = combined_tokens.into_iter().collect::<HashSet<_>>();
    let overlap = loop_tokens
        .iter()
        .filter(|token| token_set.contains(token.as_str()))
        .count();
    overlap >= 2
}

fn select_task_focus(
    current_task_prompt: &str,
    derived_task_scope: Option<&str>,
    files_in_focus: &[String],
    query_tokens: &[String],
    intent_mode: PromptIntentMode,
    recent_episodes: &[adesh_contracts::WorkEpisodeResponse],
    search_hits: &[adesh_contracts::SearchDocumentHit],
) -> String {
    let file_set = files_in_focus
        .iter()
        .map(|item| normalize_key(item))
        .collect::<HashSet<_>>();
    let token_set = query_tokens.iter().cloned().collect::<HashSet<_>>();

    let mut best: Option<(i64, &adesh_contracts::WorkEpisodeResponse)> = None;
    for episode in recent_episodes {
        let mut score = 0;
        let task_scope_match = derived_task_scope
            .map(|scope| episode.task_scope_key.as_deref() == Some(scope))
            .unwrap_or(false);
        if task_scope_match {
            score += 30;
        }
        let overlap =
            build_query_tokens(&format!("{} {}", episode.task_prompt, episode.summary), &[])
                .into_iter()
                .filter(|token| token_set.contains(token))
                .count() as i64;
        score += overlap * 8;
        let file_overlap = episode
            .files_touched
            .iter()
            .map(|item| normalize_key(item))
            .filter(|item| file_set.contains(item))
            .count() as i64;
        score += file_overlap * 20;
        score += task_focus_intent_bonus(&episode.task_prompt, &episode.summary, intent_mode);
        if Utc::now().signed_duration_since(episode.created_at) < chrono::Duration::days(1) {
            score += 6;
        }
        let strong_overlap = match intent_mode {
            PromptIntentMode::ValidationProof => overlap >= 6 && file_overlap >= 2,
            _ => file_overlap >= 2 && overlap >= 4,
        };
        let required_score = match intent_mode {
            PromptIntentMode::ValidationProof => 100,
            _ => 72,
        };
        if score >= required_score && strong_overlap {
            match best {
                Some((best_score, _)) if best_score >= score => {}
                _ => best = Some((score, episode)),
            }
        }
    }

    if let Some((_, episode)) = best {
        return episode.task_prompt.clone();
    }

    if let Some(hit) = search_hits.first() {
        if let Some(title) = hit.title.as_ref() {
            let overlap = build_query_tokens(title, &[])
                .into_iter()
                .filter(|token| token_set.contains(token))
                .count();
            let required_overlap = match intent_mode {
                PromptIntentMode::ValidationProof => 6,
                _ => 4,
            };
            let intent_ok = match intent_mode {
                PromptIntentMode::ValidationProof => {
                    task_focus_intent_bonus(title, "", intent_mode) >= 10
                }
                _ => true,
            };
            if overlap >= required_overlap && intent_ok {
                return title.clone();
            }
        }
    }

    current_task_prompt.to_string()
}

fn task_focus_intent_bonus(task_prompt: &str, summary: &str, intent_mode: PromptIntentMode) -> i64 {
    match intent_mode {
        PromptIntentMode::ValidationProof => {
            let combined = normalize_key(&format!("{task_prompt} {summary}"));
            match_intent_terms(
                &combined,
                &[
                    "validate",
                    "validation",
                    "benchmark",
                    "evaluation",
                    "prove",
                    "proof",
                    "baseline",
                    "treatment",
                    "real_use",
                    "real_task",
                    "harness",
                    "dataset",
                    "experiment",
                ],
                5,
            )
        }
        _ => 0,
    }
}

fn actionable_basis(open_loop: &MemoryClaimRecord, risks: &[MemoryClaimRecord]) -> String {
    let statement = claim_statement(open_loop);
    if risks
        .iter()
        .any(|risk| statements_materially_overlap(claim_statement(risk), statement))
    {
        "Top-ranked unresolved item reinforced by matching risk evidence".to_string()
    } else {
        "Top-ranked unresolved item with the strongest current evidence".to_string()
    }
}

fn dedupe_next_directions(items: Vec<NextDirectionItem>) -> Vec<NextDirectionItem> {
    let mut selected = Vec::new();
    let mut seen = Vec::new();
    for item in items {
        if seen
            .iter()
            .any(|existing: &String| statements_materially_overlap(existing, &item.statement))
        {
            continue;
        }
        seen.push(item.statement.clone());
        selected.push(item);
        if selected.len() >= MAX_GUIDANCE_ITEMS {
            break;
        }
    }
    selected
}

fn statements_conflict(left: &str, right: &str) -> bool {
    let left_polarity = statement_polarity(left);
    let right_polarity = statement_polarity(right);
    if !matches!(
        (left_polarity, right_polarity),
        (StatementPolarity::Positive, StatementPolarity::Negative)
            | (StatementPolarity::Negative, StatementPolarity::Positive)
    ) {
        return false;
    }

    let left_norm = normalize_key(left);
    let right_norm = normalize_key(right);
    let left_tokens = left_norm
        .split('_')
        .filter(|token| token.len() >= 3)
        .collect::<HashSet<_>>();
    let right_tokens = right_norm
        .split('_')
        .filter(|token| token.len() >= 3)
        .collect::<HashSet<_>>();
    let overlap = left_tokens.intersection(&right_tokens).count();

    statements_materially_overlap(left, right) || overlap >= 3
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatementPolarity {
    Positive,
    Negative,
    Neutral,
}

fn statement_polarity(statement: &str) -> StatementPolarity {
    let normalized = normalize_key(statement);
    if [
        "avoid",
        "without",
        "not",
        "never",
        "remove",
        "removed",
        "deprecated",
        "deprecate",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
    {
        StatementPolarity::Negative
    } else if ["prefer", "keep", "use", "include", "lead", "add", "added"]
        .iter()
        .any(|needle| normalized.contains(needle))
    {
        StatementPolarity::Positive
    } else {
        StatementPolarity::Neutral
    }
}

fn statements_materially_overlap(left: &str, right: &str) -> bool {
    let left_norm = normalize_key(left);
    let right_norm = normalize_key(right);
    if left_norm == right_norm || left_norm.contains(&right_norm) || right_norm.contains(&left_norm)
    {
        return true;
    }
    let left_tokens = left_norm
        .split('_')
        .filter(|token| token.len() >= 3)
        .collect::<HashSet<_>>();
    let right_tokens = right_norm
        .split('_')
        .filter(|token| token.len() >= 3)
        .collect::<HashSet<_>>();
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return false;
    }
    let overlap = left_tokens.intersection(&right_tokens).count() as f64;
    let max_len = left_tokens.len().max(right_tokens.len()) as f64;
    overlap / max_len >= 0.6
}

fn core_statement_overlap(left: &str, right: &str) -> f64 {
    let left_tokens = core_statement_tokens(left);
    let right_tokens = core_statement_tokens(right);
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }
    let overlap = left_tokens.intersection(&right_tokens).count() as f64;
    let max_len = left_tokens.len().max(right_tokens.len()) as f64;
    overlap / max_len
}

fn core_statement_tokens(statement: &str) -> HashSet<String> {
    normalize_key(statement)
        .split('_')
        .filter(|token| token.len() >= 3)
        .filter(|token| {
            !matches!(
                *token,
                "the"
                    | "and"
                    | "still"
                    | "after"
                    | "before"
                    | "with"
                    | "without"
                    | "into"
                    | "from"
                    | "this"
                    | "that"
                    | "path"
                    | "flow"
                    | "work"
                    | "item"
                    | "issue"
                    | "logic"
                    | "code"
                    | "test"
                    | "tests"
                    | "fails"
                    | "failing"
                    | "missing"
                    | "needs"
                    | "need"
                    | "added"
                    | "explicit"
                    | "kept"
                    | "keep"
                    | "moved"
                    | "using"
                    | "uses"
                    | "use"
            )
        })
        .map(ToString::to_string)
        .collect()
}

fn basis_from_claim(claim: &MemoryClaimRecord) -> String {
    let signals = signal_count(claim);
    if claim.status == "candidate" {
        return match claim.claim_type.as_str() {
            "decision" => {
                "Candidate decision inferred from summary or repeated sparse signals".to_string()
            }
            "preference" => {
                "Candidate preference inferred from prior work with limited corroboration"
                    .to_string()
            }
            "open_loop" => "Candidate unresolved item inferred from sparse evidence".to_string(),
            "risk" => "Candidate risk inferred from sparse evidence".to_string(),
            _ => "Candidate scoped memory".to_string(),
        };
    }
    match claim.claim_type.as_str() {
        "decision" if signals > 1 => {
            "Repeated explicit decision across related episodes".to_string()
        }
        "decision" => "Explicit decision recorded in a prior work episode".to_string(),
        "preference"
            if claim
                .evidence_quality
                .get("explicit_host_signal")
                .and_then(Value::as_bool)
                .unwrap_or(false) =>
        {
            "Explicit host preference recorded during prior work".to_string()
        }
        "preference" if signals > 1 => {
            "Repeated preference signal across multiple episodes".to_string()
        }
        "preference" => "Candidate preference signal from prior work".to_string(),
        "open_loop" if signals > 1 => {
            "Repeated unresolved item across related episodes".to_string()
        }
        "open_loop" => "Explicit unresolved item recorded in prior work".to_string(),
        "risk" if signals > 1 => "Repeated risk signal across related episodes".to_string(),
        "risk" => "Explicit risk signal from prior work".to_string(),
        _ => "Stored scoped memory".to_string(),
    }
}

fn merge_evidence(
    existing: Option<Vec<adesh_contracts::MemoryClaimEvidence>>,
    new_items: Vec<MemoryClaimEvidenceInput>,
) -> Vec<MemoryClaimEvidenceInput> {
    let mut by_ref = BTreeMap::new();
    if let Some(existing) = existing {
        for item in existing {
            by_ref.insert(
                item.evidence_ref.clone(),
                MemoryClaimEvidenceInput {
                    evidence_ref: item.evidence_ref,
                    evidence_kind: item.evidence_kind,
                    locator_json: item.locator,
                },
            );
        }
    }
    for item in new_items {
        by_ref.insert(item.evidence_ref.clone(), item);
    }
    by_ref.into_values().collect()
}

fn split_summary_sentences(summary: &str) -> Vec<String> {
    summary
        .split(['.', ';'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .take(4)
        .collect()
}

fn looks_like_inferred_decision(sentence: &str) -> bool {
    let normalized = normalize_key(sentence);
    [
        "kept",
        "keep",
        "moved",
        "separate",
        "separated",
        "restored",
        "switched",
        "explicit",
        "service_layer",
        "boundary",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
        && !looks_like_inferred_open_loop(sentence)
}

fn looks_like_inferred_open_loop(sentence: &str) -> bool {
    let normalized = normalize_key(sentence);
    if normalized.contains("unresolved_work") && !normalized.contains("still") {
        return false;
    }
    [
        "still",
        "missing",
        "not_proven",
        "not_covered",
        "fails",
        "failing",
        "unresolved",
        "needs",
        "blocked",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn looks_like_inferred_preference(sentence: &str) -> bool {
    let normalized = normalize_key(sentence);
    [
        "prefer",
        "avoid",
        "direct_language",
        "concrete_examples",
        "explicit",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn normalize_key(input: &str) -> String {
    let mut normalized = String::new();
    let mut last_was_sep = false;
    for ch in input.chars().flat_map(|value| value.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            last_was_sep = false;
        } else if !last_was_sep {
            normalized.push('_');
            last_was_sep = true;
        }
    }
    normalized.trim_matches('_').to_string()
}

fn short_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    hash[..12].to_string()
}

fn is_risk_like(statement: &str) -> bool {
    let normalized = normalize_key(statement);
    [
        "risk",
        "fail",
        "duplicate",
        "unsafe",
        "coverage",
        "partial",
        "incident",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn should_derive_risk_from_open_loop(open_loop: &ScopedGuidanceItem) -> bool {
    let normalized = normalize_key(open_loop.statement.as_str());
    if open_loop.confidence >= 0.8 {
        return true;
    }
    normalized.contains("duplicate")
        || normalized.contains("incident")
        || normalized.contains("unsafe")
        || normalized.contains("partial_write")
        || normalized.contains("rollback")
}

fn derive_risk_severity(statement: &str) -> &'static str {
    let normalized = normalize_key(statement);
    if normalized.contains("duplicate")
        || normalized.contains("incident")
        || normalized.contains("unsafe")
        || normalized.contains("partial_write")
    {
        "high"
    } else {
        "medium"
    }
}
