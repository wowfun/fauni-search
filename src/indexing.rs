use crate::{
    api::{ApiError, SearchResultItem, TextSearchData},
    model::{PreparedImport, PreparedSourceAction, SearchPlan, SearchTimeRangeFilter},
    qdrant::*,
    query_assets::visual_unit_preview_reference,
    sidecar::{embed_documents, IndexingError, QueryEmbeddingResult},
    state::SharedState,
    DEFAULT_INDEX_EMBED_BATCH_ITEMS, MULTIVECTOR_INDEX_LINE,
};
use serde_json::{json, Value};
use std::env;

pub(crate) async fn index_visual_units(
    prepared: &PreparedImport,
    state: SharedState,
    job_id: &str,
) -> Result<String, IndexingError> {
    let batch_items = index_embed_batch_items();
    let total_batches = batch_count(prepared.visual_units.len(), batch_items);
    let stage_collection_name =
        staging_collection_name(&prepared.library_id, MULTIVECTOR_INDEX_LINE, job_id);
    let write_plan = resolve_qdrant_namespace_write_plan(
        &prepared.collection_name,
        &stage_collection_name,
        !prepared.had_existing_visual_units,
    )
    .await
    .map_err(|message| IndexingError {
        phase: "stage_write",
        message,
        completed: 0,
    })?;
    let mut stage_initialized = false;

    if matches!(
        write_plan.stage_strategy,
        StageCollectionStrategy::CloneFromActive { .. }
    ) {
        create_qdrant_stage_collection(&write_plan, None)
            .await
            .map_err(|message| IndexingError {
                phase: "stage_write",
                message,
                completed: 0,
            })?;
        stage_initialized = true;
    }

    for (batch_index, visual_unit_batch) in prepared.visual_units.chunks(batch_items).enumerate() {
        {
            let mut state = state.write().await;
            state.update_job_snapshot(
                job_id,
                "running",
                "encode",
                0,
                format!(
                    "Encoding batch {}/{} ({} visual unit(s)) for staged multivector indexing.",
                    batch_index + 1,
                    total_batches,
                    visual_unit_batch.len()
                ),
            );
        }

        let embeddings = match embed_documents(visual_unit_batch).await {
            Ok(embeddings) => embeddings,
            Err(error) => {
                if stage_initialized {
                    best_effort_delete_qdrant_collection(&stage_collection_name).await;
                }
                return Err(error);
            }
        };

        {
            let mut state = state.write().await;
            state.update_job_snapshot(
                job_id,
                "running",
                "stage_write",
                0,
                format!(
                    "Writing batch {}/{} ({} visual unit(s)) into staged multivector storage.",
                    batch_index + 1,
                    total_batches,
                    visual_unit_batch.len()
                ),
            );
        }

        if !stage_initialized {
            let vector_size = embeddings
                .first()
                .and_then(|embedding| embedding.vectors.first())
                .map(Vec::len)
                .unwrap_or_default();
            if let Err(message) =
                create_qdrant_stage_collection(&write_plan, Some(vector_size)).await
            {
                return Err(IndexingError {
                    phase: "stage_write",
                    message,
                    completed: 0,
                });
            }
            stage_initialized = true;
        }

        if let Err(message) =
            upsert_qdrant_points(&stage_collection_name, visual_unit_batch, &embeddings).await
        {
            best_effort_delete_qdrant_collection(&stage_collection_name).await;
            return Err(IndexingError {
                phase: "stage_write",
                message,
                completed: 0,
            });
        }
    }

    if !stage_initialized {
        return Err(IndexingError {
            phase: "stage_write",
            message: "No staged Qdrant collection was created for the import job.".to_string(),
            completed: 0,
        });
    }

    if let Err(message) = validate_qdrant_collection(&stage_collection_name).await {
        best_effort_delete_qdrant_collection(&stage_collection_name).await;
        return Err(IndexingError {
            phase: "stage_write",
            message,
            completed: 0,
        });
    }

    if let Err(message) = switch_qdrant_active_alias(&write_plan).await {
        best_effort_delete_qdrant_collection(&stage_collection_name).await;
        return Err(IndexingError {
            phase: "activated",
            message,
            completed: prepared.accepted.len(),
        });
    }
    best_effort_cleanup_retired_stage_collections(&write_plan).await;

    Ok(format!(
        "Accepted {} path(s); indexed {} visual unit(s) into staged multivector storage and atomically activated the active namespace.",
        prepared.accepted.len(),
        prepared.visual_units.len()
    ))
}

