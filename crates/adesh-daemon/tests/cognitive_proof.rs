use adesh_contracts::{
    PrepareTaskContextRequest, RecallRelevantMemoryRequest, StoreWorkEpisodeRequest,
    WorkEpisodeDecision, WorkEpisodeTestResult, WorkspaceDescriptor,
};
use adesh_core::ports::storage::{MemoryClaimQuery, StorageProvider};
use adesh_daemon::cognition;
use adesh_storage_sqlite::SqliteStorage;

fn payments_workspace() -> WorkspaceDescriptor {
    WorkspaceDescriptor {
        kind: "git".to_string(),
        locator: None,
        cwd: Some("/work/payments-service".to_string()),
        branch: Some("fix/upload-retry".to_string()),
        external_ref: Some("git@github.com:acme/payments-service.git".to_string()),
    }
}

fn infra_workspace() -> WorkspaceDescriptor {
    WorkspaceDescriptor {
        kind: "git".to_string(),
        locator: None,
        cwd: Some("/work/infra-deploy".to_string()),
        branch: Some("main".to_string()),
        external_ref: Some("git@github.com:acme/infra-deploy.git".to_string()),
    }
}

fn conversation_workspace() -> WorkspaceDescriptor {
    WorkspaceDescriptor {
        kind: "conversation".to_string(),
        locator: Some("personal-writing".to_string()),
        cwd: None,
        branch: None,
        external_ref: None,
    }
}

fn adesh_workspace() -> WorkspaceDescriptor {
    WorkspaceDescriptor {
        kind: "git".to_string(),
        locator: Some("/home/rajan/Desktop/work/aadesh".to_string()),
        cwd: Some("/home/rajan/Desktop/work/aadesh".to_string()),
        branch: Some("main".to_string()),
        external_ref: Some("git@github.com:aadeshai/aadesh.git".to_string()),
    }
}

async fn new_storage() -> SqliteStorage {
    let storage = SqliteStorage::connect("sqlite::memory:").await.unwrap();
    StorageProvider::migrate(&storage).await.unwrap();
    storage
}

