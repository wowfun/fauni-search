use crate::{
    api::ApiError,
    model::{ActiveNamespaceProbeResult, VisualUnitRecord},
    sidecar::{QueryEmbeddingResult, SidecarEmbeddingItem},
    DEFAULT_QDRANT_MAX_UPSERT_BODY_BYTES, QDRANT_UPSERT_BODY_OVERHEAD_BYTES,
};
use axum::http::StatusCode;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::BTreeSet, env, time::Duration};

#[derive(Deserialize)]
pub(crate) struct QdrantQueryResponse {
    pub(crate) result: QdrantQueryResult,
}

#[derive(Deserialize)]
pub(crate) struct QdrantQueryResult {
    pub(crate) points: Vec<QdrantScoredPoint>,
}

#[derive(Clone, Deserialize)]
pub(crate) struct QdrantScoredPoint {
    pub(crate) score: f32,
    pub(crate) payload: Option<QdrantPointPayload>,
}

#[derive(Clone, Deserialize)]
pub(crate) struct QdrantPointPayload {
    pub(crate) visual_unit_id: String,
    pub(crate) source_id: String,
    pub(crate) source_path: String,
    pub(crate) source_type: String,
    pub(crate) kind: String,
    pub(crate) locator: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StageCollectionStrategy {
    Fresh,
    CloneFromActive { target_collection: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct QdrantNamespaceWritePlan {
    pub(crate) alias_name: String,
    pub(crate) alias_exists: bool,
    pub(crate) previous_target_collection: Option<String>,
    pub(crate) stage_collection_name: String,
    pub(crate) stage_strategy: StageCollectionStrategy,
}

pub(crate) fn staging_vector_space_collection_name(
    library_id: &str,
    vector_space_id: &str,
    job_id: &str,
) -> String {
    format!("vector_space_stage_{library_id}_{vector_space_id}_{job_id}")
}

pub(crate) async fn resolve_qdrant_namespace_write_plan(
    alias_name: &str,
    stage_collection_name: &str,
    allow_fresh_without_active: bool,
) -> Result<QdrantNamespaceWritePlan, String> {
    match probe_active_qdrant_namespace(alias_name).await? {
        ActiveNamespaceProbeResult::Ready { target_collection } => Ok(QdrantNamespaceWritePlan {
            alias_name: alias_name.to_string(),
            alias_exists: true,
            previous_target_collection: Some(target_collection.clone()),
            stage_collection_name: stage_collection_name.to_string(),
            stage_strategy: StageCollectionStrategy::CloneFromActive { target_collection },
        }),
        ActiveNamespaceProbeResult::Missing => {
            if !allow_fresh_without_active {
                return Err(
                    "The active vector space namespace is missing. Run a full library rescan to rebuild the index before applying incremental updates."
                        .to_string(),
                );
            }
            Ok(QdrantNamespaceWritePlan {
                alias_name: alias_name.to_string(),
                alias_exists: false,
                previous_target_collection: None,
                stage_collection_name: stage_collection_name.to_string(),
                stage_strategy: StageCollectionStrategy::Fresh,
            })
        }
        ActiveNamespaceProbeResult::MissingTarget { target_collection } => {
            if !allow_fresh_without_active {
                return Err(format!(
                    "The active vector space namespace alias points to missing collection {target_collection}. Run a full library rescan to rebuild the index before applying incremental updates."
                ));
            }
            Ok(QdrantNamespaceWritePlan {
                alias_name: alias_name.to_string(),
                alias_exists: true,
                previous_target_collection: Some(target_collection),
                stage_collection_name: stage_collection_name.to_string(),
                stage_strategy: StageCollectionStrategy::Fresh,
            })
        }
        ActiveNamespaceProbeResult::LegacyDirectCollection => Err(format!(
            "Legacy direct Qdrant collection {alias_name} blocks the active alias namespace. Remove the old physical index_* collection manually, then retry."
        )),
    }
}

pub(crate) fn build_qdrant_collection_create_payload(
    vector_size: usize,
    init_from: Option<&str>,
) -> Value {
    let mut payload = json!({
        "vectors": {
            "mv": {
                "size": vector_size,
                "distance": "Cosine",
                "on_disk": true,
                "multivector_config": {
                    "comparator": "max_sim"
                }
            },
            "prefetch_dense": {
                "size": vector_size,
                "distance": "Cosine",
                "on_disk": true
            }
        }
    });
    if let Some(source_collection) = init_from {
        payload["init_from"] = json!({ "collection": source_collection });
    }
    payload
}

pub(crate) async fn create_qdrant_collection(
    collection_name: &str,
    vector_size: usize,
    init_from: Option<&str>,
) -> Result<(), String> {
    let base_url = qdrant_base_url().map_err(|error| error.payload.message)?;
    let client = qdrant_client();
    let collection_url = format!("{}/collections/{}", base_url, collection_name);
    let payload = build_qdrant_collection_create_payload(vector_size, init_from);
    let create_response = client
        .put(&collection_url)
        .json(&payload)
        .send()
        .await
        .map_err(|error| format!("Qdrant collection creation failed: {error}"))?;

    if create_response.status().is_success() {
        Ok(())
    } else {
        let status = create_response.status();
        let body = create_response.text().await.unwrap_or_default();
        Err(format!(
            "Qdrant collection creation for {collection_name} failed with {}: {}.",
            status,
            qdrant_error_detail(&body)
        ))
    }
}

pub(crate) async fn create_qdrant_stage_collection(
    write_plan: &QdrantNamespaceWritePlan,
    vector_size: Option<usize>,
) -> Result<(), String> {
    match &write_plan.stage_strategy {
        StageCollectionStrategy::Fresh => {
            create_qdrant_collection(
                &write_plan.stage_collection_name,
                vector_size.ok_or_else(|| {
                    format!(
                        "Qdrant stage {} requires a known vector size before creation.",
                        write_plan.stage_collection_name
                    )
                })?,
                None,
            )
            .await
        }
        StageCollectionStrategy::CloneFromActive { target_collection } => {
            let vector_size = match vector_size {
                Some(vector_size) => vector_size,
                None => qdrant_collection_vector_size(target_collection).await?,
            };
            create_qdrant_collection(
                &write_plan.stage_collection_name,
                vector_size,
                Some(target_collection),
            )
            .await
        }
    }
}

pub(crate) async fn qdrant_collection_exists(collection_name: &str) -> Result<bool, String> {
    let collection_url = format!(
        "{}/collections/{}",
        qdrant_base_url().map_err(|error| error.payload.message)?,
        collection_name
    );
    let response = qdrant_client()
        .get(&collection_url)
        .send()
        .await
        .map_err(|error| format!("Qdrant collection probe failed: {error}"))?;

    if response.status().is_success() {
        Ok(true)
    } else if response.status() == StatusCode::NOT_FOUND {
        Ok(false)
    } else {
        Err(format!(
            "Qdrant collection probe for {collection_name} failed with {}.",
            response.status()
        ))
    }
}

pub(crate) async fn validate_qdrant_collection(collection_name: &str) -> Result<(), String> {
    match qdrant_collection_exists(collection_name).await? {
        true => Ok(()),
        false => Err(format!(
            "Qdrant staged collection {collection_name} was not found after write completion."
        )),
    }
}

pub(crate) async fn qdrant_collection_vector_size(collection_name: &str) -> Result<usize, String> {
    let response = qdrant_client()
        .get(format!(
            "{}/collections/{}",
            qdrant_base_url().map_err(|error| error.payload.message)?,
            collection_name
        ))
        .send()
        .await
        .map_err(|error| format!("Qdrant collection info probe failed: {error}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Qdrant collection info probe for {collection_name} failed with {}: {}.",
            status,
            qdrant_error_detail(&body)
        ));
    }

    let payload: Value = response
        .json()
        .await
        .map_err(|error| format!("Qdrant collection info response was invalid JSON: {error}"))?;

    payload
        .pointer("/result/config/params/vectors/mv/size")
        .and_then(Value::as_u64)
        .or_else(|| {
            payload
                .pointer("/result/config/params/vectors/size")
                .and_then(Value::as_u64)
        })
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            format!(
                "Qdrant collection info for {collection_name} did not expose a usable vector size."
            )
        })
}

