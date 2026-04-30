use crate::{
    api::{ApiError, SearchResultItem, TextSearchData},
    model::{
        ResolvedExecutionModelSelection, SearchPlan, SearchTimeRangeFilter, UnitIndexRecord,
        UnitRecord,
    },
    provider::provider_context_payload,
    qdrant::*,
    query_assets::asset_preview_reference,
    sidecar::{embed_documents, QueryEmbeddingResult},
    state::SharedState,
    DEFAULT_INDEX_EMBED_BATCH_ITEMS,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;

pub(crate) async fn index_units_into_active_namespace(
    vector_space_id: &str,
    units: &[UnitRecord],
    resolved_model: &ResolvedExecutionModelSelection,
    state: SharedState,
    job_id: &str,
    action_label: &str,
) -> Result<usize, String> {
    if units.is_empty() {
        return Ok(0);
    }

    let batch_items = index_embed_batch_items();
    let total_batches = batch_count(units.len(), batch_items);
    let progress_total = units.len();
    let mut completed_units = 0_usize;
    let mut collection_name = stable_vector_space_name(vector_space_id);
    let mut namespace_ready = false;

    for (batch_index, unit_batch) in units.chunks(batch_items).enumerate() {
        {
            let mut state = state.write().await;
            state.update_job_progress_snapshot(
                job_id,
                "running",
                "encode",
                completed_units,
                progress_total,
                "unit",
                format!(
                    "Encoding source-level batch {}/{} ({} unit(s)) for {}.",
                    batch_index + 1,
                    total_batches,
                    unit_batch.len(),
                    action_label,
                ),
            );
        }

        let embeddings =
            embed_documents(unit_batch, Some(provider_context_payload(resolved_model)))
                .await
                .map_err(|error| error.message)?;

        if !namespace_ready {
            let vector_size = embeddings
                .first()
                .and_then(|embedding| embedding.vectors.first())
                .map(Vec::len)
                .unwrap_or_default();
            collection_name = ensure_active_qdrant_namespace(vector_space_id, vector_size).await?;
            namespace_ready = true;
        }

        {
            let mut state = state.write().await;
            state.update_job_progress_snapshot(
                job_id,
                "running",
                "vector_write",
                completed_units,
                progress_total,
                "unit",
                format!(
                    "Writing source-level batch {}/{} ({} unit(s)) into active vector-space storage.",
                    batch_index + 1,
                    total_batches,
                    unit_batch.len()
                ),
            );
        }

        upsert_qdrant_points(&collection_name, unit_batch, &embeddings).await?;
        completed_units += unit_batch.len();

        {
            let mut state = state.write().await;
            state.update_job_progress_snapshot(
                job_id,
                "running",
                "vector_write",
                completed_units,
                progress_total,
                "unit",
                format!(
                    "Wrote source-level batch {}/{} ({} unit(s)) into active vector-space storage.",
                    batch_index + 1,
                    total_batches,
                    unit_batch.len()
                ),
            );
        }
    }

    Ok(completed_units)
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
            group
                .candidates
                .iter()
                .enumerate()
                .filter_map(|(rank, point)| {
                    point.payload.clone().map(|payload| {
                        (
                            point.score,
                            group.vector_space_id.clone(),
                            rank + 1,
                            group.library_id.clone(),
                            payload,
                        )
                    })
                })
        })
        .into_iter()
        .filter(|(_, vector_space_id, _, library_id, payload)| {
            search_payload_matches_filters(&plan, library_id, vector_space_id, payload)
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
        .map(
            |(page_index, (score, vector_space_id, rank, library_id, payload))| {
                let location = plan
                    .asset_locations
                    .get(&format!("{library_id}:{}", payload.asset_id))
                    .ok_or_else(|| {
                        ApiError::not_ready(
                            "Search result references an asset without an active source location.",
                            Some(json!({
                                "library_id": library_id,
                                "asset_id": payload.asset_id,
                            })),
                        )
                    })?;
                let preview = asset_preview_reference(
                    library_id,
                    &payload.asset_id,
                    &payload.asset_type,
                    &location.locator,
                )?;
                Ok(SearchResultItem {
                    library_id: library_id.clone(),
                    asset_id: payload.asset_id.clone(),
                    asset_type: payload.asset_type.clone(),
                    source_id: location.source_id.clone(),
                    preview,
                    source_uri: location.source_uri.clone(),
                    source_type: location.source_type.clone(),
                    locator: location.locator.clone(),
                    matched_units: vec![crate::api::MatchedUnitEvidence {
                        unit_id: payload.unit_id.clone(),
                        unit_type: payload.unit_type.clone(),
                        vector_space_id: vector_space_id.clone(),
                        rank: *rank,
                        raw_score: *score,
                    }],
                    cursor: encode_search_cursor(page_start + page_index + 1),
                    score: Some(*score),
                })
            },
        )
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
                        if !content_type_matches_asset(&entry.content_type, &payload.asset_type) {
                            return None;
                        }
                        Some(json!({
                            "library_id": entry.library_id,
                            "asset_id": payload.asset_id,
                            "unit_id": payload.unit_id,
                            "vector_space_id": group.vector_space_id,
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
    let prefilter = plan
        .execution_groups
        .iter()
        .map(|group| {
            json!({
                "library_id": group.library_id,
                "vector_space_id": group.vector_space_id,
                "enabled": true,
                "mode": "point_allow_list",
                "candidate_point_count": group.eligible_point_ids.len(),
                "pushed_fields": ["point_id"],
            })
        })
        .collect::<Vec<_>>();
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
            "prefilter": prefilter,
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
    pub(crate) vector_space_id: String,
    pub(crate) query_embedding: QueryEmbeddingResult,
    pub(crate) candidates: Vec<QdrantScoredPoint>,
}

fn encode_search_cursor(offset: usize) -> String {
    format!("search:v1:{offset}")
}

fn search_payload_matches_filters(
    plan: &SearchPlan,
    library_id: &str,
    vector_space_id: &str,
    payload: &QdrantPointPayload,
) -> bool {
    let scoped_asset_ref = format!("{library_id}:{}", payload.asset_id);
    let Some(location) = plan.asset_locations.get(&scoped_asset_ref) else {
        return false;
    };
    let unit_index_ref = UnitIndexRecord::key(&payload.unit_id, vector_space_id);
    plan.active_asset_refs.contains(&scoped_asset_ref)
        && plan.active_unit_index_refs.contains(&unit_index_ref)
        && plan
            .kind_filter
            .as_ref()
            .map(|expected| expected.contains(&payload.asset_type))
            .unwrap_or(true)
        && plan
            .source_type_filter
            .as_ref()
            .map(|expected| expected.contains(&location.source_type))
            .unwrap_or(true)
        && plan
            .path_prefix_filter
            .as_ref()
            .map(|prefixes| {
                prefixes
                    .iter()
                    .any(|prefix| location.source_uri.starts_with(prefix))
            })
            .unwrap_or(true)
        && plan
            .time_range_filter
            .map(|filter| payload_overlaps_time_range(&location.locator, filter))
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

fn content_type_matches_asset(content_type: &str, asset_type: &str) -> bool {
    match content_type {
        "image" => asset_type == "image",
        "document" => asset_type == "document_page",
        "video" => asset_type == "video_segment",
        "text" => asset_type == "text",
        _ => false,
    }
}
