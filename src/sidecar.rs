use crate::{
    api::{ApiError, ProviderProbeSnapshot},
    model::{ProviderConfigRecord, VisualUnitRecord},
    provider::{
        current_rfc3339_timestamp, ProviderRuntimeModelSnapshot,
    },
    SIDECAR_REQUEST_TIMEOUT_SECS,
};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{env, time::Duration};

pub(crate) struct IndexingError {
    pub(crate) phase: &'static str,
    pub(crate) message: String,
    pub(crate) completed: usize,
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
}

#[derive(Deserialize)]
pub(crate) struct SidecarErrorEnvelope {
    error: SidecarErrorPayload,
}

#[derive(Deserialize)]
pub(crate) struct SidecarErrorPayload {
    code: String,
    message: String,
    #[allow(dead_code)]
    details: Option<Value>,
}

pub(crate) async fn embed_documents(
    visual_units: &[VisualUnitRecord],
    provider_context: Option<Value>,
) -> Result<Vec<SidecarEmbeddingItem>, IndexingError> {
    let documents: Vec<_> = visual_units
        .iter()
        .map(|visual_unit| {
            json!({
                "path": visual_unit.source_path,
                "locator": visual_unit.locator,
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
                phase: "encode",
                message: error.payload.message,
                completed: 0,
            })?
        ))
        .json(&payload)
        .send()
        .await
        .map_err(|error| IndexingError {
            phase: "encode",
            message: format!("Sidecar document embedding request failed: {error}"),
            completed: 0,
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let message = parse_sidecar_error_message(&body)
            .unwrap_or_else(|| format!("Sidecar document embedding request failed with {status}."));
        return Err(IndexingError {
            phase: "encode",
            message,
            completed: 0,
        });
    }

    let envelope: SidecarEnvelope<SidecarEmbedPayload> =
        response.json().await.map_err(|error| IndexingError {
            phase: "encode",
            message: format!("Sidecar document embedding response was invalid JSON: {error}"),
            completed: 0,
        })?;

    if envelope.data.embeddings.len() != visual_units.len() {
        return Err(IndexingError {
            phase: "encode",
            message: format!(
                "Sidecar returned {} document embedding(s) for {} visual unit(s).",
                envelope.data.embeddings.len(),
                visual_units.len()
            ),
            completed: 0,
        });
    }

    for (visual_unit, embedding) in visual_units.iter().zip(envelope.data.embeddings.iter()) {
        if embedding.vectors.is_empty() || embedding.vectors[0].is_empty() {
            return Err(IndexingError {
                phase: "encode",
                message: format!(
                    "Sidecar returned an empty document embedding for {}.",
                    visual_unit.source_path
                ),
                completed: 0,
            });
        }
        if let Some(source_type) = &embedding.source_type {
            if source_type != &visual_unit.source_type {
                return Err(IndexingError {
                    phase: "encode",
                    message: format!(
                        "Sidecar returned source_type {} for {}, but the expected source_type was {}.",
                        source_type, visual_unit.source_path, visual_unit.source_type
                    ),
                    completed: 0,
                });
            }
        }
        if let Some(kind) = &embedding.kind {
            if kind != &visual_unit.kind {
                return Err(IndexingError {
                    phase: "encode",
                    message: format!(
                        "Sidecar returned kind {} for {}, but the expected kind was {}.",
                        kind, visual_unit.source_path, visual_unit.kind
                    ),
                    completed: 0,
                });
            }
        }
        if let Some(path) = &embedding.path {
            if path != &visual_unit.source_path {
                return Err(IndexingError {
                    phase: "encode",
                    message: format!(
                        "Sidecar returned a document embedding for {}, but the expected path was {}.",
                        path, visual_unit.source_path
                    ),
                    completed: 0,
                });
            }
        }
        if let Some(locator) = &embedding.locator {
            if locator != &visual_unit.locator {
                return Err(IndexingError {
                    phase: "encode",
                    message: format!(
                        "Sidecar returned locator {} for {}, but the expected locator was {}.",
                        locator, visual_unit.source_path, visual_unit.locator
                    ),
                    completed: 0,
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
            };
        }
    };

    let response = match Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("sidecar probe client should be constructible")
        .get(format!("{base_url}/capabilities"))
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            return LocalSidecarProviderSnapshot {
                probe: ProviderProbeSnapshot {
                    status: "runtime_unavailable".to_string(),
                    message: format!("Sidecar capabilities probe failed: {error}"),
                    last_probed_at: Some(now),
                },
                runtime_model: fallback_runtime_model,
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
            };
        }
    };

    let runtime_model = payload
        .get("operations")
        .and_then(Value::as_array)
        .and_then(|operations| {
            operations.iter().find_map(|operation| {
                operation.get("model").map(|model| ProviderRuntimeModelSnapshot {
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
        };
    }

    let supported_operations = payload
        .get("operations")
        .and_then(Value::as_array)
        .map(|operations| {
            operations
                .iter()
                .filter(|item| item.get("supported").and_then(Value::as_bool).unwrap_or(false))
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
    }
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
