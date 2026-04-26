use crate::{
    indexing::{index_source_action_visual_units, index_visual_units},
    model::{
        ImportJobOutcome, MaintenanceActionPlan, PreparedImport, PreparedSourceAction,
        RetiredVectorSpaceCleanupCandidate, SourceActionJobOutcome, SourceActionPlan,
        SourceActionSummary,
    },
    qdrant::cleanup_retired_vector_space_namespace,
    state::SharedState,
    RETIRED_VECTOR_SPACE_REAPER_INTERVAL_SECS, SOURCE_WATCHER_POLL_INTERVAL_SECS,
    TEMP_QUERY_ASSET_REAPER_INTERVAL_SECS,
};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

async fn job_cancellation_requested(state: &SharedState, job_id: &str) -> bool {
    let state = state.read().await;
    state.job_cancellation_requested(job_id)
}

async fn job_is_terminal(state: &SharedState, job_id: &str) -> bool {
    let state = state.read().await;
    state.job_is_terminal(job_id)
}

pub fn spawn_runtime_maintenance(state: SharedState) {
    let reaper_state = state.clone();
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(Duration::from_secs(TEMP_QUERY_ASSET_REAPER_INTERVAL_SECS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;

        loop {
            interval.tick().await;
            let summary = {
                let mut state = reaper_state.write().await;
                state.prune_temp_query_assets()
            };

            if summary.removed_count() > 0 {
                tracing::info!(
                    expired_removed = summary.expired_removed,
                    missing_removed = summary.missing_removed,
                    "Pruned expired query assets."
                );
            }
        }
    });

    let retired_cleanup_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(
            RETIRED_VECTOR_SPACE_REAPER_INTERVAL_SECS,
        ));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;

        loop {
            interval.tick().await;

            let candidates = {
                let state = retired_cleanup_state.read().await;
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_millis())
                    .unwrap_or(0);
                state.eligible_retired_vector_spaces_for_cleanup(now_ms)
            };

            if candidates.is_empty() {
                continue;
            }

            let mut cleaned = Vec::<RetiredVectorSpaceCleanupCandidate>::new();
            for candidate in candidates {
                match cleanup_retired_vector_space_namespace(
                    &candidate.library_id,
                    &candidate.vector_space_id,
                )
                .await
                {
                    Ok(()) => cleaned.push(candidate),
                    Err(error) => {
                        tracing::warn!(
                            library_id = %candidate.library_id,
                            vector_space_id = %candidate.vector_space_id,
                            "Failed to clean retired vector-space namespace: {error}"
                        );
                    }
                }
            }

            if cleaned.is_empty() {
                continue;
            }

            let cleaned_count = cleaned.len();
            let mut state = retired_cleanup_state.write().await;
            match state.forget_cleaned_retired_vector_spaces(&cleaned) {
                Ok(()) => {
                    tracing::info!(
                        cleaned_count,
                        "Cleaned expired retired vector-space namespace(s)."
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        cleaned_count,
                        "Failed to persist retired vector-space cleanup progress: {error}"
                    );
                }
            }
        }
    });

    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(Duration::from_secs(SOURCE_WATCHER_POLL_INTERVAL_SECS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;

        loop {
            interval.tick().await;

            let queued = {
                let mut state = state.write().await;
                state.poll_source_root_watchers()
            };

            for queued_action in queued {
                let background_state = state.clone();
                tokio::spawn(async move {
                    run_source_action_job(
                        background_state,
                        queued_action.job_id,
                        queued_action.plan,
                    )
                    .await;
                });
            }
        }
    });
}

