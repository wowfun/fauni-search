use super::*;
use crate::*;
use serde_json::Value;
use std::path::Path as FsPath;

impl AppState {
    pub(crate) fn get_asset(
        &self,
        library_id: &str,
        asset_id: &str,
    ) -> Result<AssetDetailData, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        let asset = library
            .assets
            .get(asset_id)
            .ok_or_else(|| ApiError::not_found("Asset was not found."))?;
        let (source_id, source_type, source_uri) = library
            .source_asset_locations
            .values()
            .find(|location| location.asset_id == asset.id)
            .and_then(|location| {
                library.sources.get(&location.source_id).map(|source| {
                    (
                        source.id.clone(),
                        source.source_type.clone(),
                        source.source_uri.clone(),
                    )
                })
            })
            .unwrap_or_else(|| {
                (
                    asset.source_id.clone(),
                    asset.source_type.clone(),
                    sources::file_source_uri(&asset.source_path),
                )
            });
        let units = asset
            .unit_ids
            .iter()
            .filter_map(|unit_id| find_unit_across_libraries(&self.libraries, unit_id))
            .map(UnitRecord::summary)
            .collect();

        Ok(AssetDetailData {
            asset: asset.snapshot(&source_id, &source_type, &source_uri),
            preview: asset_preview_reference(
                library_id,
                &asset.id,
                &asset.asset_type,
                &asset.locator,
            )?,
            neighbor_context: asset.neighbor_context.clone(),
            units,
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
        let plan_library_id = (!plan.library_id.trim().is_empty()).then(|| plan.library_id.clone());

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
                let asset = self.get_temp_query_asset_for_search(
                    plan_library_id.as_deref(),
                    temp_asset_id,
                    "image",
                    "Query image",
                )?;
                Ok((plan, ResolvedImageQueryInput::TempAsset(asset)))
            }
            "library_object" => {
                let asset_id = request.image_input.asset_id.as_deref().ok_or_else(|| {
                    ApiError::validation_failed(
                        "image_input.kind=library_object requires asset_id.",
                        Some(json!({ "field": "image_input.asset_id" })),
                    )
                })?;
                let asset = match plan_library_id.as_deref() {
                    Some(library_id) => self.get_library_asset(library_id, asset_id)?,
                    None => find_asset_across_libraries(&self.libraries, asset_id)
                        .cloned()
                        .ok_or_else(|| ApiError::not_found("Asset was not found."))?,
                };
                if !matches!(asset.asset_type.as_str(), "image" | "document_page") {
                    return Err(ApiError::not_supported(
                        "Current 110-image-search implementation only supports library image and document_page objects as query images.",
                        Some(json!({
                            "field": "image_input.asset_id",
                            "received_asset_type": asset.asset_type,
                            "supported_asset_types": ["image", "document_page"],
                        })),
                    ));
                }
                Ok((plan, ResolvedImageQueryInput::LibraryAsset(asset)))
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
        let plan_library_id = (!plan.library_id.trim().is_empty()).then(|| plan.library_id.clone());

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
                let asset = self.get_temp_query_asset_for_search(
                    plan_library_id.as_deref(),
                    temp_asset_id,
                    "video",
                    "Query video",
                )?;
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
                if let Some(asset_id) = request.video_input.asset_id.as_deref() {
                    if request.video_input.locator.is_some() {
                        return Err(ApiError::validation_failed(
                            "video_input.asset_id reuses the segment's own locator and must not carry video_input.locator.",
                            Some(json!({
                                "field": "video_input.locator",
                                "input_kind": "library_object",
                                "library_object_asset_type": "video_segment",
                            })),
                        ));
                    }

                    let asset = match plan_library_id.as_deref() {
                        Some(library_id) => self.get_library_asset(library_id, asset_id)?,
                        None => find_asset_across_libraries(&self.libraries, asset_id)
                            .cloned()
                            .ok_or_else(|| ApiError::not_found("Asset was not found."))?,
                    };
                    if asset.asset_type != "video_segment" || asset.source_type != "video" {
                        return Err(ApiError::not_supported(
                            "Current 120-video-search implementation only supports library video_segment objects as direct query video segments.",
                            Some(json!({
                                "field": "video_input.asset_id",
                                "received_asset_type": asset.asset_type,
                                "received_source_type": asset.source_type,
                                "supported_asset_type": "video_segment",
                                "supported_source_type": "video",
                            })),
                        ));
                    }

                    return Ok((
                        plan,
                        ResolvedVideoQueryInput {
                            path: asset.source_path,
                            locator: Some(asset.locator),
                        },
                    ));
                }

                let Some(plan_library_id) = plan_library_id.as_deref() else {
                    return Err(ApiError::not_supported(
                        "video_input.source_id requires single-library search_scope.",
                        Some(json!({
                            "field": "video_input.source_id",
                            "search_scope": search_scope.kind,
                        })),
                    ));
                };
                let source_id = request.video_input.source_id.as_deref().ok_or_else(|| {
                    ApiError::validation_failed(
                        "video_input.kind=library_object requires source_id or asset_id.",
                        Some(
                            json!({ "field": "video_input", "supported_fields": ["source_id", "asset_id"] }),
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
        let plan_library_id = (!plan.library_id.trim().is_empty()).then(|| plan.library_id.clone());

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
                let asset = self.get_temp_query_asset_for_search(
                    plan_library_id.as_deref(),
                    temp_asset_id,
                    "pdf",
                    "Query document",
                )?;
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
                if let Some(asset_id) = request.document_input.asset_id.as_deref() {
                    if request.document_input.locator.is_some() {
                        return Err(ApiError::validation_failed(
                            "document_input.asset_id reuses the page object's own locator and must not carry document_input.locator.",
                            Some(json!({
                                "field": "document_input.locator",
                                "input_kind": "library_object",
                                "library_object_asset_type": "document_page",
                            })),
                        ));
                    }
                    let asset = match plan_library_id.as_deref() {
                        Some(library_id) => self.get_library_asset(library_id, asset_id)?,
                        None => find_asset_across_libraries(&self.libraries, asset_id)
                            .cloned()
                            .ok_or_else(|| ApiError::not_found("Asset was not found."))?,
                    };
                    if asset.asset_type != "document_page" || asset.source_type != "pdf" {
                        return Err(ApiError::not_supported(
                            "Current 130-document-search implementation only supports library document_page objects as query documents.",
                            Some(json!({
                                "field": "document_input.asset_id",
                                "received_asset_type": asset.asset_type,
                                "received_source_type": asset.source_type,
                                "supported_asset_type": "document_page",
                                "supported_source_type": "pdf",
                            })),
                        ));
                    }
                    return Ok((
                        plan,
                        ResolvedDocumentQueryInput {
                            path: asset.source_path,
                            locator: Some(asset.locator),
                        },
                    ));
                }
                let Some(plan_library_id) = plan_library_id.as_deref() else {
                    return Err(ApiError::not_supported(
                        "document_input.source_id requires single-library search_scope.",
                        Some(json!({
                            "field": "document_input.source_id",
                            "search_scope": search_scope.kind,
                        })),
                    ));
                };
                let source_id = request.document_input.source_id.as_deref().ok_or_else(|| {
                    ApiError::validation_failed(
                        "document_input.kind=library_object requires source_id or asset_id.",
                        Some(json!({ "field": "document_input", "supported_fields": ["source_id", "asset_id"] })),
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
                self.prepare_all_libraries_search_scope(
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

    async fn prepare_all_libraries_search_scope(
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
            format!(
                "No library in the current search scope is ready for {query_input_type} search."
            ),
            Some(json!({
                "search_scope": "all_libraries",
                "query_input_type": query_input_type,
            })),
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
        let active_vector_space_ids = self
            .libraries
            .values()
            .flat_map(|library| library.unit_indexes.values())
            .filter(|index| index.status == "ready" && index.visibility == ACTIVE_INDEX_VISIBILITY)
            .map(|index| index.vector_space_id.clone())
            .collect::<BTreeSet<_>>();
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
                if active_vector_space_ids.contains(vector_space_id) {
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

        let kind_filter = read_string_filter(filters, "asset_type");
        let path_prefix_filter = read_string_filter(filters, "path_prefix");
        let source_type_filter = read_string_filter(filters, "source_type");
        let time_range_filter = resolve_time_range_filter(filters)?;
        let mut active_asset_refs = BTreeSet::new();
        let mut active_unit_index_refs = BTreeSet::new();
        let mut asset_locations = BTreeMap::new();
        let mut eligible_point_ids_by_vector_space = BTreeMap::<String, BTreeSet<u64>>::new();
        for location in library
            .source_asset_locations
            .values()
            .filter(|location| location.visibility == ACTIVE_INDEX_VISIBILITY)
        {
            let Some(asset) = find_asset_across_libraries(&self.libraries, &location.asset_id)
            else {
                continue;
            };
            if !supported_content_types
                .iter()
                .any(|content_type| content_type_matches_asset(content_type, &asset.asset_type))
            {
                continue;
            }
            let Some(source) = library.sources.get(&location.source_id) else {
                continue;
            };
            if !location_matches_filters(
                &asset,
                source,
                location,
                kind_filter.as_ref(),
                path_prefix_filter.as_ref(),
                source_type_filter.as_ref(),
                time_range_filter,
            ) {
                continue;
            }
            let scoped_ref = scoped_asset_ref(&resolved_library_id, &asset.id);
            active_asset_refs.insert(scoped_ref.clone());
            asset_locations
                .entry(scoped_ref)
                .or_insert_with(|| SearchPlanAssetLocation {
                    source_id: source.id.clone(),
                    source_uri: source.source_uri.clone(),
                    source_type: source.source_type.clone(),
                    locator: location.locator.clone(),
                });
            for unit_id in &asset.unit_ids {
                for index in active_unit_indexes_for_unit(&self.libraries, unit_id) {
                    let Some(point_id) = unit_index_point_id(index) else {
                        continue;
                    };
                    active_unit_index_refs
                        .insert(UnitIndexRecord::key(&index.unit_id, &index.vector_space_id));
                    eligible_point_ids_by_vector_space
                        .entry(index.vector_space_id.clone())
                        .or_default()
                        .insert(point_id);
                }
            }
        }
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
                    active_unit_count: eligible_point_ids_by_vector_space
                        .get(&vector_space_id)
                        .map(BTreeSet::len)
                        .unwrap_or_default(),
                    eligible_point_ids: eligible_point_ids_by_vector_space
                        .get(&vector_space_id)
                        .cloned()
                        .unwrap_or_default(),
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
            kind_filter,
            path_prefix_filter,
            source_type_filter,
            time_range_filter,
            target_content_types: supported_content_types,
            unsupported_content_types,
            active_asset_refs,
            active_unit_index_refs,
            asset_locations,
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
        let record = self.register_temp_query_asset_record(Some(library_id), staged)?;
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
        let record = self.register_temp_query_asset_record(Some(library_id), staged)?;
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
        let record = self.register_temp_query_asset_record(Some(library_id), staged)?;
        Ok(QueryDocumentAssetData {
            temp_asset_id: record.id.clone(),
            preview: query_document_preview_reference(library_id, &record.id)?,
            source_type: record.source_type.clone(),
            content_type: record.content_type.clone(),
            original_filename: record.original_filename.clone(),
            page_count: record.page_count,
        })
    }

    pub(crate) fn register_global_temp_query_asset(
        &mut self,
        staged: StagedQueryAsset,
    ) -> Result<QueryImageAssetData, ApiError> {
        let record = self.register_temp_query_asset_record(None, staged)?;
        Ok(QueryImageAssetData {
            temp_asset_id: record.id.clone(),
            preview: global_query_image_preview_reference(&record.id)?,
            source_type: record.source_type.clone(),
            content_type: record.content_type.clone(),
            original_filename: record.original_filename.clone(),
        })
    }

    pub(crate) fn register_global_temp_query_video_asset(
        &mut self,
        staged: StagedQueryAsset,
    ) -> Result<QueryVideoAssetData, ApiError> {
        let record = self.register_temp_query_asset_record(None, staged)?;
        Ok(QueryVideoAssetData {
            temp_asset_id: record.id.clone(),
            preview: global_query_video_preview_reference(&record.id)?,
            source_type: record.source_type.clone(),
            content_type: record.content_type.clone(),
            original_filename: record.original_filename.clone(),
            duration_ms: record.duration_ms,
        })
    }

    pub(crate) fn register_global_temp_query_document_asset(
        &mut self,
        staged: StagedQueryAsset,
    ) -> Result<QueryDocumentAssetData, ApiError> {
        let record = self.register_temp_query_asset_record(None, staged)?;
        Ok(QueryDocumentAssetData {
            temp_asset_id: record.id.clone(),
            preview: global_query_document_preview_reference(&record.id)?,
            source_type: record.source_type.clone(),
            content_type: record.content_type.clone(),
            original_filename: record.original_filename.clone(),
            page_count: record.page_count,
        })
    }

    pub(crate) fn register_temp_query_asset_record(
        &mut self,
        library_id: Option<&str>,
        staged: StagedQueryAsset,
    ) -> Result<TempQueryAssetRecord, ApiError> {
        if let Some(library_id) = library_id {
            if !self.libraries.contains_key(library_id) {
                return Err(ApiError::not_found("Library was not found."));
            }
        }

        self.prune_temp_query_assets();

        let before = self.temp_query_assets.clone();
        let temp_asset_id = self.next_temp_asset_id();
        let now_ms = current_unix_ms();
        let record = TempQueryAssetRecord {
            id: temp_asset_id.clone(),
            owner_scope: library_id
                .map(|_| "library".to_string())
                .unwrap_or_else(|| "global".to_string()),
            library_id: library_id.map(str::to_string),
            path: staged.path,
            content_type: staged.content_type,
            source_type: staged.source_type,
            original_filename: staged.original_filename,
            page_count: staged.page_count,
            duration_ms: staged.duration_ms,
            size_bytes: staged.size_bytes,
            created_at_ms: now_ms,
            expires_at_ms: now_ms + TEMP_QUERY_ASSET_TTL_MS,
        };

        self.temp_query_assets.insert(record.id.clone(), record);
        if let Err(message) = self.persist_durable_state() {
            let record = self.temp_query_assets.remove(&temp_asset_id);
            self.temp_query_assets = before;
            if let Some(record) = record {
                remove_temp_query_asset_file(&record.path);
            }
            return Err(ApiError::runtime_unavailable(
                format!("Query asset could not be persisted: {message}"),
                None,
            ));
        }
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

        let summary = TempQueryAssetPruneSummary {
            expired_removed,
            missing_removed,
        };
        if summary.removed_count() > 0 {
            let _ = self.persist_durable_state();
        }
        summary
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

        if !asset.is_library_scoped_to(library_id) {
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

    pub(crate) fn get_global_temp_query_asset(
        &self,
        temp_asset_id: &str,
    ) -> Result<TempQueryAssetRecord, ApiError> {
        let asset = self
            .temp_query_assets
            .get(temp_asset_id)
            .ok_or_else(|| ApiError::not_found("Query asset was not found or has expired."))?;
        if !asset.is_global() {
            return Err(ApiError::not_found(
                "Query asset was not found in the global query asset store.",
            ));
        }
        ensure_temp_query_asset_available(asset, "Query asset")?;
        Ok(asset.clone())
    }

    pub(crate) fn get_temp_query_asset_for_search(
        &self,
        library_id: Option<&str>,
        temp_asset_id: &str,
        expected_source_type: &str,
        label: &str,
    ) -> Result<TempQueryAssetRecord, ApiError> {
        let asset = self
            .temp_query_assets
            .get(temp_asset_id)
            .ok_or_else(|| ApiError::not_found(format!("{label} was not found or has expired.")))?;
        let scope_allowed = match library_id {
            Some(library_id) => asset.is_global() || asset.is_library_scoped_to(library_id),
            None => asset.is_global(),
        };
        if !scope_allowed {
            return Err(ApiError::not_found(format!(
                "{label} is not available for the selected search scope."
            )));
        }
        if asset.source_type != expected_source_type {
            return Err(ApiError::not_supported(
                format!("{label} has an unsupported source type."),
                Some(json!({
                    "received_source_type": asset.source_type,
                    "supported_source_type": expected_source_type,
                })),
            ));
        }
        ensure_temp_query_asset_available(asset, label)?;
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

        if !asset.is_library_scoped_to(library_id) {
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

        if !asset.is_library_scoped_to(library_id) {
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

    pub(crate) fn get_library_asset(
        &self,
        library_id: &str,
        asset_id: &str,
    ) -> Result<AssetRecord, ApiError> {
        if !self.libraries.contains_key(library_id) {
            return Err(ApiError::not_found("Library was not found."));
        }

        find_asset_across_libraries(&self.libraries, asset_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Asset was not found."))
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

fn content_type_matches_asset(content_type: &str, asset_kind: &str) -> bool {
    match content_type {
        "image" => asset_kind == "image",
        "document" => asset_kind == "document_page",
        "video" => asset_kind == "video_segment",
        "text" => asset_kind == "text",
        _ => false,
    }
}

fn location_matches_filters(
    asset: &AssetRecord,
    source: &SourceRecord,
    location: &SourceAssetLocationRecord,
    kind_filter: Option<&BTreeSet<String>>,
    path_prefix_filter: Option<&BTreeSet<String>>,
    source_type_filter: Option<&BTreeSet<String>>,
    time_range_filter: Option<SearchTimeRangeFilter>,
) -> bool {
    kind_filter
        .map(|expected| expected.contains(&asset.asset_type))
        .unwrap_or(true)
        && source_type_filter
            .map(|expected| expected.contains(&source.source_type))
            .unwrap_or(true)
        && path_prefix_filter
            .map(|prefixes| {
                prefixes
                    .iter()
                    .any(|prefix| source.source_uri.starts_with(prefix))
            })
            .unwrap_or(true)
        && time_range_filter
            .map(|filter| locator_overlaps_time_range(&location.locator, filter))
            .unwrap_or(true)
}

fn locator_overlaps_time_range(locator: &Value, filter: SearchTimeRangeFilter) -> bool {
    let Some(start_ms) = locator.get("start_ms").and_then(Value::as_u64) else {
        return false;
    };
    let Some(end_ms) = locator.get("end_ms").and_then(Value::as_u64) else {
        return false;
    };

    start_ms <= filter.end_ms && end_ms >= filter.start_ms
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

fn scoped_asset_ref(library_id: &str, asset_id: &str) -> String {
    format!("{library_id}:{asset_id}")
}

fn find_asset_across_libraries<'a>(
    libraries: &'a BTreeMap<String, LibraryRecord>,
    asset_id: &str,
) -> Option<&'a AssetRecord> {
    libraries
        .values()
        .find_map(|library| library.assets.get(asset_id))
}

fn find_unit_across_libraries<'a>(
    libraries: &'a BTreeMap<String, LibraryRecord>,
    unit_id: &str,
) -> Option<&'a UnitRecord> {
    libraries
        .values()
        .find_map(|library| library.units.get(unit_id))
}

fn active_unit_indexes_for_unit<'a>(
    libraries: &'a BTreeMap<String, LibraryRecord>,
    unit_id: &str,
) -> Vec<&'a UnitIndexRecord> {
    libraries
        .values()
        .flat_map(|library| library.unit_indexes.values())
        .filter(|index| {
            index.unit_id == unit_id
                && index.status == "ready"
                && index.visibility == ACTIVE_INDEX_VISIBILITY
                && index.vector_ref.is_some()
        })
        .collect()
}

fn unit_index_point_id(index: &UnitIndexRecord) -> Option<u64> {
    index
        .vector_ref
        .as_ref()
        .and_then(|value| value.get("point_id"))
        .and_then(Value::as_u64)
}

fn ensure_temp_query_asset_available(
    asset: &TempQueryAssetRecord,
    label: &str,
) -> Result<(), ApiError> {
    if asset.expires_at_ms <= current_unix_ms() {
        return Err(ApiError::not_found(format!(
            "{label} was not found or has expired."
        )));
    }
    if !FsPath::new(&asset.path).exists() {
        return Err(ApiError::not_found(format!(
            "{label} file is no longer available."
        )));
    }
    Ok(())
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
    target.active_asset_refs.extend(incoming.active_asset_refs);
    target
        .active_unit_index_refs
        .extend(incoming.active_unit_index_refs);
    target.asset_locations.extend(incoming.asset_locations);
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
