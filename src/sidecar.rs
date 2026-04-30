use crate::{
    api::{ApiError, EmbeddingCapabilities, ProviderProbeSnapshot},
    model::{ProviderConfigRecord, UnitRecord},
    provider::{
        current_rfc3339_timestamp, local_sidecar_embedding_capabilities,
        local_sidecar_execution_input_types, ProviderRuntimeModelSnapshot, QUERY_KIND_DOCUMENT,
        QUERY_KIND_IMAGE, QUERY_KIND_TEXT, QUERY_KIND_VIDEO,
    },
    SIDECAR_REQUEST_TIMEOUT_SECS,
};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{env, time::Duration};

pub(crate) struct IndexingError {
    pub(crate) message: String,
}

#[derive(Deserialize)]
pub(crate) struct SidecarEnvelope<T> {
    data: T,
}

#[derive(Deserialize)]
pub(crate) struct SidecarEmbedPayload {
    embeddings: Vec<SidecarEmbeddingItem>,
}

#[derive(Deserialize)]
pub(crate) struct SidecarEmbeddingItem {
    pub(crate) path: Option<String>,
    pub(crate) source_type: Option<String>,
    pub(crate) kind: Option<String>,
    pub(crate) locator: Option<Value>,
    pub(crate) vectors: Vec<Vec<f32>>,
    #[serde(default)]
    pub(crate) pooled_vector: Vec<f32>,
}

pub(crate) struct QueryEmbeddingResult {
    pub(crate) vectors: Vec<Vec<f32>>,
    pub(crate) pooled_vector: Vec<f32>,
}

pub(crate) struct LocalSidecarProviderSnapshot {
    pub(crate) probe: ProviderProbeSnapshot,
    pub(crate) runtime_model: ProviderRuntimeModelSnapshot,
    pub(crate) embedding_capabilities: EmbeddingCapabilities,
    pub(crate) execution_input_types: Vec<String>,
    pub(crate) runtime_adapters: Vec<String>,
}

#[derive(Deserialize)]
pub(crate) struct SidecarErrorEnvelope {
    error: SidecarErrorPayload,
}

#[derive(Deserialize)]
pub(crate) struct SidecarErrorPayload {
    code: String,
    message: String,
    // Preserve sidecar error details for wire compatibility even when the app only surfaces code/message today.
    #[allow(dead_code)]
    details: Option<Value>,
}

