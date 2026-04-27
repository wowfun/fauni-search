use super::*;
use crate::config::{
    delete_runtime_overlay_content_type, delete_runtime_overlay_library_content_type,
    delete_runtime_overlay_provider_config, delete_runtime_overlay_provider_model,
    load_config_origin_snapshot, update_runtime_overlay_content_types,
    update_runtime_overlay_library_content_types, upsert_runtime_overlay_content_type,
    upsert_runtime_overlay_library_content_type, upsert_runtime_overlay_provider_config,
    upsert_runtime_overlay_provider_model, ContentTypeConfigRecord, ContentTypeOverrideRecord,
    ProviderConfigFileRecord, ProviderModelConfigRecord,
};
use crate::*;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

const PROVIDER_PROBE_STABLE_TTL_MS: u128 = 15_000;
const PROVIDER_PROBE_FAILURE_TTL_MS: u128 = 5_000;

impl AppState {
    pub(crate) fn list_provider_configs(&self) -> ProvidersListData {
        let mut providers = self
            .provider_configs
            .values()
            .map(|provider| self.provider_config_snapshot(provider))
            .collect::<Vec<_>>();
        providers.sort_by(|left, right| {
            left.display_name
                .cmp(&right.display_name)
                .then_with(|| left.provider_id.cmp(&right.provider_id))
        });
        ProvidersListData { providers }
    }

    pub(crate) async fn get_runtime_health(&mut self) -> RuntimeHealthData {
        self.refresh_boot_provider_probe_cache().await;

        let now = current_rfc3339_timestamp();
        let app = RuntimeProcessHealthSnapshot {
            component_id: "app".to_string(),
            display_name: "App".to_string(),
            status: "available".to_string(),
            message: "FauniSearch app is serving control-plane requests.".to_string(),
            last_checked_at: now.clone(),
            details: Some(json!({
                "env": std::env::var("FAUNI_ENV").unwrap_or_else(|_| "development".to_string()),
                "libraries": self.libraries.len(),
                "jobs": self.jobs.len(),
            })),
        };

        let qdrant = match crate::qdrant::probe_qdrant_runtime_health().await {
            Ok(collection_count) => RuntimeProcessHealthSnapshot {
                component_id: "qdrant".to_string(),
                display_name: "Qdrant".to_string(),
                status: "available".to_string(),
                message: format!(
                    "Qdrant is reachable with {} collection(s) visible.",
                    collection_count
                ),
                last_checked_at: now.clone(),
                details: Some(json!({
                    "collection_count": collection_count,
                })),
            },
            Err(error) => RuntimeProcessHealthSnapshot {
                component_id: "qdrant".to_string(),
                display_name: "Qdrant".to_string(),
                status: "runtime_unavailable".to_string(),
                message: error,
                last_checked_at: now.clone(),
                details: None,
            },
        };

        let mut providers = self
            .provider_configs
            .values()
            .map(|provider| {
                let probe = self
                    .provider_probe_cache
                    .get(&provider.provider_id)
                    .cloned();
                let runtime_model = self
                    .provider_runtime_models
                    .get(&provider.provider_id)
                    .cloned();
                let embedding_capabilities = self
                    .provider_embedding_capabilities
                    .get(&provider.provider_id)
                    .cloned()
                    .filter(|capabilities| {
                        !capabilities.input_types.is_empty()
                            || !capabilities.vector_types.is_empty()
                            || capabilities.supports_mixed_inputs
                    });
                let execution_input_types = self
                    .provider_execution_input_types
                    .get(&provider.provider_id)
                    .cloned()
                    .unwrap_or_default();
                let runtime_adapters = self
                    .provider_runtime_adapters
                    .get(&provider.provider_id)
                    .cloned()
                    .unwrap_or_default();

                RuntimeProviderHealthSnapshot {
                    provider_id: provider.provider_id.clone(),
                    display_name: provider.display_name.clone(),
                    provider_kind: provider.provider_kind.clone(),
                    enabled: provider.enabled,
                    status: probe
                        .as_ref()
                        .map(|snapshot| snapshot.status.clone())
                        .unwrap_or_else(|| "unknown".to_string()),
                    message: probe
                        .as_ref()
                        .map(|snapshot| snapshot.message.clone())
                        .unwrap_or_else(|| "Provider probe has not run yet.".to_string()),
                    last_probed_at: probe
                        .as_ref()
                        .and_then(|snapshot| snapshot.last_probed_at.clone()),
                    model_id: runtime_model.as_ref().map(|model| model.model_id.clone()),
                    model_version: runtime_model.as_ref().map(|model| {
                        self.configured_model_version(&provider.provider_id, &model.model_id)
                    }),
                    model_revision: runtime_model.and_then(|model| model.model_revision),
                    embedding_capabilities,
                    execution_input_types,
                    runtime_adapters,
                }
            })
            .collect::<Vec<_>>();
        providers.sort_by(|left, right| {
            left.display_name
                .cmp(&right.display_name)
                .then_with(|| left.provider_id.cmp(&right.provider_id))
        });

        RuntimeHealthData {
            app,
            qdrant,
            providers,
        }
    }