pub(crate) async fn list_qdrant_aliases() -> Result<Vec<Value>, String> {
    let response = qdrant_client()
        .get(format!(
            "{}/aliases",
            qdrant_base_url().map_err(|error| error.payload.message)?
        ))
        .send()
        .await
        .map_err(|error| format!("Qdrant alias listing failed: {error}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Qdrant alias listing failed with {}: {}.",
            status,
            qdrant_error_detail(&body)
        ));
    }

    let payload: Value = response
        .json()
        .await
        .map_err(|error| format!("Qdrant alias listing response was invalid JSON: {error}"))?;
    Ok(payload
        .pointer("/result/aliases")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

pub(crate) async fn list_qdrant_collections() -> Result<Vec<String>, String> {
    let response = qdrant_client()
        .get(format!(
            "{}/collections",
            qdrant_base_url().map_err(|error| error.payload.message)?
        ))
        .send()
        .await
        .map_err(|error| format!("Qdrant collection listing failed: {error}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Qdrant collection listing failed with {}: {}.",
            status,
            qdrant_error_detail(&body)
        ));
    }

    let payload: Value = response
        .json()
        .await
        .map_err(|error| format!("Qdrant collection listing response was invalid JSON: {error}"))?;
    Ok(payload
        .pointer("/result/collections")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("name").and_then(Value::as_str))
        .map(str::to_string)
        .collect())
}

