use crate::{
    indexing::index_units_into_active_namespace,
    model::{
        ImportJobOutcome, MaintenanceActionPlan, PreparedImport, PreparedSourceAction,
        PreparedSourceMutation, RetiredVectorSpaceCleanupCandidate, SourceActionJobOutcome,
        SourceActionPlan, SourceActionSummary, UnitRecord, VectorSpaceExecutionGroup,
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

fn import_source_mutation(prepared: &PreparedImport, source_id: &str) -> PreparedSourceMutation {
    let source = prepared
        .sources
        .iter()
        .find(|source| source.id == source_id)
        .expect("import source id should exist")
        .clone();
    let assets = prepared
        .assets
        .iter()
        .filter(|asset| asset.source_id == source_id)
        .cloned()
        .collect::<Vec<_>>();
    let units = prepared
        .units
        .iter()
        .filter(|unit| unit.source_id == source_id)
        .cloned()
        .collect::<Vec<_>>();
    let source_asset_locations = prepared
        .source_asset_locations
        .iter()
        .filter(|location| location.source_id == source_id)
        .cloned()
        .collect::<Vec<_>>();
    let content_ids = std::iter::once(source.source_content_id.clone())
        .chain(units.iter().map(|unit| unit.content_id.clone()))
        .collect::<BTreeSet<_>>();
    let contents = prepared
        .contents
        .iter()
        .filter(|content| content_ids.contains(&content.id))
        .cloned()
        .collect::<Vec<_>>();

    PreparedSourceMutation {
        contents,
        source,
        source_asset_locations,
        assets,
        units,
    }
}

fn indexed_units_by_vector_space(
    vector_space_batches: impl Iterator<Item = (String, Vec<UnitRecord>)>,
    source_id: &str,
) -> Vec<(String, Vec<UnitRecord>)> {
    vector_space_batches
        .filter_map(|(vector_space_id, units)| {
            let units = units
                .into_iter()
                .filter(|unit| unit.source_id == source_id)
                .collect::<Vec<_>>();
            (!units.is_empty()).then_some((vector_space_id, units))
        })
        .collect()
}

async fn index_source_level_units(
    state: SharedState,
    job_id: &str,
    action_label: &str,
    execution_groups: &BTreeMap<String, VectorSpaceExecutionGroup>,
    indexed_units: &[(String, Vec<UnitRecord>)],
) -> Result<usize, String> {
    let mut indexed_count = 0usize;
    for (vector_space_id, units) in indexed_units {
        let Some(group) = execution_groups.get(vector_space_id) else {
            return Err(format!(
                "vector_space {vector_space_id}: missing resolved execution group"
            ));
        };
        let completed = index_units_into_active_namespace(
            vector_space_id,
            units,
            &group.resolved_model,
            state.clone(),
            job_id,
            action_label,
        )
        .await
        .map_err(|message| format!("vector_space {vector_space_id}: {message}"))?;
        indexed_count += completed;
    }
    Ok(indexed_count)
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
                match cleanup_retired_vector_space_namespace(&candidate.vector_space_id).await {
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

    let mut activated_vector_spaces = BTreeSet::new();
    let mut indexed_units = 0_usize;
    let mut committed_sources = 0_usize;
    let mut failures = Vec::new();

    for source in &prepared.sources {
        if job_cancellation_requested(&state, &job_id).await {
            break;
        }
        let mutation = import_source_mutation(&prepared, &source.id);
        let indexed_units_for_source = indexed_units_by_vector_space(
            prepared
                .vector_space_batches
                .iter()
                .map(|batch| (batch.vector_space_id.clone(), batch.units.clone())),
            &source.id,
        );
        match index_source_level_units(
            state.clone(),
            &job_id,
            "import",
            &execution_groups,
            &indexed_units_for_source,
        )
        .await
        {
            Ok(count) => {
                indexed_units += count;
                activated_vector_spaces.extend(
                    indexed_units_for_source
                        .iter()
                        .map(|(vector_space_id, _)| vector_space_id.clone()),
                );
            }
            Err(message) => {
                failures.push(format!("source {}: {message}", source.id));
                continue;
            }
        }

        let old_point_ids = {
            let mut state = state.write().await;
            match state.commit_source_records(
                &prepared.library_id,
                &job_id,
                &mutation.contents,
                &mutation.source,
                &mutation.source_asset_locations,
                &mutation.assets,
                &mutation.units,
                &indexed_units_for_source,
            ) {
                Ok(old_point_ids) => old_point_ids,
                Err(message) => {
                    failures.push(format!("source {}: {message}", source.id));
                    continue;
                }
            }
        };
        let _ = old_point_ids;
        committed_sources += 1;
    }

    let outcome = if job_cancellation_requested(&state, &job_id).await {
        if committed_sources == 0 {
            ImportJobOutcome::canceled(
                "Import canceled before any source-level activation.".to_string(),
                committed_sources,
            )
        } else {
            ImportJobOutcome::canceled_with_activations(
                format!(
                    "Import canceled after activating {} source(s) across {} vector space(s).",
                    committed_sources,
                    activated_vector_spaces.len()
                ),
                committed_sources,
                activated_vector_spaces,
            )
        }
    } else if failures.is_empty() {
        ImportJobOutcome::completed(
            format!(
                "Accepted {} path(s); activated {} source(s) and indexed {} unit(s) across {} vector space(s).",
                prepared.accepted.len(),
                committed_sources,
                indexed_units,
                activated_vector_spaces.len(),
            ),
            prepared.accepted.len(),
            activated_vector_spaces,
        )
    } else {
        ImportJobOutcome::failed_with_activations(
            "failed",
            format!(
                "Accepted {} path(s); activated {} source(s), indexed {} unit(s), and encountered {} failure(s): {}",
                prepared.accepted.len(),
                committed_sources,
                indexed_units,
                failures.len(),
                failures.join("; "),
            ),
            committed_sources,
            activated_vector_spaces,
        )
    };

    let mut state = state.write().await;
    if let Err(message) = state.finish_import_job_status(&job_id, outcome) {
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

            {
                let mut state = state.write().await;
                state.update_job_snapshot(
                    &job_id,
                    "running",
                    "encode",
                    0,
                    format!(
                        "Encoding {} source mutation(s) for {}.",
                        prepared.source_mutations.len(),
                        plan.action.as_str(),
                    ),
                );
            }

            let mut activated_vector_spaces = BTreeSet::new();
            let mut committed_sources = 0usize;
            let mut indexed_units = 0usize;
            let mut failures = Vec::new();
            for mutation in &prepared.source_mutations {
                if job_cancellation_requested(&state, &job_id).await {
                    break;
                }
                let indexed_units_for_source = indexed_units_by_vector_space(
                    prepared
                        .vector_space_batches
                        .iter()
                        .map(|batch| (batch.vector_space_id.clone(), batch.units_to_index.clone())),
                    &mutation.source.id,
                );
                match index_source_level_units(
                    state.clone(),
                    &job_id,
                    plan.action.as_str(),
                    &execution_groups,
                    &indexed_units_for_source,
                )
                .await
                {
                    Ok(count) => {
                        indexed_units += count;
                        activated_vector_spaces.extend(
                            indexed_units_for_source
                                .iter()
                                .map(|(vector_space_id, _)| vector_space_id.clone()),
                        );
                    }
                    Err(message) => {
                        failures.push(format!("source {}: {message}", mutation.source.id));
                        continue;
                    }
                }

                {
                    let mut state = state.write().await;
                    if let Err(message) = state.commit_source_records(
                        &prepared.library_id,
                        &job_id,
                        &mutation.contents,
                        &mutation.source,
                        &mutation.source_asset_locations,
                        &mutation.assets,
                        &mutation.units,
                        &indexed_units_for_source,
                    ) {
                        failures.push(format!("source {}: {message}", mutation.source.id));
                        continue;
                    }
                }
                committed_sources += 1;
            }

            let outcome = if job_cancellation_requested(&state, &job_id).await {
                if committed_sources == 0 {
                    SourceActionJobOutcome::canceled(
                        plan.action,
                        0,
                        "Canceled before activating any source updates.".to_string(),
                    )
                } else {
                    SourceActionJobOutcome::canceled_with_structured_changes(
                        plan.action,
                        prepared.accepted_root_count,
                        activated_vector_spaces,
                        format!(
                            "Canceled after activating {} source(s) and indexing {} unit(s).",
                            committed_sources, indexed_units
                        ),
                    )
                }
            } else if failures.is_empty() {
                SourceActionJobOutcome::completed(&prepared)
            } else {
                SourceActionJobOutcome::failed_with_structured_changes(
                    plan.action,
                    committed_sources,
                    activated_vector_spaces,
                    format!(
                        "activated {} source(s), indexed {} unit(s), and encountered {} failure(s): {}",
                        committed_sources,
                        indexed_units,
                        failures.len(),
                        failures.join("; "),
                    ),
                )
            };
            let mut state = state.write().await;
            if let Err(message) =
                state.finish_source_action_job_incremental(&job_id, prepared, outcome)
            {
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

        match cleanup_retired_vector_space_namespace(vector_space_id).await {
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
