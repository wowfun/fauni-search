use crate::api::{ContentTypesPayload, EmbeddingCapabilities};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct FauniConfig {
    #[serde(default)]
    pub(crate) provider: BTreeMap<String, ProviderConfigFileRecord>,
    #[serde(default)]
    pub(crate) content_types: BTreeMap<String, ContentTypeConfigRecord>,
    #[serde(default)]
    pub(crate) libraries: BTreeMap<String, LibraryConfigOverlayRecord>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct ProviderConfigFileRecord {
    #[serde(default)]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) display_name: Option<String>,
    #[serde(default = "default_true")]
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) active_model: Option<String>,
    #[serde(default)]
    pub(crate) base_url: Option<String>,
    #[serde(default)]
    pub(crate) models: BTreeMap<String, ProviderModelConfigRecord>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct ProviderModelConfigRecord {
    #[serde(default = "default_true")]
    pub(crate) enabled: bool,
    #[serde(default = "default_model_version")]
    pub(crate) version: String,
    #[serde(default)]
    pub(crate) backend: Option<String>,
    #[serde(default)]
    pub(crate) embedding_capabilities: EmbeddingCapabilities,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct ContentTypeConfigRecord {
    #[serde(default = "default_true")]
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) model: String,
    #[serde(default)]
    pub(crate) vector_type: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct LibraryConfigOverlayRecord {
    #[serde(default)]
    pub(crate) display_name: Option<String>,
    #[serde(default)]
    pub(crate) content_types: BTreeMap<String, ContentTypeOverrideRecord>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct ContentTypeOverrideRecord {
    #[serde(default)]
    pub(crate) enabled: Option<bool>,
    #[serde(default)]
    pub(crate) model: Option<String>,
    #[serde(default)]
    pub(crate) vector_type: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct LoadedFauniConfig {
    pub(crate) config: FauniConfig,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ConfigOriginSnapshot {
    pub(crate) repo_providers: BTreeSet<String>,
    pub(crate) runtime_providers: BTreeSet<String>,
    pub(crate) repo_provider_models: BTreeMap<String, BTreeSet<String>>,
    pub(crate) runtime_provider_models: BTreeMap<String, BTreeSet<String>>,
    pub(crate) repo_content_types: BTreeSet<String>,
    pub(crate) runtime_content_types: BTreeSet<String>,
    pub(crate) repo_library_content_types: BTreeMap<String, BTreeSet<String>>,
    pub(crate) runtime_library_content_types: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Clone, Debug)]
pub(crate) struct LocalSidecarActiveModel {
    pub(crate) model_id: String,
    pub(crate) version: String,
    pub(crate) backend: String,
}

pub(crate) fn default_model_version() -> String {
    "main".to_string()
}

fn default_true() -> bool {
    true
}

pub(crate) fn merged_config_paths_from_env() -> Result<(PathBuf, PathBuf), io::Error> {
    let repo_path = env::var("FAUNI_CONFIG_PATH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("fauni.config.json"));
    let runtime_dir = env::var("APP_RUNTIME_DIR").map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Missing required environment variable APP_RUNTIME_DIR; source .env or use scripts/local/run.sh",
        )
    })?;
    let runtime_path = PathBuf::from(runtime_dir).join("runtime-config.json");
    Ok((repo_path, runtime_path))
}

pub(crate) fn load_merged_runtime_config() -> Result<LoadedFauniConfig, io::Error> {
    let (repo_path, runtime_path) = merged_config_paths_from_env()?;
    load_merged_runtime_config_from_paths(&repo_path, &runtime_path)
}

pub(crate) fn load_config_origin_snapshot() -> Result<ConfigOriginSnapshot, io::Error> {
    let (repo_path, runtime_path) = merged_config_paths_from_env()?;
    let repo_value = load_config_value(&repo_path, true)?;
    let runtime_value = load_config_value(&runtime_path, false)?;
    Ok(ConfigOriginSnapshot {
        repo_providers: object_child_keys(&repo_value, &["provider"]),
        runtime_providers: object_child_keys(&runtime_value, &["provider"]),
        repo_provider_models: provider_model_keys(&repo_value),
        runtime_provider_models: provider_model_keys(&runtime_value),
        repo_content_types: object_child_keys(&repo_value, &["content_types"]),
        runtime_content_types: object_child_keys(&runtime_value, &["content_types"]),
        repo_library_content_types: library_content_type_keys(&repo_value),
        runtime_library_content_types: library_content_type_keys(&runtime_value),
    })
}

pub(crate) fn load_merged_runtime_config_from_paths(
    repo_path: &Path,
    runtime_path: &Path,
) -> Result<LoadedFauniConfig, io::Error> {
    let repo_value = load_config_value(repo_path, true)?;
    let runtime_value = load_config_value(runtime_path, false)?;
    decode_merged_runtime_config(repo_path, runtime_path, repo_value, runtime_value)
}

pub(crate) fn upsert_runtime_overlay_provider_config(
    provider_id: &str,
    provider: &ProviderConfigFileRecord,
) -> Result<LoadedFauniConfig, io::Error> {
    update_runtime_overlay_config(|runtime_value| {
        let providers = ensure_child_object(runtime_value, "provider");
        providers.insert(
            provider_id.to_string(),
            serde_json::to_value(provider).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to encode provider config: {error}"),
                )
            })?,
        );
        Ok(())
    })
}

