use crate::{
    api::{ApiError, SearchResultItem, TextSearchData},
    model::{
        PreparedImportVectorSpaceBatch, PreparedSourceActionVectorSpaceBatch,
        ResolvedExecutionModelSelection, SearchPlan, SearchTimeRangeFilter,
    },
    provider::provider_context_payload,
    qdrant::*,
    query_assets::visual_unit_preview_reference,
    sidecar::{embed_documents, IndexingError, QueryEmbeddingResult},
    state::SharedState,
    DEFAULT_INDEX_EMBED_BATCH_ITEMS,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;

pub(crate) async fn index_visual_units(
    library_id: &str,
    prepared: &PreparedImportVectorSpaceBatch,
    resolved_model: &ResolvedExecutionModelSelection,
    state: SharedState,
    job_id: &str,
) -> Result<usize, IndexingError> {
    if prepared.visual_units.is_empty() {
        return Ok(0);
    }
    let batch_items = index_embed_batch_items();
    let total_batches = batch_count(prepared.visual_units.len(), batch_items);
    let progress_total = prepared.visual_units.len();
    let mut completed_visual_units = 0_usize;
    let stage_collection_name =
        staging_vector_space_collection_name(library_id, &resolved_model.vector_space_id, job_id);
    let write_plan = resolve_qdrant_namespace_write_plan(
        &stable_vector_space_name(library_id, &resolved_model.vector_space_id),
        &stage_collection_name,
        !prepared.had_existing_index,
    )
    .await
    .map_err(|message| IndexingError {
        phase: "stage_write",
        message,
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
            })?;
        stage_initialized = true;
    }

    if stage_initialized && !prepared.stale_point_ids.is_empty() {
        {
            let mut state = state.write().await;
            state.update_job_progress_snapshot(
                job_id,
                "running",
                "stage_write",
                0,
                progress_total,
                "visual_unit",
                format!(
                    "Deleting {} stale point(s) from staged vector-space storage.",
                    prepared.stale_point_ids.len()
                ),
            );
        }

        if let Err(message) =
            delete_qdrant_points(&stage_collection_name, &prepared.stale_point_ids).await
        {
            best_effort_delete_qdrant_collection(&stage_collection_name).await;
            return Err(IndexingError {
                phase: "stage_write",
                message,
            });
        }
    }

    for (batch_index, visual_unit_batch) in prepared.visual_units.chunks(batch_items).enumerate() {
        {
            let mut state = state.write().await;
            state.update_job_progress_snapshot(
                job_id,
                "running",
                "encode",
                completed_visual_units,
                progress_total,
                "visual_unit",
                format!(
                    "Encoding batch {}/{} ({} visual unit(s)) for staged vector-space indexing.",
                    batch_index + 1,
                    total_batches,
                    visual_unit_batch.len()
                ),
            );
        }

        let embeddings = match embed_documents(
            visual_unit_batch,
            Some(provider_context_payload(resolved_model)),
        )
        .await
        {
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
            state.update_job_progress_snapshot(
                job_id,
                "running",
                "stage_write",
                completed_visual_units,
                progress_total,
                "visual_unit",
                format!(
                    "Writing batch {}/{} ({} visual unit(s)) into staged vector-space storage.",
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
            });
        }

        completed_visual_units += visual_unit_batch.len();
        {
            let mut state = state.write().await;
            state.update_job_progress_snapshot(
                job_id,
                "running",
                "stage_write",
                completed_visual_units,
                progress_total,
                "visual_unit",
                format!(
                    "Wrote batch {}/{} ({} visual unit(s)) into staged vector-space storage.",
                    batch_index + 1,
                    total_batches,
                    visual_unit_batch.len()
                ),
            );
        }
    }

    if !stage_initialized {
        return Err(IndexingError {
            phase: "stage_write",
            message: "No staged Qdrant collection was created for the import job.".to_string(),
        });
    }

    if let Err(message) = validate_qdrant_collection(&stage_collection_name).await {
        best_effort_delete_qdrant_collection(&stage_collection_name).await;
        return Err(IndexingError {
            phase: "stage_write",
            message,
        });
    }

    {
        let mut state = state.write().await;
        state.update_job_progress_snapshot(
            job_id,
            "running",
            "activated",
            completed_visual_units,
            progress_total,
            "visual_unit",
            "Activating staged vector-space storage.",
        );
    }

    if let Err(message) = switch_qdrant_active_alias(&write_plan).await {
        best_effort_delete_qdrant_collection(&stage_collection_name).await;
        return Err(IndexingError {
            phase: "activated",
            message,
        });
    }
    best_effort_cleanup_retired_stage_collections(&write_plan).await;
    Ok(completed_visual_units)
}

