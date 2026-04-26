use super::*;
use crate::*;
use serde_json::Value;
use std::path::Path as FsPath;

impl AppState {
    pub(crate) fn get_visual_unit(
        &self,
        library_id: &str,
        visual_unit_id: &str,
    ) -> Result<VisualUnitDetailData, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        let visual_unit = library
            .visual_units
            .get(visual_unit_id)
            .ok_or_else(|| ApiError::not_found("Visual unit was not found."))?;

        Ok(VisualUnitDetailData {
            visual_unit: visual_unit.snapshot(),
            preview: visual_unit_preview_reference(
                library_id,
                &visual_unit.id,
                &visual_unit.kind,
                &visual_unit.locator,
            )?,
            neighbor_context: visual_unit.neighbor_context.clone(),
        })
    }

    pub(crate) async fn prepare_text_search(
        &mut self,
        request: &TextSearchRequest,
    ) -> Result<SearchPlan, ApiError> {
        if request.text.trim().is_empty() {
            return Err(ApiError::validation_failed(
                "Search text must not be empty.",
                Some(json!({ "field": "text" })),
            ));
        }
        let search_scope = effective_search_scope_request(
            request.search_scope.as_ref(),
            request.library_id.as_deref(),
        )?;
        self.prepare_search_scope(
            &search_scope,
            request.filters.as_ref(),
            request.top_k,
            request.cursor.as_deref(),
            request.debug,
            request.target_content_types.as_ref(),
            "text",
        )
        .await
    }

    pub(crate) async fn prepare_image_search(
        &mut self,
        request: &ImageSearchRequest,
    ) -> Result<(SearchPlan, ResolvedImageQueryInput), ApiError> {
        let search_scope = effective_search_scope_request(
            request.search_scope.as_ref(),
            request.library_id.as_deref(),
        )?;
        let plan = self
            .prepare_search_scope(
                &search_scope,
                request.filters.as_ref(),
                request.top_k,
                request.cursor.as_deref(),
                request.debug,
                request.target_content_types.as_ref(),
                "image",
            )
            .await?;
        let plan_library_id = (!plan.library_id.trim().is_empty())
            .then(|| plan.library_id.clone())
            .ok_or_else(|| {
            ApiError::not_supported(
                "Current 110-image-search implementation only supports single-library search_scope.",
                Some(json!({
                    "field": "search_scope.kind",
                    "supported": ["library"],
                    "received": search_scope.kind,
                })),
            )
        })?;

        match request.image_input.kind.as_str() {
            "temp_asset" => {
                let temp_asset_id =
                    request
                        .image_input
                        .temp_asset_id
                        .as_deref()
                        .ok_or_else(|| {
                            ApiError::validation_failed(
                                "image_input.kind=temp_asset requires temp_asset_id.",
                                Some(json!({ "field": "image_input.temp_asset_id" })),
                            )
                        })?;
                let asset = self.get_temp_query_asset(&plan_library_id, temp_asset_id)?;
                Ok((plan, ResolvedImageQueryInput::TempAsset(asset)))
            }
            "library_object" => {
                let visual_unit_id =
                    request
                        .image_input
                        .visual_unit_id
                        .as_deref()
                        .ok_or_else(|| {
                            ApiError::validation_failed(
                                "image_input.kind=library_object requires visual_unit_id.",
                                Some(json!({ "field": "image_input.visual_unit_id" })),
                            )
                        })?;
                let visual_unit = self.get_library_visual_unit(&plan_library_id, visual_unit_id)?;
                if !matches!(visual_unit.kind.as_str(), "image" | "document_page") {
                    return Err(ApiError::not_supported(
                        "Current 110-image-search implementation only supports library image and document_page objects as query images.",
                        Some(json!({
                            "field": "image_input.visual_unit_id",
                            "received_kind": visual_unit.kind,
                            "supported_kinds": ["image", "document_page"],
                        })),
                    ));
                }
                Ok((
                    plan,
                    ResolvedImageQueryInput::LibraryVisualUnit(visual_unit),
                ))
            }
            _ => Err(ApiError::validation_failed(
                "image_input.kind must be one of the supported query image input kinds.",
                Some(json!({
                    "field": "image_input.kind",
                    "received": request.image_input.kind,
                    "supported": ["temp_asset", "library_object"],
                })),
            )),
        }
    }

    pub(crate) async fn prepare_video_search(
        &mut self,
        request: &VideoSearchRequest,
    ) -> Result<(SearchPlan, ResolvedVideoQueryInput), ApiError> {
        let search_scope = effective_search_scope_request(
            request.search_scope.as_ref(),
            request.library_id.as_deref(),
        )?;
        let plan = self
            .prepare_search_scope(
                &search_scope,
                request.filters.as_ref(),
                request.top_k,
                request.cursor.as_deref(),
                request.debug,
                request.target_content_types.as_ref(),
                "video",
            )
            .await?;
        let plan_library_id = (!plan.library_id.trim().is_empty())
            .then(|| plan.library_id.clone())
            .ok_or_else(|| {
            ApiError::not_supported(
                "Current 120-video-search implementation only supports single-library search_scope.",
                Some(json!({
                    "field": "search_scope.kind",
                    "supported": ["library"],
                    "received": search_scope.kind,
                })),
            )
        })?;

        match request.video_input.kind.as_str() {
            "temp_asset" => {
                let temp_asset_id =
                    request
                        .video_input
                        .temp_asset_id
                        .as_deref()
                        .ok_or_else(|| {
                            ApiError::validation_failed(
                                "video_input.kind=temp_asset requires temp_asset_id.",
                                Some(json!({ "field": "video_input.temp_asset_id" })),
                            )
                        })?;
                let asset = self.get_temp_query_video_asset(&plan_library_id, temp_asset_id)?;
                let locator = resolve_video_query_locator(
                    request.video_input.locator.as_ref(),
                    asset.duration_ms,
                    "video_input.locator",
                )?;
                Ok((
                    plan,
                    ResolvedVideoQueryInput {
                        path: asset.path,
                        locator,
                    },
                ))
            }
            "library_object" => {
                if let Some(visual_unit_id) = request.video_input.visual_unit_id.as_deref() {
                    if request.video_input.locator.is_some() {
                        return Err(ApiError::validation_failed(
                            "video_input.visual_unit_id reuses the segment's own locator and must not carry video_input.locator.",
                            Some(json!({
                                "field": "video_input.locator",
                                "input_kind": "library_object",
                                "library_object_kind": "video_segment",
                            })),
                        ));
                    }

                    let visual_unit =
                        self.get_library_visual_unit(&plan_library_id, visual_unit_id)?;
                    if visual_unit.kind != "video_segment" || visual_unit.source_type != "video" {
                        return Err(ApiError::not_supported(
                            "Current 120-video-search implementation only supports library video_segment objects as direct query video segments.",
                            Some(json!({
                                "field": "video_input.visual_unit_id",
                                "received_kind": visual_unit.kind,
                                "received_source_type": visual_unit.source_type,
                                "supported_kind": "video_segment",
                                "supported_source_type": "video",
                            })),
                        ));
                    }

                    return Ok((
                        plan,
                        ResolvedVideoQueryInput {
                            path: visual_unit.source_path,
                            locator: Some(visual_unit.locator),
                        },
                    ));
                }

                let source_id = request.video_input.source_id.as_deref().ok_or_else(|| {
                    ApiError::validation_failed(
                        "video_input.kind=library_object requires source_id or visual_unit_id.",
                        Some(
                            json!({ "field": "video_input", "supported_fields": ["source_id", "visual_unit_id"] }),
                        ),
                    )
                })?;
                let source = self.get_library_source(&plan_library_id, source_id)?;
                if source.source_type != "video" {
                    return Err(ApiError::not_supported(
                        "Current 120-video-search implementation only supports library video sources as query videos.",
                        Some(json!({
                            "field": "video_input.source_id",
                            "received_source_type": source.source_type,
                            "supported_source_type": "video",
                        })),
                    ));
                }
                let locator = resolve_video_query_locator(
                    request.video_input.locator.as_ref(),
                    source.duration_ms,
                    "video_input.locator",
                )?;
                Ok((
                    plan,
                    ResolvedVideoQueryInput {
                        path: source.source_path,
                        locator,
                    },
                ))
            }
            _ => Err(ApiError::validation_failed(
                "video_input.kind must be one of the supported query video input kinds.",
                Some(json!({
                    "field": "video_input.kind",
                    "received": request.video_input.kind,
                    "supported": ["temp_asset", "library_object"],
                })),
            )),
        }
    }

    pub(crate) async fn prepare_document_search(
        &mut self,
        request: &DocumentSearchRequest,
    ) -> Result<(SearchPlan, ResolvedDocumentQueryInput), ApiError> {
        let search_scope = effective_search_scope_request(
            request.search_scope.as_ref(),
            request.library_id.as_deref(),
        )?;
        let plan = self
            .prepare_search_scope(
                &search_scope,
                request.filters.as_ref(),
                request.top_k,
                request.cursor.as_deref(),
                request.debug,
                request.target_content_types.as_ref(),
                "document",
            )
            .await?;
        let plan_library_id = (!plan.library_id.trim().is_empty())
            .then(|| plan.library_id.clone())
            .ok_or_else(|| {
            ApiError::not_supported(
                "Current 130-document-search implementation only supports single-library search_scope.",
                Some(json!({
                    "field": "search_scope.kind",
                    "supported": ["library"],
                    "received": search_scope.kind,
                })),
            )
        })?;

        match request.document_input.kind.as_str() {
            "temp_asset" => {
                let temp_asset_id =
                    request
                        .document_input
                        .temp_asset_id
                        .as_deref()
                        .ok_or_else(|| {
                            ApiError::validation_failed(
                                "document_input.kind=temp_asset requires temp_asset_id.",
                                Some(json!({ "field": "document_input.temp_asset_id" })),
                            )
                        })?;
                let asset = self.get_temp_query_document_asset(&plan_library_id, temp_asset_id)?;
                let locator = resolve_document_query_locator(
                    request.document_input.locator.as_ref(),
                    asset.page_count,
                    "document_input.locator",
                )?;
                Ok((
                    plan,
                    ResolvedDocumentQueryInput {
                        path: asset.path,
                        locator,
                    },
                ))
            }
            "library_object" => {
                let source_id = request.document_input.source_id.as_deref().ok_or_else(|| {
                    ApiError::validation_failed(
                        "document_input.kind=library_object requires source_id.",
                        Some(json!({ "field": "document_input.source_id" })),
                    )
                })?;
                let source = self.get_library_source(&plan_library_id, source_id)?;
                if source.source_type != "pdf" {
                    return Err(ApiError::not_supported(
                        "Current 130-document-search implementation only supports library PDF sources as query documents.",
                        Some(json!({
                            "field": "document_input.source_id",
                            "received_source_type": source.source_type,
                            "supported_source_type": "pdf",
                        })),
                    ));
                }
                let locator = resolve_document_query_locator(
                    request.document_input.locator.as_ref(),
                    source.page_count,
                    "document_input.locator",
                )?;
                Ok((
                    plan,
                    ResolvedDocumentQueryInput {
                        path: source.source_path,
                        locator,
                    },
                ))
            }
            _ => Err(ApiError::validation_failed(
                "document_input.kind must be one of the supported query document input kinds.",
                Some(json!({
                    "field": "document_input.kind",
                    "received": request.document_input.kind,
                    "supported": ["temp_asset", "library_object"],
                })),
            )),
        }
    }

    pub(crate) async fn prepare_search_scope(
        &mut self,
        search_scope: &SearchScopeRequest,
        filters: Option<&Value>,
        top_k: Option<usize>,
        cursor: Option<&str>,
        debug: Option<bool>,
        target_content_types: Option<&Vec<String>>,
        query_input_type: &str,
    ) -> Result<SearchPlan, ApiError> {
        match search_scope.kind.trim() {
            "library" => {
                let library_id = search_scope
                    .library_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        ApiError::validation_failed(
                            "search_scope.kind=library requires library_id.",
                            Some(json!({ "field": "search_scope.library_id" })),
                        )
                    })?;
                self.prepare_single_library_search_scope(
                    library_id,
                    filters,
                    top_k,
                    cursor,
                    debug,
                    target_content_types,
                    query_input_type,
                )
                .await
            }
            "all_libraries" => {
                if query_input_type != "text" {
                    return Err(ApiError::not_supported(
                        format!(
                            "Current {} search implementation only supports single-library search_scope.",
                            match query_input_type {
                                "image" => "110-image-search",
                                "video" => "120-video-search",
                                "document" => "130-document-search",
                                _ => "search",
                            }
                        ),
                        Some(json!({
                            "field": "search_scope.kind",
                            "supported": ["library"],
                            "received": "all_libraries",
                            "query_input_type": query_input_type,
                        })),
                    ));
                }

                self.prepare_all_libraries_text_search_scope(
                    filters,
                    top_k,
                    cursor,
                    debug,
                    target_content_types,
                    query_input_type,
                )
                .await
            }
            "library_set" => Err(ApiError::not_supported(
                "Current search implementation does not yet support explicit multi-library subsets.",
                Some(json!({
                    "field": "search_scope.kind",
                    "received": "library_set",
                    "supported": ["library", "all_libraries"],
                })),
            )),
            "" => Err(ApiError::validation_failed(
                "search_scope.kind must not be empty.",
                Some(json!({ "field": "search_scope.kind" })),
            )),
            other => Err(ApiError::validation_failed(
                "search_scope.kind must be one of the supported search scope kinds.",
                Some(json!({
                    "field": "search_scope.kind",
                    "received": other,
                    "supported": ["library", "all_libraries", "library_set"],
                })),
            )),
        }
    }

    async fn prepare_all_libraries_text_search_scope(
        &mut self,
        filters: Option<&Value>,
        top_k: Option<usize>,
        cursor: Option<&str>,
        debug: Option<bool>,
        target_content_types: Option<&Vec<String>>,
        query_input_type: &str,
    ) -> Result<SearchPlan, ApiError> {
        let library_ids = self.libraries.keys().cloned().collect::<Vec<_>>();
        if library_ids.is_empty() {
            return Err(ApiError::not_found("Library was not found."));
        }

        let mut aggregate_plan: Option<SearchPlan> = None;
        let mut first_not_enabled = None;
        let mut first_not_ready = None;
        let mut first_runtime_unavailable = None;
        let mut first_not_supported = None;

        for library_id in library_ids {
            match self
                .prepare_single_library_search_scope(
                    &library_id,
                    filters,
                    top_k,
                    cursor,
                    debug,
                    target_content_types,
                    query_input_type,
                )
                .await
            {
                Ok(library_plan) => {
                    if let Some(plan) = &mut aggregate_plan {
                        merge_search_plan(plan, library_plan);
                    } else {
                        aggregate_plan = Some(SearchPlan {
                            search_scope_kind: "all_libraries".to_string(),
                            library_id: String::new(),
                            ..library_plan
                        });
                    }
                }
                Err(error) => match error.payload.code.as_str() {
                    "not_enabled" => {
                        if first_not_enabled.is_none() {
                            first_not_enabled = Some(error);
                        }
                    }
                    "not_ready" => {
                        if first_not_ready.is_none() {
                            first_not_ready = Some(error);
                        }
                    }
                    "runtime_unavailable" => {
                        if first_runtime_unavailable.is_none() {
                            first_runtime_unavailable = Some(error);
                        }
                    }
                    "not_supported" => {
                        if first_not_supported.is_none() {
                            first_not_supported = Some(error);
                        }
                    }
                    _ => return Err(error),
                },
            }
        }

        if let Some(plan) = aggregate_plan {
            return Ok(plan);
        }

        if let Some(error) = first_not_enabled {
            return Err(error);
        }
        if let Some(error) = first_not_ready {
            return Err(error);
        }
        if let Some(error) = first_runtime_unavailable {
            return Err(error);
        }
        if let Some(error) = first_not_supported {
            return Err(error);
        }

        Err(ApiError::not_ready(
            "No library in the current search scope is ready for text search.",
            Some(json!({ "search_scope": "all_libraries" })),
        ))
    }

    async fn prepare_single_library_search_scope(
        &mut self,
        library_id: &str,
        filters: Option<&Value>,
        top_k: Option<usize>,
        cursor: Option<&str>,
        debug: Option<bool>,
        target_content_types: Option<&Vec<String>>,
        query_input_type: &str,
    ) -> Result<SearchPlan, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;
        let resolved_library_id = library.id.clone();
        let active_vector_spaces = library.active_vector_spaces.clone();
        let latest_job_id = library.latest_job_id.clone();
        let effective_content_types = self.get_library_content_types(&resolved_library_id)?;
        let enabled_content_types = effective_content_types
            .content_types
            .content_types
            .iter()
            .filter(|(_, binding)| binding.enabled)
            .map(|(content_type, _)| content_type.clone())
            .collect::<Vec<_>>();

        let target_content_types = target_content_types
            .cloned()
            .map(normalize_content_type_targets)
            .filter(|content_types| !content_types.is_empty())
            .unwrap_or_else(|| enabled_content_types.clone());

        let enabled_types: BTreeSet<_> = enabled_content_types.iter().cloned().collect();
        let invalid_target_content_types: Vec<_> = target_content_types
            .iter()
            .filter(|content_type| !enabled_types.contains(*content_type))
            .cloned()
            .collect();

        if !invalid_target_content_types.is_empty() {
            return Err(ApiError::not_enabled(
                "Requested content types are not enabled for the selected library.",
                Some(json!({ "target_content_types": invalid_target_content_types })),
            ));
        }

        let resolved_content_models = self
            .get_resolved_content_models(&resolved_library_id)
            .await?
            .content_types;
        let not_ready_content_types = target_content_types
            .iter()
            .filter_map(|content_type| {
                let resolved = resolved_content_models.get(content_type)?;
                let vector_space_id = resolved.vector_space_id.as_ref()?;
                if active_vector_spaces.contains(vector_space_id) {
                    return None;
                }

                let job_summary = latest_job_id.as_ref().and_then(|job_id| {
                    self.jobs.get(job_id).map(|job| {
                        json!({
                            "job_id": job.snapshot.job_id,
                            "status": job.snapshot.status,
                            "phase": job.snapshot.phase,
                        })
                    })
                });

                Some(json!({
                    "content_type": content_type,
                    "status": "not_ready",
                    "job": job_summary,
                }))
            })
            .collect::<Vec<_>>();

        if !not_ready_content_types.is_empty() {
            return Err(ApiError::not_ready(
                "The requested content types are enabled but do not have an active index yet.",
                Some(json!({ "content_types": not_ready_content_types })),
            ));
        }

        if let Some(unavailable) = target_content_types
            .iter()
            .filter_map(|content_type| resolved_content_models.get(content_type))
            .find(|resolved| resolved.status != "available")
        {
            return Err(ApiError::runtime_unavailable(
                unavailable.message.clone(),
                Some(json!({
                    "content_type": unavailable.content_type,
                    "provider_id": unavailable.provider_id,
                    "model_id": unavailable.model_id,
                    "status": unavailable.status,
                })),
            ));
        }

        let unsupported_content_types = target_content_types
            .iter()
            .filter_map(|content_type| {
                let resolved = resolved_content_models.get(content_type)?;
                let execution_input_types = self
                    .provider_execution_input_types
                    .get(&resolved.provider_id)
                    .cloned()
                    .unwrap_or_default();
                if execution_input_types_support_input_type(
                    &execution_input_types,
                    query_input_type,
                ) {
                    return None;
                }

                Some(UnsupportedContentTypeSnapshot {
                    content_type: content_type.clone(),
                    model: format!("{}/{}", resolved.provider_id, resolved.model_id),
                    vector_type: resolved.vector_type.clone(),
                    reason: format!(
                        "model does not support query input type {}",
                        query_input_type
                    ),
                })
            })
            .collect::<Vec<_>>();
        let supported_content_types = target_content_types
            .iter()
            .filter(|content_type| {
                !unsupported_content_types
                    .iter()
                    .any(|item| item.content_type == **content_type)
            })
            .cloned()
            .collect::<Vec<_>>();

        if supported_content_types.is_empty() {
            return Err(ApiError::not_supported(
                "None of the requested content types can execute the current query input.",
                Some(json!({ "unsupported_content_types": unsupported_content_types })),
            ));
        }

        let active_visual_unit_refs = library
            .visual_units
            .iter()
            .filter(|(_, visual_unit)| {
                supported_content_types.iter().any(|content_type| {
                    content_type_matches_visual_unit(content_type, &visual_unit.kind)
                })
            })
            .map(|(visual_unit_id, _)| scoped_visual_unit_ref(&resolved_library_id, visual_unit_id))
            .collect::<BTreeSet<_>>();
        let resolved_content_models = resolved_content_models
            .into_iter()
            .filter(|(content_type, _)| supported_content_types.contains(content_type))
            .collect::<BTreeMap<_, _>>();
        let mut execution_groups_by_id = BTreeMap::<String, VectorSpaceExecutionGroup>::new();
        for content_type in &supported_content_types {
            let Some(resolved) = resolved_content_models.get(content_type) else {
                continue;
            };
            let Some(vector_space_id) = resolved.vector_space_id.clone() else {
                continue;
            };
            execution_groups_by_id
                .entry(vector_space_id.clone())
                .and_modify(|group| group.content_types.push(content_type.clone()))
                .or_insert_with(|| VectorSpaceExecutionGroup {
                    library_id: resolved_library_id.clone(),
                    vector_space_id: vector_space_id.clone(),
                    active_visual_unit_count: library.visual_units.len(),
                    content_types: vec![content_type.clone()],
                    resolved_model: resolved_execution_selection_from_content_model(
                        resolved,
                        self.provider_execution_input_types
                            .get(&resolved.provider_id)
                            .cloned()
                            .unwrap_or_default(),
                    ),
                });
        }

        let cursor_offset = decode_search_cursor_offset(cursor)?;
        let time_range_filter = resolve_time_range_filter(filters)?;

        Ok(SearchPlan {
            search_scope_kind: "library".to_string(),
            library_id: resolved_library_id.clone(),
            top_k: top_k.unwrap_or(10).max(1),
            cursor_offset,
            kind_filter: read_string_filter(filters, "visual_unit.kind")
                .or_else(|| read_string_filter(filters, "kind")),
            path_prefix_filter: read_string_filter(filters, "path_prefix"),
            source_type_filter: read_string_filter(filters, "source_type"),
            time_range_filter,
            target_content_types: supported_content_types,
            unsupported_content_types,
            active_visual_unit_refs,
            execution_groups: execution_groups_by_id.into_values().collect(),
            debug_content_types: resolved_content_models
                .into_iter()
                .map(
                    |(content_type, resolved_model)| SearchContentTypeDebugEntry {
                        library_id: resolved_library_id.clone(),
                        content_type,
                        resolved_model,
                    },
                )
                .collect(),
            debug: debug.unwrap_or(false),
        })
    }

    pub(crate) fn register_temp_query_asset(
        &mut self,
        library_id: &str,
        staged: StagedQueryAsset,
    ) -> Result<QueryImageAssetData, ApiError> {
        let record = self.register_temp_query_asset_record(library_id, staged)?;
        Ok(QueryImageAssetData {
            temp_asset_id: record.id.clone(),
            preview: query_image_preview_reference(library_id, &record.id)?,
            source_type: record.source_type.clone(),
            content_type: record.content_type.clone(),
            original_filename: record.original_filename.clone(),
        })
    }

    pub(crate) fn register_temp_query_video_asset(
        &mut self,
        library_id: &str,
        staged: StagedQueryAsset,
    ) -> Result<QueryVideoAssetData, ApiError> {
        let record = self.register_temp_query_asset_record(library_id, staged)?;
        Ok(QueryVideoAssetData {
            temp_asset_id: record.id.clone(),
            preview: query_video_preview_reference(library_id, &record.id)?,
            source_type: record.source_type.clone(),
            content_type: record.content_type.clone(),
            original_filename: record.original_filename.clone(),
            duration_ms: record.duration_ms,
        })
    }

    pub(crate) fn register_temp_query_document_asset(
        &mut self,
        library_id: &str,
        staged: StagedQueryAsset,
    ) -> Result<QueryDocumentAssetData, ApiError> {
        let record = self.register_temp_query_asset_record(library_id, staged)?;
        Ok(QueryDocumentAssetData {
            temp_asset_id: record.id.clone(),
            preview: query_document_preview_reference(library_id, &record.id)?,
            source_type: record.source_type.clone(),
            content_type: record.content_type.clone(),
            original_filename: record.original_filename.clone(),
            page_count: record.page_count,
        })
    }

    pub(crate) fn register_temp_query_asset_record(
        &mut self,
        library_id: &str,
        staged: StagedQueryAsset,
    ) -> Result<TempQueryAssetRecord, ApiError> {
        if !self.libraries.contains_key(library_id) {
            return Err(ApiError::not_found("Library was not found."));
        }

        self.prune_temp_query_assets();

        let temp_asset_id = self.next_temp_asset_id();
        let record = TempQueryAssetRecord {
            id: temp_asset_id.clone(),
            library_id: library_id.to_string(),
            path: staged.path,
            content_type: staged.content_type,
            source_type: staged.source_type,
            original_filename: staged.original_filename,
            page_count: staged.page_count,
            duration_ms: staged.duration_ms,
            expires_at_ms: current_unix_ms() + TEMP_QUERY_ASSET_TTL_MS,
        };

        self.temp_query_assets.insert(record.id.clone(), record);
        Ok(self.temp_query_assets[&temp_asset_id].clone())
    }

    pub(crate) fn prune_temp_query_assets(&mut self) -> TempQueryAssetPruneSummary {
        let now_ms = current_unix_ms();
        let mut expired_ids = Vec::new();
        let mut missing_ids = Vec::new();

        for (temp_asset_id, asset) in &self.temp_query_assets {
            if asset.expires_at_ms <= now_ms {
                expired_ids.push(temp_asset_id.clone());
            } else if !FsPath::new(&asset.path).exists() {
                missing_ids.push(temp_asset_id.clone());
            }
        }

        let expired_removed = expired_ids
            .into_iter()
            .filter_map(|temp_asset_id| self.temp_query_assets.remove(&temp_asset_id))
            .map(|asset| {
                remove_temp_query_asset_file(&asset.path);
                1usize
            })
            .sum();

        let missing_removed = missing_ids
            .into_iter()
            .filter_map(|temp_asset_id| self.temp_query_assets.remove(&temp_asset_id))
            .count();

        TempQueryAssetPruneSummary {
            expired_removed,
            missing_removed,
        }
    }

    pub(crate) fn get_temp_query_asset(
        &self,
        library_id: &str,
        temp_asset_id: &str,
    ) -> Result<TempQueryAssetRecord, ApiError> {
        let asset = self
            .temp_query_assets
            .get(temp_asset_id)
            .ok_or_else(|| ApiError::not_found("Query image was not found or has expired."))?;

        if asset.library_id != library_id {
            return Err(ApiError::not_found(
                "Query image was not found for the selected library.",
            ));
        }
        if asset.expires_at_ms <= current_unix_ms() {
            return Err(ApiError::not_found(
                "Query image was not found or has expired.",
            ));
        }
        if !FsPath::new(&asset.path).exists() {
            return Err(ApiError::not_found(
                "Query image file is no longer available.",
            ));
        }
        Ok(asset.clone())
    }

    pub(crate) fn get_temp_query_video_asset(
        &self,
        library_id: &str,
        temp_asset_id: &str,
    ) -> Result<TempQueryAssetRecord, ApiError> {
        let asset = self
            .temp_query_assets
            .get(temp_asset_id)
            .ok_or_else(|| ApiError::not_found("Query video was not found or has expired."))?;

        if asset.library_id != library_id {
            return Err(ApiError::not_found(
                "Query video was not found for the selected library.",
            ));
        }
        if asset.source_type != "video" {
            return Err(ApiError::not_supported(
                "Current 120-video-search implementation only accepts video temp assets as query videos.",
                Some(json!({
                    "field": "video_input.temp_asset_id",
                    "received_source_type": asset.source_type,
                    "supported_source_type": "video",
                })),
            ));
        }
        if asset.expires_at_ms <= current_unix_ms() {
            return Err(ApiError::not_found(
                "Query video was not found or has expired.",
            ));
        }
        if !FsPath::new(&asset.path).exists() {
            return Err(ApiError::not_found(
                "Query video file is no longer available.",
            ));
        }
        Ok(asset.clone())
    }

    pub(crate) fn get_temp_query_document_asset(
        &self,
        library_id: &str,
        temp_asset_id: &str,
    ) -> Result<TempQueryAssetRecord, ApiError> {
        let asset = self
            .temp_query_assets
            .get(temp_asset_id)
            .ok_or_else(|| ApiError::not_found("Query document was not found or has expired."))?;

        if asset.library_id != library_id {
            return Err(ApiError::not_found(
                "Query document was not found for the selected library.",
            ));
        }
        if asset.source_type != "pdf" {
            return Err(ApiError::not_supported(
                "Current 130-document-search implementation only accepts PDF temp assets as query documents.",
                Some(json!({
                    "field": "document_input.temp_asset_id",
                    "received_source_type": asset.source_type,
                    "supported_source_type": "pdf",
                })),
            ));
        }
        if asset.expires_at_ms <= current_unix_ms() {
            return Err(ApiError::not_found(
                "Query document was not found or has expired.",
            ));
        }
        if !FsPath::new(&asset.path).exists() {
            return Err(ApiError::not_found(
                "Query document file is no longer available.",
            ));
        }
        Ok(asset.clone())
    }

    pub(crate) fn get_library_visual_unit(
        &self,
        library_id: &str,
        visual_unit_id: &str,
    ) -> Result<VisualUnitRecord, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        library
            .visual_units
            .get(visual_unit_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Visual unit was not found."))
    }

    pub(crate) fn get_library_source(
        &self,
        library_id: &str,
        source_id: &str,
    ) -> Result<SourceRecord, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        library
            .sources
            .get(source_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Source object was not found."))
            .and_then(|source| {
                if source.status == "active" {
                    Ok(source)
                } else {
                    Err(ApiError::not_ready(
                        "Source object is no longer active for query reuse.",
                        Some(json!({
                            "source_id": source.id,
                            "status": source.status,
                            "reason": source.status_reason,
                        })),
                    ))
                }
            })
    }
}

