use crate::{
    api::{
        ApiError, ModelDefaultsPayload, ModelOverridesPayload, ModelSelectionOverridePayload,
        ModelSelectionPayload, ProviderProbeSnapshot,
    },
    model::{ProviderConfigRecord, ResolvedExecutionModelSelection},
    MULTIVECTOR_INDEX_LINE,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    env,
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

const DASHSCOPE_MULTIVECTOR_MODEL_IDS: &[&str] = &[
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

pub(crate) fn supported_index_lines() -> [&'static str; 1] {
    [MULTIVECTOR_INDEX_LINE]
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
                "Connection and model are derived from the local runtime environment."
                    .to_string(),
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

pub(crate) fn default_global_model_defaults() -> ModelDefaultsPayload {
    let mut index_lines = BTreeMap::new();
    index_lines.insert(
        MULTIVECTOR_INDEX_LINE.to_string(),
        default_local_sidecar_model_selection(),
    );
    ModelDefaultsPayload { index_lines }
}

pub(crate) fn default_library_model_overrides() -> ModelOverridesPayload {
    ModelOverridesPayload::default()
}

pub(crate) fn default_local_sidecar_model_selection() -> ModelSelectionPayload {
    let runtime = fallback_local_sidecar_runtime_model();
    ModelSelectionPayload {
        provider_id: LOCAL_SIDECAR_PROVIDER_ID.to_string(),
        model_id: runtime.model_id,
    }
}

pub(crate) fn fallback_local_sidecar_runtime_model() -> ProviderRuntimeModelSnapshot {
    let model_id = env::var("TEXT_SEARCH_MODEL_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "runtime-bound-local-sidecar".to_string());
    let model_revision = env::var("TEXT_SEARCH_MODEL_REVISION")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    ProviderRuntimeModelSnapshot {
        model_id,
        model_revision,
    }
}

pub(crate) fn ensure_default_model_config_state(
    provider_configs: &mut BTreeMap<String, ProviderConfigRecord>,
    global_model_defaults: &mut ModelDefaultsPayload,
) {
    let defaults = default_provider_configs();
    for (provider_id, config) in defaults {
        provider_configs.entry(provider_id).or_insert(config);
    }

    if !global_model_defaults
        .index_lines
        .contains_key(MULTIVECTOR_INDEX_LINE)
    {
        global_model_defaults.index_lines.insert(
            MULTIVECTOR_INDEX_LINE.to_string(),
            default_local_sidecar_model_selection(),
        );
    }
}

pub(crate) fn normalize_model_defaults(
    payload: ModelDefaultsPayload,
) -> Result<ModelDefaultsPayload, ApiError> {
    let selection = payload
        .index_lines
        .get(MULTIVECTOR_INDEX_LINE)
        .cloned()
        .ok_or_else(|| {
            ApiError::validation_failed(
                "model defaults must include index_lines.multivector.",
                Some(json!({ "field": "index_lines.multivector" })),
            )
        })?;

    let mut index_lines = BTreeMap::new();
    index_lines.insert(
        MULTIVECTOR_INDEX_LINE.to_string(),
        normalize_model_selection(selection, "index_lines.multivector")?,
    );
    Ok(ModelDefaultsPayload { index_lines })
}

pub(crate) fn normalize_model_overrides(
    payload: ModelOverridesPayload,
) -> Result<ModelOverridesPayload, ApiError> {
    let mut normalized = ModelOverridesPayload::default();
    if let Some(selection) = payload.index_lines.get(MULTIVECTOR_INDEX_LINE).cloned() {
        normalized.index_lines.insert(
            MULTIVECTOR_INDEX_LINE.to_string(),
            normalize_model_selection_override(selection, "index_lines.multivector")?,
        );
    }
    Ok(normalized)
}

pub(crate) fn effective_library_model_overrides_payload(
    overrides: &ModelOverridesPayload,
) -> ModelOverridesPayload {
    let mut payload = overrides.clone();
    payload
        .index_lines
        .entry(MULTIVECTOR_INDEX_LINE.to_string())
        .or_default();
    payload
}

pub(crate) fn provider_context_payload(
    selection: &ResolvedExecutionModelSelection,
) -> Value {
    json!({
        "provider_id": selection.summary.provider_id,
        "provider_kind": selection.summary.provider_kind,
        "model_id": selection.summary.model_id,
        "model_revision": selection.summary.model_revision,
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

fn normalize_model_selection(
    selection: ModelSelectionPayload,
    field_prefix: &str,
) -> Result<ModelSelectionPayload, ApiError> {
    let provider_id = normalize_provider_id(&selection.provider_id, field_prefix)?;
    let model_id = normalize_required_string(
        &selection.model_id,
        &format!("{field_prefix}.model_id"),
    )?;
    validate_provider_selection_shape(&provider_id, &model_id, field_prefix)?;

    Ok(ModelSelectionPayload {
        provider_id,
        model_id,
    })
}

fn normalize_model_selection_override(
    selection: ModelSelectionOverridePayload,
    field_prefix: &str,
) -> Result<ModelSelectionOverridePayload, ApiError> {
    let provider_id = selection
        .provider_id
        .map(|value| normalize_provider_id(&value, field_prefix))
        .transpose()?;
    let model_id = selection
        .model_id
        .map(|value| normalize_required_string(&value, &format!("{field_prefix}.model_id")))
        .transpose()?;

    if let (Some(provider_id), Some(model_id)) = (provider_id.as_deref(), model_id.as_deref()) {
        validate_provider_selection_shape(provider_id, model_id, field_prefix)?;
    }

    Ok(ModelSelectionOverridePayload {
        provider_id,
        model_id,
    })
}

fn normalize_provider_id(value: &str, field_prefix: &str) -> Result<String, ApiError> {
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

fn normalize_required_string(value: &str, field: &str) -> Result<String, ApiError> {
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
    if provider_id == DASHSCOPE_PROVIDER_ID && !is_supported_dashscope_multivector_model(model_id) {
        return Err(ApiError::validation_failed(
            "model_id is not supported for multivector DashScope selection.",
            Some(json!({
                "field": format!("{field_prefix}.model_id"),
                "supported": DASHSCOPE_MULTIVECTOR_MODEL_IDS,
                "received": model_id,
            })),
        ));
    }

    Ok(())
}

pub(crate) fn dashscope_multivector_model_ids() -> &'static [&'static str] {
    DASHSCOPE_MULTIVECTOR_MODEL_IDS
}

pub(crate) fn is_supported_dashscope_multivector_model(model_id: &str) -> bool {
    DASHSCOPE_MULTIVECTOR_MODEL_IDS.contains(&model_id)
}
