use crate::{
    api::{ApiError, EmbeddingCapabilities, ProviderProbeSnapshot},
    config::{
        load_merged_runtime_config, load_merged_runtime_config_from_paths,
        resolve_local_sidecar_active_model,
    },
    model::{ProviderConfigRecord, ResolvedExecutionModelSelection},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    env,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) const LOCAL_SIDECAR_PROVIDER_ID: &str = "local_sidecar";
pub(crate) const DASHSCOPE_PROVIDER_ID: &str = "dashscope";
pub(crate) const LOCAL_SIDECAR_PROVIDER_KIND: &str = "local_sidecar";
pub(crate) const DASHSCOPE_PROVIDER_KIND: &str = "dashscope";

pub(crate) const QUERY_KIND_TEXT: &str = "text";
pub(crate) const QUERY_KIND_IMAGE: &str = "image";
pub(crate) const QUERY_KIND_VIDEO: &str = "video";
pub(crate) const QUERY_KIND_DOCUMENT: &str = "document";
pub(crate) const VECTOR_TYPE_SINGLE: &str = "single_vector";
pub(crate) const VECTOR_TYPE_INDEPENDENT: &str = "independent_vectors";
pub(crate) const VECTOR_TYPE_MULTI_VECTOR_LATE_INTERACTION: &str = "multi_vector_late_interaction";

const DASHSCOPE_SUPPORTED_CONTENT_MODEL_IDS: &[&str] = &[
    "multimodal-embedding-v1",
    "qwen2.5-vl-embedding",
    "qwen3-vl-embedding",
    "tongyi-embedding-vision-flash",
    "tongyi-embedding-vision-flash-2026-03-06",
    "tongyi-embedding-vision-plus",
    "tongyi-embedding-vision-plus-2026-03-06",
];

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct ProviderRuntimeModelSnapshot {
    pub(crate) model_id: String,
    pub(crate) model_revision: Option<String>,
}

pub(crate) fn empty_embedding_capabilities() -> EmbeddingCapabilities {
    EmbeddingCapabilities::default()
}

pub(crate) fn local_sidecar_embedding_capabilities() -> EmbeddingCapabilities {
    EmbeddingCapabilities {
        input_types: vec![QUERY_KIND_TEXT.to_string(), QUERY_KIND_IMAGE.to_string()],
        vector_types: vec![VECTOR_TYPE_MULTI_VECTOR_LATE_INTERACTION.to_string()],
        supports_mixed_inputs: false,
    }
}

pub(crate) fn local_sidecar_execution_input_types() -> Vec<String> {
    vec![
        QUERY_KIND_TEXT.to_string(),
        QUERY_KIND_IMAGE.to_string(),
        QUERY_KIND_DOCUMENT.to_string(),
        QUERY_KIND_VIDEO.to_string(),
    ]
}

pub(crate) fn dashscope_embedding_capabilities(model_id: &str) -> EmbeddingCapabilities {
    let input_types = vec![QUERY_KIND_TEXT.to_string(), QUERY_KIND_IMAGE.to_string()];
    let (vector_types, supports_mixed_inputs) = match model_id {
        "multimodal-embedding-v1" => (vec![VECTOR_TYPE_INDEPENDENT.to_string()], true),
        "qwen2.5-vl-embedding" => (vec![VECTOR_TYPE_SINGLE.to_string()], true),
        "qwen3-vl-embedding"
        | "tongyi-embedding-vision-flash"
        | "tongyi-embedding-vision-flash-2026-03-06"
        | "tongyi-embedding-vision-plus"
        | "tongyi-embedding-vision-plus-2026-03-06" => (
            vec![
                VECTOR_TYPE_SINGLE.to_string(),
                VECTOR_TYPE_INDEPENDENT.to_string(),
            ],
            true,
        ),
        _ => (vec![VECTOR_TYPE_SINGLE.to_string()], true),
    };

    EmbeddingCapabilities {
        input_types,
        vector_types,
        supports_mixed_inputs,
    }
}

pub(crate) fn embedding_capabilities_supports_input_type(
    capabilities: &EmbeddingCapabilities,
    input_type: &str,
) -> bool {
    capabilities
        .input_types
        .iter()
        .any(|value| value == input_type)
}

pub(crate) fn execution_input_types_support_input_type(
    execution_input_types: &[String],
    input_type: &str,
) -> bool {
    execution_input_types
        .iter()
        .any(|value| value == input_type)
}