pub(crate) fn delete_runtime_overlay_provider_config(
    provider_id: &str,
) -> Result<LoadedFauniConfig, io::Error> {
    update_runtime_overlay_config(|runtime_value| {
        if let Some(providers) = runtime_value
            .as_object_mut()
            .and_then(|root| root.get_mut("provider"))
            .and_then(Value::as_object_mut)
        {
            providers.remove(provider_id);
            if providers.is_empty() {
                if let Some(root) = runtime_value.as_object_mut() {
                    root.remove("provider");
                }
            }
        }
        Ok(())
    })
}

pub(crate) fn upsert_runtime_overlay_provider_model(
    provider_id: &str,
    model_id: &str,
    model: &ProviderModelConfigRecord,
) -> Result<LoadedFauniConfig, io::Error> {
    update_runtime_overlay_config(|runtime_value| {
        let providers = ensure_child_object(runtime_value, "provider");
        let provider = ensure_child_object_map(providers, provider_id);
        let models = ensure_child_object_map(provider, "models");
        models.insert(
            model_id.to_string(),
            serde_json::to_value(model).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to encode provider model config: {error}"),
                )
            })?,
        );
        Ok(())
    })
}

pub(crate) fn delete_runtime_overlay_provider_model(
    provider_id: &str,
    model_id: &str,
) -> Result<LoadedFauniConfig, io::Error> {
    update_runtime_overlay_config(|runtime_value| {
        let Some(root) = runtime_value.as_object_mut() else {
            return Ok(());
        };
        let Some(providers) = root.get_mut("provider").and_then(Value::as_object_mut) else {
            return Ok(());
        };
        let Some(provider) = providers
            .get_mut(provider_id)
            .and_then(Value::as_object_mut)
        else {
            return Ok(());
        };
        if let Some(models) = provider.get_mut("models").and_then(Value::as_object_mut) {
            models.remove(model_id);
            if models.is_empty() {
                provider.remove("models");
            }
        }
        if provider.is_empty() {
            providers.remove(provider_id);
        }
        if providers.is_empty() {
            root.remove("provider");
        }
        Ok(())
    })
}

pub(crate) fn update_runtime_overlay_content_types(
    payload: &ContentTypesPayload,
) -> Result<LoadedFauniConfig, io::Error> {
    update_runtime_overlay_config(|runtime_value| {
        let root = ensure_object(runtime_value);
        root.insert(
            "content_types".to_string(),
            serde_json::to_value(&payload.content_types).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to encode content types payload: {error}"),
                )
            })?,
        );
        Ok(())
    })
}

pub(crate) fn upsert_runtime_overlay_content_type(
    content_type: &str,
    binding: &ContentTypeConfigRecord,
) -> Result<LoadedFauniConfig, io::Error> {
    update_runtime_overlay_config(|runtime_value| {
        let content_types = ensure_child_object(runtime_value, "content_types");
        content_types.insert(
            content_type.to_string(),
            serde_json::to_value(binding).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to encode content type binding: {error}"),
                )
            })?,
        );
        Ok(())
    })
}