pub(crate) async fn run_import_job(state: SharedState, job_id: String, prepared: PreparedImport) {
    if job_is_terminal(&state, &job_id).await {
        return;
    }

    let execution_groups = {
        let mut state = state.write().await;
        match state
            .resolve_execution_groups_for_library(&prepared.library_id)
            .await
        {
            Ok(groups) => groups
                .into_iter()
                .map(|group| (group.vector_space_id.clone(), group))
                .collect::<BTreeMap<_, _>>(),
            Err(error) => {
                let outcome = ImportJobOutcome::failed(
                    "encode",
                    error.payload.message,
                    prepared.accepted.len(),
                );
                if let Err(message) = state.finalize_import_job(&job_id, prepared, outcome) {
                    tracing::warn!("Failed to finalize import job {job_id}: {message}");
                }
                return;
            }
        }
    };

    {
        let mut state = state.write().await;
        state.update_job_snapshot(
            &job_id,
            "running",
            "encode",
            0,
            format!(
                "Encoding {} accepted path(s) into vector-space embeddings.",
                prepared.accepted.len()
            ),
        );
    }

    let outcome = if prepared.vector_space_batches.is_empty() {
        ImportJobOutcome::completed(
            format!(
                "Accepted {} path(s); stored structured sources and visual units without indexing because no enabled content types matched the imported assets.",
                prepared.accepted.len()
            ),
            prepared.accepted.len(),
            BTreeSet::new(),
        )
    } else {
        let mut activated_vector_spaces = BTreeSet::new();
        let mut indexed_visual_units = 0_usize;
        let mut indexed_vector_spaces = 0_usize;
        let mut failures = Vec::new();
        for batch in &prepared.vector_space_batches {
            if job_cancellation_requested(&state, &job_id).await {
                break;
            }
            let Some(group) = execution_groups.get(&batch.vector_space_id) else {
                failures.push(format!(
                    "vector_space {}: missing resolved execution group",
                    batch.vector_space_id
                ));
                continue;
            };
            match index_visual_units(
                &prepared.library_id,
                batch,
                &group.resolved_model,
                state.clone(),
                &job_id,
            )
            .await
            {
                Ok(_) => {}
                Err(error) => {
                    failures.push(format!(
                        "vector_space {}: {} failed: {}",
                        batch.vector_space_id, error.phase, error.message
                    ));
                    continue;
                }
            }
            indexed_visual_units += batch.visual_units.len();
            indexed_vector_spaces += 1;
            activated_vector_spaces.insert(batch.vector_space_id.clone());
        }

        if job_cancellation_requested(&state, &job_id).await {
            if activated_vector_spaces.is_empty() {
                ImportJobOutcome::canceled(
                    "Import canceled before any vector-space activation.".to_string(),
                    prepared.accepted.len(),
                )
            } else {
                ImportJobOutcome::canceled_with_activations(
                    format!(
                        "Import canceled after activating {} vector space(s); structured sources and visual units were kept for consistency.",
                        activated_vector_spaces.len()
                    ),
                    prepared.accepted.len(),
                    activated_vector_spaces,
                )
            }
        } else if failures.is_empty() {
            ImportJobOutcome::completed(
                format!(
                    "Accepted {} path(s); indexed {} visual unit(s) across {} vector space(s) and activated the resulting namespaces.",
                    prepared.accepted.len(),
                    indexed_visual_units,
                    indexed_vector_spaces,
                ),
                prepared.accepted.len(),
                activated_vector_spaces,
            )
        } else {
            ImportJobOutcome::failed_with_activations(
                "failed",
                format!(
                    "Accepted {} path(s); indexed {} visual unit(s) across {} vector space(s), activated {} vector space(s), and encountered {} failure(s): {}",
                    prepared.accepted.len(),
                    indexed_visual_units,
                    prepared.vector_space_batches.len(),
                    activated_vector_spaces.len(),
                    failures.len(),
                    failures.join("; "),
                ),
                prepared.accepted.len(),
                activated_vector_spaces,
            )
        }
    };

    let mut state = state.write().await;
    if let Err(message) = state.finalize_import_job(&job_id, prepared, outcome) {
        tracing::warn!("Failed to finalize import job {job_id}: {message}");
    }
}