pub(crate) async fn index_source_action_visual_units(
    library_id: &str,
    action: &str,
    prepared: &PreparedSourceActionVectorSpaceBatch,
    resolved_model: &ResolvedExecutionModelSelection,
    state: SharedState,
    job_id: &str,
) -> Result<usize, String> {
    let batch_items = index_embed_batch_items();
    let total_batches = batch_count(prepared.visual_units_to_index.len(), batch_items);
    let progress_total = prepared.visual_units_to_index.len();
    let mut completed_visual_units = 0_usize;
    let stage_collection_name =
        staging_vector_space_collection_name(library_id, &resolved_model.vector_space_id, job_id);
    let write_plan = resolve_qdrant_namespace_write_plan(
        &stable_vector_space_name(library_id, &resolved_model.vector_space_id),
        &stage_collection_name,
        !prepared.had_existing_index || prepared.can_rebuild_from_scratch,
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
            state.update_job_progress_snapshot(
                job_id,
                "running",
                "stage_write",
                0,
                progress_total,
                "visual_unit",
                format!(
                    "Deleting {} stale point(s) from staged vector-space storage.",
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
            state.update_job_progress_snapshot(
                job_id,
                "running",
                "encode",
                completed_visual_units,
                progress_total,
                "visual_unit",
                format!(
                    "Encoding batch {}/{} ({} visual unit(s)) for {}.",
                    batch_index + 1,
                    total_batches,
                    visual_unit_batch.len(),
                    action,
                ),
            );
        }

        let embeddings = match embed_documents(
            visual_unit_batch,
            Some(provider_context_payload(resolved_model)),
        )
        .await
        {
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
            state.update_job_progress_snapshot(
                job_id,
                "running",
                "stage_write",
                completed_visual_units,
                progress_total,
                "visual_unit",
                format!(
                    "Writing batch {}/{} ({} visual unit(s)) into staged vector-space storage.",
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

        completed_visual_units += visual_unit_batch.len();
        {
            let mut state = state.write().await;
            state.update_job_progress_snapshot(
                job_id,
                "running",
                "stage_write",
                completed_visual_units,
                progress_total,
                "visual_unit",
                format!(
                    "Wrote batch {}/{} ({} visual unit(s)) into staged vector-space storage.",
                    batch_index + 1,
                    total_batches,
                    visual_unit_batch.len()
                ),
            );
        }
    }

    if !stage_initialized {
        return Ok(0);
    }

    validate_qdrant_collection(&stage_collection_name).await?;
    {
        let mut state = state.write().await;
        state.update_job_progress_snapshot(
            job_id,
            "running",
            "activated",
            completed_visual_units,
            progress_total,
            "visual_unit",
            "Activating staged vector-space storage.",
        );
    }
    if let Err(message) = switch_qdrant_active_alias(&write_plan).await {
        best_effort_delete_qdrant_collection(&stage_collection_name).await;
        return Err(message);
    }
    best_effort_cleanup_retired_stage_collections(&write_plan).await;
    Ok(completed_visual_units)
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
    executed_groups: Vec<ExecutedSearchGroup>,
) -> Result<TextSearchData, ApiError> {
    let result_count = executed_groups
        .iter()
        .map(|group| group.candidates.len())
        .sum::<usize>();
    let top_score = executed_groups
        .iter()
        .flat_map(|group| group.candidates.iter().map(|point| point.score))
        .max_by(|left, right| left.total_cmp(right));
    let mut filtered_candidates = executed_groups
        .iter()
        .flat_map(|group| {
            group.candidates.iter().filter_map(|point| {
                point
                    .payload
                    .clone()
                    .map(|payload| (point.score, group.library_id.clone(), payload))
            })
        })
        .into_iter()
        .filter(|(_, library_id, payload)| {
            search_payload_matches_filters(&plan, library_id, payload)
        })
        .collect::<Vec<_>>();
    filtered_candidates.sort_by(|left, right| right.0.total_cmp(&left.0));
    let filtered_result_count = filtered_candidates.len();
    let page_start = plan.cursor_offset.min(filtered_result_count);
    let results = filtered_candidates
        .iter()
        .skip(page_start)
        .take(plan.top_k)
        .enumerate()
        .map(|(page_index, (score, library_id, payload))| {
            let preview = visual_unit_preview_reference(
                library_id,
                &payload.visual_unit_id,
                &payload.kind,
                &payload.locator,
            )?;
            Ok(SearchResultItem {
                library_id: library_id.clone(),
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
    let next_cursor =
        (next_offset < filtered_result_count).then(|| encode_search_cursor(next_offset));
    let content_types_debug = plan
        .debug_content_types
        .iter()
        .map(|entry| {
            let raw_scores = executed_groups
                .iter()
                .filter(|group| group.library_id == entry.library_id)
                .flat_map(|group| {
                    group.candidates.iter().filter_map(|point| {
                        let payload = point.payload.as_ref()?;
                        if !content_type_matches_visual_unit(&entry.content_type, &payload.kind) {
                            return None;
                        }
                        Some(json!({
                            "library_id": entry.library_id,
                            "visual_unit_id": payload.visual_unit_id,
                            "score": point.score,
                        }))
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "library_id": entry.library_id,
                "content_type": entry.content_type,
                "resolved_model": entry.resolved_model.clone(),
                "raw_scores": raw_scores,
            })
        })
        .collect::<Vec<_>>();
    let mut vector_spaces = BTreeMap::new();
    for group in &plan.execution_groups {
        let selection = &group.resolved_model.summary;
        let entry = vector_spaces
            .entry(group.vector_space_id.clone())
            .or_insert_with(|| {
                json!({
                    "library_id": group.library_id,
                    "vector_space_id": group.vector_space_id,
                    "provider_id": selection.provider_id,
                    "provider_kind": selection.provider_kind,
                    "model_id": selection.model_id,
                    "model_version": selection.model_version,
                    "model_revision": selection.model_revision,
                    "vector_type": group.resolved_model.vector_type,
                    "execution_input_types": group.resolved_model.execution_input_types.clone(),
                    "status": selection.status,
                    "content_types": group.content_types,
                })
            });
        let _ = entry;
    }

    Ok(TextSearchData {
        results,
        next_cursor,
        unsupported_content_types: plan.unsupported_content_types,
        debug: plan.debug.then_some(json!({
            "backend": "qdrant",
            "vector_type": "multi_vector_late_interaction",
            "content_types": content_types_debug,
            "vector_spaces": vector_spaces.into_values().collect::<Vec<_>>(),
            "query_vector_count": executed_groups
                .iter()
                .map(|group| group.query_embedding.vectors.len())
                .sum::<usize>(),
            "retrieved_points": result_count,
            "filtered_results": filtered_result_count,
            "returned_results": returned_result_count,
            "top_score": top_score,
        })),
    })
}

pub(crate) struct ExecutedSearchGroup {
    pub(crate) library_id: String,
    pub(crate) query_embedding: QueryEmbeddingResult,
    pub(crate) candidates: Vec<QdrantScoredPoint>,
}

fn encode_search_cursor(offset: usize) -> String {
    format!("search:v1:{offset}")
}

fn search_payload_matches_filters(
    plan: &SearchPlan,
    library_id: &str,
    payload: &QdrantPointPayload,
) -> bool {
    plan.active_visual_unit_refs
        .contains(&format!("{library_id}:{}", payload.visual_unit_id))
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

fn content_type_matches_visual_unit(content_type: &str, visual_unit_kind: &str) -> bool {
    match content_type {
        "image" => visual_unit_kind == "image",
        "document" => visual_unit_kind == "document_page",
        "video" => visual_unit_kind == "video_segment",
        "text" => visual_unit_kind == "text",
        _ => false,
    }
}