pub(crate) async fn index_source_action_visual_units(
    prepared: &PreparedSourceAction,
    state: SharedState,
    job_id: &str,
) -> Result<(), String> {
    let batch_items = index_embed_batch_items();
    let total_batches = batch_count(prepared.visual_units_to_index.len(), batch_items);
    let stage_collection_name =
        staging_collection_name(&prepared.library_id, MULTIVECTOR_INDEX_LINE, job_id);
    let write_plan = resolve_qdrant_namespace_write_plan(
        &prepared.collection_name,
        &stage_collection_name,
        !prepared.had_existing_visual_units || prepared.can_rebuild_from_scratch,
    )
    .await?;
    let mut stage_initialized = false;

    if matches!(
        write_plan.stage_strategy,
        StageCollectionStrategy::CloneFromActive { .. }
    ) {
        create_qdrant_stage_collection(&write_plan, None).await?;
        stage_initialized = true;
    }

    if stage_initialized && !prepared.stale_point_ids.is_empty() {
        {
            let mut state = state.write().await;
            state.update_job_snapshot(
                job_id,
                "running",
                "stage_write",
                0,
                format!(
                    "Deleting {} stale point(s) from staged multivector storage.",
                    prepared.stale_point_ids.len()
                ),
            );
        }
        if let Err(message) =
            delete_qdrant_points(&stage_collection_name, &prepared.stale_point_ids).await
        {
            best_effort_delete_qdrant_collection(&stage_collection_name).await;
            return Err(message);
        }
    }

    for (batch_index, visual_unit_batch) in prepared
        .visual_units_to_index
        .chunks(batch_items)
        .enumerate()
    {
        {
            let mut state = state.write().await;
            state.update_job_snapshot(
                job_id,
                "running",
                "encode",
                0,
                format!(
                    "Encoding batch {}/{} ({} visual unit(s)) for {}.",
                    batch_index + 1,
                    total_batches,
                    visual_unit_batch.len(),
                    prepared.action.as_str(),
                ),
            );
        }

        let embeddings = match embed_documents(visual_unit_batch).await {
            Ok(embeddings) => embeddings,
            Err(error) => {
                if stage_initialized {
                    best_effort_delete_qdrant_collection(&stage_collection_name).await;
                }
                return Err(error.message);
            }
        };

        {
            let mut state = state.write().await;
            state.update_job_snapshot(
                job_id,
                "running",
                "stage_write",
                0,
                format!(
                    "Writing batch {}/{} ({} visual unit(s)) into staged multivector storage.",
                    batch_index + 1,
                    total_batches,
                    visual_unit_batch.len()
                ),
            );
        }

        if !stage_initialized {
            let vector_size = embeddings
                .first()
                .and_then(|embedding| embedding.vectors.first())
                .map(Vec::len)
                .unwrap_or_default();
            create_qdrant_stage_collection(&write_plan, Some(vector_size)).await?;
            stage_initialized = true;
        }

        if let Err(message) =
            upsert_qdrant_points(&stage_collection_name, visual_unit_batch, &embeddings).await
        {
            best_effort_delete_qdrant_collection(&stage_collection_name).await;
            return Err(message);
        }
    }

    if !stage_initialized {
        return Ok(());
    }

    validate_qdrant_collection(&stage_collection_name).await?;
    if let Err(message) = switch_qdrant_active_alias(&write_plan).await {
        best_effort_delete_qdrant_collection(&stage_collection_name).await;
        return Err(message);
    }
    best_effort_cleanup_retired_stage_collections(&write_plan).await;
    Ok(())
}

pub(crate) fn index_embed_batch_items() -> usize {
    read_optional_usize_env("INDEX_EMBED_BATCH_ITEMS", DEFAULT_INDEX_EMBED_BATCH_ITEMS).max(1)
}