pub(crate) async fn run_source_action_job(
    state: SharedState,
    job_id: String,
    plan: SourceActionPlan,
) {
    if job_is_terminal(&state, &job_id).await {
        return;
    }

    {
        let mut state = state.write().await;
        state.mark_source_action_running(&plan, &job_id);
    }

    let execution_groups = {
        let mut state = state.write().await;
        match state
            .resolve_execution_groups_for_library(&plan.library_id)
            .await
        {
            Ok(groups) => groups
                .into_iter()
                .map(|group| (group.vector_space_id.clone(), group))
                .collect::<BTreeMap<_, _>>(),
            Err(error) => {
                let prepared = PreparedSourceAction {
                    library_id: plan.library_id.clone(),
                    action: plan.action,
                    accepted_root_count: plan.target_root_ids.len(),
                    root_updates: Vec::new(),
                    source_mutations: Vec::new(),
                    summary: SourceActionSummary::default(),
                    vector_space_batches: Vec::new(),
                };
                if let Err(message) = state.finalize_source_action_job(
                    &job_id,
                    prepared,
                    SourceActionJobOutcome::failed(plan.action, 0, error.payload.message),
                ) {
                    tracing::warn!("Failed to finalize source action job {job_id}: {message}");
                }
                return;
            }
        }
    };

    let prepared = {
        let mut state = state.write().await;
        state.prepare_source_action_execution(&plan)
    };

    let outcome = match prepared {
        Ok(prepared) => {
            if job_cancellation_requested(&state, &job_id).await {
                let outcome = SourceActionJobOutcome::canceled(
                    plan.action,
                    0,
                    "Canceled before applying any structured source updates.".to_string(),
                );
                let mut state = state.write().await;
                if let Err(message) = state.finalize_source_action_job(&job_id, prepared, outcome) {
                    tracing::warn!("Failed to finalize source action job {job_id}: {message}");
                }
                return;
            }

            if prepared.requires_index_update() {
                {
                    let mut state = state.write().await;
                    state.update_job_snapshot(
                        &job_id,
                        "running",
                        "encode",
                        0,
                        format!(
                            "Encoding {} visual unit(s) for {}.",
                            prepared
                                .vector_space_batches
                                .iter()
                                .map(|batch| batch.visual_units_to_index.len())
                                .sum::<usize>(),
                            plan.action.as_str(),
                        ),
                    );
                }

                let mut activated_vector_spaces = BTreeSet::new();
                let mut failures = Vec::new();
                for batch in prepared.vector_space_batches.clone() {
                    if job_cancellation_requested(&state, &job_id).await {
                        break;
                    }
                    let Some(group) = execution_groups.get(&batch.vector_space_id) else {
                        failures.push(format!(
                            "vector_space {}: missing resolved execution group",
                            batch.vector_space_id
                        ));
                        continue;
                    };
                    match index_source_action_visual_units(
                        &prepared.library_id,
                        plan.action.as_str(),
                        &batch,
                        &group.resolved_model,
                        state.clone(),
                        &job_id,
                    )
                    .await
                    {
                        Ok(_) => {}
                        Err(message) => {
                            failures.push(format!(
                                "vector_space {}: {}",
                                batch.vector_space_id, message
                            ));
                            continue;
                        }
                    }
                    activated_vector_spaces.insert(batch.vector_space_id.clone());
                }

                let outcome = if job_cancellation_requested(&state, &job_id).await {
                    if activated_vector_spaces.is_empty() {
                        SourceActionJobOutcome::canceled(
                            plan.action,
                            0,
                            "Canceled before activating any vector-space updates.".to_string(),
                        )
                    } else {
                        SourceActionJobOutcome::canceled_with_structured_changes(
                            plan.action,
                            prepared.accepted_root_count,
                            activated_vector_spaces,
                            "Canceled after partial activation; structured source updates were kept for consistency."
                                .to_string(),
                        )
                    }
                } else if failures.is_empty() {
                    SourceActionJobOutcome::completed(&prepared)
                } else {
                    let activated_count = activated_vector_spaces.len();
                    SourceActionJobOutcome::failed_with_structured_changes(
                        plan.action,
                        prepared.accepted_root_count,
                        activated_vector_spaces,
                        format!(
                            "applied structured updates, activated {} vector space(s), and encountered {} failure(s): {}",
                            activated_count,
                            failures.len(),
                            failures.join("; "),
                        ),
                    )
                };
                let mut state = state.write().await;
                if let Err(message) = state.finalize_source_action_job(&job_id, prepared, outcome) {
                    tracing::warn!("Failed to finalize source action job {job_id}: {message}");
                }
                return;
            }

            let outcome = if job_cancellation_requested(&state, &job_id).await {
                SourceActionJobOutcome::canceled(
                    plan.action,
                    0,
                    "Canceled before applying structured source updates.".to_string(),
                )
            } else {
                SourceActionJobOutcome::completed(&prepared)
            };
            let mut state = state.write().await;
            if let Err(message) = state.finalize_source_action_job(&job_id, prepared, outcome) {
                tracing::warn!("Failed to finalize source action job {job_id}: {message}");
            }
            return;
        }
        Err(message) => SourceActionJobOutcome::failed(plan.action, 0, message),
    };

    let mut state = state.write().await;
    let prepared = PreparedSourceAction {
        library_id: plan.library_id,
        action: plan.action,
        accepted_root_count: plan.target_root_ids.len(),
        root_updates: Vec::new(),
        source_mutations: Vec::new(),
        summary: SourceActionSummary::default(),
        vector_space_batches: Vec::new(),
    };
    if let Err(message) = state.finalize_source_action_job(&job_id, prepared, outcome) {
        tracing::warn!("Failed to finalize source action job {job_id}: {message}");
    }
}