pub(crate) async fn probe_qdrant_runtime_health() -> Result<usize, String> {
    let response = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("qdrant health probe client should be constructible")
        .get(format!(
            "{}/collections",
            qdrant_base_url().map_err(|error| error.payload.message)?
        ))
        .send()
        .await
        .map_err(|error| format!("Qdrant runtime health probe failed: {error}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Qdrant runtime health probe failed with {}: {}.",
            status,
            qdrant_error_detail(&body)
        ));
    }

    let payload: Value = response
        .json()
        .await
        .map_err(|error| format!("Qdrant runtime health probe returned invalid JSON: {error}"))?;
    Ok(payload
        .pointer("/result/collections")
        .and_then(Value::as_array)
        .map(|collections| collections.len())
        .unwrap_or(0))
}

pub(crate) async fn qdrant_alias_target(alias_name: &str) -> Result<Option<String>, String> {
    for alias in list_qdrant_aliases().await? {
        let matches = alias
            .get("alias_name")
            .and_then(Value::as_str)
            .map(|value| value == alias_name)
            .unwrap_or(false);
        if matches {
            return Ok(alias
                .get("collection_name")
                .and_then(Value::as_str)
                .map(str::to_string));
        }
    }
    Ok(None)
}

pub(crate) async fn probe_active_qdrant_namespace(
    alias_name: &str,
) -> Result<ActiveNamespaceProbeResult, String> {
    if let Some(target_collection) = qdrant_alias_target(alias_name).await? {
        return match qdrant_collection_exists(&target_collection).await? {
            true => Ok(ActiveNamespaceProbeResult::Ready { target_collection }),
            false => Ok(ActiveNamespaceProbeResult::MissingTarget { target_collection }),
        };
    }

    if qdrant_collection_exists(alias_name).await? {
        Ok(ActiveNamespaceProbeResult::LegacyDirectCollection)
    } else {
        Ok(ActiveNamespaceProbeResult::Missing)
    }
}

pub(crate) async fn switch_qdrant_active_alias(
    write_plan: &QdrantNamespaceWritePlan,
) -> Result<(), String> {
    let mut actions = Vec::new();
    if write_plan.alias_exists {
        actions.push(json!({
            "delete_alias": {
                "alias_name": write_plan.alias_name,
            }
        }));
    }
    actions.push(json!({
        "create_alias": {
            "collection_name": write_plan.stage_collection_name,
            "alias_name": write_plan.alias_name,
        }
    }));
    let response = qdrant_client()
        .post(format!(
            "{}/collections/aliases",
            qdrant_base_url().map_err(|error| error.payload.message)?
        ))
        .json(&json!({ "actions": actions }))
        .send()
        .await
        .map_err(|error| format!("Qdrant alias activation failed: {error}"))?;

    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(format!(
            "Qdrant alias activation for {} failed with {}: {}.",
            write_plan.alias_name,
            status,
            qdrant_error_detail(&body)
        ))
    }
}

