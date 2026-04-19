use super::*;
use crate::*;
use serde_json::json;
use std::collections::BTreeMap;

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

    pub(crate) async fn update_provider_config(
        &mut self,
        provider_id: &str,
        request: UpdateProviderConfigRequest,
    ) -> Result<ProviderConfigSnapshot, ApiError> {
        let existing = self
            .provider_configs
            .get(provider_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Provider was not found."))?;

        if existing.provider_id == LOCAL_SIDECAR_PROVIDER_ID && request.base_url.is_some() {
            return Err(ApiError::not_supported(
                "local_sidecar connection details are derived from runtime env and cannot be edited here.",
                Some(json!({ "provider_id": provider_id })),
            ));
        }

        let base_url = normalize_optional_string(request.base_url).or(existing.base_url.clone());
        let enabled = request.enabled.unwrap_or(existing.enabled);

        self.commit_durable_api(|state| {
            let provider = state
                .provider_configs
                .get_mut(provider_id)
                .ok_or_else(|| ApiError::not_found("Provider was not found."))?;
            provider.enabled = enabled;
            if provider.provider_id == DASHSCOPE_PROVIDER_ID {
                provider.base_url = base_url.clone();
            }
            Ok(())
        })?;

        self.refresh_provider_probe_snapshot(provider_id).await;
        Ok(self
            .provider_configs
            .get(provider_id)
            .map(|provider| self.provider_config_snapshot(provider))
            .expect("updated provider config should be present"))
    }

    pub(crate) async fn list_model_catalog(&mut self) -> ModelCatalogData {
        if self.provider_runtime_models.get(LOCAL_SIDECAR_PROVIDER_ID).is_none() {
            self.refresh_provider_probe_snapshot(LOCAL_SIDECAR_PROVIDER_ID)
                .await;
        }

        let local_provider = self
            .provider_configs
            .get(LOCAL_SIDECAR_PROVIDER_ID)
            .cloned()
            .unwrap_or_else(|| {
                default_provider_configs()
                    .get(LOCAL_SIDECAR_PROVIDER_ID)
                    .cloned()
                    .expect("default local_sidecar provider should exist")
            });
        let runtime = self
            .provider_runtime_models
            .get(LOCAL_SIDECAR_PROVIDER_ID)
            .cloned()
            .unwrap_or_else(fallback_local_sidecar_runtime_model);
        if self.provider_probe_cache.get(DASHSCOPE_PROVIDER_ID).is_none() {
            self.refresh_provider_probe_snapshot(DASHSCOPE_PROVIDER_ID).await;
        }
        let local_probe = self.provider_probe_cache.get(LOCAL_SIDECAR_PROVIDER_ID).cloned();
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
        let dashscope_probe = self.provider_probe_cache.get(DASHSCOPE_PROVIDER_ID).cloned();

        let mut entries = vec![ModelCatalogEntry {
            provider_id: LOCAL_SIDECAR_PROVIDER_ID.to_string(),
            provider_kind: local_provider.provider_kind,
            model_id: runtime.model_id,
            model_revision: runtime.model_revision,
            supported_index_lines: vec![MULTIVECTOR_INDEX_LINE.to_string()],
            embedding_capabilities: self
                .provider_embedding_capabilities
                .get(LOCAL_SIDECAR_PROVIDER_ID)
                .cloned()
                .unwrap_or_else(local_sidecar_embedding_capabilities),
            editable: false,
            status: local_probe
                .as_ref()
                .map(|item| item.status.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            message: local_probe
                .as_ref()
                .map(|item| item.message.clone())
                .unwrap_or_else(|| "Model metadata is derived from the local runtime.".to_string()),
        }];

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

        for model_id in dashscope_multivector_model_ids() {
            entries.push(ModelCatalogEntry {
                provider_id: DASHSCOPE_PROVIDER_ID.to_string(),
                provider_kind: dashscope_provider.provider_kind.clone(),
                model_id: (*model_id).to_string(),
                model_revision: None,
                supported_index_lines: vec![MULTIVECTOR_INDEX_LINE.to_string()],
                embedding_capabilities: dashscope_embedding_capabilities(model_id),
                editable: true,
                status: dashscope_status.clone(),
                message: dashscope_message.clone(),
            });
        }

        ModelCatalogData { entries }
    }

    pub(crate) fn get_global_model_defaults(&self) -> GlobalModelDefaultsData {
        GlobalModelDefaultsData {
            defaults: self.global_model_defaults.clone(),
        }
    }

    pub(crate) async fn update_global_model_defaults(
        &mut self,
        payload: ModelDefaultsPayload,
    ) -> Result<GlobalModelDefaultsData, ApiError> {
        let normalized = normalize_model_defaults(payload)?;
        self.validate_model_defaults(&normalized)?;

        self.commit_durable_api(|state| {
            state.global_model_defaults = normalized.clone();
            Ok(())
        })?;

        for provider_id in referenced_global_provider_ids(&self.global_model_defaults) {
            self.refresh_provider_probe_snapshot(&provider_id).await;
        }

        Ok(self.get_global_model_defaults())
    }

    pub(crate) fn get_library_model_overrides(
        &self,
        library_id: &str,
    ) -> Result<LibraryModelOverridesData, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;
        Ok(LibraryModelOverridesData {
            overrides: effective_library_model_overrides_payload(&library.model_overrides),
        })
    }

    pub(crate) async fn update_library_model_overrides(
        &mut self,
        library_id: &str,
        payload: ModelOverridesPayload,
    ) -> Result<LibraryModelOverridesData, ApiError> {
        let normalized = normalize_model_overrides(payload)?;
        self.validate_library_exists(library_id)?;
        self.validate_model_overrides(&normalized)?;

        self.commit_durable_api(|state| {
            let library = state
                .libraries
                .get_mut(library_id)
                .ok_or_else(|| ApiError::not_found("Library was not found."))?;
            library.model_overrides = normalized.clone();
            Ok(())
        })?;

        for provider_id in referenced_library_provider_ids(&normalized) {
            self.refresh_provider_probe_snapshot(&provider_id).await;
        }

        self.get_library_model_overrides(library_id)
    }

    pub(crate) async fn get_resolved_models(
        &mut self,
        library_id: &str,
    ) -> Result<ResolvedModelsData, ApiError> {
        self.validate_library_exists(library_id)?;
        let mut index_lines = BTreeMap::new();
        for index_line in supported_index_lines() {
            index_lines.insert(
                index_line.to_string(),
                self.inspect_index_model_selection(library_id, index_line).await?,
            );
        }
        Ok(ResolvedModelsData { index_lines })
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
    ) -> Result<ModelTestData, ApiError> {
        let provider_id = normalize_provider_id(provider_id, "provider")?;
        let model_id = normalize_required_string(model_id, "model_id")?;
        let input_modality = normalize_model_test_modality(input_modality)?;
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
                if self.provider_probe_cache.get(LOCAL_SIDECAR_PROVIDER_ID).is_none() {
                    self.refresh_provider_probe_snapshot(LOCAL_SIDECAR_PROVIDER_ID)
                        .await;
                }
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
                )
                {
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
                let resolved_model = ResolvedModelSelectionPayload {
                    binding_source: "settings_draft".to_string(),
                    provider_id: provider.provider_id.clone(),
                    provider_kind: provider.provider_kind.clone(),
                    model_id: runtime_model.model_id,
                    model_revision: runtime_model.model_revision,
                    embedding_capabilities,
                    status: "available".to_string(),
                    message: format!(
                        "Validated settings draft via {}.",
                        model_test_operation_kind(&input_modality)
                    ),
                    last_probed_at: probe.last_probed_at.clone(),
                };
                let provider_context = Some(provider_context_payload(&ResolvedExecutionModelSelection {
                    summary: resolved_model.clone(),
                }));

                let (vectors, pooled_vector, input_summary) = match input_modality.as_str() {
                    QUERY_KIND_TEXT => {
                        let text = normalize_required_string(
                            text_input.unwrap_or_default(),
                            "text",
                        )?;
                        let result = embed_query_text(&text, provider_context).await?;
                        (
                            result.vectors,
                            result.pooled_vector,
                            ModelTestInputSummary {
                                kind: "text".to_string(),
                                text_preview: Some(truncate_text_preview(&text)),
                                original_filename: None,
                                content_type: None,
                                size_bytes: Some(text.len()),
                            },
                        )
                    }
                    QUERY_KIND_IMAGE => {
                        let file = file_input.ok_or_else(|| {
                            ApiError::validation_failed(
                                "image model test requires one file input.",
                                Some(json!({ "field": "file" })),
                            )
                        })?;
                        let result =
                            embed_query_image(&file.path, None, provider_context).await?;
                        (
                            result.vectors,
                            result.pooled_vector,
                            ModelTestInputSummary {
                                kind: "file".to_string(),
                                text_preview: None,
                                original_filename: file.original_filename.clone(),
                                content_type: Some(file.content_type.clone()),
                                size_bytes: Some(file.size_bytes),
                            },
                        )
                    }
                    _ => {
                        return Err(ApiError::validation_failed(
                            "input_modality is not supported by the selected model's embedding capabilities.",
                            Some(json!({
                                "field": "input_modality",
                                "received": input_modality,
                            })),
                        ));
                    }
                };

                Ok(ModelTestData {
                    resolved_model,
                    input_modality: input_modality.clone(),
                    operation_kind: model_test_operation_kind(&input_modality).to_string(),
                    vector_shape: vector_shape(&vectors),
                    vectors,
                    pooled_vector,
                    input_summary,
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

    pub(crate) async fn resolve_index_model_for_execution(
        &mut self,
        library_id: &str,
        index_line: &str,
    ) -> Result<ResolvedExecutionModelSelection, ApiError> {
        let summary = self.inspect_index_model_selection(library_id, index_line).await?;
        if summary.status != "available" {
            return Err(model_selection_error(&summary));
        }
        Ok(ResolvedExecutionModelSelection { summary })
    }

    pub(crate) async fn resolve_query_model_for_execution(
        &mut self,
        library_id: &str,
    ) -> Result<ResolvedExecutionModelSelection, ApiError> {
        self.resolve_index_model_for_execution(library_id, MULTIVECTOR_INDEX_LINE)
            .await
    }

    pub(crate) async fn refresh_boot_provider_probe_cache(&mut self) {
        let provider_ids = self.provider_configs.keys().cloned().collect::<Vec<_>>();
        for provider_id in provider_ids {
            self.refresh_provider_probe_snapshot(&provider_id).await;
        }
    }

    pub(crate) async fn refresh_provider_probe_snapshot(
        &mut self,
        provider_id: &str,
    ) -> Option<ProviderProbeSnapshot> {
        let provider = self.provider_configs.get(provider_id)?.clone();
        let (probe, runtime_model) = match provider.provider_id.as_str() {
            LOCAL_SIDECAR_PROVIDER_ID => {
                let snapshot = probe_local_sidecar_provider(&provider).await;
                self.provider_embedding_capabilities.insert(
                    provider_id.to_string(),
                    snapshot.embedding_capabilities.clone(),
                );
                (snapshot.probe, Some(snapshot.runtime_model))
            }
            DASHSCOPE_PROVIDER_ID => {
                self.provider_embedding_capabilities.insert(
                    provider_id.to_string(),
                    empty_embedding_capabilities(),
                );
                (
                    static_not_supported_probe(
                    "dashscope is configurable in the current slice but not executable yet.",
                    ),
                    None,
                )
            }
            _ => {
                self.provider_embedding_capabilities.insert(
                    provider_id.to_string(),
                    empty_embedding_capabilities(),
                );
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
        if let Some(runtime_model) = runtime_model {
            self.provider_runtime_models
                .insert(provider_id.to_string(), runtime_model);
        }
        Some(probe)
    }

    fn provider_config_snapshot(&self, provider: &ProviderConfigRecord) -> ProviderConfigSnapshot {
        let mut snapshot = provider.snapshot();
        if provider.provider_id == LOCAL_SIDECAR_PROVIDER_ID {
            snapshot.base_url = sidecar_base_url().ok();
        }
        snapshot.probe = self.provider_probe_cache.get(&provider.provider_id).cloned();
        snapshot
    }

    fn validate_library_exists(&self, library_id: &str) -> Result<(), ApiError> {
        if !self.libraries.contains_key(library_id) {
            return Err(ApiError::not_found("Library was not found."));
        }
        Ok(())
    }

    fn validate_model_defaults(
        &self,
        defaults: &ModelDefaultsPayload,
    ) -> Result<(), ApiError> {
        let selection = defaults.index_lines.get(MULTIVECTOR_INDEX_LINE).ok_or_else(|| {
            ApiError::validation_failed(
                "model defaults must include index_lines.multivector.",
                Some(json!({ "field": "index_lines.multivector" })),
            )
        })?;
        self.validate_provider_selection(selection)?;
        Ok(())
    }

    fn validate_model_overrides(
        &self,
        overrides: &ModelOverridesPayload,
    ) -> Result<(), ApiError> {
        if let Some(selection) = overrides.index_lines.get(MULTIVECTOR_INDEX_LINE) {
            let defaults = self
                .global_model_defaults
                .index_lines
                .get(MULTIVECTOR_INDEX_LINE)
                .cloned()
                .unwrap_or_else(default_local_sidecar_model_selection);
            let effective = merge_model_selection_override(&defaults, Some(selection.clone()));
            self.validate_provider_selection(&effective)?;
        }
        Ok(())
    }

    fn validate_provider_selection(
        &self,
        selection: &ModelSelectionPayload,
    ) -> Result<(), ApiError> {
        validate_provider_selection_shape(
            &selection.provider_id,
            &selection.model_id,
            "index_lines.multivector",
        )?;
        let provider = self.validate_provider_reference(&selection.provider_id)?;
        self.validate_provider_model_binding_for_field(
            provider,
            selection,
            "index_lines.multivector.model_id",
        )?;
        Ok(())
    }

    fn validate_provider_reference(&self, provider_id: &str) -> Result<&ProviderConfigRecord, ApiError> {
        let provider = self.provider_configs.get(provider_id).ok_or_else(|| {
            ApiError::conflict(
                "Model selection references a missing provider.",
                Some(json!({ "provider_id": provider_id })),
            )
        })?;
        if !provider.enabled {
            return Err(ApiError::not_enabled(
                "Model selection references a disabled provider.",
                Some(json!({ "provider_id": provider_id })),
            ));
        }
        Ok(provider)
    }

    fn validate_provider_model_binding_for_field(
        &self,
        provider: &ProviderConfigRecord,
        selection: &ModelSelectionPayload,
        field: &str,
    ) -> Result<(), ApiError> {
        match provider.provider_id.as_str() {
            LOCAL_SIDECAR_PROVIDER_ID => {
                let runtime_model = self
                    .provider_runtime_models
                    .get(LOCAL_SIDECAR_PROVIDER_ID)
                    .cloned()
                    .unwrap_or_else(fallback_local_sidecar_runtime_model);
                if selection.model_id != runtime_model.model_id {
                    return Err(ApiError::validation_failed(
                        "local_sidecar model_id is derived from the active runtime and cannot be changed here.",
                        Some(json!({
                            "field": field,
                            "expected": runtime_model.model_id,
                            "received": selection.model_id,
                        })),
                    ));
                }
                Ok(())
            }
            DASHSCOPE_PROVIDER_ID => {
                if !is_supported_dashscope_multivector_model(&selection.model_id) {
                    return Err(ApiError::validation_failed(
                        "DashScope model_id is not supported for multivector indexing/search.",
                        Some(json!({
                            "field": field,
                            "supported": dashscope_multivector_model_ids(),
                            "received": selection.model_id,
                        })),
                    ));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    async fn inspect_index_model_selection(
        &mut self,
        library_id: &str,
        index_line: &str,
    ) -> Result<ResolvedModelSelectionPayload, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;
        let global_selection = self
            .global_model_defaults
            .index_lines
            .get(index_line)
            .cloned()
            .ok_or_else(|| {
                ApiError::conflict(
                    "Global model defaults are missing the requested index line.",
                    Some(json!({ "index_line": index_line })),
                )
            })?;
        let override_selection = library.model_overrides.index_lines.get(index_line).cloned();

        let binding_source = if has_model_override(&override_selection) {
            "library_override"
        } else {
            "global_default"
        };
        let effective_selection = merge_model_selection_override(&global_selection, override_selection);
        Ok(self
            .build_resolved_model_selection(binding_source, effective_selection, index_line)
            .await)
    }

    async fn build_resolved_model_selection(
        &mut self,
        binding_source: &str,
        selection: ModelSelectionPayload,
        index_line: &str,
    ) -> ResolvedModelSelectionPayload {
        let Some(provider) = self.provider_configs.get(&selection.provider_id).cloned() else {
            return ResolvedModelSelectionPayload {
                binding_source: binding_source.to_string(),
                provider_id: selection.provider_id,
                provider_kind: "missing".to_string(),
                model_id: selection.model_id,
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
                if self.provider_probe_cache.get(&provider.provider_id).is_none() {
                    self.refresh_provider_probe_snapshot(&provider.provider_id).await;
                }
                let runtime_model = self
                    .provider_runtime_models
                    .get(&provider.provider_id)
                    .cloned()
                    .unwrap_or_else(fallback_local_sidecar_runtime_model);
                let probe = self.provider_probe_cache.get(&provider.provider_id).cloned();
                if let Some(probe) = &probe {
                    if probe.status != "available" {
                        return ResolvedModelSelectionPayload {
                            binding_source: binding_source.to_string(),
                            provider_id: provider.provider_id.clone(),
                            provider_kind: provider.provider_kind.clone(),
                            model_id: runtime_model.model_id,
                            model_revision: runtime_model.model_revision,
                            embedding_capabilities: self
                                .provider_embedding_capabilities
                                .get(&provider.provider_id)
                                .cloned()
                                .unwrap_or_else(local_sidecar_embedding_capabilities),
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
                    model_id: runtime_model.model_id,
                    model_revision: runtime_model.model_revision,
                    embedding_capabilities: self
                        .provider_embedding_capabilities
                        .get(&provider.provider_id)
                        .cloned()
                        .unwrap_or_else(local_sidecar_embedding_capabilities),
                    status: "available".to_string(),
                    message: format!(
                        "Resolved runtime-bound model for index_line={index_line} via local_sidecar."
                    ),
                    last_probed_at: probe.and_then(|item| item.last_probed_at),
                }
            }
            DASHSCOPE_PROVIDER_ID => ResolvedModelSelectionPayload {
                binding_source: binding_source.to_string(),
                provider_id: provider.provider_id.clone(),
                provider_kind: provider.provider_kind.clone(),
                model_id: selection.model_id.clone(),
                model_revision: None,
                embedding_capabilities: dashscope_embedding_capabilities(&selection.model_id),
                status: "not_supported".to_string(),
                message:
                    "dashscope is configurable but not executable in the current 005 slice."
                        .to_string(),
                last_probed_at: self
                    .provider_probe_cache
                    .get(&provider.provider_id)
                    .and_then(|probe| probe.last_probed_at.clone()),
            },
            _ => ResolvedModelSelectionPayload {
                binding_source: binding_source.to_string(),
                provider_id: provider.provider_id.clone(),
                provider_kind: provider.provider_kind.clone(),
                model_id: selection.model_id,
                model_revision: None,
                embedding_capabilities: empty_embedding_capabilities(),
                status: "conflict".to_string(),
                message: "Unknown provider kind.".to_string(),
                last_probed_at: None,
            },
        }
    }
}

fn has_model_override(selection: &Option<ModelSelectionOverridePayload>) -> bool {
    selection
        .as_ref()
        .map(|selection| selection.provider_id.is_some() || selection.model_id.is_some())
        .unwrap_or(false)
}

fn merge_model_selection_override(
    defaults: &ModelSelectionPayload,
    override_selection: Option<ModelSelectionOverridePayload>,
) -> ModelSelectionPayload {
    let Some(override_selection) = override_selection else {
        return defaults.clone();
    };

    ModelSelectionPayload {
        provider_id: override_selection
            .provider_id
            .unwrap_or_else(|| defaults.provider_id.clone()),
        model_id: override_selection
            .model_id
            .unwrap_or_else(|| defaults.model_id.clone()),
    }
}

fn referenced_global_provider_ids(defaults: &ModelDefaultsPayload) -> Vec<String> {
    let mut ids = defaults
        .index_lines
        .values()
        .map(|selection| selection.provider_id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn referenced_library_provider_ids(overrides: &ModelOverridesPayload) -> Vec<String> {
    let mut ids = overrides
        .index_lines
        .values()
        .filter_map(|selection| selection.provider_id.clone())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
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