pub(crate) fn delete_runtime_overlay_content_type(
    content_type: &str,
) -> Result<LoadedFauniConfig, io::Error> {
    update_runtime_overlay_config(|runtime_value| {
        if let Some(content_types) = runtime_value
            .as_object_mut()
            .and_then(|root| root.get_mut("content_types"))
            .and_then(Value::as_object_mut)
        {
            content_types.remove(content_type);
            if content_types.is_empty() {
                if let Some(root) = runtime_value.as_object_mut() {
                    root.remove("content_types");
                }
            }
        }
        Ok(())
    })
}

pub(crate) fn update_runtime_overlay_library_content_types(
    library_id: &str,
    payload: &ContentTypesPayload,
) -> Result<LoadedFauniConfig, io::Error> {
    update_runtime_overlay_config(|runtime_value| {
        let libraries = ensure_child_object(runtime_value, "libraries");
        let library_record = ensure_child_object_map(libraries, library_id);
        let mut content_types = BTreeMap::new();
        for (content_type, binding) in &payload.content_types {
            content_types.insert(
                content_type.clone(),
                ContentTypeOverrideRecord {
                    enabled: Some(binding.enabled),
                    model: Some(binding.model.clone()),
                    vector_type: Some(binding.vector_type.clone()),
                },
            );
        }
        library_record.insert(
            "content_types".to_string(),
            serde_json::to_value(content_types).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to encode library content types payload: {error}"),
                )
            })?,
        );
        Ok(())
    })
}

pub(crate) fn upsert_runtime_overlay_library_content_type(
    library_id: &str,
    content_type: &str,
    binding: &ContentTypeOverrideRecord,
) -> Result<LoadedFauniConfig, io::Error> {
    update_runtime_overlay_config(|runtime_value| {
        let libraries = ensure_child_object(runtime_value, "libraries");
        let library_record = ensure_child_object_map(libraries, library_id);
        let content_types = ensure_child_object_map(library_record, "content_types");
        content_types.insert(
            content_type.to_string(),
            serde_json::to_value(binding).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to encode library content type binding: {error}"),
                )
            })?,
        );
        Ok(())
    })
}

pub(crate) fn delete_runtime_overlay_library_content_type(
    library_id: &str,
    content_type: &str,
) -> Result<LoadedFauniConfig, io::Error> {
    update_runtime_overlay_config(|runtime_value| {
        let Some(root) = runtime_value.as_object_mut() else {
            return Ok(());
        };
        let Some(libraries) = root.get_mut("libraries").and_then(Value::as_object_mut) else {
            return Ok(());
        };
        let Some(library) = libraries.get_mut(library_id).and_then(Value::as_object_mut) else {
            return Ok(());
        };
        if let Some(content_types) = library
            .get_mut("content_types")
            .and_then(Value::as_object_mut)
        {
            content_types.remove(content_type);
            if content_types.is_empty() {
                library.remove("content_types");
            }
        }
        if library.is_empty() {
            libraries.remove(library_id);
        }
        if libraries.is_empty() {
            root.remove("libraries");
        }
        Ok(())
    })
}

fn update_runtime_overlay_config<F>(mutator: F) -> Result<LoadedFauniConfig, io::Error>
where
    F: FnOnce(&mut Value) -> Result<(), io::Error>,
{
    let (repo_path, runtime_path) = merged_config_paths_from_env()?;
    let repo_value = load_config_value(&repo_path, true)?;
    let mut runtime_value = load_config_value(&runtime_path, false)?;
    mutator(&mut runtime_value)?;
    let loaded =
        decode_merged_runtime_config(&repo_path, &runtime_path, repo_value, runtime_value.clone())?;
    write_runtime_overlay_value(&runtime_path, &runtime_value)?;
    Ok(loaded)
}

fn decode_merged_runtime_config(
    repo_path: &Path,
    runtime_path: &Path,
    mut repo_value: Value,
    runtime_value: Value,
) -> Result<LoadedFauniConfig, io::Error> {
    deep_merge_values(&mut repo_value, runtime_value);
    let config = serde_json::from_value::<FauniConfig>(repo_value).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Failed to decode merged Fauni config from {} and {}: {error}",
                repo_path.display(),
                runtime_path.display()
            ),
        )
    })?;
    validate_fauni_config(&config)?;
    Ok(LoadedFauniConfig { config })
}