pub(crate) async fn delete_qdrant_alias(alias_name: &str) -> Result<(), String> {
    let response = qdrant_client()
        .post(format!(
            "{}/collections/aliases",
            qdrant_base_url().map_err(|error| error.payload.message)?
        ))
        .json(&json!({
            "actions": [{
                "delete_alias": {
                    "alias_name": alias_name,
                }
            }]
        }))
        .send()
        .await
        .map_err(|error| format!("Qdrant alias deletion failed: {error}"))?;

    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(format!(
            "Qdrant alias deletion for {alias_name} failed with {}: {}.",
            status,
            qdrant_error_detail(&body)
        ))
    }
}

pub(crate) async fn delete_qdrant_points(
    collection_name: &str,
    point_ids: &[u64],
) -> Result<(), String> {
    if point_ids.is_empty() {
        return Ok(());
    }

    let response = qdrant_client()
        .post(format!(
            "{}/collections/{}/points/delete?wait=true",
            qdrant_base_url().map_err(|error| error.payload.message)?,
            collection_name
        ))
        .json(&json!({ "points": point_ids }))
        .send()
        .await
        .map_err(|error| format!("Qdrant delete request failed: {error}"))?;

    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(format!(
            "Qdrant delete request for {collection_name} failed with {}: {}.",
            status,
            qdrant_error_detail(&body)
        ))
    }
}

pub(crate) async fn delete_qdrant_collection(collection_name: &str) -> Result<(), String> {
    let response = qdrant_client()
        .delete(format!(
            "{}/collections/{}",
            qdrant_base_url().map_err(|error| error.payload.message)?,
            collection_name
        ))
        .send()
        .await
        .map_err(|error| format!("Qdrant collection deletion failed: {error}"))?;

    if response.status().is_success() || response.status() == StatusCode::NOT_FOUND {
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(format!(
            "Qdrant collection deletion for {collection_name} failed with {}: {}.",
            status,
            qdrant_error_detail(&body)
        ))
    }
}

pub(crate) async fn best_effort_delete_qdrant_collection(collection_name: &str) {
    if let Err(error) = delete_qdrant_collection(collection_name).await {
        tracing::warn!(
            collection_name = %collection_name,
            "Failed to delete staged Qdrant collection during cleanup: {error}"
        );
    }
}

pub(crate) async fn best_effort_cleanup_retired_stage_collections(
    write_plan: &QdrantNamespaceWritePlan,
) {
    let mut keep = BTreeSet::from([write_plan.stage_collection_name.clone()]);
    if let Some(previous_target_collection) = write_plan.previous_target_collection.clone() {
        keep.insert(previous_target_collection);
    }

    let Some(namespace_tail) = write_plan.alias_name.strip_prefix("vector_space_") else {
        return;
    };
    let prefix = format!("vector_space_stage_{namespace_tail}_");
    cleanup_qdrant_collection_prefix(&prefix, &keep).await;
}

pub(crate) async fn cleanup_retired_vector_space_namespace(
    library_id: &str,
    vector_space_id: &str,
) -> Result<(), String> {
    let alias_name = stable_vector_space_name(library_id, vector_space_id);
    match probe_active_qdrant_namespace(&alias_name).await? {
        ActiveNamespaceProbeResult::Ready { target_collection }
        | ActiveNamespaceProbeResult::MissingTarget { target_collection } => {
            delete_qdrant_alias(&alias_name).await?;
            delete_qdrant_collection(&target_collection).await?;
        }
        ActiveNamespaceProbeResult::Missing => {}
        ActiveNamespaceProbeResult::LegacyDirectCollection => {
            return Err(format!(
                "Legacy direct Qdrant collection {alias_name} blocks retired vector-space cleanup. Remove it manually."
            ));
        }
    }

    let prefix = format!("vector_space_stage_{library_id}_{vector_space_id}_");
    cleanup_qdrant_collection_prefix(&prefix, &BTreeSet::new()).await;
    Ok(())
}