pub(crate) async fn embed_documents(
    units: &[UnitRecord],
    provider_context: Option<Value>,
) -> Result<Vec<SidecarEmbeddingItem>, IndexingError> {
    let documents: Vec<_> = units
        .iter()
        .map(|unit| {
            json!({
                "path": unit.source_path,
                "locator": unit.locator,
            })
        })
        .collect();
    let mut payload = json!({
        "operation_kind": "document_embedding",
        "inputs": {
            "documents": documents,
        },
    });
    if let Some(provider_context) = provider_context {
        payload["provider_context"] = provider_context;
    }

    let response = sidecar_client()
        .post(format!(
            "{}/embed",
            sidecar_base_url().map_err(|error| IndexingError {
                message: error.payload.message,
            })?
        ))
        .json(&payload)
        .send()
        .await
        .map_err(|error| IndexingError {
            message: format!("Sidecar document embedding request failed: {error}"),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let message = parse_sidecar_error_message(&body)
            .unwrap_or_else(|| format!("Sidecar document embedding request failed with {status}."));
        return Err(IndexingError { message });
    }

    let envelope: SidecarEnvelope<SidecarEmbedPayload> =
        response.json().await.map_err(|error| IndexingError {
            message: format!("Sidecar document embedding response was invalid JSON: {error}"),
        })?;

    if envelope.data.embeddings.len() != units.len() {
        return Err(IndexingError {
            message: format!(
                "Sidecar returned {} document embedding(s) for {} unit(s).",
                envelope.data.embeddings.len(),
                units.len()
            ),
        });
    }

    for (unit, embedding) in units.iter().zip(envelope.data.embeddings.iter()) {
        if embedding.vectors.is_empty() || embedding.vectors[0].is_empty() {
            return Err(IndexingError {
                message: format!(
                    "Sidecar returned an empty document embedding for {}.",
                    unit.source_path
                ),
            });
        }
        if let Some(source_type) = &embedding.source_type {
            if source_type != &unit.source_type {
                return Err(IndexingError {
                    message: format!(
                        "Sidecar returned source_type {} for {}, but the expected source_type was {}.",
                        source_type,
                        unit.source_path,
                        unit.source_type
                    ),
                });
            }
        }
        if let Some(kind) = &embedding.kind {
            if kind != &unit.asset_type {
                return Err(IndexingError {
                    message: format!(
                        "Sidecar returned kind {} for {}, but the expected kind was {}.",
                        kind, unit.source_path, unit.asset_type
                    ),
                });
            }
        }
        if let Some(path) = &embedding.path {
            if path != &unit.source_path {
                return Err(IndexingError {
                    message: format!(
                        "Sidecar returned a document embedding for {}, but the expected path was {}.",
                        path, unit.source_path
                    ),
                });
            }
        }
        if let Some(locator) = &embedding.locator {
            if locator != &unit.locator {
                return Err(IndexingError {
                    message: format!(
                        "Sidecar returned locator {} for {}, but the expected locator was {}.",
                        locator, unit.source_path, unit.locator
                    ),
                });
            }
        }
    }

    Ok(envelope.data.embeddings)
}

pub(crate) async fn embed_query_text(
    text: &str,
    provider_context: Option<Value>,
) -> Result<QueryEmbeddingResult, ApiError> {
    let mut payload = json!({
        "operation_kind": "query_embedding",
        "inputs": {
            "queries": [text],
        },
    });
    if let Some(provider_context) = provider_context {
        payload["provider_context"] = provider_context;
    }
    let response = sidecar_client()
        .post(format!("{}/embed", sidecar_base_url()?))
        .json(&payload)
        .send()
        .await
        .map_err(|error| {
            ApiError::runtime_unavailable(
                format!("Sidecar query embedding request failed: {error}"),
                Some(json!({ "service": "sidecar" })),
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let message = parse_sidecar_error_message(&body)
            .unwrap_or_else(|| format!("Sidecar query embedding request failed with {status}."));
        return Err(ApiError::runtime_unavailable(
            message,
            Some(json!({ "service": "sidecar" })),
        ));
    }

    let envelope: SidecarEnvelope<SidecarEmbedPayload> =
        response.json().await.map_err(|error| {
            ApiError::runtime_unavailable(
                format!("Sidecar query embedding response was invalid JSON: {error}"),
                Some(json!({ "service": "sidecar" })),
            )
        })?;
    let embedding = envelope.data.embeddings.into_iter().next().ok_or_else(|| {
        ApiError::runtime_unavailable(
            "Sidecar query embedding response did not include any embeddings.",
            Some(json!({ "service": "sidecar" })),
        )
    })?;

    let pooled_vector = if embedding.pooled_vector.is_empty() {
        mean_pool_vectors(&embedding.vectors).ok_or_else(|| {
            ApiError::runtime_unavailable(
                "Sidecar query embedding response did not include usable vectors.",
                Some(json!({ "service": "sidecar" })),
            )
        })?
    } else {
        embedding.pooled_vector
    };

    Ok(QueryEmbeddingResult {
        vectors: embedding.vectors,
        pooled_vector,
    })
}

pub(crate) async fn embed_query_image(
    path: &str,
    locator: Option<Value>,
    provider_context: Option<Value>,
) -> Result<QueryEmbeddingResult, ApiError> {
    let mut payload = json!({
        "operation_kind": "image_query_embedding",
        "inputs": {
            "images": [{
                "path": path,
                "locator": locator,
            }],
        },
    });
    if let Some(provider_context) = provider_context {
        payload["provider_context"] = provider_context;
    }
    let response = sidecar_client()
        .post(format!("{}/embed", sidecar_base_url()?))
        .json(&payload)
        .send()
        .await
        .map_err(|error| {
            ApiError::runtime_unavailable(
                format!("Sidecar image query embedding request failed: {error}"),
                Some(json!({ "service": "sidecar" })),
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let message = parse_sidecar_error_message(&body).unwrap_or_else(|| {
            format!("Sidecar image query embedding request failed with {status}.")
        });
        return Err(ApiError::runtime_unavailable(
            message,
            Some(json!({ "service": "sidecar" })),
        ));
    }

    let envelope: SidecarEnvelope<SidecarEmbedPayload> =
        response.json().await.map_err(|error| {
            ApiError::runtime_unavailable(
                format!("Sidecar image query embedding response was invalid JSON: {error}"),
                Some(json!({ "service": "sidecar" })),
            )
        })?;
    let embedding = envelope.data.embeddings.into_iter().next().ok_or_else(|| {
        ApiError::runtime_unavailable(
            "Sidecar image query embedding response did not include any embeddings.",
            Some(json!({ "service": "sidecar" })),
        )
    })?;

    let pooled_vector = if embedding.pooled_vector.is_empty() {
        mean_pool_vectors(&embedding.vectors).ok_or_else(|| {
            ApiError::runtime_unavailable(
                "Sidecar image query embedding response did not include usable vectors.",
                Some(json!({ "service": "sidecar" })),
            )
        })?
    } else {
        embedding.pooled_vector
    };

    Ok(QueryEmbeddingResult {
        vectors: embedding.vectors,
        pooled_vector,
    })
}

pub(crate) async fn embed_query_video(
    path: &str,
    locator: Option<Value>,
    provider_context: Option<Value>,
) -> Result<QueryEmbeddingResult, ApiError> {
    let mut payload = json!({
        "operation_kind": "video_query_embedding",
        "inputs": {
            "videos": [{
                "path": path,
                "locator": locator,
            }],
        },
    });
    if let Some(provider_context) = provider_context {
        payload["provider_context"] = provider_context;
    }
    let response = sidecar_client()
        .post(format!("{}/embed", sidecar_base_url()?))
        .json(&payload)
        .send()
        .await
        .map_err(|error| {
            ApiError::runtime_unavailable(
                format!("Sidecar video query embedding request failed: {error}"),
                Some(json!({ "service": "sidecar" })),
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let message = parse_sidecar_error_message(&body).unwrap_or_else(|| {
            format!("Sidecar video query embedding request failed with {status}.")
        });
        return Err(ApiError::runtime_unavailable(
            message,
            Some(json!({ "service": "sidecar" })),
        ));
    }

    let envelope: SidecarEnvelope<SidecarEmbedPayload> =
        response.json().await.map_err(|error| {
            ApiError::runtime_unavailable(
                format!("Sidecar video query embedding response was invalid JSON: {error}"),
                Some(json!({ "service": "sidecar" })),
            )
        })?;
    let embedding = envelope.data.embeddings.into_iter().next().ok_or_else(|| {
        ApiError::runtime_unavailable(
            "Sidecar video query embedding response did not include any embeddings.",
            Some(json!({ "service": "sidecar" })),
        )
    })?;

    let pooled_vector = if embedding.pooled_vector.is_empty() {
        mean_pool_vectors(&embedding.vectors).ok_or_else(|| {
            ApiError::runtime_unavailable(
                "Sidecar video query embedding response did not include usable vectors.",
                Some(json!({ "service": "sidecar" })),
            )
        })?
    } else {
        embedding.pooled_vector
    };

    Ok(QueryEmbeddingResult {
        vectors: embedding.vectors,
        pooled_vector,
    })
}

pub(crate) async fn embed_query_document(
    path: &str,
    locator: Option<Value>,
    provider_context: Option<Value>,
) -> Result<QueryEmbeddingResult, ApiError> {
    let mut payload = json!({
        "operation_kind": "document_query_embedding",
        "inputs": {
            "documents": [{
                "path": path,
                "locator": locator,
            }],
        },
    });
    if let Some(provider_context) = provider_context {
        payload["provider_context"] = provider_context;
    }
    let response = sidecar_client()
        .post(format!("{}/embed", sidecar_base_url()?))
        .json(&payload)
        .send()
        .await
        .map_err(|error| {
            ApiError::runtime_unavailable(
                format!("Sidecar document query embedding request failed: {error}"),
                Some(json!({ "service": "sidecar" })),
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let message = parse_sidecar_error_message(&body).unwrap_or_else(|| {
            format!("Sidecar document query embedding request failed with {status}.")
        });
        return Err(ApiError::runtime_unavailable(
            message,
            Some(json!({ "service": "sidecar" })),
        ));
    }

    let envelope: SidecarEnvelope<SidecarEmbedPayload> =
        response.json().await.map_err(|error| {
            ApiError::runtime_unavailable(
                format!("Sidecar document query embedding response was invalid JSON: {error}"),
                Some(json!({ "service": "sidecar" })),
            )
        })?;
    let embedding = envelope.data.embeddings.into_iter().next().ok_or_else(|| {
        ApiError::runtime_unavailable(
            "Sidecar document query embedding response did not include any embeddings.",
            Some(json!({ "service": "sidecar" })),
        )
    })?;

    let pooled_vector = if embedding.pooled_vector.is_empty() {
        mean_pool_vectors(&embedding.vectors).ok_or_else(|| {
            ApiError::runtime_unavailable(
                "Sidecar document query embedding response did not include usable vectors.",
                Some(json!({ "service": "sidecar" })),
            )
        })?
    } else {
        embedding.pooled_vector
    };

    Ok(QueryEmbeddingResult {
        vectors: embedding.vectors,
        pooled_vector,
    })
}

pub(crate) fn mean_pool_vectors(vectors: &[Vec<f32>]) -> Option<Vec<f32>> {
    let dimension = vectors.first()?.len();
    if dimension == 0 || vectors.iter().any(|vector| vector.len() != dimension) {
        return None;
    }

    let mut pooled = vec![0.0; dimension];
    for vector in vectors {
        for (index, value) in vector.iter().enumerate() {
            pooled[index] += value;
        }
    }
    let count = vectors.len() as f32;
    for value in &mut pooled {
        *value /= count;
    }
    Some(pooled)
}

pub(crate) fn read_required_env(name: &'static str) -> Result<String, ApiError> {
    env::var(name).map_err(|_| {
        ApiError::runtime_unavailable(
            format!("Missing required environment variable {name}; source .env or use scripts/local/run.sh"),
            Some(json!({ "field": name })),
        )
    })
}

pub(crate) fn sidecar_base_url() -> Result<String, ApiError> {
    Ok(format!(
        "http://{}:{}",
        read_required_env("SIDECAR_HOST")?,
        read_required_env("SIDECAR_PORT")?,
    ))
}

pub(crate) fn sidecar_client() -> Client {
    Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(SIDECAR_REQUEST_TIMEOUT_SECS))
        .build()
        .expect("sidecar client should be constructible")
}

pub(crate) fn sidecar_probe_client() -> Client {
    Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("sidecar probe client should be constructible")
}

pub(crate) fn parse_sidecar_error_message(body: &str) -> Option<String> {
    serde_json::from_str::<SidecarErrorEnvelope>(body)
        .ok()
        .map(|envelope| {
            format!(
                "Sidecar {}: {}",
                envelope.error.code, envelope.error.message
            )
        })
}

pub(crate) async fn probe_local_sidecar_provider(
    _provider: &ProviderConfigRecord,
    client: &Client,
) -> LocalSidecarProviderSnapshot {
    let now = current_rfc3339_timestamp();
    let fallback_runtime_model = crate::provider::fallback_local_sidecar_runtime_model();
    let base_url = match sidecar_base_url() {
        Ok(base_url) => base_url,
        Err(error) => {
            return LocalSidecarProviderSnapshot {
                probe: ProviderProbeSnapshot {
                    status: "runtime_unavailable".to_string(),
                    message: error.payload.message,
                    last_probed_at: Some(now),
                },
                runtime_model: fallback_runtime_model,
                embedding_capabilities: local_sidecar_embedding_capabilities(),
                execution_input_types: Vec::new(),
                runtime_adapters: Vec::new(),
            };
        }
    };

    let response = match client.get(format!("{base_url}/capabilities")).send().await {
        Ok(response) => response,
        Err(error) => {
            return LocalSidecarProviderSnapshot {
                probe: ProviderProbeSnapshot {
                    status: "runtime_unavailable".to_string(),
                    message: format!("Sidecar capabilities probe failed: {error}"),
                    last_probed_at: Some(now),
                },
                runtime_model: fallback_runtime_model,
                embedding_capabilities: local_sidecar_embedding_capabilities(),
                execution_input_types: Vec::new(),
                runtime_adapters: Vec::new(),
            };
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return LocalSidecarProviderSnapshot {
            probe: ProviderProbeSnapshot {
                status: "runtime_unavailable".to_string(),
                message: parse_sidecar_error_message(&body)
                    .unwrap_or_else(|| format!("Sidecar capabilities probe failed with {status}.")),
                last_probed_at: Some(now),
            },
            runtime_model: fallback_runtime_model,
            embedding_capabilities: local_sidecar_embedding_capabilities(),
            execution_input_types: Vec::new(),
            runtime_adapters: Vec::new(),
        };
    }

    let payload: Value = match response.json().await {
        Ok(payload) => payload,
        Err(error) => {
            return LocalSidecarProviderSnapshot {
                probe: ProviderProbeSnapshot {
                    status: "runtime_unavailable".to_string(),
                    message: format!("Sidecar capabilities probe returned invalid JSON: {error}"),
                    last_probed_at: Some(now),
                },
                runtime_model: fallback_runtime_model,
                embedding_capabilities: local_sidecar_embedding_capabilities(),
                execution_input_types: Vec::new(),
                runtime_adapters: Vec::new(),
            };
        }
    };

    let runtime_model = payload
        .get("operations")
        .and_then(Value::as_array)
        .and_then(|operations| {
            operations.iter().find_map(|operation| {
                operation
                    .get("model")
                    .map(|model| ProviderRuntimeModelSnapshot {
                        model_id: model
                            .get("model_id")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .unwrap_or_else(|| fallback_runtime_model.model_id.clone()),
                        model_revision: model
                            .get("revision")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .or_else(|| fallback_runtime_model.model_revision.clone()),
                    })
            })
        })
        .unwrap_or_else(|| fallback_runtime_model.clone());
    let embedding_capabilities = payload
        .get("embedding_capabilities")
        .cloned()
        .and_then(|value| serde_json::from_value::<EmbeddingCapabilities>(value).ok())
        .filter(|capabilities| {
            !capabilities.input_types.is_empty() && !capabilities.vector_types.is_empty()
        })
        .unwrap_or_else(local_sidecar_embedding_capabilities);
    let runtime_adapters = payload
        .get("runtime_adapters")
        .and_then(Value::as_array)
        .map(|adapters| {
            adapters
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let execution_input_types = payload
        .get("execution_input_types")
        .and_then(Value::as_array)
        .map(|input_types| {
            input_types
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|input_types| !input_types.is_empty())
        .unwrap_or_else(|| derive_execution_input_types_from_capabilities(&payload));

    let can_service = payload
        .pointer("/availability/can_service")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !can_service {
        let message = payload
            .pointer("/availability/load_error")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                "local_sidecar runtime reported that it cannot currently service requests."
                    .to_string()
            });
        return LocalSidecarProviderSnapshot {
            probe: ProviderProbeSnapshot {
                status: "runtime_unavailable".to_string(),
                message,
                last_probed_at: Some(now),
            },
            runtime_model,
            embedding_capabilities,
            execution_input_types,
            runtime_adapters: runtime_adapters,
        };
    }

    let supported_operations = payload
        .get("operations")
        .and_then(Value::as_array)
        .map(|operations| {
            operations
                .iter()
                .filter(|item| {
                    item.get("supported")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                })
                .filter_map(|item| item.get("operation_kind").and_then(Value::as_str))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let missing_operations = required_local_sidecar_operations()
        .into_iter()
        .filter(|operation| !supported_operations.iter().any(|item| item == operation))
        .collect::<Vec<_>>();
    if !missing_operations.is_empty() {
        return LocalSidecarProviderSnapshot {
            probe: ProviderProbeSnapshot {
                status: "runtime_unavailable".to_string(),
                message: format!(
                    "local_sidecar runtime is missing required operations: {}.",
                    missing_operations.join(", ")
                ),
                last_probed_at: Some(now),
            },
            runtime_model,
            embedding_capabilities,
            execution_input_types,
            runtime_adapters: runtime_adapters,
        };
    }

    LocalSidecarProviderSnapshot {
        probe: ProviderProbeSnapshot {
            status: "available".to_string(),
            message: format!(
                "local_sidecar runtime is available for {} required operation(s).",
                required_local_sidecar_operations().len()
            ),
            last_probed_at: Some(now),
        },
        runtime_model,
        embedding_capabilities,
        execution_input_types,
        runtime_adapters: runtime_adapters,
    }
}

fn derive_execution_input_types_from_capabilities(payload: &Value) -> Vec<String> {
    let mut execution_input_types = Vec::new();
    let supported_operations = payload
        .get("operations")
        .and_then(Value::as_array)
        .map(|operations| {
            operations
                .iter()
                .filter(|operation| {
                    operation
                        .get("supported")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                })
                .filter_map(|operation| operation.get("operation_kind").and_then(Value::as_str))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let runtime_adapters = payload
        .get("runtime_adapters")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();

    if supported_operations.contains(&"query_embedding") {
        execution_input_types.push(QUERY_KIND_TEXT.to_string());
    }
    if supported_operations.contains(&"image_query_embedding") {
        execution_input_types.push(QUERY_KIND_IMAGE.to_string());
    }
    if supported_operations.contains(&"document_query_embedding")
        || runtime_adapters.contains(&"document_query_via_page_images")
    {
        execution_input_types.push(QUERY_KIND_DOCUMENT.to_string());
    }
    if supported_operations.contains(&"video_query_embedding")
        || runtime_adapters.contains(&"video_query_via_frame_images")
    {
        execution_input_types.push(QUERY_KIND_VIDEO.to_string());
    }

    if execution_input_types.is_empty() {
        return local_sidecar_execution_input_types();
    }

    execution_input_types.sort();
    execution_input_types.dedup();
    execution_input_types
}

fn required_local_sidecar_operations() -> Vec<String> {
    vec![
        "query_embedding".to_string(),
        "image_query_embedding".to_string(),
        "video_query_embedding".to_string(),
        "document_query_embedding".to_string(),
        "document_embedding".to_string(),
    ]
}