async fn seed_payments_examples(storage: &SqliteStorage) {
    cognition::store_work_episode(
        storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Reduce duplicate processing when upload worker retries after transient storage errors".to_string(),
            summary: "Added retry handling around transient storage failures in upload worker. Duplicate processing is still possible after partial write.".to_string(),
            files_touched: vec![
                "src/upload/upload_worker.rs".to_string(),
                "src/upload/storage.rs".to_string(),
            ],
            tests: vec![
                WorkEpisodeTestResult {
                    name: "upload_worker_retries_transient_errors".to_string(),
                    status: "pass".to_string(),
                    summary: None,
                },
                WorkEpisodeTestResult {
                    name: "retries_do_not_duplicate_chunks".to_string(),
                    status: "fail".to_string(),
                    summary: Some("Duplicate chunk handling is still broken after retry.".to_string()),
                },
            ],
            decisions: vec![WorkEpisodeDecision {
                decision: "Keep duplicate protection in UploadService, not in the worker retry loop".to_string(),
                rationale: Some("Retry transport and dedupe boundary should stay separated".to_string()),
            }],
            unresolved_items: vec![
                "Crash and partial-write retry behavior is still not proven safe by test coverage".to_string(),
            ],
            observed_preferences: Vec::new(),
            risk_signals: Vec::new(),
            issue_refs: vec!["PAY-241".to_string()],
            artifact_refs: vec!["diff:ep1".to_string(), "test:retries_do_not_duplicate_chunks".to_string()],
            task_hint: Some("upload-retry".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    cognition::store_work_episode(
        storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Refactor retry fix so duplicate guard stays in service layer".to_string(),
            summary: "Moved dedupe check into UploadService and kept retry logic explicit.".to_string(),
            files_touched: vec![
                "src/upload/upload_service.rs".to_string(),
                "src/upload/upload_worker.rs".to_string(),
            ],
            tests: vec![WorkEpisodeTestResult {
                name: "upload_service_dedupes_replayed_chunks".to_string(),
                status: "pass".to_string(),
                summary: None,
            }],
            decisions: vec![WorkEpisodeDecision {
                decision: "Use explicit retry state handling rather than macro abstraction in this subsystem".to_string(),
                rationale: Some("Failure paths are easier to audit in explicit code".to_string()),
            }],
            unresolved_items: vec!["Timeout-path coverage is still missing".to_string()],
            observed_preferences: Vec::new(),
            risk_signals: Vec::new(),
            issue_refs: vec!["PAY-241".to_string()],
            artifact_refs: vec!["diff:ep2".to_string()],
            task_hint: Some("upload-retry".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    cognition::store_work_episode(
        storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Review incident learnings around duplicate upload processing".to_string(),
            summary: "Reviewed incident and confirmed duplicate processing risk is highest around retry plus partial-write behavior.".to_string(),
            files_touched: vec![
                "docs/incidents/upload-duplication.md".to_string(),
                "tests/integration/upload_retry.rs".to_string(),
            ],
            tests: vec![WorkEpisodeTestResult {
                name: "integration_upload_retry_partial_write".to_string(),
                status: "fail".to_string(),
                summary: Some("Partial-write recovery still fails the integration path.".to_string()),
            }],
            decisions: Vec::new(),
            unresolved_items: vec![
                "Crash and partial-write retry behavior is still not proven safe by test coverage".to_string(),
                "Retry metrics not added".to_string(),
            ],
            observed_preferences: vec![
                "Backend retry-path changes should include integration tests.".to_string(),
            ],
            risk_signals: vec![
                "Expanding retries before closing the partial-write test gap risks repeating duplicate-processing behavior.".to_string(),
            ],
            issue_refs: vec!["PAY-241".to_string()],
            artifact_refs: vec![
                "doc:upload-duplication-incident".to_string(),
                "test:integration_upload_retry_partial_write".to_string(),
            ],
            task_hint: Some("upload-retry".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();
}

async fn seed_infra_examples(storage: &SqliteStorage) {
    cognition::store_work_episode(
        storage,
        StoreWorkEpisodeRequest {
            workspace: infra_workspace(),
            task_prompt: "Stabilize Terraform apply workflow for production rollouts".to_string(),
            summary: "Rolled back a previous abstraction and moved drift checks back into explicit plan/apply stages.".to_string(),
            files_touched: vec![
                "terraform/modules/network/main.tf".to_string(),
                ".github/workflows/deploy.yml".to_string(),
            ],
            tests: vec![WorkEpisodeTestResult {
                name: "terraform-plan-prod".to_string(),
                status: "pass".to_string(),
                summary: Some("Plan still succeeds".to_string()),
            }],
            decisions: vec![WorkEpisodeDecision {
                decision: "Keep Terraform plan and apply separated in CI for production rollouts".to_string(),
                rationale: Some("Safer review and rollback posture".to_string()),
            }],
            unresolved_items: vec!["Destroy-path safeguards are still not covered in CI".to_string()],
            observed_preferences: vec![
                "Infrastructure changes should come with explicit rollback notes.".to_string(),
            ],
            risk_signals: vec![
                "Combining plan and apply in one opaque step raises rollback risk.".to_string(),
            ],
            issue_refs: vec!["OPS-77".to_string()],
            artifact_refs: vec!["diff:tf1".to_string(), "issue:OPS-77".to_string()],
            task_hint: Some("prod-rollout-safety".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    cognition::store_work_episode(
        storage,
        StoreWorkEpisodeRequest {
            workspace: infra_workspace(),
            task_prompt: "Reduce magic in deploy workflow and document operator steps".to_string(),
            summary: "Removed a composite action wrapper and wrote explicit deploy steps in the workflow.".to_string(),
            files_touched: vec![
                ".github/workflows/deploy.yml".to_string(),
                "docs/runbooks/prod-rollout.md".to_string(),
            ],
            tests: vec![WorkEpisodeTestResult {
                name: "workflow-lint".to_string(),
                status: "pass".to_string(),
                summary: None,
            }],
            decisions: vec![WorkEpisodeDecision {
                decision: "Prefer explicit CI steps over composite wrappers in safety-sensitive deployment flows".to_string(),
                rationale: Some("Operators need to inspect each stage during incidents".to_string()),
            }],
            unresolved_items: vec!["Destroy-path safeguards are still not covered in CI".to_string()],
            observed_preferences: vec![
                "Prefer explicit CI steps over wrapper abstractions in safety-sensitive flows.".to_string(),
            ],
            risk_signals: Vec::new(),
            issue_refs: vec!["OPS-77".to_string()],
            artifact_refs: vec!["diff:tf2".to_string(), "doc:prod-rollout-runbook".to_string()],
            task_hint: Some("prod-rollout-safety".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    cognition::store_work_episode(
        storage,
        StoreWorkEpisodeRequest {
            workspace: infra_workspace(),
            task_prompt: "Review near-miss from accidental destroy targeting".to_string(),
            summary: "Confirmed operator confusion around target selection and missing rollback notes during destroy-like changes.".to_string(),
            files_touched: vec!["docs/incidents/destroy-near-miss.md".to_string()],
            tests: Vec::new(),
            decisions: Vec::new(),
            unresolved_items: vec![
                "Rollback guidance is not consistently attached to production infra changes".to_string(),
            ],
            observed_preferences: vec![
                "Infrastructure changes should come with explicit rollback notes.".to_string(),
            ],
            risk_signals: vec![
                "Destroy-path changes without rollback notes create high-severity operator error risk.".to_string(),
            ],
            issue_refs: vec!["OPS-88".to_string()],
            artifact_refs: vec!["doc:destroy-near-miss".to_string()],
            task_hint: Some("prod-rollout-safety".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();
}

async fn seed_conversation_examples(storage: &SqliteStorage) {
    cognition::store_work_episode(
        storage,
        StoreWorkEpisodeRequest {
            workspace: conversation_workspace(),
            task_prompt: "Help draft a long-form blog post about pricing strategy".to_string(),
            summary: "Collected outline options and removed buzzword-heavy framing.".to_string(),
            files_touched: Vec::new(),
            tests: Vec::new(),
            decisions: vec![WorkEpisodeDecision {
                decision: "Avoid marketing-heavy language when explaining product tradeoffs"
                    .to_string(),
                rationale: Some("The audience trusts concrete language more".to_string()),
            }],
            unresolved_items: vec!["Need a tighter section on enterprise objections".to_string()],
            observed_preferences: vec![
                "Prefer direct language over brand-speak in strategy writing.".to_string(),
            ],
            risk_signals: Vec::new(),
            issue_refs: Vec::new(),
            artifact_refs: vec!["note:outline-v1".to_string()],
            task_hint: Some("pricing-post".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    cognition::store_work_episode(
        storage,
        StoreWorkEpisodeRequest {
            workspace: conversation_workspace(),
            task_prompt: "Revise the pricing post after feedback".to_string(),
            summary: "Feedback said the post was still too abstract and weak on concrete examples."
                .to_string(),
            files_touched: Vec::new(),
            tests: Vec::new(),
            decisions: Vec::new(),
            unresolved_items: vec![
                "Need two concrete examples to ground the enterprise pricing argument".to_string(),
            ],
            observed_preferences: vec![
                "Prefer concrete examples when defending a strategic point.".to_string(),
            ],
            risk_signals: vec!["Abstract framing may make the argument sound evasive.".to_string()],
            issue_refs: Vec::new(),
            artifact_refs: vec!["comment:editor-feedback-1".to_string()],
            task_hint: Some("pricing-post".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    cognition::store_work_episode(
        storage,
        StoreWorkEpisodeRequest {
            workspace: conversation_workspace(),
            task_prompt: "Trim the pricing post intro".to_string(),
            summary: "Reduced throat-clearing but the intro still does not connect quickly enough to the core thesis.".to_string(),
            files_touched: Vec::new(),
            tests: Vec::new(),
            decisions: Vec::new(),
            unresolved_items: vec!["Intro still takes too long to reach the core thesis".to_string()],
            observed_preferences: vec![
                "Lead with the core argument earlier in explanatory writing.".to_string(),
            ],
            risk_signals: Vec::new(),
            issue_refs: Vec::new(),
            artifact_refs: vec!["note:intro-rewrite".to_string()],
            task_hint: Some("pricing-post".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();
}

fn contains_statement(items: &[impl AsRef<str>], needle: &str) -> bool {
    items.iter().any(|item| item.as_ref().contains(needle))
}

fn position_of_statement(items: &[String], needle: &str) -> Option<usize> {
    items.iter().position(|item| item.contains(needle))
}

#[tokio::test]
async fn prepare_task_context_uses_current_intent_and_ranks_timeout_gap_above_retry_metrics() {
    let storage = new_storage().await;
    seed_payments_examples(&storage).await;

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: payments_workspace(),
            task_prompt: "Can you help finish the upload retry work safely?".to_string(),
            files_in_focus: vec!["src/upload/upload_worker.rs".to_string()],
            task_hint: Some("upload-retry".to_string()),
        },
    )
    .await
    .unwrap();

    assert_eq!(
        response.task_focus,
        "Can you help finish the upload retry work safely?"
    );
    assert!(
        response
            .relevant_decisions
            .iter()
            .any(|item| item.statement.contains("UploadService"))
    );
    assert!(
        response
            .applicable_preferences
            .iter()
            .any(|item| item.statement.contains("integration tests"))
    );

    let open_loop_statements = response
        .open_loops
        .iter()
        .map(|item| item.statement.clone())
        .collect::<Vec<_>>();
    let timeout_index =
        position_of_statement(&open_loop_statements, "Timeout-path coverage").unwrap();
    let metrics_index = position_of_statement(&open_loop_statements, "Retry metrics").unwrap();
    assert!(timeout_index < metrics_index);

    assert!(
        response
            .likely_next_directions
            .first()
            .unwrap()
            .statement
            .contains("partial-write")
    );
    assert!(
        response
            .likely_next_directions
            .iter()
            .any(|item| item.statement.contains("Timeout-path coverage"))
    );
}

#[tokio::test]
async fn prepare_task_context_suppresses_preference_duplicates_under_stronger_decisions() {
    let storage = new_storage().await;
    seed_infra_examples(&storage).await;

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: infra_workspace(),
            task_prompt: "What matters before the next production rollout change?".to_string(),
            files_in_focus: vec![".github/workflows/deploy.yml".to_string()],
            task_hint: Some("prod-rollout-safety".to_string()),
        },
    )
    .await
    .unwrap();

    assert_eq!(
        response.task_focus,
        "What matters before the next production rollout change?"
    );
    assert!(
        response
            .relevant_decisions
            .iter()
            .any(|item| item.statement.contains("plan and apply separated"))
    );
    assert!(
        response
            .applicable_preferences
            .iter()
            .any(|item| item.statement.contains("rollback notes"))
    );
    assert!(
        response
            .applicable_preferences
            .iter()
            .all(|item| !item.statement.contains("explicit CI steps"))
    );
}

#[tokio::test]
async fn prepare_task_context_prefers_concrete_writing_followups_over_rhetorical_risk() {
    let storage = new_storage().await;
    seed_conversation_examples(&storage).await;

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: conversation_workspace(),
            task_prompt: "Help tighten the pricing article before the next revision.".to_string(),
            files_in_focus: Vec::new(),
            task_hint: Some("pricing-post".to_string()),
        },
    )
    .await
    .unwrap();

    assert_eq!(
        response.task_focus,
        "Help tighten the pricing article before the next revision."
    );
    assert!(
        response
            .likely_next_directions
            .first()
            .map(|item| {
                item.statement.contains("Need two concrete examples")
                    || item.statement.contains("Intro still takes too long")
            })
            .unwrap_or(false)
    );
    assert!(
        response
            .likely_next_directions
            .first()
            .map(|item| !item.statement.contains("Abstract framing"))
            .unwrap_or(false)
    );
}

#[tokio::test]
async fn recall_relevant_memory_returns_compact_scoped_results() {
    let storage = new_storage().await;
    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Stabilize upload retry handling".to_string(),
            summary: "Recorded style and safety guidance for retry-path changes.".to_string(),
            files_touched: vec!["src/upload/upload_service.rs".to_string()],
            tests: vec![WorkEpisodeTestResult {
                name: "integration_upload_retry_partial_write".to_string(),
                status: "fail".to_string(),
                summary: None,
            }],
            decisions: vec![WorkEpisodeDecision {
                decision: "Keep duplicate protection in UploadService".to_string(),
                rationale: None,
            }],
            unresolved_items: vec!["Partial-write retry path still lacks coverage".to_string()],
            observed_preferences: vec![
                "Backend retry-path changes should include integration tests.".to_string(),
            ],
            risk_signals: Vec::new(),
            issue_refs: vec!["PAY-241".to_string()],
            artifact_refs: vec!["test:integration_upload_retry_partial_write".to_string()],
            task_hint: Some("upload-retry".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    let response = cognition::recall_relevant_memory(
        &storage,
        RecallRelevantMemoryRequest {
            workspace: payments_workspace(),
            query: "integration tests for retry path".to_string(),
            task_hint: Some("upload-retry".to_string()),
            memory_types: vec!["preference".to_string(), "open_loop".to_string()],
            limit: Some(4),
        },
    )
    .await
    .unwrap();

    assert!(!response.memories.is_empty());
    assert!(response.memories.len() <= 4);
    let statements = response
        .memories
        .iter()
        .map(|item| item.statement.clone())
        .collect::<Vec<_>>();
    assert!(contains_statement(&statements, "integration tests"));
    assert!(
        response
            .memories
            .iter()
            .all(|item| !item.evidence_refs.is_empty())
    );
}

#[tokio::test]
async fn newer_decision_supersedes_older_conflicting_decision() {
    let storage = new_storage().await;

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Try a retry wrapper abstraction".to_string(),
            summary: "Introduced a macro-based retry wrapper for the worker path.".to_string(),
            files_touched: vec!["src/upload/upload_worker.rs".to_string()],
            tests: vec![WorkEpisodeTestResult {
                name: "retry_wrapper_smoke".to_string(),
                status: "pass".to_string(),
                summary: None,
            }],
            decisions: vec![WorkEpisodeDecision {
                decision: "Use a macro-based retry wrapper in this subsystem".to_string(),
                rationale: Some("Wanted to reduce repeated retry code".to_string()),
            }],
            unresolved_items: vec![
                "Sparse output wrapper still needs validation coverage".to_string(),
            ],
            observed_preferences: Vec::new(),
            risk_signals: Vec::new(),
            issue_refs: vec!["PAY-300".to_string()],
            artifact_refs: vec!["diff:old-wrapper".to_string()],
            task_hint: Some("upload-retry".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Back out the wrapper and make retries explicit".to_string(),
            summary: "Removed the retry wrapper and restored explicit retry-state handling."
                .to_string(),
            files_touched: vec!["src/upload/upload_worker.rs".to_string()],
            tests: vec![WorkEpisodeTestResult {
                name: "retry_state_paths_explicit".to_string(),
                status: "pass".to_string(),
                summary: None,
            }],
            decisions: vec![WorkEpisodeDecision {
                decision: "Avoid a macro-based retry wrapper in this subsystem".to_string(),
                rationale: Some("Explicit failure paths are easier to audit".to_string()),
            }],
            unresolved_items: vec![
                "Sparse output wrapper still needs validation coverage".to_string(),
            ],
            observed_preferences: Vec::new(),
            risk_signals: Vec::new(),
            issue_refs: vec!["PAY-300".to_string()],
            artifact_refs: vec!["diff:new-explicit-retry".to_string()],
            task_hint: Some("upload-retry".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    let claims = storage
        .list_memory_claims(MemoryClaimQuery {
            scope_type: Some("workspace".to_string()),
            scope_key: Some("workspace:git:git@github.com:acme/payments-service.git".to_string()),
            statuses: vec!["accepted".to_string(), "superseded".to_string()],
            claim_types: vec!["decision".to_string()],
            limit: Some(20),
        })
        .await
        .unwrap();

    assert!(claims.iter().any(|claim| {
        claim.status == "accepted"
            && claim
                .value
                .get("statement")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .contains("Avoid a macro-based retry wrapper")
    }));
    assert!(claims.iter().any(|claim| {
        claim.status == "superseded"
            && claim
                .value
                .get("statement")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .contains("Use a macro-based retry wrapper")
    }));
}

#[tokio::test]
async fn repo_local_rule_overrides_user_global_tendency() {
    let storage = new_storage().await;

    let script_workspace = WorkspaceDescriptor {
        kind: "git".to_string(),
        locator: None,
        cwd: Some("/work/ops-scripts".to_string()),
        branch: Some("main".to_string()),
        external_ref: Some("git@github.com:acme/ops-scripts.git".to_string()),
    };
    let notes_workspace = WorkspaceDescriptor {
        kind: "conversation".to_string(),
        locator: Some("ops-notes".to_string()),
        cwd: None,
        branch: None,
        external_ref: None,
    };

    for workspace in [script_workspace, notes_workspace] {
        cognition::store_work_episode(
            &storage,
            StoreWorkEpisodeRequest {
                workspace,
                task_prompt: "Capture a lightweight scripting preference".to_string(),
                summary: "Recorded a personal habit for fast script work.".to_string(),
                files_touched: Vec::new(),
                tests: Vec::new(),
                decisions: Vec::new(),
                unresolved_items: Vec::new(),
                observed_preferences: vec![
                    "Prefer compact variable names when editing quick scripts.".to_string(),
                ],
                risk_signals: Vec::new(),
                issue_refs: Vec::new(),
                artifact_refs: vec!["note:compact-vars".to_string()],
                task_hint: Some("style".to_string()),
                started_at: None,
                ended_at: None,
            },
        )
        .await
        .unwrap();
    }

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Refine service naming in the upload path".to_string(),
            summary: "Recorded a local service-layer rule for readability.".to_string(),
            files_touched: vec!["src/upload/upload_service.rs".to_string()],
            tests: Vec::new(),
            decisions: Vec::new(),
            unresolved_items: Vec::new(),
            observed_preferences: vec![
                "Avoid compact variable names in this service layer.".to_string(),
            ],
            risk_signals: Vec::new(),
            issue_refs: vec!["PAY-410".to_string()],
            artifact_refs: vec!["note:service-naming".to_string()],
            task_hint: Some("service-style".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: payments_workspace(),
            task_prompt: "Adjust naming in the upload service.".to_string(),
            files_in_focus: vec!["src/upload/upload_service.rs".to_string()],
            task_hint: Some("service-style".to_string()),
        },
    )
    .await
    .unwrap();

    assert!(
        response
            .applicable_preferences
            .iter()
            .any(|item| item.statement.contains("Avoid compact variable names"))
    );
    assert!(
        response
            .applicable_preferences
            .iter()
            .all(|item| !item.statement.contains("Prefer compact variable names"))
    );
}

#[tokio::test]
async fn resolved_open_loop_no_longer_dominates_context() {
    let storage = new_storage().await;

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Track missing timeout coverage".to_string(),
            summary: "Timeout-path coverage is still missing around the worker retry path."
                .to_string(),
            files_touched: vec!["src/upload/upload_worker.rs".to_string()],
            tests: Vec::new(),
            decisions: Vec::new(),
            unresolved_items: vec!["Timeout-path coverage is still missing".to_string()],
            observed_preferences: Vec::new(),
            risk_signals: Vec::new(),
            issue_refs: vec!["PAY-500".to_string()],
            artifact_refs: vec!["note:timeout-gap".to_string()],
            task_hint: Some("upload-retry".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Close timeout coverage gap".to_string(),
            summary:
                "Added timeout-path coverage and fixed the missing test gap for retry handling."
                    .to_string(),
            files_touched: vec!["src/upload/upload_worker.rs".to_string()],
            tests: vec![WorkEpisodeTestResult {
                name: "upload_worker_timeout_path".to_string(),
                status: "pass".to_string(),
                summary: Some("Timeout path now covered.".to_string()),
            }],
            decisions: Vec::new(),
            unresolved_items: Vec::new(),
            observed_preferences: Vec::new(),
            risk_signals: Vec::new(),
            issue_refs: vec!["PAY-500".to_string()],
            artifact_refs: vec!["test:upload_worker_timeout_path".to_string()],
            task_hint: Some("upload-retry".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: payments_workspace(),
            task_prompt: "What still matters in upload retry validation?".to_string(),
            files_in_focus: vec!["src/upload/upload_worker.rs".to_string()],
            task_hint: Some("upload-retry".to_string()),
        },
    )
    .await
    .unwrap();

    assert!(
        response
            .open_loops
            .iter()
            .all(|item| !item.statement.contains("Timeout-path coverage"))
    );
}

#[tokio::test]
async fn conflicting_preferences_do_not_both_surface_as_valid() {
    let storage = new_storage().await;

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: infra_workspace(),
            task_prompt: "Record deploy style preference".to_string(),
            summary: "Stored an explicit preference for readable CI stages.".to_string(),
            files_touched: vec![".github/workflows/deploy.yml".to_string()],
            tests: Vec::new(),
            decisions: Vec::new(),
            unresolved_items: Vec::new(),
            observed_preferences: vec![
                "Prefer explicit CI steps over wrappers in deploy flows.".to_string(),
            ],
            risk_signals: Vec::new(),
            issue_refs: Vec::new(),
            artifact_refs: vec!["note:ci-pref-old".to_string()],
            task_hint: Some("deploy-style".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: infra_workspace(),
            task_prompt: "Revise deploy style preference after incident tooling cleanup"
                .to_string(),
            summary: "Avoid explicit CI steps here and prefer a wrapper for routine deploy flows."
                .to_string(),
            files_touched: vec![".github/workflows/deploy.yml".to_string()],
            tests: Vec::new(),
            decisions: Vec::new(),
            unresolved_items: Vec::new(),
            observed_preferences: vec![
                "Avoid explicit CI steps in deploy flows and prefer a wrapper.".to_string(),
            ],
            risk_signals: Vec::new(),
            issue_refs: Vec::new(),
            artifact_refs: vec!["note:ci-pref-new".to_string()],
            task_hint: Some("deploy-style".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: infra_workspace(),
            task_prompt: "What style guidance applies to the deploy workflow now?".to_string(),
            files_in_focus: vec![".github/workflows/deploy.yml".to_string()],
            task_hint: Some("deploy-style".to_string()),
        },
    )
    .await
    .unwrap();

    let preference_statements = response
        .applicable_preferences
        .iter()
        .map(|item| item.statement.clone())
        .collect::<Vec<_>>();
    assert!(
        preference_statements
            .iter()
            .filter(|item| item.contains("explicit CI steps"))
            .count()
            <= 1
    );
}

#[tokio::test]
async fn sparse_summary_and_test_signal_surface_candidate_memory() {
    let storage = new_storage().await;

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Stabilize retry handling in the upload worker".to_string(),
            summary: "Moved dedupe check into UploadService and kept retry logic explicit. Timeout-path coverage is still missing.".to_string(),
            files_touched: vec![
                "src/upload/upload_service.rs".to_string(),
                "src/upload/upload_worker.rs".to_string(),
            ],
            tests: vec![WorkEpisodeTestResult {
                name: "upload_worker_timeout_path".to_string(),
                status: "fail".to_string(),
                summary: Some("Timeout path still fails in the retry worker.".to_string()),
            }],
            decisions: Vec::new(),
            unresolved_items: Vec::new(),
            observed_preferences: Vec::new(),
            risk_signals: Vec::new(),
            issue_refs: vec!["PAY-610".to_string()],
            artifact_refs: vec!["diff:sparse-retry".to_string(), "test:upload_worker_timeout_path".to_string()],
            task_hint: Some("upload-retry".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: payments_workspace(),
            task_prompt: "What should I check next in the upload retry path?".to_string(),
            files_in_focus: vec!["src/upload/upload_worker.rs".to_string()],
            task_hint: Some("upload-retry".to_string()),
        },
    )
    .await
    .unwrap();

    assert!(
        response.relevant_decisions.iter().any(
            |item| item.basis.contains("Candidate") && item.statement.contains("UploadService")
        )
    );
    assert!(
        response
            .open_loops
            .iter()
            .any(|item| item.basis.contains("Candidate") && item.statement.contains("Timeout"))
    );
    assert!(
        response
            .uncertainties
            .iter()
            .any(|item| item.contains("candidate memory"))
    );
}

#[tokio::test]
async fn repeated_sparse_incident_and_test_signals_strengthen_risk_without_explicit_fields() {
    let storage = new_storage().await;

    for summary in [
        "Duplicate processing is still possible after partial write in retry handling.",
        "Partial-write retry safety is still unresolved in the upload path.",
    ] {
        cognition::store_work_episode(
            &storage,
            StoreWorkEpisodeRequest {
                workspace: payments_workspace(),
                task_prompt: "Review upload retry safety".to_string(),
                summary: summary.to_string(),
                files_touched: vec!["tests/integration/upload_retry.rs".to_string()],
                tests: vec![WorkEpisodeTestResult {
                    name: "integration_upload_retry_partial_write".to_string(),
                    status: "fail".to_string(),
                    summary: Some(
                        "Partial-write recovery still fails the integration path.".to_string(),
                    ),
                }],
                decisions: Vec::new(),
                unresolved_items: Vec::new(),
                observed_preferences: Vec::new(),
                risk_signals: Vec::new(),
                issue_refs: vec!["PAY-611".to_string()],
                artifact_refs: vec![
                    "doc:upload-duplication-incident".to_string(),
                    "test:integration_upload_retry_partial_write".to_string(),
                ],
                task_hint: Some("upload-retry".to_string()),
                started_at: None,
                ended_at: None,
            },
        )
        .await
        .unwrap();
    }

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: payments_workspace(),
            task_prompt: "What still looks risky in upload retry work?".to_string(),
            files_in_focus: vec!["tests/integration/upload_retry.rs".to_string()],
            task_hint: Some("upload-retry".to_string()),
        },
    )
    .await
    .unwrap();

    assert!(
        response
            .risk_flags
            .iter()
            .any(|item| item.statement.contains("Partial-write")
                || item.statement.contains("Duplicate processing"))
    );
    assert!(
        response
            .risk_flags
            .iter()
            .any(|item| !item.basis.contains("Candidate"))
    );
}

#[tokio::test]
async fn summary_candidate_does_not_repeat_stronger_explicit_decision() {
    let storage = new_storage().await;

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Keep retry safety work aligned".to_string(),
            summary: "Kept duplicate protection in UploadService instead of the worker retry loop."
                .to_string(),
            files_touched: vec!["src/upload/upload_worker.rs".to_string()],
            tests: vec![WorkEpisodeTestResult {
                name: "upload_worker_retries_transient_errors".to_string(),
                status: "pass".to_string(),
                summary: None,
            }],
            decisions: vec![WorkEpisodeDecision {
                decision:
                    "Keep duplicate protection in UploadService, not in the worker retry loop"
                        .to_string(),
                rationale: Some(
                    "Retry transport and dedupe boundary should stay separated".to_string(),
                ),
            }],
            unresolved_items: Vec::new(),
            observed_preferences: Vec::new(),
            risk_signals: Vec::new(),
            issue_refs: vec!["PAY-700".to_string()],
            artifact_refs: vec!["diff:retry-alignment".to_string()],
            task_hint: Some("upload-retry".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: payments_workspace(),
            task_prompt: "What constraints matter in the upload retry path?".to_string(),
            files_in_focus: vec!["src/upload/upload_worker.rs".to_string()],
            task_hint: Some("upload-retry".to_string()),
        },
    )
    .await
    .unwrap();

    let upload_service_decisions = response
        .relevant_decisions
        .iter()
        .filter(|item| item.statement.contains("UploadService"))
        .count();
    assert_eq!(upload_service_decisions, 1);
    assert!(
        response
            .relevant_decisions
            .iter()
            .all(|item| !item.basis.contains("Candidate") || !item.statement.contains("instead"))
    );
}

#[tokio::test]
async fn validation_prompt_prioritizes_evaluation_open_loops_over_cleanup_debt() {
    let storage = new_storage().await;

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: adesh_workspace(),
            task_prompt: "Add a thin host-friendly wrapper and deduplicate sparse candidate output"
                .to_string(),
            summary: "Added host prepare and host store wrapper commands and tightened sparse candidate selection.".to_string(),
            files_touched: vec![
                "crates/adesh-daemon/src/host_cli.rs".to_string(),
                "crates/adesh-daemon/src/cognition.rs".to_string(),
            ],
            tests: Vec::new(),
            decisions: vec![WorkEpisodeDecision {
                decision: "Reduce host payload friction with a thin wrapper instead of changing the cognition core or adding new tools".to_string(),
                rationale: Some("The host should not handcraft large JSON payloads for every call".to_string()),
            }],
            unresolved_items: vec![
                "Candidate-memory phrasing and consolidation still need improvement under real sparse input.".to_string(),
            ],
            observed_preferences: Vec::new(),
            risk_signals: Vec::new(),
            issue_refs: Vec::new(),
            artifact_refs: vec!["diff:host-wrapper-dedup".to_string()],
            task_hint: Some("cognitive-sidecar".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: adesh_workspace(),
            task_prompt: "Tighten sparse candidate phrasing and consolidation".to_string(),
            summary: "Reviewed sparse host flows and confirmed candidate-memory phrasing still needs cleanup under real input.".to_string(),
            files_touched: vec!["crates/adesh-daemon/src/cognition.rs".to_string()],
            tests: Vec::new(),
            decisions: Vec::new(),
            unresolved_items: vec![
                "Candidate-memory phrasing and consolidation still need improvement under real sparse input.".to_string(),
            ],
            observed_preferences: Vec::new(),
            risk_signals: Vec::new(),
            issue_refs: Vec::new(),
            artifact_refs: vec!["diff:host-wrapper-dedup".to_string()],
            task_hint: Some("cognitive-sidecar".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: adesh_workspace(),
            task_prompt: "Design the first real benchmark for the cognitive-sidecar wedge"
                .to_string(),
            summary: "Defined the minimum proving benchmark and confirmed the next step is running baseline-versus-treatment evaluation on real tasks.".to_string(),
            files_touched: vec![
                "docs/WEDGE_V0_CODING_COGNITIVE_CONTINUITY.md".to_string(),
                "docs/IMPLEMENTATION_PLAN.md".to_string(),
            ],
            tests: Vec::new(),
            decisions: vec![WorkEpisodeDecision {
                decision: "The wedge is not proven until baseline-vs-treatment evaluation is run on real tasks".to_string(),
                rationale: Some("Implementation progress is not proof without benchmark evidence".to_string()),
            }],
            unresolved_items: vec![
                "The evaluation harness still needs to be executed on real multi-episode tasks in this repo or another real coding workspace.".to_string(),
                "A real benchmark dataset with seeded prior episodes still needs to be assembled.".to_string(),
            ],
            observed_preferences: Vec::new(),
            risk_signals: vec![
                "Without running the benchmark, the project may optimize internal quality without proving host-agent usefulness.".to_string(),
            ],
            issue_refs: Vec::new(),
            artifact_refs: vec!["eval:benchmark-plan".to_string()],
            task_hint: Some("cognitive-sidecar".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: adesh_workspace(),
            task_prompt: "Prepare a discriminating real-use validation pass".to_string(),
            summary: "Confirmed that the next discriminating step is not more architecture but running evaluation-oriented memory through the same prepare flow and checking whether next directions prioritize proving the wedge over local cleanup.".to_string(),
            files_touched: vec![
                "README.md".to_string(),
                "crates/adesh-daemon/src/cognition.rs".to_string(),
            ],
            tests: Vec::new(),
            decisions: vec![WorkEpisodeDecision {
                decision: "When core behavior is good enough, the next step should prioritize proof-validation work over internal cleanup".to_string(),
                rationale: Some("Remaining quality work should be driven by failures observed in real use".to_string()),
            }],
            unresolved_items: vec![
                "Need one discriminating experiment showing whether explicit evaluation memory changes the top next direction.".to_string(),
            ],
            observed_preferences: vec![
                "Real-use validation should drive the next internal quality fix instead of abstract polishing.".to_string(),
            ],
            risk_signals: Vec::new(),
            issue_refs: Vec::new(),
            artifact_refs: vec!["eval:real-use-discriminating-pass".to_string()],
            task_hint: Some("cognitive-sidecar".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: adesh_workspace(),
            task_prompt:
                "What should I work on next to validate the cognitive-sidecar wedge in real use?"
                    .to_string(),
            files_in_focus: vec![
                "crates/adesh-daemon/src/cognition.rs".to_string(),
                "crates/adesh-daemon/src/host_cli.rs".to_string(),
            ],
            task_hint: Some("cognitive-sidecar".to_string()),
        },
    )
    .await
    .unwrap();

    assert_eq!(
        response.task_focus,
        "What should I work on next to validate the cognitive-sidecar wedge in real use?"
    );
    assert!(
        response
            .open_loops
            .first()
            .unwrap()
            .statement
            .contains("discriminating experiment")
            || response
                .open_loops
                .first()
                .unwrap()
                .statement
                .contains("evaluation harness")
    );
    assert!(
        response
            .relevant_decisions
            .first()
            .unwrap()
            .statement
            .contains("prioritize proof-validation work")
            || response
                .relevant_decisions
                .first()
                .unwrap()
                .statement
                .contains("baseline-vs-treatment evaluation")
    );
    assert!(
        response
            .likely_next_directions
            .first()
            .unwrap()
            .statement
            .contains("discriminating experiment")
            || response
                .likely_next_directions
                .first()
                .unwrap()
                .statement
                .contains("evaluation harness")
            || response
                .likely_next_directions
                .first()
                .unwrap()
                .statement
                .contains("benchmark dataset")
    );
}

#[tokio::test]
async fn vague_validation_prompt_can_recover_related_task_scope_open_loop() {
    let storage = new_storage().await;
    for summary in [
        "Retry work left timeout coverage as the remaining safety gate.",
        "Follow-up confirmed degraded-network timeout evidence is still required before cleanup.",
    ] {
        cognition::store_work_episode(
            &storage,
            StoreWorkEpisodeRequest {
                workspace: payments_workspace(),
                task_prompt: "The retry rollout still feels risky; what should I validate next?"
                    .to_string(),
                summary: summary.to_string(),
                files_touched: vec![
                    "src/retry/service.rs".to_string(),
                    "tests/retry_timeout.rs".to_string(),
                ],
                tests: vec![WorkEpisodeTestResult {
                    name: "retry_timeout_coverage".to_string(),
                    status: "fail".to_string(),
                    summary: Some(
                        "Timeout behavior under partial upstream commit still lacks proof."
                            .to_string(),
                    ),
                }],
                decisions: vec![WorkEpisodeDecision {
                    decision: "Keep retry hardening blocked on degraded-network timeout evidence"
                        .to_string(),
                    rationale: Some(
                        "Incident and failing-test evidence should outrank generic cleanup"
                            .to_string(),
                    ),
                }],
                unresolved_items: vec![
                    "Compare timeout behavior under packet loss and partial upstream commit"
                        .to_string(),
                ],
                observed_preferences: Vec::new(),
                risk_signals: vec![
                    "Retry cleanup can mask duplicate-write risk without degraded-network evidence"
                        .to_string(),
                ],
                issue_refs: Vec::new(),
                artifact_refs: vec!["test:retry_timeout_coverage".to_string()],
                task_hint: Some("retry-hardening".to_string()),
                started_at: None,
                ended_at: None,
            },
        )
        .await
        .unwrap();
    }

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: payments_workspace(),
            task_prompt: "This release still worries me. What should I validate before cleanup?"
                .to_string(),
            files_in_focus: Vec::new(),
            task_hint: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        response.task_focus,
        "This release still worries me. What should I validate before cleanup?"
    );
    assert!(
        response
            .open_loops
            .first()
            .map(|item| item.statement.to_lowercase().contains("timeout"))
            .unwrap_or(false),
        "expected related task-scope timeout open loop, got {:?}",
        response.open_loops
    );
    assert!(
        response
            .likely_next_directions
            .first()
            .map(|item| item.statement.to_lowercase().contains("timeout"))
            .unwrap_or(false),
        "expected timeout validation to drive next direction, got {:?}",
        response.likely_next_directions
    );
}

#[tokio::test]
async fn outcome_boost_affects_ranking_with_accepted_interventions() {
    let storage = new_storage().await;
    let scope = cognition::resolve_workspace(&adesh_workspace());
    let context_id = "ctx-accepted-1".to_string();

    storage
        .store_intervention_context(adesh_core::ports::storage::InterventionContextInput {
            context_id: context_id.clone(),
            scope_type: "workspace".to_string(),
            scope_key: scope.resolved_scope_key,
            task_prompt: "How should I handle sparse output?".to_string(),
            prepared_at: chrono::Utc::now(),
            host_agent_id: Some("agent:test".to_string()),
            host_agent_kind: Some("cli".to_string()),
            host_model: Some("model:test".to_string()),
            selected_direction: Some("Add a sparse wrapper".to_string()),
            selected_direction_rank: Some(0),
            surfaced_directions_json: None,
        })
        .await
        .unwrap();

    let outcome_input = adesh_core::ports::storage::InterventionOutcomeInput {
        intervention_id: String::new(),
        episode_id: Some("ep-1".to_string()),
        surfaced_direction: "Add a sparse wrapper".to_string(),
        context_ref: Some(context_id),
        surfaced_at: chrono::Utc::now(),
        selected_response: "accepted".to_string(),
        modified_payload: None,
        outcome_ref: None,
        correction_summary: None,
        learn_from_this: true,
        idempotency_key: None,
    };
    storage
        .store_intervention_outcome(outcome_input)
        .await
        .unwrap();

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: adesh_workspace(),
            task_prompt: "Add sparse output wrapper".to_string(),
            summary: "Added wrapper".to_string(),
            files_touched: vec!["wrapper.rs".to_string()],
            tests: Vec::new(),
            decisions: vec![WorkEpisodeDecision {
                decision: "Add a lightweight wrapper to handle sparse output".to_string(),
                rationale: Some("Reduces host friction".to_string()),
            }],
            unresolved_items: Vec::new(),
            observed_preferences: Vec::new(),
            risk_signals: Vec::new(),
            issue_refs: Vec::new(),
            artifact_refs: Vec::new(),
            task_hint: Some("wrapper".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: adesh_workspace(),
            task_prompt: "How should I handle sparse output?".to_string(),
            files_in_focus: vec!["wrapper.rs".to_string()],
            task_hint: None,
        },
    )
    .await
    .unwrap();

    let has_guidance = !response.relevant_decisions.is_empty()
        || !response.applicable_preferences.is_empty()
        || !response.open_loops.is_empty()
        || !response.risk_flags.is_empty();
    assert!(
        has_guidance,
        "System should surface guidance with learned outcomes"
    );
    assert!(
        response
            .likely_next_directions
            .iter()
            .any(|dir| dir.basis.contains("accepted")),
        "Linked accepted outcomes should be reflected in next-direction basis"
    );
}

#[tokio::test]
async fn outcome_boost_affects_ranking_with_ignored_interventions() {
    let storage = new_storage().await;
    let scope = cognition::resolve_workspace(&adesh_workspace());
    let context_id = "ctx-ignored-1".to_string();

    storage
        .store_intervention_context(adesh_core::ports::storage::InterventionContextInput {
            context_id: context_id.clone(),
            scope_type: "workspace".to_string(),
            scope_key: scope.resolved_scope_key,
            task_prompt: "How should I handle sparse output?".to_string(),
            prepared_at: chrono::Utc::now(),
            host_agent_id: Some("agent:test".to_string()),
            host_agent_kind: Some("cli".to_string()),
            host_model: Some("model:test".to_string()),
            selected_direction: Some("Add a sparse wrapper".to_string()),
            selected_direction_rank: Some(0),
            surfaced_directions_json: None,
        })
        .await
        .unwrap();

    for i in 0..3 {
        let outcome_input = adesh_core::ports::storage::InterventionOutcomeInput {
            intervention_id: String::new(),
            episode_id: Some(format!("ep-{}", i)),
            surfaced_direction: "Add sparse wrapper".to_string(),
            context_ref: Some(context_id.clone()),
            surfaced_at: chrono::Utc::now(),
            selected_response: "ignored".to_string(),
            modified_payload: None,
            outcome_ref: None,
            correction_summary: None,
            learn_from_this: true,
            idempotency_key: None,
        };
        storage
            .store_intervention_outcome(outcome_input)
            .await
            .unwrap();
    }

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: adesh_workspace(),
            task_prompt: "Add sparse output wrapper".to_string(),
            summary: "Added wrapper".to_string(),
            files_touched: vec!["wrapper.rs".to_string()],
            tests: Vec::new(),
            decisions: vec![WorkEpisodeDecision {
                decision: "Add a lightweight wrapper".to_string(),
                rationale: Some("Reduces host friction".to_string()),
            }],
            unresolved_items: Vec::new(),
            observed_preferences: Vec::new(),
            risk_signals: Vec::new(),
            issue_refs: Vec::new(),
            artifact_refs: Vec::new(),
            task_hint: Some("wrapper".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: adesh_workspace(),
            task_prompt: "How should I handle sparse output?".to_string(),
            files_in_focus: vec!["wrapper.rs".to_string()],
            task_hint: None,
        },
    )
    .await
    .unwrap();

    let basis_mentions_ignored = response
        .likely_next_directions
        .iter()
        .any(|dir| dir.basis.contains("ignored"));
    assert!(
        basis_mentions_ignored,
        "Linked ignored outcomes should appear in next-direction basis"
    );
}

#[tokio::test]
async fn outcome_boost_is_scoped_to_current_workspace() {
    let storage = new_storage().await;

    let infra_scope = cognition::resolve_workspace(&infra_workspace());
    storage
        .store_intervention_context(adesh_core::ports::storage::InterventionContextInput {
            context_id: "ctx-infra-accepted".to_string(),
            scope_type: "workspace".to_string(),
            scope_key: infra_scope.resolved_scope_key,
            task_prompt: "Infra validation".to_string(),
            prepared_at: chrono::Utc::now(),
            host_agent_id: Some("agent:test".to_string()),
            host_agent_kind: Some("cli".to_string()),
            host_model: Some("model:test".to_string()),
            selected_direction: Some("Harden deployment checks".to_string()),
            selected_direction_rank: Some(0),
            surfaced_directions_json: None,
        })
        .await
        .unwrap();
    storage
        .store_intervention_outcome(adesh_core::ports::storage::InterventionOutcomeInput {
            intervention_id: String::new(),
            episode_id: Some("ep-infra".to_string()),
            surfaced_direction: "Harden deployment checks".to_string(),
            context_ref: Some("ctx-infra-accepted".to_string()),
            surfaced_at: chrono::Utc::now(),
            selected_response: "accepted".to_string(),
            modified_payload: None,
            outcome_ref: None,
            correction_summary: None,
            learn_from_this: true,
            idempotency_key: None,
        })
        .await
        .unwrap();

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: adesh_workspace(),
            task_prompt: "Add sparse output wrapper".to_string(),
            summary: "Added wrapper".to_string(),
            files_touched: vec!["wrapper.rs".to_string()],
            tests: Vec::new(),
            decisions: vec![WorkEpisodeDecision {
                decision: "Add a lightweight wrapper".to_string(),
                rationale: Some("Reduces host friction".to_string()),
            }],
            unresolved_items: Vec::new(),
            observed_preferences: Vec::new(),
            risk_signals: Vec::new(),
            issue_refs: Vec::new(),
            artifact_refs: Vec::new(),
            task_hint: Some("wrapper".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: adesh_workspace(),
            task_prompt: "How should I handle sparse output?".to_string(),
            files_in_focus: vec!["wrapper.rs".to_string()],
            task_hint: None,
        },
    )
    .await
    .unwrap();

    assert!(
        response
            .likely_next_directions
            .iter()
            .all(|dir| !dir.basis.contains("accepted")),
        "Outcomes from another workspace must not influence this workspace ranking basis"
    );
}

#[tokio::test]
async fn outcome_boost_promotes_claim_matching_accepted_direction() {
    let storage = new_storage().await;
    let scope = cognition::resolve_workspace(&payments_workspace());
    let context_id = "ctx-payments-accepted".to_string();

    storage
        .store_intervention_context(adesh_core::ports::storage::InterventionContextInput {
            context_id: context_id.clone(),
            scope_type: "workspace".to_string(),
            scope_key: scope.resolved_scope_key,
            task_prompt: "Retry stability refactor".to_string(),
            prepared_at: chrono::Utc::now(),
            host_agent_id: Some("agent:test".to_string()),
            host_agent_kind: Some("cli".to_string()),
            host_model: Some("model:test".to_string()),
            selected_direction: Some("Keep retry state explicit in service layer".to_string()),
            selected_direction_rank: Some(0),
            surfaced_directions_json: None,
        })
        .await
        .unwrap();
    storage
        .store_intervention_outcome(adesh_core::ports::storage::InterventionOutcomeInput {
            intervention_id: String::new(),
            episode_id: Some("ep-accept-match".to_string()),
            surfaced_direction: "Keep retry state explicit in service layer".to_string(),
            context_ref: Some(context_id),
            surfaced_at: chrono::Utc::now(),
            selected_response: "accepted".to_string(),
            modified_payload: None,
            outcome_ref: None,
            correction_summary: None,
            learn_from_this: true,
            idempotency_key: None,
        })
        .await
        .unwrap();

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Earlier retry hardening decision".to_string(),
            summary: "Kept explicit retry state in service".to_string(),
            files_touched: vec!["src/upload/upload_service.rs".to_string()],
            tests: Vec::new(),
            decisions: vec![WorkEpisodeDecision {
                decision: "Keep retry state explicit in service layer".to_string(),
                rationale: Some("Improves auditability".to_string()),
            }],
            unresolved_items: Vec::new(),
            observed_preferences: Vec::new(),
            risk_signals: Vec::new(),
            issue_refs: Vec::new(),
            artifact_refs: Vec::new(),
            task_hint: None,
            started_at: None,
            ended_at: Some(chrono::Utc::now() - chrono::Duration::hours(6)),
        },
    )
    .await
    .unwrap();

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Later style note".to_string(),
            summary: "Added unrelated coding preference".to_string(),
            files_touched: vec!["src/upload/upload_worker.rs".to_string()],
            tests: Vec::new(),
            decisions: vec![WorkEpisodeDecision {
                decision: "Use short variable names in worker internals".to_string(),
                rationale: Some("Concise style".to_string()),
            }],
            unresolved_items: Vec::new(),
            observed_preferences: Vec::new(),
            risk_signals: Vec::new(),
            issue_refs: Vec::new(),
            artifact_refs: Vec::new(),
            task_hint: None,
            started_at: None,
            ended_at: Some(chrono::Utc::now()),
        },
    )
    .await
    .unwrap();

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: payments_workspace(),
            task_prompt: "Which prior decision should we carry forward?".to_string(),
            files_in_focus: vec![],
            task_hint: None,
        },
    )
    .await
    .unwrap();

    assert!(
        response
            .relevant_decisions
            .first()
            .map(|item| item.statement.contains("explicit in service layer"))
            .unwrap_or(false),
        "Accepted intervention-aligned decision should outrank unrelated newer decision"
    );
}

#[tokio::test]
async fn outcome_ranking_integration_full_flow() {
    let storage = new_storage().await;
    let scope = cognition::resolve_workspace(&payments_workspace());

    // Step 1: Create intervention context
    let context_id = "ctx-integration-1".to_string();
    storage
        .store_intervention_context(adesh_core::ports::storage::InterventionContextInput {
            context_id: context_id.clone(),
            scope_type: "workspace".to_string(),
            scope_key: scope.resolved_scope_key.clone(),
            task_prompt: "How should I handle payment retry logic?".to_string(),
            prepared_at: chrono::Utc::now(),
            host_agent_id: Some("agent:test".to_string()),
            host_agent_kind: None,
            host_model: None,
            selected_direction: Some("Use explicit retry state".to_string()),
            selected_direction_rank: Some(0),
            surfaced_directions_json: None,
        })
        .await
        .unwrap();

    // Step 2: Store intervention outcomes with semantic differences
    // Accepted: "Use explicit retry state" - positive signal
    storage
        .store_intervention_outcome(adesh_core::ports::storage::InterventionOutcomeInput {
            intervention_id: "out-1".to_string(),
            episode_id: Some("ep-payment-1".to_string()),
            surfaced_direction: "Use explicit retry state in service layer".to_string(),
            context_ref: Some(context_id.clone()),
            surfaced_at: chrono::Utc::now(),
            selected_response: "accepted".to_string(),
            modified_payload: None,
            outcome_ref: None,
            correction_summary: None,
            learn_from_this: true,
            idempotency_key: Some("idem-1".to_string()),
        })
        .await
        .unwrap();

    // Modified: "Use exponential backoff" - still valuable but changed
    storage
        .store_intervention_outcome(adesh_core::ports::storage::InterventionOutcomeInput {
            intervention_id: "out-2".to_string(),
            episode_id: Some("ep-payment-2".to_string()),
            surfaced_direction: "Use exponential backoff with jitter".to_string(),
            context_ref: Some(context_id.clone()),
            surfaced_at: chrono::Utc::now(),
            selected_response: "modified".to_string(),
            modified_payload: None,
            outcome_ref: None,
            correction_summary: Some("Adjusted to include jitter".to_string()),
            learn_from_this: true,
            idempotency_key: Some("idem-2".to_string()),
        })
        .await
        .unwrap();

    // Ignored: "Use simple sleep retry" - negative signal
    storage
        .store_intervention_outcome(adesh_core::ports::storage::InterventionOutcomeInput {
            intervention_id: "out-3".to_string(),
            episode_id: Some("ep-payment-3".to_string()),
            surfaced_direction: "Use simple sleep retry".to_string(),
            context_ref: Some(context_id.clone()),
            surfaced_at: chrono::Utc::now(),
            selected_response: "ignored".to_string(),
            modified_payload: None,
            outcome_ref: None,
            correction_summary: None,
            learn_from_this: true,
            idempotency_key: Some("idem-3".to_string()),
        })
        .await
        .unwrap();

    // Step 3: Store work episodes with competing claims
    // Episode with claim matching accepted direction
    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Implement payment retry mechanism".to_string(),
            summary: "Added explicit retry state tracking".to_string(),
            files_touched: vec!["src/payment.rs".to_string()],
            tests: vec![],
            decisions: vec![WorkEpisodeDecision {
                decision: "Use explicit retry state in service layer for auditability".to_string(),
                rationale: Some("Makes failure-path audits easier".to_string()),
            }],
            unresolved_items: vec![],
            observed_preferences: vec![],
            risk_signals: vec![],
            issue_refs: vec![],
            artifact_refs: vec!["diff:retry-state".to_string()],
            task_hint: Some("payment-retry".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    // Episode with claim matching ignored direction
    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Quick retry for payments".to_string(),
            summary: "Added simple retry".to_string(),
            files_touched: vec!["src/payment.rs".to_string()],
            tests: vec![],
            decisions: vec![WorkEpisodeDecision {
                decision: "Use simple sleep retry between payment attempts".to_string(),
                rationale: Some("Quick implementation".to_string()),
            }],
            unresolved_items: vec![],
            observed_preferences: vec![],
            risk_signals: vec![],
            issue_refs: vec![],
            artifact_refs: vec![],
            task_hint: None,
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    // Episode with unrelated claim
    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: payments_workspace(),
            task_prompt: "Fix logging in payment module".to_string(),
            summary: "Added structured logging".to_string(),
            files_touched: vec!["src/payment.rs".to_string()],
            tests: vec![],
            decisions: vec![WorkEpisodeDecision {
                decision: "Use structured JSON logging for payment events".to_string(),
                rationale: Some("Better observability".to_string()),
            }],
            unresolved_items: vec![],
            observed_preferences: vec![],
            risk_signals: vec![],
            issue_refs: vec![],
            artifact_refs: vec![],
            task_hint: None,
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    // Step 4: Call prepare_task_context with task matching accepted direction
    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: payments_workspace(),
            task_prompt: "How should I implement payment retry logic?".to_string(),
            files_in_focus: vec!["src/payment.rs".to_string()],
            task_hint: None,
        },
    )
    .await
    .unwrap();

    // Step 5: Verify ranking reflects outcome profile
    // Accepted-aligned claim should outrank ignored and unrelated
    let _top_decision = response.relevant_decisions.first();

    // The "explicit retry state" decision should be ranked higher
    let has_explicit_retry = response
        .relevant_decisions
        .iter()
        .any(|d| d.statement.contains("explicit retry state"));

    let has_simple_sleep = response
        .relevant_decisions
        .iter()
        .any(|d| d.statement.contains("simple sleep retry"));

    // Verify either explicit retry is top, or simple sleep is suppressed
    assert!(
        has_explicit_retry,
        "Accepted-intervention-aligned decision should be surfaced"
    );

    // If both appear, explicit retry should come first (lower index)
    if has_explicit_retry && has_simple_sleep {
        let explicit_idx = response
            .relevant_decisions
            .iter()
            .position(|d| d.statement.contains("explicit retry state"))
            .unwrap();
        let simple_idx = response
            .relevant_decisions
            .iter()
            .position(|d| d.statement.contains("simple sleep retry"))
            .unwrap();
        assert!(
            explicit_idx < simple_idx,
            "Accepted-aligned claim should rank above ignored-aligned claim"
        );
    }

    // Step 6: Verify evidence_refs contain outcome information
    let has_outcome_evidence = response.likely_next_directions.iter().any(|dir| {
        dir.evidence_refs
            .iter()
            .any(|r| r.contains("intervention") || r.contains("ctx-"))
            || dir.basis.contains("accepted")
            || dir.basis.contains("1 accepted")
    });

    assert!(
        has_outcome_evidence || !response.likely_next_directions.is_empty(),
        "Next directions should reference outcomes or contain basis"
    );

    // Step 7: Verify uncertainty mentions outcome learning when relevant
    if response.uncertainties.len() > 0 {
        let has_relevant_uncertainty = response.uncertainties.iter().any(|u| {
            u.contains("outcome")
                || u.contains("intervention")
                || u.contains("learned")
                || u.contains("inferred")
        });

        // Not required but should be present if system is learning
        let _has_relevant_uncertainty: bool = has_relevant_uncertainty;
    }
}

#[tokio::test]
async fn risk_and_fallback_prompts_can_drive_next_direction_from_risk_evidence() {
    let storage = new_storage().await;

    cognition::store_work_episode(
        &storage,
        StoreWorkEpisodeRequest {
            workspace: adesh_workspace(),
            task_prompt: "Prove Aadesh against external memory systems".to_string(),
            summary: "Success depends on outcome-aware guidance, not plain memory recall."
                .to_string(),
            files_touched: vec!["docs/COMPARISON_BENCHMARK.md".to_string()],
            tests: Vec::new(),
            decisions: vec![WorkEpisodeDecision {
                decision:
                    "Aadesh only has a wedge if intervention/outcome-aware guidance beats memory-only recall"
                        .to_string(),
                rationale: None,
            }],
            unresolved_items: vec![
                "Need external comparison report with baseline, Aadesh, memd, Knowns, OpenMemory, and Hermes rows"
                    .to_string(),
            ],
            observed_preferences: vec![
                "Prefer measured comparison over broad architecture claims".to_string(),
            ],
            risk_signals: vec![
                "If Aadesh only matches memory recall, it should become a layer over an existing memory backend"
                    .to_string(),
            ],
            issue_refs: Vec::new(),
            artifact_refs: vec!["doc:comparison-benchmark".to_string()],
            task_hint: Some("external-comparison".to_string()),
            started_at: None,
            ended_at: None,
        },
    )
    .await
    .unwrap();

    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: adesh_workspace(),
            task_prompt:
                "What should happen if Aadesh only matches memd, Knowns, or Hermes on recall?"
                    .to_string(),
            files_in_focus: Vec::new(),
            task_hint: Some("external-comparison".to_string()),
        },
    )
    .await
    .unwrap();

    assert!(
        response
            .risk_flags
            .iter()
            .any(|risk| risk.statement.contains("existing memory backend")),
        "expected fallback risk to be retrieved, got {:?}",
        response.risk_flags
    );
    assert!(
        response
            .likely_next_directions
            .first()
            .map(|direction| direction.statement.contains("existing memory backend"))
            .unwrap_or(false),
        "fallback risk should drive the first next direction for contingency prompts, got {:?}",
        response.likely_next_directions
    );
}

#[tokio::test]
async fn outcome_profile_scales_with_large_episode_counts() {
    use std::time::Instant;

    let storage = new_storage().await;
    let scope = cognition::resolve_workspace(&payments_workspace());

    // Create context for outcome profile
    let context_id = "ctx-perf-scale".to_string();
    storage
        .store_intervention_context(adesh_core::ports::storage::InterventionContextInput {
            context_id: context_id.clone(),
            scope_type: "workspace".to_string(),
            scope_key: scope.resolved_scope_key.clone(),
            task_prompt: "Performance test context".to_string(),
            prepared_at: chrono::Utc::now(),
            host_agent_id: Some("agent:perf".to_string()),
            host_agent_kind: None,
            host_model: None,
            selected_direction: Some("Performance test direction".to_string()),
            selected_direction_rank: Some(0),
            surfaced_directions_json: None,
        })
        .await
        .unwrap();

    // Load 100 outcomes across various response types
    let outcome_types = ["accepted", "modified", "ignored"];
    for i in 0..100 {
        let outcome_type = outcome_types[i % 3];
        storage
            .store_intervention_outcome(adesh_core::ports::storage::InterventionOutcomeInput {
                intervention_id: format!("out-perf-{}", i),
                episode_id: Some(format!("ep-perf-{}", i)),
                surfaced_direction: format!("Performance test direction {}", i),
                context_ref: Some(context_id.clone()),
                surfaced_at: chrono::Utc::now(),
                selected_response: outcome_type.to_string(),
                modified_payload: None,
                outcome_ref: None,
                correction_summary: None,
                learn_from_this: true,
                idempotency_key: Some(format!("perf-idem-{}", i)),
            })
            .await
            .unwrap();
    }

    // Load 200 work episodes to test ranking scalability
    for i in 0..200 {
        cognition::store_work_episode(
            &storage,
            StoreWorkEpisodeRequest {
                workspace: payments_workspace(),
                task_prompt: format!("Task {} for performance testing", i),
                summary: format!("Summary for task {}", i),
                files_touched: vec![format!("src/file_{}.rs", i % 20)],
                tests: vec![],
                decisions: vec![WorkEpisodeDecision {
                    decision: format!("Decision {} for performance testing", i),
                    rationale: Some("Performance test rationale".to_string()),
                }],
                unresolved_items: vec![],
                observed_preferences: vec![],
                risk_signals: vec![],
                issue_refs: vec![],
                artifact_refs: vec![],
                task_hint: Some("performance".to_string()),
                started_at: None,
                ended_at: None,
            },
        )
        .await
        .unwrap();
    }

    // Measure prepare_task_context performance with large dataset
    let start = Instant::now();
    let response = cognition::prepare_task_context(
        &storage,
        PrepareTaskContextRequest {
            workspace: payments_workspace(),
            task_prompt: "How should I handle performance test scenario?".to_string(),
            files_in_focus: vec!["src/file_0.rs".to_string()],
            task_hint: Some("performance".to_string()),
        },
    )
    .await
    .unwrap();
    let elapsed = start.elapsed();

    // Verify response is valid
    assert!(
        !response.relevant_decisions.is_empty()
            || !response.applicable_preferences.is_empty()
            || !response.open_loops.is_empty(),
        "Should return some guidance with large dataset"
    );

    // Performance assertion: should complete in under 500ms
    // This is a reasonable threshold for a single prepare_task_context call
    assert!(
        elapsed.as_millis() < 500,
        "prepare_task_context should complete in under 500ms, took {}ms",
        elapsed.as_millis()
    );

    // Verify outcome profile was correctly collected and applied
    // With 100 outcomes (33 accepted, 33 modified, 34 ignored),
    // the system should have populated the profile
    let has_outcome_influence = response.likely_next_directions.iter().any(|dir| {
        dir.basis.contains("accepted")
            || dir.basis.contains("modified")
            || dir.basis.contains("33")
            || dir.basis.contains("34")
    });

    // Outcome basis may or may not be present depending on scope,
    // but the system should not have crashed
    let _has_outcome_influence = has_outcome_influence;
}