pub(crate) async fn run_maintenance_action_job(
    state: SharedState,
    job_id: String,
    plan: MaintenanceActionPlan,
) {
    if job_is_terminal(&state, &job_id).await {
        return;
    }

    {
        let mut state = state.write().await;
        state.update_job_snapshot(
            &job_id,
            "running",
            "cleanup",
            0,
            format!(
                "Cleaning {} retired vector-space namespace(s).",
                plan.target_vector_space_ids.len()
            ),
        );
    }

    let total = plan.target_vector_space_ids.len();
    let mut cleaned = Vec::<RetiredVectorSpaceCleanupCandidate>::new();
    let mut failures = Vec::<String>::new();

    for (index, vector_space_id) in plan.target_vector_space_ids.iter().enumerate() {
        if job_cancellation_requested(&state, &job_id).await {
            break;
        }
        {
            let mut state = state.write().await;
            state.update_job_snapshot(
                &job_id,
                "running",
                "cleanup",
                index,
                format!(
                    "Cleaning retired vector-space {vector_space_id} ({}/{total}).",
                    index + 1
                ),
            );
        }

        match cleanup_retired_vector_space_namespace(&plan.library_id, vector_space_id).await {
            Ok(()) => cleaned.push(RetiredVectorSpaceCleanupCandidate {
                library_id: plan.library_id.clone(),
                vector_space_id: vector_space_id.clone(),
            }),
            Err(error) => failures.push(format!("{vector_space_id}: {error}")),
        }
    }

    if job_cancellation_requested(&state, &job_id).await {
        let mut state = state.write().await;
        if let Err(message) = state.forget_cleaned_retired_vector_spaces(&cleaned) {
            tracing::warn!("Failed to persist maintenance cancel progress for {job_id}: {message}");
        }
        if let Err(message) = state.finalize_job_as_canceled(
            &job_id,
            cleaned.len(),
            if cleaned.is_empty() {
                "Cleanup canceled before deleting any retired vector-space namespace.".to_string()
            } else {
                format!(
                    "Cleanup canceled after deleting {} retired vector-space namespace(s).",
                    cleaned.len()
                )
            },
        ) {
            tracing::warn!("Failed to finalize canceled maintenance job {job_id}: {message}");
        }
        return;
    }

    let mut state = state.write().await;
    if let Err(message) = state.finalize_maintenance_action_job(&job_id, &plan, &cleaned, &failures)
    {
        tracing::warn!("Failed to finalize maintenance action job {job_id}: {message}");
    }
}