pub(crate) fn resolve_local_sidecar_active_model(
    config: &FauniConfig,
) -> Result<LocalSidecarActiveModel, io::Error> {
    resolve_local_sidecar_model(config, None)
}

pub(crate) fn resolve_local_sidecar_model(
    config: &FauniConfig,
    selected_model_id: Option<&str>,
) -> Result<LocalSidecarActiveModel, io::Error> {
    let provider = config.provider.get("local_sidecar").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Fauni config must define provider.local_sidecar.",
        )
    })?;
    let model_id = match selected_model_id {
        Some(value) => value.trim(),
        None => provider
            .active_model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "provider.local_sidecar.active_model must be a non-empty string.",
                )
            })?,
    };
    if model_id.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "local_sidecar model id must be a non-empty string.",
        ));
    }
    let model = provider.models.get(model_id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("provider.local_sidecar.models does not define model {model_id}."),
        )
    })?;
    if !model.enabled {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("provider.local_sidecar.models.{model_id} is disabled."),
        ));
    }
    Ok(LocalSidecarActiveModel {
        model_id: model_id.to_string(),
        version: model.version.clone(),
        backend: model
            .backend
            .clone()
            .unwrap_or_else(|| "colqwen3_5".to_string()),
    })
}

pub fn resolve_local_sidecar_active_model_from_env() -> Result<(String, String, String), io::Error>
{
    resolve_local_sidecar_model_from_env(None)
}

pub fn resolve_local_sidecar_model_from_env(
    selected_model_id: Option<&str>,
) -> Result<(String, String, String), io::Error> {
    let loaded = load_merged_runtime_config()?;
    let active = resolve_local_sidecar_model(&loaded.config, selected_model_id)?;
    Ok((active.model_id, active.version, active.backend))
}

fn load_config_value(path: &Path, required: bool) -> Result<Value, io::Error> {
    if !path.exists() {
        return if required {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Fauni config file was not found: {}", path.display()),
            ))
        } else {
            Ok(Value::Object(Map::new()))
        };
    }

    let payload = fs::read_to_string(path).map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Failed to read Fauni config file {}: {error}",
                path.display()
            ),
        )
    })?;
    serde_json::from_str::<Value>(&payload).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Failed to parse Fauni config file {}: {error}",
                path.display()
            ),
        )
    })
}

fn write_runtime_overlay_value(path: &Path, value: &Value) -> Result<(), io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Failed to create runtime config directory {}: {error}",
                    parent.display()
                ),
            )
        })?;
    }

    let payload = serde_json::to_string_pretty(value).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to encode runtime config JSON: {error}"),
        )
    })?;
    fs::write(path, format!("{payload}\n")).map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to write runtime config {}: {error}", path.display()),
        )
    })
}

fn validate_fauni_config(config: &FauniConfig) -> Result<(), io::Error> {
    for (provider_id, provider) in &config.provider {
        if let Some(active_model) = provider
            .active_model
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if !provider.models.contains_key(active_model) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "provider.{provider_id}.active_model points to missing model {active_model}."
                    ),
                ));
            }
        }

        for (model_id, model) in &provider.models {
            if model.version.trim().is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("provider.{provider_id}.models.{model_id}.version cannot be empty."),
                ));
            }
            if provider_id == "local_sidecar" {
                let backend = model
                    .backend
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("colqwen3_5");
                if !["colqwen3_5", "qwen3_vl_embedding"].contains(&backend) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "provider.local_sidecar.models.{model_id}.backend is not supported: {backend}."
                        ),
                    ));
                }
            }
        }
    }

    for (content_type, binding) in &config.content_types {
        validate_model_reference(
            config,
            &binding.model,
            &format!("content_types.{content_type}.model"),
        )?;
        let (_, model) = resolve_model_reference(config, &binding.model)?;
        if !model
            .embedding_capabilities
            .vector_types
            .iter()
            .any(|value| value == &binding.vector_type)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "content_types.{content_type}.vector_type={} is not supported by model {}.",
                    binding.vector_type, binding.model
                ),
            ));
        }
    }

    Ok(())
}

fn validate_model_reference(
    config: &FauniConfig,
    model_ref: &str,
    field: &str,
) -> Result<(), io::Error> {
    resolve_model_reference(config, model_ref)
        .map(|_| ())
        .map_err(|error| io::Error::new(error.kind(), format!("{field} is invalid: {}", error)))
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    match value {
        Value::Object(map) => map,
        _ => unreachable!("value was just converted to an object"),
    }
}