pub(crate) fn read_optional_usize_env(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

pub(crate) fn batch_count(total_items: usize, batch_items: usize) -> usize {
    if total_items == 0 {
        0
    } else {
        ((total_items - 1) / batch_items) + 1
    }
}

pub(crate) fn build_search_response(
    plan: SearchPlan,
    embedding: QueryEmbeddingResult,
    candidates: Vec<QdrantScoredPoint>,
) -> Result<TextSearchData, ApiError> {
    let result_count = candidates.len();
    let top_score = candidates.first().map(|point| point.score);
    let filtered_candidates = candidates
        .into_iter()
        .filter_map(|point| point.payload.map(|payload| (point.score, payload)))
        .filter(|(_, payload)| search_payload_matches_filters(&plan, payload))
        .collect::<Vec<_>>();
    let filtered_result_count = filtered_candidates.len();
    let page_start = plan.cursor_offset.min(filtered_result_count);
    let raw_scores = filtered_candidates
        .iter()
        .map(|(score, payload)| {
            json!({
                "visual_unit_id": payload.visual_unit_id,
                "score": score,
            })
        })
        .collect::<Vec<_>>();
    let results = filtered_candidates
        .iter()
        .skip(page_start)
        .take(plan.top_k)
        .enumerate()
        .map(|(page_index, (score, payload))| {
            let preview = visual_unit_preview_reference(
                &plan.library_id,
                &payload.visual_unit_id,
                &payload.kind,
                &payload.locator,
            )?;
            Ok(SearchResultItem {
                visual_unit_id: payload.visual_unit_id.clone(),
                source_id: payload.source_id.clone(),
                preview,
                source_path: payload.source_path.clone(),
                source_type: payload.source_type.clone(),
                kind: payload.kind.clone(),
                locator: payload.locator.clone(),
                cursor: encode_search_cursor(page_start + page_index + 1),
                score: Some(*score),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let returned_result_count = results.len();
    let next_offset = page_start + returned_result_count;
    let next_cursor = (next_offset < filtered_result_count).then(|| encode_search_cursor(next_offset));
    let index_lines_debug = plan
        .target_index_lines
        .iter()
        .map(|index_line| {
            json!({
                "index_line": index_line,
                "raw_scores": raw_scores.clone(),
            })
        })
        .collect::<Vec<_>>();

    Ok(TextSearchData {
        results,
        next_cursor,
        debug: plan.debug.then_some(json!({
            "backend": "qdrant",
            "repr_kind": "multivector",
            "provider": {
                "model_profile": "local_python",
                "retrieval_backend": "qdrant",
            },
            "index_lines": index_lines_debug,
            "query_vector_count": embedding.vectors.len(),
            "retrieved_points": result_count,
            "filtered_results": filtered_result_count,
            "returned_results": returned_result_count,
            "top_score": top_score,
        })),
    })
}

fn encode_search_cursor(offset: usize) -> String {
    format!("search:v1:{offset}")
}

fn search_payload_matches_filters(plan: &SearchPlan, payload: &QdrantPointPayload) -> bool {
    plan.active_visual_unit_ids.contains(&payload.visual_unit_id)
        && plan
            .kind_filter
            .as_ref()
            .map(|expected| expected.contains(&payload.kind))
            .unwrap_or(true)
        && plan
            .source_type_filter
            .as_ref()
            .map(|expected| expected.contains(&payload.source_type))
            .unwrap_or(true)
        && plan
            .path_prefix_filter
            .as_ref()
            .map(|prefixes| {
                prefixes
                    .iter()
                    .any(|prefix| payload.source_path.starts_with(prefix))
            })
            .unwrap_or(true)
        && plan
            .time_range_filter
            .map(|filter| payload_overlaps_time_range(&payload.locator, filter))
            .unwrap_or(true)
}

fn payload_overlaps_time_range(locator: &Value, filter: SearchTimeRangeFilter) -> bool {
    let Some(start_ms) = locator.get("start_ms").and_then(Value::as_u64) else {
        return false;
    };
    let Some(end_ms) = locator.get("end_ms").and_then(Value::as_u64) else {
        return false;
    };

    start_ms <= filter.end_ms && end_ms >= filter.start_ms
}