fn decode_search_cursor_offset(cursor: Option<&str>) -> Result<usize, ApiError> {
    let Some(cursor) = cursor.map(str::trim).filter(|cursor| !cursor.is_empty()) else {
        return Ok(0);
    };

    let encoded_offset = cursor
        .strip_prefix("search:v1:")
        .ok_or_else(|| invalid_search_cursor(cursor))?;

    encoded_offset
        .parse::<usize>()
        .map_err(|_| invalid_search_cursor(cursor))
}

fn invalid_search_cursor(cursor: &str) -> ApiError {
    ApiError::validation_failed(
        "cursor is not a valid search pagination token.",
        Some(json!({
            "field": "cursor",
            "cursor": cursor,
        })),
    )
}

fn resolve_time_range_filter(
    filters: Option<&Value>,
) -> Result<Option<SearchTimeRangeFilter>, ApiError> {
    let Some(value) = filters.and_then(|filters| filters.get("time_range")) else {
        return Ok(None);
    };

    let start_ms = value
        .get("start_ms")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_time_range_filter(value))?;
    let end_ms = value
        .get("end_ms")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_time_range_filter(value))?;

    if start_ms >= end_ms {
        return Err(invalid_time_range_filter(value));
    }

    Ok(Some(SearchTimeRangeFilter { start_ms, end_ms }))
}