pub(crate) fn default_provider_configs() -> BTreeMap<String, ProviderConfigRecord> {
    let mut configs = BTreeMap::new();
    configs.insert(
        LOCAL_SIDECAR_PROVIDER_ID.to_string(),
        ProviderConfigRecord {
            provider_id: LOCAL_SIDECAR_PROVIDER_ID.to_string(),
            display_name: "Local Sidecar".to_string(),
            provider_kind: LOCAL_SIDECAR_PROVIDER_KIND.to_string(),
            enabled: true,
            base_url: None,
            readonly_reason: Some(
                "Connection and model are derived from the local runtime environment.".to_string(),
            ),
        },
    );
    configs.insert(
        DASHSCOPE_PROVIDER_ID.to_string(),
        ProviderConfigRecord {
            provider_id: DASHSCOPE_PROVIDER_ID.to_string(),
            display_name: "DashScope".to_string(),
            provider_kind: DASHSCOPE_PROVIDER_KIND.to_string(),
            enabled: true,
            base_url: None,
            readonly_reason: Some(
                "DashScope is configurable in this slice but not yet executable.".to_string(),
            ),
        },
    );
    configs
}

pub(crate) fn fallback_local_sidecar_runtime_model() -> ProviderRuntimeModelSnapshot {
    if let Ok(loaded) = load_merged_runtime_config() {
        if let Ok(active_model) = resolve_local_sidecar_active_model(&loaded.config) {
            return ProviderRuntimeModelSnapshot {
                model_id: active_model.model_id,
                model_revision: Some(active_model.version),
            };
        }
    }

    let repo_path = env::var("FAUNI_CONFIG_PATH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("fauni.config.json"));
    let runtime_path = env::var("APP_RUNTIME_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|runtime_dir| PathBuf::from(runtime_dir).join("runtime-config.json"))
        .unwrap_or_else(|| PathBuf::from(".fauni-missing-runtime-config.json"));
    if let Ok(loaded) = load_merged_runtime_config_from_paths(&repo_path, &runtime_path) {
        if let Ok(active_model) = resolve_local_sidecar_active_model(&loaded.config) {
            return ProviderRuntimeModelSnapshot {
                model_id: active_model.model_id,
                model_revision: Some(active_model.version),
            };
        }
    }

    let model_id = env::var("EMBEDDING_MODEL_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "runtime-bound-local-sidecar".to_string());
    let model_revision = env::var("EMBEDDING_MODEL_REVISION")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    ProviderRuntimeModelSnapshot {
        model_id,
        model_revision,
    }
}

pub(crate) fn provider_context_payload(selection: &ResolvedExecutionModelSelection) -> Value {
    json!({
        "provider_id": selection.summary.provider_id,
        "provider_kind": selection.summary.provider_kind,
        "model_id": selection.summary.model_id,
        "model_version": selection.summary.model_version,
        "model_revision": selection.summary.model_revision,
        "vector_type": selection.vector_type,
        "vector_space_id": selection.vector_space_id,
        "binding_source": selection.summary.binding_source,
    })
}

pub(crate) fn static_not_supported_probe(message: &str) -> ProviderProbeSnapshot {
    ProviderProbeSnapshot {
        status: "not_supported".to_string(),
        message: message.to_string(),
        last_probed_at: Some(current_rfc3339_timestamp()),
    }
}

pub(crate) fn current_rfc3339_timestamp() -> String {
    time::OffsetDateTime::from(SystemTime::now())
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs().to_string())
                .unwrap_or_else(|_| "0".to_string())
        })
}

pub(crate) fn normalize_provider_id(value: &str, field_prefix: &str) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ApiError::validation_failed(
            "provider_id must not be empty.",
            Some(json!({ "field": format!("{field_prefix}.provider_id") })),
        ));
    }

    if !matches!(trimmed, LOCAL_SIDECAR_PROVIDER_ID | DASHSCOPE_PROVIDER_ID) {
        return Err(ApiError::validation_failed(
            "provider_id must be one of the supported built-in providers.",
            Some(json!({
                "field": format!("{field_prefix}.provider_id"),
                "supported": [LOCAL_SIDECAR_PROVIDER_ID, DASHSCOPE_PROVIDER_ID],
                "received": trimmed,
            })),
        ));
    }

    Ok(trimmed.to_string())
}

pub(crate) fn normalize_required_string(value: &str, field: &str) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ApiError::validation_failed(
            "field must not be empty.",
            Some(json!({ "field": field })),
        ));
    }
    Ok(trimmed.to_string())
}

pub(crate) fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn validate_provider_selection_shape(
    provider_id: &str,
    model_id: &str,
    field_prefix: &str,
) -> Result<(), ApiError> {
    if provider_id == DASHSCOPE_PROVIDER_ID && !is_supported_dashscope_content_model(model_id) {
        return Err(ApiError::validation_failed(
            "model_id is not supported for the current DashScope content-type execution slice.",
            Some(json!({
                "field": format!("{field_prefix}.model_id"),
                "supported": DASHSCOPE_SUPPORTED_CONTENT_MODEL_IDS,
                "received": model_id,
            })),
        ));
    }

    Ok(())
}

pub(crate) fn dashscope_supported_content_model_ids() -> &'static [&'static str] {
    DASHSCOPE_SUPPORTED_CONTENT_MODEL_IDS
}

pub(crate) fn is_supported_dashscope_content_model(model_id: &str) -> bool {
    DASHSCOPE_SUPPORTED_CONTENT_MODEL_IDS.contains(&model_id)
}