    pub(crate) async fn update_provider_config(
        &mut self,
        provider_id: &str,
        request: UpdateProviderConfigRequest,
    ) -> Result<ProviderConfigSnapshot, ApiError> {
        let existing = self.provider_configs.get(provider_id).cloned();
        let provider_models = self
            .provider_models
            .get(provider_id)
            .cloned()
            .unwrap_or_default();
        let provider_kind = normalize_optional_string(request.provider_kind)
            .or_else(|| {
                existing
                    .as_ref()
                    .map(|provider| provider.provider_kind.clone())
            })
            .unwrap_or_else(|| provider_id.to_string());
        let display_name = normalize_optional_string(request.display_name)
            .or_else(|| {
                existing
                    .as_ref()
                    .map(|provider| provider.display_name.clone())
            })
            .unwrap_or_else(|| provider_id.to_string());
        let enabled = request.enabled.unwrap_or_else(|| {
            existing
                .as_ref()
                .map(|provider| provider.enabled)
                .unwrap_or(true)
        });
        let base_url = normalize_optional_string(request.base_url).or_else(|| {
            existing
                .as_ref()
                .and_then(|provider| provider.base_url.clone())
        });
        let active_model = normalize_optional_string(request.active_model)
            .or_else(|| self.configured_active_model(provider_id))
            .or_else(|| provider_models.keys().next().cloned());

        let provider = ProviderConfigFileRecord {
            kind: provider_kind,
            display_name: Some(display_name),
            enabled,
            active_model,
            base_url,
            models: provider_models,
        };

        let loaded =
            upsert_runtime_overlay_provider_config(provider_id, &provider).map_err(|error| {
                ApiError::runtime_unavailable(
                    format!("Failed to write runtime config: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            })?;
        self.apply_config_backed_model_state(&loaded.config)
            .map_err(|error| {
                ApiError::runtime_unavailable(
                    format!("Failed to reload merged config state: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            })?;

        self.refresh_provider_probe_snapshot(provider_id).await;
        Ok(self
            .provider_configs
            .get(provider_id)
            .map(|provider| self.provider_config_snapshot(provider))
            .expect("updated provider config should be present"))
    }

    pub(crate) async fn delete_provider_config(
        &mut self,
        provider_id: &str,
    ) -> Result<ProvidersListData, ApiError> {
        let loaded = delete_runtime_overlay_provider_config(provider_id).map_err(|error| {
            ApiError::runtime_unavailable(
                format!("Failed to write runtime config: {error}"),
                Some(json!({ "config": "runtime-config.json" })),
            )
        })?;
        self.apply_config_backed_model_state(&loaded.config)
            .map_err(|error| {
                ApiError::runtime_unavailable(
                    format!("Failed to reload merged config state: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            })?;
        if self.provider_configs.contains_key(provider_id) {
            self.refresh_provider_probe_snapshot(provider_id).await;
        } else {
            self.clear_provider_probe_state(provider_id);
        }
        Ok(self.list_provider_configs())
    }

    pub(crate) async fn update_provider_model_config(
        &mut self,
        provider_id: &str,
        model_id: &str,
        request: UpdateProviderModelConfigRequest,
    ) -> Result<ProviderConfigSnapshot, ApiError> {
        if !self.provider_configs.contains_key(provider_id) {
            return Err(ApiError::not_found("Provider was not found."));
        }
        let existing = self
            .provider_models
            .get(provider_id)
            .and_then(|models| models.get(model_id))
            .cloned();
        let version = normalize_optional_string(request.version)
            .or_else(|| existing.as_ref().map(|model| model.version.clone()))
            .unwrap_or_else(default_model_version);
        let backend = normalize_optional_string(request.backend)
            .or_else(|| existing.as_ref().and_then(|model| model.backend.clone()));
        let embedding_capabilities = request
            .embedding_capabilities
            .or_else(|| {
                existing
                    .as_ref()
                    .map(|model| model.embedding_capabilities.clone())
            })
            .unwrap_or_default();
        let model = ProviderModelConfigRecord {
            enabled: request
                .enabled
                .unwrap_or_else(|| existing.as_ref().map(|model| model.enabled).unwrap_or(true)),
            version,
            backend,
            embedding_capabilities,
        };
        let loaded = upsert_runtime_overlay_provider_model(provider_id, model_id, &model).map_err(
            |error| {
                ApiError::runtime_unavailable(
                    format!("Failed to write runtime config: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            },
        )?;
        self.apply_config_backed_model_state(&loaded.config)
            .map_err(|error| {
                ApiError::runtime_unavailable(
                    format!("Failed to reload merged config state: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            })?;
        self.refresh_provider_probe_snapshot(provider_id).await;
        Ok(self
            .provider_configs
            .get(provider_id)
            .map(|provider| self.provider_config_snapshot(provider))
            .expect("updated provider config should be present"))
    }

    pub(crate) async fn delete_provider_model_config(
        &mut self,
        provider_id: &str,
        model_id: &str,
    ) -> Result<ProviderConfigSnapshot, ApiError> {
        let loaded =
            delete_runtime_overlay_provider_model(provider_id, model_id).map_err(|error| {
                ApiError::runtime_unavailable(
                    format!("Failed to write runtime config: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            })?;
        self.apply_config_backed_model_state(&loaded.config)
            .map_err(|error| {
                ApiError::runtime_unavailable(
                    format!("Failed to reload merged config state: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            })?;
        self.refresh_provider_probe_snapshot(provider_id).await;
        self.provider_configs
            .get(provider_id)
            .map(|provider| self.provider_config_snapshot(provider))
            .ok_or_else(|| ApiError::not_found("Provider was not found."))
    }

    pub(crate) async fn list_model_catalog(&mut self) -> ModelCatalogData {
        self.refresh_provider_probe_snapshot_if_stale(LOCAL_SIDECAR_PROVIDER_ID)
            .await;
        self.refresh_provider_probe_snapshot_if_stale(DASHSCOPE_PROVIDER_ID)
            .await;
        let dashscope_provider = self
            .provider_configs
            .get(DASHSCOPE_PROVIDER_ID)
            .cloned()
            .unwrap_or_else(|| {
                default_provider_configs()
                    .get(DASHSCOPE_PROVIDER_ID)
                    .cloned()
                    .expect("default dashscope provider should exist")
            });
        let dashscope_probe = self
            .provider_probe_cache
            .get(DASHSCOPE_PROVIDER_ID)
            .cloned();

        let mut entries = Vec::new();
        for (provider_id, models) in &self.provider_models {
            let Some(provider) = self.provider_configs.get(provider_id) else {
                continue;
            };
            let probe = self.provider_probe_cache.get(provider_id).cloned();
            let runtime = self.provider_runtime_models.get(provider_id).cloned();
            for (model_id, model) in models {
                let status = if !provider.enabled || !model.enabled {
                    "not_enabled".to_string()
                } else {
                    probe
                        .as_ref()
                        .map(|item| item.status.clone())
                        .unwrap_or_else(|| "unknown".to_string())
                };
                let message = if !provider.enabled {
                    format!("Provider {} is disabled.", provider.provider_id)
                } else if !model.enabled {
                    format!("Model {model_id} is disabled.")
                } else {
                    probe
                        .as_ref()
                        .map(|item| item.message.clone())
                        .unwrap_or_else(|| "Model metadata is available from config.".to_string())
                };
                entries.push(ModelCatalogEntry {
                    provider_id: provider_id.clone(),
                    provider_kind: provider.provider_kind.clone(),
                    model_id: model_id.clone(),
                    model_version: model.version.clone(),
                    model_revision: runtime
                        .as_ref()
                        .filter(|runtime| runtime.model_id == *model_id)
                        .and_then(|runtime| runtime.model_revision.clone()),
                    embedding_capabilities: model.embedding_capabilities.clone(),
                    editable: true,
                    status,
                    message,
                });
            }
        }

        let dashscope_status = if !dashscope_provider.enabled {
            "not_enabled".to_string()
        } else {
            dashscope_probe
                .as_ref()
                .map(|item| item.status.clone())
                .unwrap_or_else(|| "unknown".to_string())
        };
        let dashscope_message = if !dashscope_provider.enabled {
            "DashScope provider is disabled.".to_string()
        } else {
            dashscope_probe
                .as_ref()
                .map(|item| item.message.clone())
                .unwrap_or_else(|| "DashScope catalog metadata is available.".to_string())
        };

        for model_id in dashscope_supported_content_model_ids() {
            if entries.iter().any(|entry| {
                entry.provider_id == DASHSCOPE_PROVIDER_ID && entry.model_id == *model_id
            }) {
                continue;
            }
            entries.push(ModelCatalogEntry {
                provider_id: DASHSCOPE_PROVIDER_ID.to_string(),
                provider_kind: dashscope_provider.provider_kind.clone(),
                model_id: (*model_id).to_string(),
                model_version: self.configured_model_version(DASHSCOPE_PROVIDER_ID, model_id),
                model_revision: None,
                embedding_capabilities: dashscope_embedding_capabilities(model_id),
                editable: true,
                status: dashscope_status.clone(),
                message: dashscope_message.clone(),
            });
        }

        ModelCatalogData { entries }
    }

    pub(crate) fn get_global_content_types(&self) -> GlobalContentTypesData {
        let origins = load_config_origin_snapshot().unwrap_or_default();
        GlobalContentTypesData {
            content_types: ContentTypesPayload {
                content_types: self
                    .global_content_types
                    .iter()
                    .map(|(content_type, binding)| {
                        (
                            content_type.clone(),
                            ContentTypeBindingPayload {
                                enabled: binding.enabled,
                                model: binding.model.clone(),
                                vector_type: binding.vector_type.clone(),
                            },
                        )
                    })
                    .collect(),
            },
            origins: content_type_origin_map(
                self.global_content_types.keys(),
                &origins.repo_content_types,
                &origins.runtime_content_types,
                false,
            ),
        }
    }

    pub(crate) async fn update_global_content_types(
        &mut self,
        payload: ContentTypesPayload,
    ) -> Result<GlobalContentTypesData, ApiError> {
        let normalized = normalize_content_types_payload(payload)?;
        self.validate_content_types_payload(&normalized)?;

        let loaded = update_runtime_overlay_content_types(&normalized).map_err(|error| {
            ApiError::runtime_unavailable(
                format!("Failed to write runtime config: {error}"),
                Some(json!({ "config": "runtime-config.json" })),
            )
        })?;
        self.apply_config_backed_model_state(&loaded.config)
            .map_err(|error| {
                ApiError::runtime_unavailable(
                    format!("Failed to reload merged config state: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            })?;

        for provider_id in referenced_content_type_provider_ids(&normalized.content_types) {
            self.refresh_provider_probe_snapshot(&provider_id).await;
        }

        Ok(self.get_global_content_types())
    }

    pub(crate) async fn update_global_content_type(
        &mut self,
        content_type: &str,
        binding: ContentTypeBindingPayload,
    ) -> Result<GlobalContentTypesData, ApiError> {
        validate_settings_content_type_key(content_type)?;
        let normalized = normalize_content_type_binding(content_type, binding)?;
        self.validate_content_type_binding(content_type, &normalized)?;
        let config_binding = ContentTypeConfigRecord {
            enabled: normalized.enabled,
            model: normalized.model.clone(),
            vector_type: normalized.vector_type.clone(),
        };
        let loaded = upsert_runtime_overlay_content_type(content_type, &config_binding).map_err(
            |error| {
                ApiError::runtime_unavailable(
                    format!("Failed to write runtime config: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            },
        )?;
        self.apply_config_backed_model_state(&loaded.config)
            .map_err(|error| {
                ApiError::runtime_unavailable(
                    format!("Failed to reload merged config state: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            })?;
        for provider_id in referenced_content_type_provider_ids(&BTreeMap::from([(
            content_type.to_string(),
            normalized,
        )])) {
            self.refresh_provider_probe_snapshot(&provider_id).await;
        }
        Ok(self.get_global_content_types())
    }

    pub(crate) async fn delete_global_content_type(
        &mut self,
        content_type: &str,
    ) -> Result<GlobalContentTypesData, ApiError> {
        validate_settings_content_type_key(content_type)?;
        let loaded = delete_runtime_overlay_content_type(content_type).map_err(|error| {
            ApiError::runtime_unavailable(
                format!("Failed to write runtime config: {error}"),
                Some(json!({ "config": "runtime-config.json" })),
            )
        })?;
        self.apply_config_backed_model_state(&loaded.config)
            .map_err(|error| {
                ApiError::runtime_unavailable(
                    format!("Failed to reload merged config state: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            })?;
        let provider_id = self
            .global_content_types
            .get(content_type)
            .and_then(|binding| split_model_reference(&binding.model))
            .map(|(provider_id, _)| provider_id.to_string());
        if let Some(provider_id) = provider_id {
            self.refresh_provider_probe_snapshot(&provider_id).await;
        }
        Ok(self.get_global_content_types())
    }

    pub(crate) fn get_library_content_types(
        &self,
        library_id: &str,
    ) -> Result<LibraryContentTypesData, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;
        Ok(LibraryContentTypesData {
            content_types: ContentTypesPayload {
                content_types: effective_library_content_type_bindings(
                    &self.global_content_types,
                    &library.content_type_overrides,
                ),
            },
            origins: {
                let origins = load_config_origin_snapshot().unwrap_or_default();
                let repo_keys = origins
                    .repo_library_content_types
                    .get(library_id)
                    .cloned()
                    .unwrap_or_default();
                let runtime_keys = origins
                    .runtime_library_content_types
                    .get(library_id)
                    .cloned()
                    .unwrap_or_default();
                content_type_origin_map(
                    self.global_content_types.keys(),
                    &repo_keys,
                    &runtime_keys,
                    true,
                )
            },
        })
    }

    pub(crate) async fn update_library_content_types(
        &mut self,
        library_id: &str,
        payload: ContentTypesPayload,
    ) -> Result<LibraryContentTypesData, ApiError> {
        self.validate_library_exists(library_id)?;
        let normalized = normalize_content_types_payload(payload)?;
        self.validate_content_types_payload(&normalized)?;

        let loaded = update_runtime_overlay_library_content_types(library_id, &normalized)
            .map_err(|error| {
                ApiError::runtime_unavailable(
                    format!("Failed to write runtime config: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            })?;
        self.apply_config_backed_model_state(&loaded.config)
            .map_err(|error| {
                ApiError::runtime_unavailable(
                    format!("Failed to reload merged config state: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            })?;

        for provider_id in referenced_content_type_provider_ids(&normalized.content_types) {
            self.refresh_provider_probe_snapshot(&provider_id).await;
        }

        self.get_library_content_types(library_id)
    }

    pub(crate) async fn update_library_content_type(
        &mut self,
        library_id: &str,
        content_type: &str,
        binding: ContentTypeBindingPayload,
    ) -> Result<LibraryContentTypesData, ApiError> {
        self.validate_library_exists(library_id)?;
        validate_settings_content_type_key(content_type)?;
        let normalized = normalize_content_type_binding(content_type, binding)?;
        self.validate_content_type_binding(content_type, &normalized)?;
        let override_record = ContentTypeOverrideRecord {
            enabled: Some(normalized.enabled),
            model: Some(normalized.model.clone()),
            vector_type: Some(normalized.vector_type.clone()),
        };
        let loaded =
            upsert_runtime_overlay_library_content_type(library_id, content_type, &override_record)
                .map_err(|error| {
                    ApiError::runtime_unavailable(
                        format!("Failed to write runtime config: {error}"),
                        Some(json!({ "config": "runtime-config.json" })),
                    )
                })?;
        self.apply_config_backed_model_state(&loaded.config)
            .map_err(|error| {
                ApiError::runtime_unavailable(
                    format!("Failed to reload merged config state: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            })?;
        for provider_id in referenced_content_type_provider_ids(&BTreeMap::from([(
            content_type.to_string(),
            normalized,
        )])) {
            self.refresh_provider_probe_snapshot(&provider_id).await;
        }
        self.get_library_content_types(library_id)
    }

    pub(crate) async fn delete_library_content_type(
        &mut self,
        library_id: &str,
        content_type: &str,
    ) -> Result<LibraryContentTypesData, ApiError> {
        self.validate_library_exists(library_id)?;
        validate_settings_content_type_key(content_type)?;
        let loaded = delete_runtime_overlay_library_content_type(library_id, content_type)
            .map_err(|error| {
                ApiError::runtime_unavailable(
                    format!("Failed to write runtime config: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            })?;
        self.apply_config_backed_model_state(&loaded.config)
            .map_err(|error| {
                ApiError::runtime_unavailable(
                    format!("Failed to reload merged config state: {error}"),
                    Some(json!({ "config": "runtime-config.json" })),
                )
            })?;
        let provider_id = self
            .libraries
            .get(library_id)
            .map(|library| {
                effective_library_content_type_bindings(
                    &self.global_content_types,
                    &library.content_type_overrides,
                )
            })
            .and_then(|bindings| {
                bindings
                    .get(content_type)
                    .and_then(|binding| split_model_reference(&binding.model))
                    .map(|(provider_id, _)| provider_id.to_string())
            });
        if let Some(provider_id) = provider_id {
            self.refresh_provider_probe_snapshot(&provider_id).await;
        }
        self.get_library_content_types(library_id)
    }

    pub(crate) async fn get_resolved_content_models(
        &mut self,
        library_id: &str,
    ) -> Result<ResolvedContentModelsData, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;
        let effective = effective_library_content_type_bindings(
            &self.global_content_types,
            &library.content_type_overrides,
        );

        let mut content_types = BTreeMap::new();
        for (content_type, binding) in effective {
            let binding_source = if library.content_type_overrides.contains_key(&content_type) {
                "library_content_type"
            } else {
                "global_content_type"
            };
            content_types.insert(
                content_type.clone(),
                self.resolve_content_type_selection(binding_source, &content_type, &binding)
                    .await,
            );
        }

        Ok(ResolvedContentModelsData { content_types })
    }

    pub(crate) async fn get_vector_space_diagnostics(
        &mut self,
        library_id: &str,
    ) -> Result<VectorSpaceDiagnosticsData, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;
        let resolved = self.get_resolved_content_models(library_id).await?;

        let mut active_vector_spaces = BTreeMap::<String, VectorSpaceDiagnosticSnapshot>::new();
        for selection in resolved.content_types.into_values() {
            let Some(vector_space_id) = selection.vector_space_id.clone() else {
                continue;
            };
            if !library.active_vector_spaces.contains(&vector_space_id) {
                continue;
            }

            let entry = active_vector_spaces
                .entry(vector_space_id.clone())
                .or_insert_with(|| VectorSpaceDiagnosticSnapshot {
                    vector_space_id: vector_space_id.clone(),
                    lifecycle_state: "active".to_string(),
                    content_types: Vec::new(),
                    provider_id: Some(selection.provider_id.clone()),
                    provider_kind: Some(selection.provider_kind.clone()),
                    model_id: Some(selection.model_id.clone()),
                    model_version: Some(selection.model_version.clone()),
                    vector_type: Some(selection.vector_type.clone()),
                    retired_at_ms: None,
                });
            entry.content_types.push(selection.content_type);
        }

        let mut vector_spaces = active_vector_spaces.into_values().collect::<Vec<_>>();
        for snapshot in &mut vector_spaces {
            snapshot.content_types.sort();
            snapshot.content_types.dedup();
        }

        let mut retired_vector_spaces = library
            .retired_vector_spaces
            .into_iter()
            .filter(|(vector_space_id, _)| !library.active_vector_spaces.contains(vector_space_id))
            .map(|(vector_space_id, retired)| VectorSpaceDiagnosticSnapshot {
                vector_space_id,
                lifecycle_state: "retired".to_string(),
                content_types: Vec::new(),
                provider_id: None,
                provider_kind: None,
                model_id: None,
                model_version: None,
                vector_type: None,
                retired_at_ms: Some(retired.retired_at_ms),
            })
            .collect::<Vec<_>>();

        vector_spaces.append(&mut retired_vector_spaces);
        vector_spaces.sort_by(|left, right| {
            left.lifecycle_state
                .cmp(&right.lifecycle_state)
                .then_with(|| left.vector_space_id.cmp(&right.vector_space_id))
        });

        Ok(VectorSpaceDiagnosticsData { vector_spaces })
    }

    async fn resolve_content_type_selection(
        &mut self,
        binding_source: &str,
        content_type: &str,
        binding: &ContentTypeBindingPayload,
    ) -> ResolvedContentModelSelectionPayload {
        let Some((provider_id, model_id)) = split_model_reference(&binding.model) else {
            return ResolvedContentModelSelectionPayload {
                binding_source: binding_source.to_string(),
                content_type: content_type.to_string(),
                provider_id: "invalid".to_string(),
                provider_kind: "invalid".to_string(),
                model_id: binding.model.clone(),
                model_version: default_model_version(),
                model_revision: None,
                vector_type: binding.vector_type.clone(),
                vector_space_id: None,
                embedding_capabilities: empty_embedding_capabilities(),
                status: "conflict".to_string(),
                message: "Configured model reference must use provider_id/model_id.".to_string(),
                last_probed_at: None,
            };
        };

        if !binding.enabled {
            let model_version = self.configured_model_version(provider_id, model_id);
            return ResolvedContentModelSelectionPayload {
                binding_source: binding_source.to_string(),
                content_type: content_type.to_string(),
                provider_id: provider_id.to_string(),
                provider_kind: self
                    .provider_configs
                    .get(provider_id)
                    .map(|provider| provider.provider_kind.clone())
                    .unwrap_or_else(|| "unknown".to_string()),
                model_id: model_id.to_string(),
                model_version: model_version.clone(),
                model_revision: None,
                vector_type: binding.vector_type.clone(),
                vector_space_id: Some(vector_space_id(
                    provider_id,
                    model_id,
                    &model_version,
                    &binding.vector_type,
                )),
                embedding_capabilities: self
                    .provider_embedding_capabilities
                    .get(provider_id)
                    .cloned()
                    .unwrap_or_else(|| {
                        if provider_id == DASHSCOPE_PROVIDER_ID {
                            dashscope_embedding_capabilities(model_id)
                        } else {
                            empty_embedding_capabilities()
                        }
                    }),
                status: "not_enabled".to_string(),
                message: format!("content_type={content_type} is disabled."),
                last_probed_at: self
                    .provider_probe_cache
                    .get(provider_id)
                    .and_then(|probe| probe.last_probed_at.clone()),
            };
        }

        let summary = self
            .build_resolved_model_selection(
                binding_source,
                ModelSelectionPayload {
                    provider_id: provider_id.to_string(),
                    model_id: model_id.to_string(),
                },
                content_type,
            )
            .await;

        let provider_id = summary.provider_id.clone();
        let model_id = summary.model_id.clone();
        let model_version = summary.model_version.clone();
        ResolvedContentModelSelectionPayload {
            binding_source: summary.binding_source,
            content_type: content_type.to_string(),
            provider_id: provider_id.clone(),
            provider_kind: summary.provider_kind,
            model_id: model_id.clone(),
            model_version: model_version.clone(),
            model_revision: summary.model_revision,
            vector_type: binding.vector_type.clone(),
            vector_space_id: Some(vector_space_id(
                &provider_id,
                &model_id,
                &model_version,
                &binding.vector_type,
            )),
            embedding_capabilities: summary.embedding_capabilities,
            status: summary.status,
            message: summary.message,
            last_probed_at: summary.last_probed_at,
        }
    }

    pub(crate) async fn test_model_selection(
        &mut self,
        provider_id: &str,
        model_id: &str,
        input_modality: &str,
        provider_enabled: Option<bool>,
        provider_base_url: Option<String>,
        text_input: Option<&str>,
        file_input: Option<&StagedSettingsModelTestFile>,
        comparison_input_modality: Option<&str>,
        comparison_text_input: Option<&str>,
        comparison_file_input: Option<&StagedSettingsModelTestFile>,
    ) -> Result<ModelTestData, ApiError> {
        let provider_id = normalize_provider_id(provider_id, "provider")?;
        let model_id = normalize_required_string(model_id, "model_id")?;
        let input_modality = normalize_model_test_modality(input_modality)?;
        let comparison_input_modality = comparison_input_modality
            .map(normalize_model_test_modality)
            .transpose()?;
        let mut provider = self
            .provider_configs
            .get(&provider_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Provider was not found."))?;

        let draft_base_url = normalize_optional_string(provider_base_url);
        if provider.provider_id == LOCAL_SIDECAR_PROVIDER_ID && draft_base_url.is_some() {
            return Err(ApiError::not_supported(
                "local_sidecar connection details are derived from runtime env and cannot be edited here.",
                Some(json!({ "provider_id": provider.provider_id })),
            ));
        }

        if let Some(enabled) = provider_enabled {
            provider.enabled = enabled;
        }
        if provider.provider_id == DASHSCOPE_PROVIDER_ID {
            provider.base_url = draft_base_url.or(provider.base_url.clone());
        }

        if !provider.enabled {
            return Err(ApiError::not_enabled(
                "Model test references a disabled provider.",
                Some(json!({ "provider_id": provider.provider_id })),
            ));
        }

        let selection = ModelSelectionPayload {
            provider_id: provider.provider_id.clone(),
            model_id,
        };
        validate_provider_selection_shape(
            &selection.provider_id,
            &selection.model_id,
            "model_test",
        )?;
        self.validate_provider_model_binding_for_field(&provider, &selection, "model_id")?;

        match provider.provider_id.as_str() {
            LOCAL_SIDECAR_PROVIDER_ID => {
                self.refresh_provider_probe_snapshot_if_stale(LOCAL_SIDECAR_PROVIDER_ID)
                    .await;
                let probe = self
                    .provider_probe_cache
                    .get(LOCAL_SIDECAR_PROVIDER_ID)
                    .cloned()
                    .ok_or_else(|| {
                        ApiError::runtime_unavailable(
                            "local_sidecar probe snapshot is unavailable.",
                            Some(json!({ "provider_id": LOCAL_SIDECAR_PROVIDER_ID })),
                        )
                    })?;
                if probe.status != "available" {
                    return Err(ApiError::runtime_unavailable(
                        probe.message,
                        Some(json!({ "provider_id": LOCAL_SIDECAR_PROVIDER_ID })),
                    ));
                }

                let embedding_capabilities = self
                    .provider_embedding_capabilities
                    .get(LOCAL_SIDECAR_PROVIDER_ID)
                    .cloned()
                    .unwrap_or_else(local_sidecar_embedding_capabilities);
                if !embedding_capabilities_supports_input_type(
                    &embedding_capabilities,
                    &input_modality,
                ) {
                    return Err(ApiError::validation_failed(
                        "The selected local_sidecar runtime does not support the requested model test input type.",
                        Some(json!({
                            "provider_id": LOCAL_SIDECAR_PROVIDER_ID,
                            "input_modality": input_modality,
                            "supported": embedding_capabilities.input_types,
                        })),
                    ));
                }

                let runtime_model = self
                    .provider_runtime_models
                    .get(LOCAL_SIDECAR_PROVIDER_ID)
                    .cloned()
                    .unwrap_or_else(fallback_local_sidecar_runtime_model);
                let vector_type = embedding_capabilities
                    .vector_types
                    .first()
                    .cloned()
                    .unwrap_or_default();
                let resolved_model = ResolvedModelSelectionPayload {
                    binding_source: "settings_model_test".to_string(),
                    provider_id: provider.provider_id.clone(),
                    provider_kind: provider.provider_kind.clone(),
                    model_id: runtime_model.model_id,
                    model_version: self
                        .configured_model_version(LOCAL_SIDECAR_PROVIDER_ID, &selection.model_id),
                    model_revision: runtime_model.model_revision,
                    embedding_capabilities,
                    status: "available".to_string(),
                    message: format!(
                        "Validated settings model test via {}.",
                        model_test_operation_kind(&input_modality)
                    ),
                    last_probed_at: probe.last_probed_at.clone(),
                };
                let provider_context =
                    Some(provider_context_payload(&ResolvedExecutionModelSelection {
                        summary: resolved_model.clone(),
                        vector_type,
                        vector_space_id: "settings_model_test".to_string(),
                        execution_input_types: self
                            .provider_execution_input_types
                            .get(LOCAL_SIDECAR_PROVIDER_ID)
                            .cloned()
                            .unwrap_or_default(),
                    }));

                let primary = run_settings_model_test_input(
                    input_modality.as_str(),
                    text_input,
                    file_input,
                    provider_context.clone(),
                    "input_modality",
                )
                .await?;

                let comparison = if let Some(comparison_modality) =
                    comparison_input_modality.as_deref()
                {
                    let comparison = run_settings_model_test_input(
                        comparison_modality,
                        comparison_text_input,
                        comparison_file_input,
                        provider_context,
                        "comparison_input_modality",
                    )
                    .await?;
                    let similarity_to_primary =
                        cosine_similarity(&primary.pooled_vector, &comparison.pooled_vector)?;
                    Some(ModelTestComparisonData {
                        input_modality: comparison_modality.to_string(),
                        operation_kind: model_test_operation_kind(comparison_modality).to_string(),
                        vector_shape: vector_shape(&comparison.vectors),
                        vectors: comparison.vectors,
                        pooled_vector: comparison.pooled_vector,
                        input_summary: comparison.input_summary,
                        similarity_to_primary,
                    })
                } else if comparison_text_input.is_some() || comparison_file_input.is_some() {
                    return Err(ApiError::validation_failed(
                        "comparison_input_modality is required when providing a second model test input.",
                        Some(json!({ "field": "comparison_input_modality" })),
                    ));
                } else {
                    None
                };

                Ok(ModelTestData {
                    resolved_model,
                    input_modality: input_modality.clone(),
                    operation_kind: model_test_operation_kind(&input_modality).to_string(),
                    vector_shape: vector_shape(&primary.vectors),
                    vectors: primary.vectors,
                    pooled_vector: primary.pooled_vector,
                    input_summary: primary.input_summary,
                    comparison,
                })
            }
            DASHSCOPE_PROVIDER_ID => Err(ApiError::not_supported(
                "dashscope is configurable but not executable in the current 005 slice.",
                Some(json!({
                    "provider_id": DASHSCOPE_PROVIDER_ID,
                    "input_modality": input_modality,
                })),
            )),
            _ => Err(ApiError::conflict(
                "Unknown provider kind.",
                Some(json!({ "provider_id": provider.provider_id })),
            )),
        }
    }

    pub(crate) fn configured_vector_space_bindings_for_library(
        &self,
        library_id: &str,
    ) -> Result<Vec<ConfiguredVectorSpaceBinding>, ApiError> {
        let content_types = self.get_library_content_types(library_id)?;
        self.configured_vector_space_bindings_from_payload(
            &content_types.content_types.content_types,
        )
    }

    pub(crate) async fn resolve_execution_groups_for_library(
        &mut self,
        library_id: &str,
    ) -> Result<Vec<VectorSpaceExecutionGroup>, ApiError> {
        let bindings = self.configured_vector_space_bindings_for_library(library_id)?;
        let active_visual_unit_count = self
            .libraries
            .get(library_id)
            .map(|library| library.visual_units.len())
            .unwrap_or(0);
        let mut groups = Vec::new();
        for binding in bindings {
            let summary = self
                .build_resolved_model_selection(
                    "content_type_execution",
                    binding.selection.clone(),
                    "vector_space",
                )
                .await;
            if summary.status != "available" {
                return Err(model_selection_error(&summary));
            }
            groups.push(VectorSpaceExecutionGroup {
                library_id: library_id.to_string(),
                vector_space_id: binding.vector_space_id.clone(),
                active_visual_unit_count,
                content_types: binding.content_types.clone(),
                resolved_model: ResolvedExecutionModelSelection {
                    summary,
                    vector_type: binding.vector_type.clone(),
                    vector_space_id: binding.vector_space_id,
                    execution_input_types: self
                        .provider_execution_input_types
                        .get(&binding.selection.provider_id)
                        .cloned()
                        .unwrap_or_default(),
                },
            });
        }
        Ok(groups)
    }

    pub(crate) async fn refresh_boot_provider_probe_cache(&mut self) {
        let provider_ids = self.provider_configs.keys().cloned().collect::<Vec<_>>();
        for provider_id in provider_ids {
            self.refresh_provider_probe_snapshot_if_stale(&provider_id)
                .await;
        }
    }

    pub(crate) async fn refresh_provider_probe_snapshot(
        &mut self,
        provider_id: &str,
    ) -> Option<ProviderProbeSnapshot> {
        let checked_at_ms = current_unix_ms();
        self.force_refresh_provider_probe_snapshot_at(provider_id, checked_at_ms)
            .await
    }

    async fn refresh_provider_probe_snapshot_if_stale(
        &mut self,
        provider_id: &str,
    ) -> Option<ProviderProbeSnapshot> {
        let now_ms = current_unix_ms();
        if self.provider_probe_cache_is_fresh(provider_id, now_ms) {
            return self.provider_probe_cache.get(provider_id).cloned();
        }
        self.force_refresh_provider_probe_snapshot_at(provider_id, now_ms)
            .await
    }

    async fn force_refresh_provider_probe_snapshot_at(
        &mut self,
        provider_id: &str,
        checked_at_ms: u128,
    ) -> Option<ProviderProbeSnapshot> {
        let provider = self.provider_configs.get(provider_id)?.clone();
        let (probe, runtime_model) = match provider.provider_id.as_str() {
            LOCAL_SIDECAR_PROVIDER_ID => {
                let snapshot =
                    probe_local_sidecar_provider(&provider, &self.provider_probe_client).await;
                self.provider_embedding_capabilities.insert(
                    provider_id.to_string(),
                    snapshot.embedding_capabilities.clone(),
                );
                self.provider_execution_input_types.insert(
                    provider_id.to_string(),
                    snapshot.execution_input_types.clone(),
                );
                self.provider_runtime_adapters
                    .insert(provider_id.to_string(), snapshot.runtime_adapters.clone());
                (snapshot.probe, Some(snapshot.runtime_model))
            }
            DASHSCOPE_PROVIDER_ID => {
                self.provider_embedding_capabilities
                    .insert(provider_id.to_string(), empty_embedding_capabilities());
                self.provider_execution_input_types.remove(provider_id);
                self.provider_runtime_adapters.remove(provider_id);
                (
                    static_not_supported_probe(
                        "dashscope is configurable in the current slice but not executable yet.",
                    ),
                    None,
                )
            }
            _ => {
                self.provider_embedding_capabilities
                    .insert(provider_id.to_string(), empty_embedding_capabilities());
                self.provider_execution_input_types.remove(provider_id);
                self.provider_runtime_adapters.remove(provider_id);
                (
                    ProviderProbeSnapshot {
                        status: "not_supported".to_string(),
                        message: format!("Unknown provider {}.", provider.provider_id),
                        last_probed_at: Some(current_rfc3339_timestamp()),
                    },
                    None,
                )
            }
        };
        self.provider_probe_cache
            .insert(provider_id.to_string(), probe.clone());
        self.provider_probe_checked_at_ms
            .insert(provider_id.to_string(), checked_at_ms);
        if let Some(runtime_model) = runtime_model {
            self.provider_runtime_models
                .insert(provider_id.to_string(), runtime_model);
        }
        Some(probe)
    }

    pub(super) fn provider_probe_cache_is_fresh(&self, provider_id: &str, now_ms: u128) -> bool {
        let Some(probe) = self.provider_probe_cache.get(provider_id) else {
            return false;
        };
        let Some(checked_at_ms) = self.provider_probe_checked_at_ms.get(provider_id) else {
            return false;
        };
        now_ms.saturating_sub(*checked_at_ms) < provider_probe_ttl_ms(probe)
    }

    fn clear_provider_probe_state(&mut self, provider_id: &str) {
        self.provider_probe_cache.remove(provider_id);
        self.provider_probe_checked_at_ms.remove(provider_id);
        self.provider_runtime_models.remove(provider_id);
        self.provider_embedding_capabilities.remove(provider_id);
        self.provider_execution_input_types.remove(provider_id);
        self.provider_runtime_adapters.remove(provider_id);
    }

    fn provider_config_snapshot(&self, provider: &ProviderConfigRecord) -> ProviderConfigSnapshot {
        let mut snapshot = provider.snapshot();
        if provider.provider_id == LOCAL_SIDECAR_PROVIDER_ID {
            snapshot.base_url = sidecar_base_url().ok();
        }
        let origins = load_config_origin_snapshot().unwrap_or_default();
        snapshot.origin = origin_label(
            origins.repo_providers.contains(&provider.provider_id),
            origins.runtime_providers.contains(&provider.provider_id),
            false,
        );
        snapshot.active_model = self.configured_active_model(&provider.provider_id);
        snapshot.models = self
            .provider_models
            .get(&provider.provider_id)
            .map(|models| {
                models
                    .iter()
                    .map(|(model_id, model)| ProviderModelConfigSnapshot {
                        model_id: model_id.clone(),
                        enabled: model.enabled,
                        version: model.version.clone(),
                        backend: model.backend.clone(),
                        embedding_capabilities: model.embedding_capabilities.clone(),
                        origin: origin_label(
                            origins
                                .repo_provider_models
                                .get(&provider.provider_id)
                                .map(|items| items.contains(model_id))
                                .unwrap_or(false),
                            origins
                                .runtime_provider_models
                                .get(&provider.provider_id)
                                .map(|items| items.contains(model_id))
                                .unwrap_or(false),
                            false,
                        ),
                    })
                    .collect()
            })
            .unwrap_or_default();
        snapshot.probe = self
            .provider_probe_cache
            .get(&provider.provider_id)
            .cloned();
        snapshot
    }

    fn configured_active_model(&self, provider_id: &str) -> Option<String> {
        self.provider_active_models.get(provider_id).cloned()
    }

    fn validate_library_exists(&self, library_id: &str) -> Result<(), ApiError> {
        if !self.libraries.contains_key(library_id) {
            return Err(ApiError::not_found("Library was not found."));
        }
        Ok(())
    }

    pub(crate) fn validate_content_types_payload(
        &self,
        payload: &ContentTypesPayload,
    ) -> Result<(), ApiError> {
        for (content_type, binding) in &payload.content_types {
            let binding = normalize_content_type_binding(content_type, binding.clone())?;
            self.validate_content_type_binding(content_type, &binding)?;
        }
        Ok(())
    }

    fn validate_content_type_binding(
        &self,
        content_type: &str,
        binding: &ContentTypeBindingPayload,
    ) -> Result<(), ApiError> {
        let (provider_id, model_id) = split_model_reference(&binding.model).ok_or_else(|| {
            ApiError::validation_failed(
                "content type model must use provider_id/model_id format.",
                Some(json!({
                    "field": format!("content_types.{content_type}.model"),
                    "received": binding.model,
                })),
            )
        })?;

        let provider = self
            .provider_configs
            .get(provider_id)
            .cloned()
            .unwrap_or_else(|| ProviderConfigRecord {
                provider_id: provider_id.to_string(),
                display_name: provider_id.to_string(),
                provider_kind: "unknown".to_string(),
                enabled: true,
                base_url: None,
                readonly_reason: None,
            });
        let selection = ModelSelectionPayload {
            provider_id: provider_id.to_string(),
            model_id: model_id.to_string(),
        };
        self.validate_provider_model_binding_for_field(
            &provider,
            &selection,
            &format!("content_types.{content_type}.model"),
        )?;

        let vector_types = if let Some(model) = self
            .provider_models
            .get(provider_id)
            .and_then(|models| models.get(model_id))
        {
            model.embedding_capabilities.vector_types.clone()
        } else if provider_id == LOCAL_SIDECAR_PROVIDER_ID {
            local_sidecar_embedding_capabilities().vector_types
        } else if provider_id == DASHSCOPE_PROVIDER_ID {
            dashscope_embedding_capabilities(model_id).vector_types
        } else {
            Vec::new()
        };

        if !vector_types.is_empty()
            && !vector_types
                .iter()
                .any(|value| value == &binding.vector_type)
        {
            return Err(ApiError::validation_failed(
                "content type vector_type is not supported by the selected model.",
                Some(json!({
                    "field": format!("content_types.{content_type}.vector_type"),
                    "received": binding.vector_type,
                    "supported": vector_types,
                })),
            ));
        }

        Ok(())
    }

    fn validate_provider_model_binding_for_field(
        &self,
        provider: &ProviderConfigRecord,
        selection: &ModelSelectionPayload,
        field: &str,
    ) -> Result<(), ApiError> {
        match provider.provider_id.as_str() {
            LOCAL_SIDECAR_PROVIDER_ID => {
                let configured_model = self
                    .provider_models
                    .get(LOCAL_SIDECAR_PROVIDER_ID)
                    .and_then(|models| models.get(&selection.model_id));
                if configured_model.is_none() {
                    return Err(ApiError::validation_failed(
                        "local_sidecar model_id is not configured in provider.local_sidecar.models.",
                        Some(json!({
                            "field": field,
                            "supported": self.provider_models
                                .get(LOCAL_SIDECAR_PROVIDER_ID)
                                .map(|models| models.keys().cloned().collect::<Vec<_>>())
                                .unwrap_or_default(),
                            "received": selection.model_id,
                        })),
                    ));
                }
                Ok(())
            }
            DASHSCOPE_PROVIDER_ID => {
                if !is_supported_dashscope_content_model(&selection.model_id) {
                    return Err(ApiError::validation_failed(
                        "DashScope model_id is not supported for the current content-type execution slice.",
                        Some(json!({
                            "field": field,
                            "supported": dashscope_supported_content_model_ids(),
                            "received": selection.model_id,
                        })),
                    ));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn configured_model_version(&self, provider_id: &str, model_id: &str) -> String {
        self.provider_models
            .get(provider_id)
            .and_then(|models| models.get(model_id))
            .map(|model| model.version.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(default_model_version)
    }

    async fn build_resolved_model_selection(
        &mut self,
        binding_source: &str,
        selection: ModelSelectionPayload,
        selection_target: &str,
    ) -> ResolvedModelSelectionPayload {
        let Some(provider) = self.provider_configs.get(&selection.provider_id).cloned() else {
            return ResolvedModelSelectionPayload {
                binding_source: binding_source.to_string(),
                provider_id: selection.provider_id,
                provider_kind: "missing".to_string(),
                model_id: selection.model_id,
                model_version: default_model_version(),
                model_revision: None,
                embedding_capabilities: empty_embedding_capabilities(),
                status: "conflict".to_string(),
                message: "Selected provider does not exist.".to_string(),
                last_probed_at: None,
            };
        };

        if !provider.enabled {
            return ResolvedModelSelectionPayload {
                binding_source: binding_source.to_string(),
                provider_id: provider.provider_id.clone(),
                provider_kind: provider.provider_kind.clone(),
                model_id: selection.model_id.clone(),
                model_version: self
                    .configured_model_version(&provider.provider_id, &selection.model_id),
                model_revision: None,
                embedding_capabilities: self
                    .provider_embedding_capabilities
                    .get(&provider.provider_id)
                    .cloned()
                    .unwrap_or_else(|| {
                        if provider.provider_id == DASHSCOPE_PROVIDER_ID {
                            dashscope_embedding_capabilities(&selection.model_id)
                        } else {
                            empty_embedding_capabilities()
                        }
                    }),
                status: "not_enabled".to_string(),
                message: format!("Provider {} is disabled.", provider.provider_id),
                last_probed_at: self
                    .provider_probe_cache
                    .get(&provider.provider_id)
                    .and_then(|probe| probe.last_probed_at.clone()),
            };
        }

        match provider.provider_id.as_str() {
            LOCAL_SIDECAR_PROVIDER_ID => {
                self.refresh_provider_probe_snapshot_if_stale(&provider.provider_id)
                    .await;
                let runtime_model = self
                    .provider_runtime_models
                    .get(&provider.provider_id)
                    .cloned()
                    .unwrap_or_else(fallback_local_sidecar_runtime_model);
                let model_config = self
                    .provider_models
                    .get(&provider.provider_id)
                    .and_then(|models| models.get(&selection.model_id))
                    .cloned();
                let embedding_capabilities = model_config
                    .as_ref()
                    .map(|model| model.embedding_capabilities.clone())
                    .unwrap_or_else(local_sidecar_embedding_capabilities);
                let model_revision = if runtime_model.model_id == selection.model_id {
                    runtime_model.model_revision.clone()
                } else {
                    Some(self.configured_model_version(&provider.provider_id, &selection.model_id))
                };
                let probe = self
                    .provider_probe_cache
                    .get(&provider.provider_id)
                    .cloned();
                if let Some(probe) = &probe {
                    if probe.status != "available" {
                        return ResolvedModelSelectionPayload {
                            binding_source: binding_source.to_string(),
                            provider_id: provider.provider_id.clone(),
                            provider_kind: provider.provider_kind.clone(),
                            model_id: selection.model_id.clone(),
                            model_version: self.configured_model_version(
                                &provider.provider_id,
                                &selection.model_id,
                            ),
                            model_revision,
                            embedding_capabilities,
                            status: probe.status.clone(),
                            message: probe.message.clone(),
                            last_probed_at: probe.last_probed_at.clone(),
                        };
                    }
                }

                ResolvedModelSelectionPayload {
                    binding_source: binding_source.to_string(),
                    provider_id: provider.provider_id.clone(),
                    provider_kind: provider.provider_kind.clone(),
                    model_id: selection.model_id.clone(),
                    model_version: self
                        .configured_model_version(&provider.provider_id, &selection.model_id),
                    model_revision,
                    embedding_capabilities,
                    status: "available".to_string(),
                    message: format!(
                        "Resolved runtime-bound model for target={selection_target} via local_sidecar."
                    ),
                    last_probed_at: probe.and_then(|item| item.last_probed_at),
                }
            }
            DASHSCOPE_PROVIDER_ID => ResolvedModelSelectionPayload {
                binding_source: binding_source.to_string(),
                provider_id: provider.provider_id.clone(),
                provider_kind: provider.provider_kind.clone(),
                model_id: selection.model_id.clone(),
                model_version: self
                    .configured_model_version(&provider.provider_id, &selection.model_id),
                model_revision: None,
                embedding_capabilities: dashscope_embedding_capabilities(&selection.model_id),
                status: "not_supported".to_string(),
                message: "dashscope is configurable but not executable in the current 005 slice."
                    .to_string(),
                last_probed_at: self
                    .provider_probe_cache
                    .get(&provider.provider_id)
                    .and_then(|probe| probe.last_probed_at.clone()),
            },
            _ => {
                let model_id = selection.model_id.clone();
                ResolvedModelSelectionPayload {
                    binding_source: binding_source.to_string(),
                    provider_id: provider.provider_id.clone(),
                    provider_kind: provider.provider_kind.clone(),
                    model_id: model_id.clone(),
                    model_version: self.configured_model_version(&provider.provider_id, &model_id),
                    model_revision: None,
                    embedding_capabilities: empty_embedding_capabilities(),
                    status: "conflict".to_string(),
                    message: "Unknown provider kind.".to_string(),
                    last_probed_at: None,
                }
            }
        }
    }

    fn configured_vector_space_bindings_from_payload(
        &self,
        content_types: &BTreeMap<String, ContentTypeBindingPayload>,
    ) -> Result<Vec<ConfiguredVectorSpaceBinding>, ApiError> {
        let mut groups = BTreeMap::<String, ConfiguredVectorSpaceBinding>::new();
        for (content_type, binding) in content_types {
            if !binding.enabled {
                continue;
            }
            let (provider_id, model_id) =
                split_model_reference(&binding.model).ok_or_else(|| {
                    ApiError::validation_failed(
                        "enabled content type model must use provider_id/model_id format.",
                        Some(json!({
                            "field": format!("content_types.{content_type}.model"),
                            "received": binding.model,
                        })),
                    )
                })?;
            let selection = ModelSelectionPayload {
                provider_id: provider_id.to_string(),
                model_id: model_id.to_string(),
            };
            let version = self.configured_model_version(provider_id, model_id);
            let vector_space_id =
                vector_space_id(provider_id, model_id, &version, &binding.vector_type);
            groups
                .entry(vector_space_id.clone())
                .and_modify(|group| group.content_types.push(content_type.clone()))
                .or_insert_with(|| ConfiguredVectorSpaceBinding {
                    vector_space_id,
                    selection,
                    vector_type: binding.vector_type.clone(),
                    content_types: vec![content_type.clone()],
                });
        }
        Ok(groups.into_values().collect())
    }
}

fn normalize_content_types_payload(
    payload: ContentTypesPayload,
) -> Result<ContentTypesPayload, ApiError> {
    let mut content_types = BTreeMap::new();
    for (content_type, binding) in payload.content_types {
        validate_settings_content_type_key(&content_type)?;
        content_types.insert(
            content_type.clone(),
            normalize_content_type_binding(&content_type, binding)?,
        );
    }
    Ok(ContentTypesPayload { content_types })
}

fn validate_settings_content_type_key(content_type: &str) -> Result<(), ApiError> {
    if [
        QUERY_KIND_IMAGE,
        QUERY_KIND_DOCUMENT,
        QUERY_KIND_VIDEO,
        QUERY_KIND_TEXT,
    ]
    .contains(&content_type)
    {
        return Ok(());
    }
    Err(ApiError::validation_failed(
        "content_type is not supported by Settings runtime config CRUD.",
        Some(json!({
            "field": "content_type",
            "received": content_type,
            "supported": [
                QUERY_KIND_IMAGE,
                QUERY_KIND_DOCUMENT,
                QUERY_KIND_VIDEO,
                QUERY_KIND_TEXT,
            ],
        })),
    ))
}

fn provider_probe_ttl_ms(probe: &ProviderProbeSnapshot) -> u128 {
    if probe.status == "runtime_unavailable" {
        PROVIDER_PROBE_FAILURE_TTL_MS
    } else {
        PROVIDER_PROBE_STABLE_TTL_MS
    }
}

fn origin_label(repo_has_value: bool, runtime_has_value: bool, inherited: bool) -> String {
    if runtime_has_value {
        "runtime_overlay".to_string()
    } else if inherited {
        "inherited".to_string()
    } else if repo_has_value {
        "baseline".to_string()
    } else {
        "builtin".to_string()
    }
}

fn content_type_origin_map<'a>(
    keys: impl Iterator<Item = &'a String>,
    repo_keys: &BTreeSet<String>,
    runtime_keys: &BTreeSet<String>,
    inherited_when_absent: bool,
) -> BTreeMap<String, ContentTypeOriginSnapshot> {
    keys.map(|content_type| {
        let runtime_has_value = runtime_keys.contains(content_type);
        let repo_has_value = repo_keys.contains(content_type);
        (
            content_type.clone(),
            ContentTypeOriginSnapshot {
                origin: origin_label(
                    repo_has_value,
                    runtime_has_value,
                    inherited_when_absent && !runtime_has_value && !repo_has_value,
                ),
                has_runtime_overlay: runtime_has_value,
            },
        )
    })
    .collect()
}

fn normalize_content_type_binding(
    content_type: &str,
    binding: ContentTypeBindingPayload,
) -> Result<ContentTypeBindingPayload, ApiError> {
    let model = normalize_required_string(
        &binding.model,
        &format!("content_types.{content_type}.model"),
    )?;
    let vector_type = normalize_required_string(
        &binding.vector_type,
        &format!("content_types.{content_type}.vector_type"),
    )?;
    Ok(ContentTypeBindingPayload {
        enabled: binding.enabled,
        model,
        vector_type,
    })
}

fn effective_library_content_type_bindings(
    global: &BTreeMap<String, ContentTypeConfigRecord>,
    overrides: &BTreeMap<String, ContentTypeOverrideRecord>,
) -> BTreeMap<String, ContentTypeBindingPayload> {
    global
        .iter()
        .map(|(content_type, base)| {
            let override_record = overrides.get(content_type);
            (
                content_type.clone(),
                ContentTypeBindingPayload {
                    enabled: override_record
                        .and_then(|record| record.enabled)
                        .unwrap_or(base.enabled),
                    model: override_record
                        .and_then(|record| record.model.clone())
                        .unwrap_or_else(|| base.model.clone()),
                    vector_type: override_record
                        .and_then(|record| record.vector_type.clone())
                        .unwrap_or_else(|| base.vector_type.clone()),
                },
            )
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConfiguredVectorSpaceBinding {
    pub(crate) vector_space_id: String,
    selection: ModelSelectionPayload,
    pub(crate) vector_type: String,
    pub(crate) content_types: Vec<String>,
}

fn referenced_content_type_provider_ids(
    content_types: &BTreeMap<String, ContentTypeBindingPayload>,
) -> Vec<String> {
    let mut ids = content_types
        .values()
        .filter_map(|binding| {
            split_model_reference(&binding.model).map(|(provider_id, _)| provider_id.to_string())
        })
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn split_model_reference(model_ref: &str) -> Option<(&str, &str)> {
    model_ref.split_once('/')
}

fn model_selection_error(summary: &ResolvedModelSelectionPayload) -> ApiError {
    let details = Some(json!({
        "binding_source": summary.binding_source,
        "provider_id": summary.provider_id,
        "provider_kind": summary.provider_kind,
        "model_id": summary.model_id,
        "model_revision": summary.model_revision,
        "status": summary.status,
    }));

    match summary.status.as_str() {
        "not_enabled" => ApiError::not_enabled(summary.message.clone(), details),
        "not_supported" => ApiError::not_supported(summary.message.clone(), details),
        "runtime_unavailable" => ApiError::runtime_unavailable(summary.message.clone(), details),
        "conflict" => ApiError::conflict(summary.message.clone(), details),
        _ => ApiError::runtime_unavailable(summary.message.clone(), details),
    }
}

fn normalize_model_test_modality(value: &str) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if matches!(trimmed, QUERY_KIND_TEXT | QUERY_KIND_IMAGE) {
        return Ok(trimmed.to_string());
    }

    Err(ApiError::validation_failed(
        "input_modality must be one of the supported settings model input types.",
        Some(json!({
            "field": "input_modality",
            "received": trimmed,
            "supported": [
                QUERY_KIND_TEXT,
                QUERY_KIND_IMAGE,
            ],
        })),
    ))
}

fn model_test_operation_kind(input_modality: &str) -> &'static str {
    match input_modality {
        QUERY_KIND_TEXT => "query_embedding",
        QUERY_KIND_IMAGE => "image_query_embedding",
        _ => "query_embedding",
    }
}

struct ExecutedModelTestInput {
    vectors: Vec<Vec<f32>>,
    pooled_vector: Vec<f32>,
    input_summary: ModelTestInputSummary,
}

async fn run_settings_model_test_input(
    input_modality: &str,
    text_input: Option<&str>,
    file_input: Option<&StagedSettingsModelTestFile>,
    provider_context: Option<serde_json::Value>,
    modality_field: &str,
) -> Result<ExecutedModelTestInput, ApiError> {
    let text_field = if modality_field == "comparison_input_modality" {
        "comparison_text"
    } else {
        "text"
    };
    match input_modality {
        QUERY_KIND_TEXT => {
            let text = normalize_required_string(text_input.unwrap_or_default(), text_field)?;
            let result = embed_query_text(&text, provider_context).await?;
            Ok(ExecutedModelTestInput {
                vectors: result.vectors,
                pooled_vector: result.pooled_vector,
                input_summary: ModelTestInputSummary {
                    kind: "text".to_string(),
                    text_preview: Some(truncate_text_preview(&text)),
                    original_filename: None,
                    content_type: None,
                    size_bytes: Some(text.len()),
                },
            })
        }
        QUERY_KIND_IMAGE => {
            let field_name = if modality_field == "comparison_input_modality" {
                "comparison_file"
            } else {
                "file"
            };
            let file = file_input.ok_or_else(|| {
                ApiError::validation_failed(
                    "image model test requires one file input.",
                    Some(json!({ "field": field_name })),
                )
            })?;
            let result = embed_query_image(&file.path, None, provider_context).await?;
            Ok(ExecutedModelTestInput {
                vectors: result.vectors,
                pooled_vector: result.pooled_vector,
                input_summary: ModelTestInputSummary {
                    kind: "file".to_string(),
                    text_preview: None,
                    original_filename: file.original_filename.clone(),
                    content_type: Some(file.content_type.clone()),
                    size_bytes: Some(file.size_bytes),
                },
            })
        }
        _ => Err(ApiError::validation_failed(
            "input_modality is not supported by the selected model's embedding capabilities.",
            Some(json!({
                "field": modality_field,
                "received": input_modality,
            })),
        )),
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> Result<f32, ApiError> {
    if left.is_empty() || right.is_empty() || left.len() != right.len() {
        return Err(ApiError::runtime_unavailable(
            "Model test similarity could not be computed because the pooled vectors were incompatible.",
            Some(json!({
                "left_dim": left.len(),
                "right_dim": right.len(),
            })),
        ));
    }

    let mut dot = 0.0_f32;
    let mut left_norm = 0.0_f32;
    let mut right_norm = 0.0_f32;
    for (left_item, right_item) in left.iter().zip(right.iter()) {
        dot += left_item * right_item;
        left_norm += left_item * left_item;
        right_norm += right_item * right_item;
    }

    if left_norm <= 0.0 || right_norm <= 0.0 {
        return Err(ApiError::runtime_unavailable(
            "Model test similarity could not be computed because one of the pooled vectors was degenerate.",
            None,
        ));
    }

    Ok(dot / (left_norm.sqrt() * right_norm.sqrt()))
}

fn vector_shape(vectors: &[Vec<f32>]) -> Vec<usize> {
    let dim = vectors.first().map(|vector| vector.len()).unwrap_or(0);
    vec![vectors.len(), dim]
}

fn truncate_text_preview(text: &str) -> String {
    const LIMIT: usize = 160;
    let trimmed = text.trim();
    if trimmed.chars().count() <= LIMIT {
        return trimmed.to_string();
    }

    let preview = trimmed.chars().take(LIMIT).collect::<String>();
    format!("{preview}...")
}