async fn cleanup_qdrant_collection_prefix(prefix: &str, keep: &BTreeSet<String>) {
    let collections = match list_qdrant_collections().await {
        Ok(collections) => collections,
        Err(error) => {
            tracing::warn!("Failed to list Qdrant collections for staging cleanup: {error}");
            return;
        }
    };

    for collection_name in collections {
        if !collection_name.starts_with(&prefix) || keep.contains(&collection_name) {
            continue;
        }
        if let Err(error) = delete_qdrant_collection(&collection_name).await {
            tracing::warn!(
                collection_name = %collection_name,
                "Failed to delete retired staged Qdrant collection: {error}"
            );
        }
    }
}

pub(crate) async fn upsert_qdrant_points(
    collection_name: &str,
    visual_units: &[VisualUnitRecord],
    embeddings: &[SidecarEmbeddingItem],
) -> Result<(), String> {
    let max_body_bytes = qdrant_max_upsert_body_bytes();
    if max_body_bytes <= QDRANT_UPSERT_BODY_OVERHEAD_BYTES {
        return Err(
            "Qdrant upsert body limit must be larger than the request envelope.".to_string(),
        );
    }

    let mut chunk_index = 0usize;
    let mut current_chunk = Vec::new();
    let mut current_size = QDRANT_UPSERT_BODY_OVERHEAD_BYTES;
    for (visual_unit, embedding) in visual_units.iter().zip(embeddings.iter()) {
        let point = build_qdrant_point((visual_unit, embedding));
        let point_size = serde_json::to_vec(&point)
            .map_err(|error| format!("Failed to serialize Qdrant point payload: {error}"))?
            .len();
        let separator_size = usize::from(!current_chunk.is_empty());
        let next_size = current_size + separator_size + point_size;

        if !current_chunk.is_empty() && next_size > max_body_bytes {
            chunk_index += 1;
            send_qdrant_point_chunk(collection_name, chunk_index, &current_chunk).await?;
            current_chunk.clear();
            current_size = QDRANT_UPSERT_BODY_OVERHEAD_BYTES;
        }

        current_size += usize::from(!current_chunk.is_empty()) + point_size;
        current_chunk.push(point);
    }

    if !current_chunk.is_empty() {
        chunk_index += 1;
        send_qdrant_point_chunk(collection_name, chunk_index, &current_chunk).await?;
    }

    Ok(())
}

