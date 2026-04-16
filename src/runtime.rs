use crate::{
    indexing::{index_source_action_visual_units, index_visual_units},
    model::{
        ImportJobOutcome, PreparedImport, PreparedSourceAction, SourceActionJobOutcome,
        SourceActionPlan, SourceActionSummary,
    },
    state::SharedState,
    SOURCE_WATCHER_POLL_INTERVAL_SECS, TEMP_QUERY_ASSET_REAPER_INTERVAL_SECS,
};
use std::time::Duration;

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
    {
        let mut state = state.write().await;
        state.update_job_snapshot(
            &job_id,
            "running",
            "encode",
            0,
            format!(
                "Encoding {} accepted path(s) into multivector embeddings.",
                prepared.accepted.len()
            ),
        );
    }

    let outcome = match index_visual_units(&prepared, state.clone(), &job_id).await {
        Ok(summary) => ImportJobOutcome::completed(summary, prepared.accepted.len()),
        Err(error) => ImportJobOutcome::failed(error.phase, error.message, error.completed),
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
    {
        let mut state = state.write().await;
        state.mark_source_action_running(&plan, &job_id);
    }

    let prepared = {
        let mut state = state.write().await;
        state.prepare_source_action_execution(&plan)
    };

    let outcome = match prepared {
        Ok(prepared) => {
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
                            prepared.visual_units_to_index.len(),
                            plan.action.as_str(),
                        ),
                    );
                }

                let indexing =
                    index_source_action_visual_units(&prepared, state.clone(), &job_id).await;
                match indexing {
                    Ok(()) => {
                        let outcome = SourceActionJobOutcome::completed(&prepared);
                        let mut state = state.write().await;
                        if let Err(message) =
                            state.finalize_source_action_job(&job_id, prepared, outcome)
                        {
                            tracing::warn!(
                                "Failed to finalize source action job {job_id}: {message}"
                            );
                        }
                        return;
                    }
                    Err(message) => {
                        let mut state = state.write().await;
                        if let Err(finalize_error) = state.finalize_source_action_job(
                            &job_id,
                            prepared,
                            SourceActionJobOutcome::failed(plan.action, 0, message),
                        ) {
                            tracing::warn!(
                                "Failed to finalize failed source action job {job_id}: {finalize_error}"
                            );
                        }
                        return;
                    }
                }
            }

            let outcome = SourceActionJobOutcome::completed(&prepared);
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
        collection_name: String::new(),
        action: plan.action,
        accepted_root_count: plan.target_root_ids.len(),
        can_rebuild_from_scratch: false,
        had_existing_visual_units: false,
        root_updates: Vec::new(),
        source_mutations: Vec::new(),
        stale_point_ids: Vec::new(),
        visual_units_to_index: Vec::new(),
        summary: SourceActionSummary::default(),
    };
    if let Err(message) = state.finalize_source_action_job(&job_id, prepared, outcome) {
        tracing::warn!("Failed to finalize source action job {job_id}: {message}");
    }
}
