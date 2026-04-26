use crate::api::{ContentTypesPayload, EmbeddingCapabilities};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
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

#[derive(Clone, Debug)]
pub(crate) struct LocalSidecarActiveModel {
    pub(crate) model_id: String,
    pub(crate) version: String,
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

pub(crate) fn load_merged_runtime_config_from_paths(
    repo_path: &Path,
    runtime_path: &Path,
) -> Result<LoadedFauniConfig, io::Error> {
    let repo_value = load_config_value(repo_path, true)?;
    let runtime_value = load_config_value(runtime_path, false)?;
    decode_merged_runtime_config(repo_path, runtime_path, repo_value, runtime_value)
}

pub(crate) fn update_runtime_overlay_provider_config(
    provider_id: &str,
    enabled: bool,
    base_url: Option<&str>,
) -> Result<LoadedFauniConfig, io::Error> {
    update_runtime_overlay_config(|runtime_value| {
        let provider = ensure_child_object(runtime_value, "provider");
        let provider_record = ensure_child_object_map(provider, provider_id);
        provider_record.insert("enabled".to_string(), Value::Bool(enabled));
        match base_url.filter(|value| !value.trim().is_empty()) {
            Some(value) => {
                provider_record.insert("base_url".to_string(), Value::String(value.to_string()));
            }
            None => {
                provider_record.remove("base_url");
            }
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
    let provider = config.provider.get("local_sidecar").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Fauni config must define provider.local_sidecar.",
        )
    })?;
    let active_model = provider
        .active_model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "provider.local_sidecar.active_model must be a non-empty string.",
            )
        })?;
    let model = provider.models.get(active_model).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("provider.local_sidecar.active_model points to missing model {active_model}."),
        )
    })?;
    if !model.enabled {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("provider.local_sidecar.models.{active_model} is disabled."),
        ));
    }
    Ok(LocalSidecarActiveModel {
        model_id: active_model.to_string(),
        version: model.version.clone(),
    })
}

pub fn resolve_local_sidecar_active_model_from_env() -> Result<(String, String), io::Error> {
    let loaded = load_merged_runtime_config()?;
    let active = resolve_local_sidecar_active_model(&loaded.config)?;
    Ok((active.model_id, active.version))
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
