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

    pub(crate) fn prepare_text_search(
        &self,
        request: &TextSearchRequest,
    ) -> Result<SearchPlan, ApiError> {
        if request.text.trim().is_empty() {
            return Err(ApiError::validation_failed(
                "Search text must not be empty.",
                Some(json!({ "field": "text" })),
            ));
        }
        self.prepare_search_scope(
            request.library_id.trim(),
            request.filters.as_ref(),
            request.top_k,
            request.cursor.as_deref(),
            request.debug,
            request.target_index_lines.as_ref(),
        )
    }

    pub(crate) fn prepare_image_search(
        &self,
        request: &ImageSearchRequest,
    ) -> Result<(SearchPlan, ResolvedImageQueryInput), ApiError> {
        let plan = self.prepare_search_scope(
            request.library_id.trim(),
            request.filters.as_ref(),
            request.top_k,
            request.cursor.as_deref(),
            request.debug,
            request.target_index_lines.as_ref(),
        )?;

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
                let asset = self.get_temp_query_asset(&plan.library_id, temp_asset_id)?;
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
                let visual_unit = self.get_library_visual_unit(&plan.library_id, visual_unit_id)?;
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

    pub(crate) fn prepare_video_search(
        &self,
        request: &VideoSearchRequest,
    ) -> Result<(SearchPlan, ResolvedVideoQueryInput), ApiError> {
        let plan = self.prepare_search_scope(
            request.library_id.trim(),
            request.filters.as_ref(),
            request.top_k,
            request.cursor.as_deref(),
            request.debug,
            request.target_index_lines.as_ref(),
        )?;

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
                let asset = self.get_temp_query_video_asset(&plan.library_id, temp_asset_id)?;
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
                        self.get_library_visual_unit(&plan.library_id, visual_unit_id)?;
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
                let source = self.get_library_source(&plan.library_id, source_id)?;
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

    pub(crate) fn prepare_document_search(
        &self,
        request: &DocumentSearchRequest,
    ) -> Result<(SearchPlan, ResolvedDocumentQueryInput), ApiError> {
        let plan = self.prepare_search_scope(
            request.library_id.trim(),
            request.filters.as_ref(),
            request.top_k,
            request.cursor.as_deref(),
            request.debug,
            request.target_index_lines.as_ref(),
        )?;

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
                let asset = self.get_temp_query_document_asset(&plan.library_id, temp_asset_id)?;
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
                let source = self.get_library_source(&plan.library_id, source_id)?;
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

    pub(crate) fn prepare_search_scope(
        &self,
        library_id: &str,
        filters: Option<&Value>,
        top_k: Option<usize>,
        cursor: Option<&str>,
        debug: Option<bool>,
        target_index_lines: Option<&Vec<String>>,
    ) -> Result<SearchPlan, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        let target_index_lines = target_index_lines
            .cloned()
            .map(|lines| normalize_index_lines(Some(lines)))
            .filter(|lines| !lines.is_empty())
            .unwrap_or_else(|| library.config.enabled_index_lines.clone());

        let enabled_lines: BTreeSet<_> =
            library.config.enabled_index_lines.iter().cloned().collect();
        let invalid_target_lines: Vec<_> = target_index_lines
            .iter()
            .filter(|line| !enabled_lines.contains(*line))
            .cloned()
            .collect();

        if !invalid_target_lines.is_empty() {
            return Err(ApiError::not_enabled(
                "Requested index lines are not enabled for the selected library.",
                Some(json!({ "target_index_lines": invalid_target_lines })),
            ));
        }

        let not_ready_lines: Vec<_> = target_index_lines
            .iter()
            .filter(|line| !library.active_index_lines.contains(*line))
            .map(|line| {
                let job_summary = library.latest_job_id.as_ref().and_then(|job_id| {
                    self.jobs.get(job_id).map(|job| {
                        json!({
                            "job_id": job.snapshot.job_id,
                            "status": job.snapshot.status,
                            "phase": job.snapshot.phase,
                        })
                    })
                });

                json!({
                    "index_line": line,
                    "status": "not_ready",
                    "job": job_summary,
                })
            })
            .collect();

        if !not_ready_lines.is_empty() {
            return Err(ApiError::not_ready(
                "The requested index lines are enabled but do not have an active index yet.",
                Some(json!({ "index_lines": not_ready_lines })),
            ));
        }

        let cursor_offset = decode_search_cursor_offset(cursor)?;
        let time_range_filter = resolve_time_range_filter(filters)?;

        Ok(SearchPlan {
            library_id: library.id.clone(),
            collection_name: library.collection_name.clone(),
            top_k: top_k.unwrap_or(10).max(1),
            cursor_offset,
            kind_filter: read_string_filter(filters, "visual_unit.kind")
                .or_else(|| read_string_filter(filters, "kind")),
            path_prefix_filter: read_string_filter(filters, "path_prefix"),
            source_type_filter: read_string_filter(filters, "source_type"),
            time_range_filter,
            target_index_lines: target_index_lines.clone(),
            active_visual_unit_ids: library.visual_units.keys().cloned().collect(),
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

fn resolve_time_range_filter(filters: Option<&Value>) -> Result<Option<SearchTimeRangeFilter>, ApiError> {
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