pub(crate) async fn send_qdrant_point_chunk(
    collection_name: &str,
    chunk_index: usize,
    points_chunk: &[Value],
) -> Result<(), String> {
    let response = qdrant_client()
        .put(format!(
            "{}/collections/{}/points?wait=true",
            qdrant_base_url().map_err(|error| error.payload.message)?,
            collection_name
        ))
        .json(&json!({ "points": points_chunk }))
        .send()
        .await
        .map_err(|error| {
            format!(
                "Qdrant upsert request for {collection_name} chunk {chunk_index} failed: {error}"
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let detail = qdrant_error_detail(&body);
        return Err(format!(
            "Qdrant upsert for {collection_name} chunk {chunk_index} failed with {}: {}.",
            status, detail
        ));
    }

    Ok(())
}

pub(crate) fn build_qdrant_point(
    (visual_unit, embedding): (&VisualUnitRecord, &SidecarEmbeddingItem),
) -> Value {
    json!({
        "id": visual_unit.point_id,
        "vector": {
            "mv": embedding.vectors,
            "prefetch_dense": embedding.pooled_vector,
        },
        "payload": {
            "visual_unit_id": visual_unit.id,
            "source_id": visual_unit.source_id,
            "source_path": visual_unit.source_path,
            "source_type": visual_unit.source_type,
            "kind": visual_unit.kind,
            "locator": visual_unit.locator,
        }
    })
}

// Kept with unit coverage as the reusable Qdrant upsert body chunking helper.
#[allow(dead_code)]
pub(crate) fn chunk_qdrant_points(
    points: Vec<Value>,
    max_body_bytes: usize,
) -> Result<Vec<Vec<Value>>, String> {
    if max_body_bytes <= QDRANT_UPSERT_BODY_OVERHEAD_BYTES {
        return Err(
            "Qdrant upsert body limit must be larger than the request envelope.".to_string(),
        );
    }

    let mut chunks: Vec<Vec<Value>> = Vec::new();
    let mut current_chunk: Vec<Value> = Vec::new();
    let mut current_size = QDRANT_UPSERT_BODY_OVERHEAD_BYTES;

    for point in points {
        let point_size = serde_json::to_vec(&point)
            .map_err(|error| format!("Failed to serialize Qdrant point payload: {error}"))?
            .len();
        let separator_size = usize::from(!current_chunk.is_empty());
        let next_size = current_size + separator_size + point_size;

        if !current_chunk.is_empty() && next_size > max_body_bytes {
            chunks.push(current_chunk);
            current_chunk = Vec::new();
            current_size = QDRANT_UPSERT_BODY_OVERHEAD_BYTES;
        }

        current_size += usize::from(!current_chunk.is_empty()) + point_size;
        current_chunk.push(point);
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    Ok(chunks)
}

pub(crate) fn qdrant_error_detail(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "empty response body".to_string();
    }

    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
        if let Some(error) = parsed
            .pointer("/status/error")
            .and_then(Value::as_str)
            .or_else(|| parsed.get("error").and_then(Value::as_str))
        {
            return error.to_string();
        }
    }

    trimmed.to_string()
}

pub(crate) async fn query_qdrant(
    library_id: &str,
    vector_space_id: &str,
    active_visual_unit_count: usize,
    cursor_limit: usize,
    embedding: &QueryEmbeddingResult,
) -> Result<Vec<QdrantScoredPoint>, ApiError> {
    let prefetch_limit = active_visual_unit_count.max(cursor_limit).max(20);
    let payload = json!({
        "prefetch": {
            "query": embedding.pooled_vector,
            "using": "prefetch_dense",
            "limit": prefetch_limit,
        },
        "query": embedding.vectors,
        "using": "mv",
        "limit": prefetch_limit,
        "with_payload": true,
    });
    let response = qdrant_client()
        .post(format!(
            "{}/collections/{}/points/query",
            qdrant_base_url()?.trim_end_matches('/'),
            stable_vector_space_name(library_id, vector_space_id)
        ))
        .json(&payload)
        .send()
        .await
        .map_err(|error| {
            ApiError::runtime_unavailable(
                format!("Qdrant query request failed: {error}"),
                Some(json!({ "service": "qdrant" })),
            )
        })?;

    if !response.status().is_success() {
        return Err(ApiError::runtime_unavailable(
            format!("Qdrant query request failed with {}.", response.status()),
            Some(json!({ "service": "qdrant" })),
        ));
    }

    let parsed: QdrantQueryResponse = response.json().await.map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Qdrant query response was invalid JSON: {error}"),
            Some(json!({ "service": "qdrant" })),
        )
    })?;
    Ok(parsed.result.points)
}

pub(crate) fn qdrant_max_upsert_body_bytes() -> usize {
    read_optional_usize_env(
        "INDEX_QDRANT_UPSERT_BODY_BYTES",
        DEFAULT_QDRANT_MAX_UPSERT_BODY_BYTES,
    )
}

pub(crate) fn read_optional_usize_env(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

pub(crate) fn stable_vector_space_name(library_id: &str, vector_space_id: &str) -> String {
    format!("vector_space_{library_id}_{vector_space_id}")
}

pub(crate) fn vector_space_id(
    provider_id: &str,
    model_id: &str,
    version: &str,
    vector_type: &str,
) -> String {
    let signature = format!("{provider_id}\n{model_id}\n{version}\n{vector_type}");
    format!("{:016x}", stable_fnv1a64(signature.as_bytes()))
}

fn stable_fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x00000100000001b3;

    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

pub(crate) fn read_required_env(name: &'static str) -> Result<String, ApiError> {
    env::var(name).map_err(|_| {
        ApiError::runtime_unavailable(
            format!("Missing required environment variable {name}; source .env or use scripts/local/run.sh"),
            Some(json!({ "field": name })),
        )
    })
}

pub(crate) fn qdrant_base_url() -> Result<String, ApiError> {
    read_required_env("QDRANT_URL")
}

pub(crate) fn qdrant_client() -> Client {
    Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("qdrant client should be constructible")
}