fn ensure_child_object<'a>(value: &'a mut Value, key: &str) -> &'a mut Map<String, Value> {
    let root = ensure_object(value);
    ensure_child_object_map(root, key)
}

fn ensure_child_object_map<'a>(
    parent: &'a mut Map<String, Value>,
    key: &str,
) -> &'a mut Map<String, Value> {
    let entry = parent
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !entry.is_object() {
        *entry = Value::Object(Map::new());
    }
    match entry {
        Value::Object(map) => map,
        _ => unreachable!("entry was just converted to an object"),
    }
}

fn value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.as_object()?.get(*segment)?;
    }
    Some(current)
}

fn object_child_keys(value: &Value, path: &[&str]) -> BTreeSet<String> {
    value_at_path(value, path)
        .and_then(Value::as_object)
        .map(|map| map.keys().cloned().collect())
        .unwrap_or_default()
}

fn provider_model_keys(value: &Value) -> BTreeMap<String, BTreeSet<String>> {
    value_at_path(value, &["provider"])
        .and_then(Value::as_object)
        .map(|providers| {
            providers
                .iter()
                .filter_map(|(provider_id, provider)| {
                    let model_ids = provider
                        .get("models")
                        .and_then(Value::as_object)
                        .map(|models| models.keys().cloned().collect::<BTreeSet<_>>())
                        .unwrap_or_default();
                    if model_ids.is_empty() {
                        None
                    } else {
                        Some((provider_id.clone(), model_ids))
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn library_content_type_keys(value: &Value) -> BTreeMap<String, BTreeSet<String>> {
    value_at_path(value, &["libraries"])
        .and_then(Value::as_object)
        .map(|libraries| {
            libraries
                .iter()
                .filter_map(|(library_id, library)| {
                    let content_types = library
                        .get("content_types")
                        .and_then(Value::as_object)
                        .map(|items| items.keys().cloned().collect::<BTreeSet<_>>())
                        .unwrap_or_default();
                    if content_types.is_empty() {
                        None
                    } else {
                        Some((library_id.clone(), content_types))
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn resolve_model_reference<'a>(
    config: &'a FauniConfig,
    model_ref: &str,
) -> Result<(&'a ProviderConfigFileRecord, &'a ProviderModelConfigRecord), io::Error> {
    let (provider_id, model_id) = model_ref.split_once('/').ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("model reference must be provider_id/model_id, got {model_ref}."),
        )
    })?;
    let provider = config.provider.get(provider_id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("provider {provider_id} was not found for model reference {model_ref}."),
        )
    })?;
    let model = provider.models.get(model_id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("model {model_id} was not found for provider {provider_id}."),
        )
    })?;
    Ok((provider, model))
}

fn deep_merge_values(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base_map), Value::Object(overlay_map)) => {
            for (key, overlay_value) in overlay_map {
                match base_map.get_mut(&key) {
                    Some(base_value) => deep_merge_values(base_value, overlay_value),
                    None => {
                        base_map.insert(key, overlay_value);
                    }
                }
            }
        }
        (base_slot, overlay_value) => *base_slot = overlay_value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("fauni-config-{stamp}-{name}"))
    }

    #[test]
    fn merged_runtime_config_overlays_provider_models_and_content_types() {
        let dir = unique_temp_dir("merge");
        fs::create_dir_all(&dir).unwrap();
        let repo = dir.join("fauni.config.json");
        let runtime_dir = dir.join("runtime");
        fs::create_dir_all(&runtime_dir).unwrap();
        let runtime = runtime_dir.join("runtime-config.json");

        fs::write(
            &repo,
            r#"{
              "provider": {
                "local_sidecar": {
                  "kind": "local_sidecar",
                  "active_model": "model-a",
                  "models": {
                    "model-a": {
                      "version": "main",
                      "embedding_capabilities": {
                        "vector_types": ["multi_vector_late_interaction"]
                      }
                    }
                  }
                }
              },
              "content_types": {
                "image": {
                  "enabled": true,
                  "model": "local_sidecar/model-a",
                  "vector_type": "multi_vector_late_interaction"
                }
              }
            }"#,
        )
        .unwrap();
        fs::write(
            &runtime,
            r#"{
              "provider": {
                "local_sidecar": {
                  "models": {
                    "model-a": {
                      "version": "custom-tag"
                    }
                  }
                }
              }
            }"#,
        )
        .unwrap();

        let loaded = load_merged_runtime_config_from_paths(&repo, &runtime).unwrap();
        let active = resolve_local_sidecar_active_model(&loaded.config).unwrap();
        assert_eq!(active.model_id, "model-a");
        assert_eq!(active.version, "custom-tag");
        assert_eq!(active.backend, "colqwen3_5");
    }

    #[test]
    fn local_sidecar_model_defaults_version_to_main() {
        let dir = unique_temp_dir("default-version");
        fs::create_dir_all(&dir).unwrap();
        let repo = dir.join("fauni.config.json");
        let runtime_dir = dir.join("runtime");
        fs::create_dir_all(&runtime_dir).unwrap();
        let runtime = runtime_dir.join("runtime-config.json");

        fs::write(
            &repo,
            r#"{
              "provider": {
                "local_sidecar": {
                  "kind": "local_sidecar",
                  "active_model": "model-a",
                  "models": {
                    "model-a": {
                      "embedding_capabilities": {
                        "vector_types": ["multi_vector_late_interaction"]
                      }
                    }
                  }
                }
              },
              "content_types": {
                "image": {
                  "enabled": true,
                  "model": "local_sidecar/model-a",
                  "vector_type": "multi_vector_late_interaction"
                }
              }
            }"#,
        )
        .unwrap();

        let loaded = load_merged_runtime_config_from_paths(&repo, &runtime).unwrap();
        let active = resolve_local_sidecar_active_model(&loaded.config).unwrap();
        assert_eq!(active.version, "main");
        assert_eq!(active.backend, "colqwen3_5");
    }

    #[test]
    fn explicit_local_sidecar_model_resolves_backend() {
        let dir = unique_temp_dir("selected-model");
        fs::create_dir_all(&dir).unwrap();
        let repo = dir.join("fauni.config.json");
        let runtime_dir = dir.join("runtime");
        fs::create_dir_all(&runtime_dir).unwrap();
        let runtime = runtime_dir.join("runtime-config.json");

        fs::write(
            &repo,
            r#"{
              "provider": {
                "local_sidecar": {
                  "kind": "local_sidecar",
                  "active_model": "model-a",
                  "models": {
                    "model-a": {
                      "version": "main",
                      "backend": "colqwen3_5",
                      "embedding_capabilities": {
                        "vector_types": ["multi_vector_late_interaction"]
                      }
                    },
                    "model-b": {
                      "version": "qwen-tag",
                      "backend": "qwen3_vl_embedding",
                      "embedding_capabilities": {
                        "vector_types": ["single_vector"]
                      }
                    }
                  }
                }
              },
              "content_types": {
                "image": {
                  "enabled": true,
                  "model": "local_sidecar/model-a",
                  "vector_type": "multi_vector_late_interaction"
                },
                "text": {
                  "enabled": false,
                  "model": "local_sidecar/model-b",
                  "vector_type": "single_vector"
                }
              }
            }"#,
        )
        .unwrap();

        let loaded = load_merged_runtime_config_from_paths(&repo, &runtime).unwrap();
        let selected = resolve_local_sidecar_model(&loaded.config, Some("model-b")).unwrap();
        assert_eq!(selected.model_id, "model-b");
        assert_eq!(selected.version, "qwen-tag");
        assert_eq!(selected.backend, "qwen3_vl_embedding");
    }

    #[test]
    fn missing_active_model_errors() {
        let dir = unique_temp_dir("missing-model");
        fs::create_dir_all(&dir).unwrap();
        let repo = dir.join("fauni.config.json");
        let runtime_dir = dir.join("runtime");
        fs::create_dir_all(&runtime_dir).unwrap();
        let runtime = runtime_dir.join("runtime-config.json");

        fs::write(
            &repo,
            r#"{
              "provider": {
                "local_sidecar": {
                  "kind": "local_sidecar",
                  "active_model": "missing",
                  "models": {}
                }
              }
            }"#,
        )
        .unwrap();

        let error = load_merged_runtime_config_from_paths(&repo, &runtime).unwrap_err();
        assert!(error.to_string().contains("active_model"));
    }
}