fn invalid_time_range_filter(value: &Value) -> ApiError {
    ApiError::validation_failed(
        "filters.time_range must provide both start_ms and end_ms, and start_ms must be smaller than end_ms.",
        Some(json!({
            "field": "filters.time_range",
            "value": value,
        })),
    )
}

fn normalize_content_type_targets(content_types: Vec<String>) -> Vec<String> {
    let mut normalized = content_types
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn content_type_matches_visual_unit(content_type: &str, visual_unit_kind: &str) -> bool {
    match content_type {
        "image" => visual_unit_kind == "image",
        "document" => visual_unit_kind == "document_page",
        "video" => visual_unit_kind == "video_segment",
        "text" => visual_unit_kind == "text",
        _ => false,
    }
}

fn resolved_execution_selection_from_content_model(
    resolved: &ResolvedContentModelSelectionPayload,
    execution_input_types: Vec<String>,
) -> ResolvedExecutionModelSelection {
    ResolvedExecutionModelSelection {
        summary: ResolvedModelSelectionPayload {
            binding_source: resolved.binding_source.clone(),
            provider_id: resolved.provider_id.clone(),
            provider_kind: resolved.provider_kind.clone(),
            model_id: resolved.model_id.clone(),
            model_version: resolved.model_version.clone(),
            model_revision: resolved.model_revision.clone(),
            embedding_capabilities: resolved.embedding_capabilities.clone(),
            status: resolved.status.clone(),
            message: resolved.message.clone(),
            last_probed_at: resolved.last_probed_at.clone(),
        },
        vector_type: resolved.vector_type.clone(),
        vector_space_id: resolved
            .vector_space_id
            .clone()
            .unwrap_or_else(|| format!("unresolved:{}", resolved.content_type)),
        execution_input_types,
    }
}

fn scoped_visual_unit_ref(library_id: &str, visual_unit_id: &str) -> String {
    format!("{library_id}:{visual_unit_id}")
}

fn effective_search_scope_request(
    search_scope: Option<&SearchScopeRequest>,
    legacy_library_id: Option<&str>,
) -> Result<SearchScopeRequest, ApiError> {
    if let Some(search_scope) = search_scope {
        return Ok(search_scope.clone());
    }

    let library_id = legacy_library_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ApiError::validation_failed(
                "search_scope must be provided, or a legacy library_id must be present during the transition.",
                Some(json!({ "field": "search_scope" })),
            )
        })?;

    Ok(SearchScopeRequest {
        kind: "library".to_string(),
        library_id: Some(library_id.to_string()),
        library_ids: None,
    })
}

fn merge_search_plan(target: &mut SearchPlan, incoming: SearchPlan) {
    target
        .active_visual_unit_refs
        .extend(incoming.active_visual_unit_refs);
    target.execution_groups.extend(incoming.execution_groups);
    target
        .debug_content_types
        .extend(incoming.debug_content_types);

    for content_type in incoming.target_content_types {
        if !target.target_content_types.contains(&content_type) {
            target.target_content_types.push(content_type);
        }
    }

    for unsupported in incoming.unsupported_content_types {
        if !target.unsupported_content_types.iter().any(|existing| {
            existing.content_type == unsupported.content_type
                && existing.model == unsupported.model
                && existing.vector_type == unsupported.vector_type
                && existing.reason == unsupported.reason
        }) {
            target.unsupported_content_types.push(unsupported);
        }
    }
}
