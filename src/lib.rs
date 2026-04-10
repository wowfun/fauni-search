use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use lopdf::Document as PdfDocument;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::Path as FsPath,
    process::Command,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;

const MULTIVECTOR_INDEX_LINE: &str = "multivector";
const QDRANT_MAX_UPSERT_BODY_BYTES: usize = 8 * 1024 * 1024;
const QDRANT_UPSERT_BODY_OVERHEAD_BYTES: usize = br#"{"points":[]}"#.len();
const SIDECAR_REQUEST_TIMEOUT_SECS: u64 = 600;
const TEMP_QUERY_ASSET_TTL_MS: u128 = 60 * 60 * 1000;
const TEMP_QUERY_ASSET_REAPER_INTERVAL_SECS: u64 = 60;
const VIDEO_SEGMENT_WINDOW_MS: u64 = 8_000;
const VIDEO_SEGMENT_OVERLAP_MS: u64 = 2_000;
const APP_BODY_LIMIT_BYTES: usize = 64 * 1024 * 1024;

pub type SharedState = Arc<RwLock<AppState>>;

pub fn new_state() -> SharedState {
    Arc::new(RwLock::new(AppState::default()))
}

pub fn build_app(state: SharedState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/libraries", get(list_libraries).post(create_library))
        .route("/libraries/:library_id", get(get_library))
        .route("/libraries/:library_id/imports", post(import_paths))
        .route(
            "/libraries/:library_id/video-sources",
            get(list_video_sources),
        )
        .route(
            "/libraries/:library_id/query-assets/images",
            post(upload_query_image),
        )
        .route(
            "/libraries/:library_id/query-assets/videos",
            post(upload_query_video),
        )
        .route(
            "/libraries/:library_id/query-assets/images/:temp_asset_id/preview",
            get(get_query_image_preview),
        )
        .route(
            "/libraries/:library_id/query-assets/videos/:temp_asset_id/preview",
            get(get_query_video_preview),
        )
        .route(
            "/libraries/:library_id/video-sources/:source_id/preview",
            get(get_video_source_preview),
        )
        .route(
            "/libraries/:library_id/visual-units/:visual_unit_id",
            get(get_visual_unit),
        )
        .route(
            "/libraries/:library_id/visual-units/:visual_unit_id/preview",
            get(get_visual_unit_preview),
        )
        .route("/jobs", get(list_jobs))
        .route("/jobs/:job_id", get(get_job))
        .route("/search/text", post(search_text))
        .route("/search/image", post(search_image))
        .route("/search/video", post(search_video))
        .layer(DefaultBodyLimit::max(APP_BODY_LIMIT_BYTES))
        .with_state(state)
}

pub fn spawn_runtime_maintenance(state: SharedState) {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(Duration::from_secs(TEMP_QUERY_ASSET_REAPER_INTERVAL_SECS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;

        loop {
            interval.tick().await;
            let summary = {
                let mut state = state.write().await;
                state.prune_temp_query_assets()
            };

            if summary.removed_count() > 0 {
                tracing::info!(
                    expired_removed = summary.expired_removed,
                    missing_removed = summary.missing_removed,
                    "Pruned expired query assets."
                );
            }
        }
    });
}

pub struct AppState {
    runtime_token: String,
    next_library_seq: u64,
    next_job_seq: u64,
    next_visual_unit_seq: u64,
    next_source_seq: u64,
    next_temp_asset_seq: u64,
    libraries: BTreeMap<String, LibraryRecord>,
    library_order: Vec<String>,
    jobs: BTreeMap<String, JobRecord>,
    job_order: Vec<String>,
    temp_query_assets: BTreeMap<String, TempQueryAssetRecord>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            runtime_token: runtime_token(),
            next_library_seq: 0,
            next_job_seq: 0,
            next_visual_unit_seq: 0,
            next_source_seq: 0,
            next_temp_asset_seq: 0,
            libraries: BTreeMap::new(),
            library_order: Vec::new(),
            jobs: BTreeMap::new(),
            job_order: Vec::new(),
            temp_query_assets: BTreeMap::new(),
        }
    }
}

impl AppState {
    fn list_libraries(&self) -> LibrariesListData {
        let libraries = self
            .library_order
            .iter()
            .filter_map(|id| self.libraries.get(id))
            .map(|record| self.library_snapshot(record))
            .collect();

        LibrariesListData { libraries }
    }

    fn get_library(&self, library_id: &str) -> Result<LibrarySnapshot, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        Ok(self.library_snapshot(library))
    }

    fn list_video_sources(&self, library_id: &str) -> Result<VideoSourcesData, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        let sources = library
            .source_order
            .iter()
            .filter_map(|source_id| library.sources.get(source_id))
            .filter(|source| source.source_type == "video")
            .map(|source| {
                Ok(VideoSourceSummary {
                    source_id: source.id.clone(),
                    source_path: source.source_path.clone(),
                    source_type: source.source_type.clone(),
                    duration_ms: source.duration_ms,
                    preview: video_source_preview_reference(library_id, &source.id)?,
                })
            })
            .collect::<Result<Vec<_>, ApiError>>()?;

        Ok(VideoSourcesData { sources })
    }

    fn create_library(
        &mut self,
        request: CreateLibraryRequest,
    ) -> Result<LibrarySnapshot, ApiError> {
        let name = request.name.trim();
        if name.is_empty() {
            return Err(ApiError::validation_failed(
                "Library name must not be empty.",
                Some(json!({ "field": "name" })),
            ));
        }

        let enabled_index_lines =
            normalize_index_lines(request.config.map(|config| config.enabled_index_lines));

        if enabled_index_lines != [MULTIVECTOR_INDEX_LINE.to_string()] {
            return Err(ApiError::validation_failed(
                "Current 100-text-search implementation requires config.enabled_index_lines to be exactly [\"multivector\"].",
                Some(json!({
                    "field": "config.enabled_index_lines",
                    "expected": [MULTIVECTOR_INDEX_LINE],
                    "received": enabled_index_lines,
                })),
            ));
        }

        let library_id = self.next_library_id();
        let record = LibraryRecord {
            id: library_id.clone(),
            name: name.to_string(),
            collection_name: format!(
                "text_search_{}_{}_{}",
                self.runtime_token, library_id, MULTIVECTOR_INDEX_LINE
            ),
            config: LibraryConfigPayload {
                enabled_index_lines,
            },
            sources: BTreeMap::new(),
            source_order: Vec::new(),
            visual_units: BTreeMap::new(),
            visual_unit_order: Vec::new(),
            latest_job_id: None,
            active_index_lines: BTreeSet::new(),
        };

        let snapshot = self.library_snapshot(&record);
        self.library_order.push(library_id.clone());
        self.libraries.insert(library_id, record);
        Ok(snapshot)
    }

    fn prepare_import(
        &mut self,
        library_id: &str,
        request: ImportPathsRequest,
    ) -> Result<PreparedImport, ApiError> {
        let collection_name = self
            .libraries
            .get(library_id)
            .map(|library| library.collection_name.clone())
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        let mut new_sources = Vec::new();
        let mut new_visual_units = Vec::new();

        for original in request.paths {
            match self.inspect_import_path(&original) {
                Ok(classification) => {
                    let source = self.source_record_from_classification(&classification);
                    let visual_units = self.new_visual_units_from_classification(&classification);
                    let visual_unit_summaries =
                        visual_units.iter().map(VisualUnitRecord::summary).collect();
                    new_sources.push(source);
                    new_visual_units.extend(visual_units);

                    accepted.push(ImportAcceptedItem {
                        original_path: original,
                        normalized_path: Some(classification.normalized_path),
                        reason_code: "accepted".to_string(),
                        message: format!(
                            "Accepted as {} input for the pending multivector index.",
                            classification.source_type
                        ),
                        source_id: Some(classification.source_id),
                        source_type: classification.source_type,
                        kind: classification.kind,
                        visual_units: visual_unit_summaries,
                    });
                }
                Err(rejection) => rejected.push(ImportRejectedItem {
                    original_path: original,
                    normalized_path: rejection.normalized_path,
                    reason_code: rejection.reason_code,
                    message: rejection.message,
                }),
            }
        }

        Ok(PreparedImport {
            library_id: library_id.to_string(),
            collection_name,
            accepted,
            rejected,
            sources: new_sources,
            visual_units: new_visual_units,
        })
    }

    fn queue_import(&mut self, prepared: &PreparedImport) -> Result<ImportPathsData, ApiError> {
        if prepared.accepted.is_empty() {
            return Ok(ImportPathsData {
                accepted: prepared.accepted.clone(),
                rejected: prepared.rejected.clone(),
                job_handle: None,
                job: None,
            });
        }

        let job_id = self.next_job_id();
        let snapshot = JobSnapshot {
            job_id: job_id.clone(),
            library_id: prepared.library_id.clone(),
            kind: "import".to_string(),
            status: "queued".to_string(),
            phase: "intake".to_string(),
            progress: JobProgress {
                completed: 0,
                total: prepared.accepted.len(),
                unit: "item".to_string(),
            },
            cancelable: false,
            current_attempt: JobAttemptSnapshot {
                attempt: 1,
                status: "queued".to_string(),
                summary: format!(
                    "Accepted {} path(s); queued for multivector indexing.",
                    prepared.accepted.len()
                ),
            },
        };

        let library = self
            .libraries
            .get_mut(&prepared.library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;
        library.latest_job_id = Some(job_id.clone());

        self.jobs.insert(
            job_id.clone(),
            JobRecord {
                snapshot: snapshot.clone(),
            },
        );
        self.job_order.push(job_id.clone());

        Ok(ImportPathsData {
            accepted: prepared.accepted.clone(),
            rejected: prepared.rejected.clone(),
            job_handle: Some(job_id),
            job: Some(snapshot),
        })
    }

    fn update_job_snapshot(
        &mut self,
        job_id: &str,
        status: &str,
        phase: &str,
        completed: usize,
        summary: impl Into<String>,
    ) {
        if let Some(job) = self.jobs.get_mut(job_id) {
            job.snapshot.status = status.to_string();
            job.snapshot.phase = phase.to_string();
            job.snapshot.progress.completed = completed.min(job.snapshot.progress.total);
            job.snapshot.current_attempt.status = status.to_string();
            job.snapshot.current_attempt.summary = summary.into();
        }
    }

    fn finalize_import_job(
        &mut self,
        job_id: &str,
        prepared: PreparedImport,
        outcome: ImportJobOutcome,
    ) -> Result<(), String> {
        let library = self
            .libraries
            .get_mut(&prepared.library_id)
            .ok_or_else(|| "Library was not found.".to_string())?;

        for source in &prepared.sources {
            library.source_order.push(source.id.clone());
            library.sources.insert(source.id.clone(), source.clone());
        }

        for visual_unit in &prepared.visual_units {
            library.visual_unit_order.push(visual_unit.id.clone());
            library
                .visual_units
                .insert(visual_unit.id.clone(), visual_unit.clone());
        }

        if outcome.activate_index {
            library
                .active_index_lines
                .insert(MULTIVECTOR_INDEX_LINE.to_string());
        }

        let job = self
            .jobs
            .get_mut(job_id)
            .ok_or_else(|| "Job was not found.".to_string())?;
        job.snapshot.status = outcome.status.to_string();
        job.snapshot.phase = outcome.phase.to_string();
        job.snapshot.progress.completed = outcome.completed.min(job.snapshot.progress.total);
        job.snapshot.current_attempt.status = outcome.status.to_string();
        job.snapshot.current_attempt.summary = outcome.summary;

        Ok(())
    }

    fn list_jobs(&self, library_id: Option<&str>) -> JobsListData {
        let jobs = self
            .job_order
            .iter()
            .rev()
            .filter_map(|job_id| self.jobs.get(job_id))
            .filter(|job| {
                library_id
                    .map(|expected| job.snapshot.library_id == expected)
                    .unwrap_or(true)
            })
            .map(|job| job.snapshot.clone())
            .collect();

        JobsListData { jobs }
    }

    fn get_visual_unit(
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

    fn get_job(&self, job_id: &str) -> Result<JobSnapshot, ApiError> {
        self.jobs
            .get(job_id)
            .map(|job| job.snapshot.clone())
            .ok_or_else(|| ApiError::not_found("Job was not found."))
    }

    fn prepare_text_search(&self, request: &TextSearchRequest) -> Result<SearchPlan, ApiError> {
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
            request.debug,
            request.target_index_lines.as_ref(),
        )
    }

    fn prepare_image_search(
        &self,
        request: &ImageSearchRequest,
    ) -> Result<(SearchPlan, ResolvedImageQueryInput), ApiError> {
        let plan = self.prepare_search_scope(
            request.library_id.trim(),
            request.filters.as_ref(),
            request.top_k,
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

    fn prepare_video_search(
        &self,
        request: &VideoSearchRequest,
    ) -> Result<(SearchPlan, ResolvedVideoQueryInput), ApiError> {
        let plan = self.prepare_search_scope(
            request.library_id.trim(),
            request.filters.as_ref(),
            request.top_k,
            request.debug,
            request.target_index_lines.as_ref(),
        )?;

        match request.video_input.kind.as_str() {
            "temp_asset" => {
                let temp_asset_id = request
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

    fn prepare_search_scope(
        &self,
        library_id: &str,
        filters: Option<&Value>,
        top_k: Option<usize>,
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

        Ok(SearchPlan {
            library_id: library.id.clone(),
            collection_name: library.collection_name.clone(),
            top_k: top_k.unwrap_or(10).max(1),
            kind_filter: read_string_filter(filters, "visual_unit.kind")
                .or_else(|| read_string_filter(filters, "kind")),
            source_type_filter: read_string_filter(filters, "source_type"),
            debug: debug.unwrap_or(false),
        })
    }

    fn register_temp_query_asset(
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

    fn register_temp_query_video_asset(
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

    fn register_temp_query_asset_record(
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
            duration_ms: staged.duration_ms,
            expires_at_ms: current_unix_ms() + TEMP_QUERY_ASSET_TTL_MS,
        };

        self.temp_query_assets.insert(record.id.clone(), record);
        Ok(self.temp_query_assets[&temp_asset_id].clone())
    }

    fn prune_temp_query_assets(&mut self) -> TempQueryAssetPruneSummary {
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

    fn get_temp_query_asset(
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

    fn get_temp_query_video_asset(
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

    fn get_library_visual_unit(
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

    fn get_library_source(
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
    }

    fn inspect_import_path(
        &mut self,
        original_path: &str,
    ) -> Result<PathClassification, ImportRejection> {
        let trimmed = original_path.trim();
        if trimmed.is_empty() {
            return Err(ImportRejection {
                normalized_path: None,
                reason_code: "empty_path".to_string(),
                message: "Path must not be empty.".to_string(),
            });
        }

        let path = FsPath::new(trimmed);
        if !path.exists() {
            return Err(ImportRejection {
                normalized_path: None,
                reason_code: "not_found".to_string(),
                message: "Path does not exist.".to_string(),
            });
        }

        let metadata = fs::metadata(path).map_err(|_| ImportRejection {
            normalized_path: None,
            reason_code: "not_readable".to_string(),
            message: "Path metadata could not be read.".to_string(),
        })?;

        if !metadata.is_file() {
            return Err(ImportRejection {
                normalized_path: None,
                reason_code: "not_file".to_string(),
                message: "Only file paths are accepted in the current implementation.".to_string(),
            });
        }

        let normalized_path = fs::canonicalize(path)
            .map(|resolved| resolved.to_string_lossy().to_string())
            .unwrap_or_else(|_| trimmed.to_string());

        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());

        match extension.as_deref() {
            Some("pdf") => {
                let page_count = pdf_page_count(path).map_err(|message| ImportRejection {
                    normalized_path: Some(normalized_path.clone()),
                    reason_code: "invalid_pdf".to_string(),
                    message,
                })?;
                Ok(PathClassification {
                    source_id: self.next_source_id(),
                    normalized_path,
                    source_type: "pdf".to_string(),
                    kind: "document_page".to_string(),
                    page_count: Some(page_count),
                    duration_ms: None,
                })
            }
            Some("png") | Some("jpg") | Some("jpeg") | Some("webp") | Some("bmp") | Some("gif") => {
                Ok(PathClassification {
                    source_id: self.next_source_id(),
                    normalized_path,
                    source_type: "image".to_string(),
                    kind: "image".to_string(),
                    page_count: None,
                    duration_ms: None,
                })
            }
            Some("mp4") | Some("mov") | Some("m4v") => {
                let duration_ms = video_duration_ms(path).map_err(|message| ImportRejection {
                    normalized_path: Some(normalized_path.clone()),
                    reason_code: "invalid_video".to_string(),
                    message,
                })?;
                Ok(PathClassification {
                    source_id: self.next_source_id(),
                    normalized_path,
                    source_type: "video".to_string(),
                    kind: "video_segment".to_string(),
                    page_count: None,
                    duration_ms: Some(duration_ms),
                })
            }
            _ => Err(ImportRejection {
                normalized_path: Some(normalized_path),
                reason_code: "unsupported_type".to_string(),
                message: "Only PDF, common image files, and mp4/mov video files are accepted right now.".to_string(),
            }),
        }
    }

    fn library_snapshot(&self, library: &LibraryRecord) -> LibrarySnapshot {
        let pending_jobs = self
            .jobs
            .values()
            .filter(|job| {
                job.snapshot.library_id == library.id
                    && !matches!(
                        job.snapshot.status.as_str(),
                        "completed" | "failed" | "canceled"
                    )
            })
            .count();

        let index_lines = library
            .config
            .enabled_index_lines
            .iter()
            .map(|index_line| LibraryIndexLineStatus {
                index_line: index_line.clone(),
                status: if library.active_index_lines.contains(index_line) {
                    "ready".to_string()
                } else {
                    "not_ready".to_string()
                },
            })
            .collect();

        LibrarySnapshot {
            id: library.id.clone(),
            name: library.name.clone(),
            config: library.config.clone(),
            index_lines,
            counts: LibraryCounts {
                accepted_items: library.visual_units.len(),
                pending_jobs,
            },
            latest_job_id: library.latest_job_id.clone(),
        }
    }

    fn next_library_id(&mut self) -> String {
        self.next_library_seq += 1;
        format!("lib_{:06}", self.next_library_seq)
    }

    fn next_job_id(&mut self) -> String {
        self.next_job_seq += 1;
        format!("job_{:06}", self.next_job_seq)
    }

    fn next_visual_unit_id(&mut self) -> String {
        self.next_visual_unit_seq += 1;
        format!("vu_{:06}", self.next_visual_unit_seq)
    }

    fn next_source_id(&mut self) -> String {
        self.next_source_seq += 1;
        format!("src_{:06}", self.next_source_seq)
    }

    fn next_temp_asset_id(&mut self) -> String {
        self.next_temp_asset_seq += 1;
        format!("temp_asset_{:06}", self.next_temp_asset_seq)
    }

    fn source_record_from_classification(&self, classification: &PathClassification) -> SourceRecord {
        SourceRecord {
            id: classification.source_id.clone(),
            source_path: classification.normalized_path.clone(),
            source_type: classification.source_type.clone(),
            duration_ms: classification.duration_ms,
        }
    }

    fn new_visual_units_from_classification(
        &mut self,
        classification: &PathClassification,
    ) -> Vec<VisualUnitRecord> {
        if classification.kind == "document_page" {
            let page_count = classification.page_count.unwrap_or(1);
            return (1..=page_count)
                .map(|page_number| {
                    self.new_visual_unit_record(
                        classification,
                        json!({
                            "page": page_number,
                            "page_label": page_number.to_string(),
                        }),
                        json!({
                            "previous_page": (page_number > 1).then_some(page_number - 1),
                            "current_page": page_number,
                            "next_page": (page_number < page_count).then_some(page_number + 1),
                            "total_pages": page_count,
                            "source_path": classification.normalized_path,
                            "source_type": classification.source_type,
                        }),
                    )
                })
                .collect();
        }

        if classification.kind == "video_segment" {
            let duration_ms = classification.duration_ms.unwrap_or(1);
            let segments = build_video_segment_ranges(duration_ms);
            return segments
                .iter()
                .enumerate()
                .map(|(segment_index, (start_ms, end_ms))| {
                    let previous = segment_index
                        .checked_sub(1)
                        .and_then(|index| segments.get(index))
                        .map(|(start_ms, end_ms)| json!({ "start_ms": start_ms, "end_ms": end_ms }));
                    let next = segments
                        .get(segment_index + 1)
                        .map(|(start_ms, end_ms)| json!({ "start_ms": start_ms, "end_ms": end_ms }));
                    self.new_visual_unit_record(
                        classification,
                        json!({
                            "start_ms": start_ms,
                            "end_ms": end_ms,
                            "duration_ms": duration_ms,
                        }),
                        json!({
                            "previous_segment": previous,
                            "current_segment": {
                                "start_ms": start_ms,
                                "end_ms": end_ms,
                            },
                            "next_segment": next,
                            "total_segments": segments.len(),
                            "source_path": classification.normalized_path,
                            "source_type": classification.source_type,
                            "duration_ms": duration_ms,
                        }),
                    )
                })
                .collect();
        }

        vec![self.new_visual_unit_record(
            classification,
            json!({
                "path": classification.normalized_path,
            }),
            json!({
                "source_type": classification.source_type,
                "source_path": classification.normalized_path,
            }),
        )]
    }

    fn new_visual_unit_record(
        &mut self,
        classification: &PathClassification,
        locator: Value,
        neighbor_context: Value,
    ) -> VisualUnitRecord {
        let point_id = self.next_visual_unit_seq + 1;
        let visual_unit_id = self.next_visual_unit_id();

        VisualUnitRecord {
            id: visual_unit_id,
            point_id,
            source_id: classification.source_id.clone(),
            source_path: classification.normalized_path.clone(),
            source_type: classification.source_type.clone(),
            kind: classification.kind.clone(),
            locator,
            neighbor_context,
        }
    }
}

#[derive(Clone)]
struct LibraryRecord {
    id: String,
    name: String,
    collection_name: String,
    config: LibraryConfigPayload,
    sources: BTreeMap<String, SourceRecord>,
    source_order: Vec<String>,
    visual_units: BTreeMap<String, VisualUnitRecord>,
    visual_unit_order: Vec<String>,
    latest_job_id: Option<String>,
    active_index_lines: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct SourceRecord {
    id: String,
    source_path: String,
    source_type: String,
    duration_ms: Option<u64>,
}

#[derive(Clone, Debug)]
struct VisualUnitRecord {
    id: String,
    point_id: u64,
    source_id: String,
    source_path: String,
    source_type: String,
    kind: String,
    locator: Value,
    neighbor_context: Value,
}

impl VisualUnitRecord {
    fn summary(&self) -> VisualUnitSummary {
        VisualUnitSummary {
            visual_unit_id: self.id.clone(),
            source_id: self.source_id.clone(),
            kind: self.kind.clone(),
            source_type: self.source_type.clone(),
            locator: self.locator.clone(),
        }
    }

    fn snapshot(&self) -> VisualUnitSnapshot {
        VisualUnitSnapshot {
            visual_unit_id: self.id.clone(),
            source_id: self.source_id.clone(),
            kind: self.kind.clone(),
            source_type: self.source_type.clone(),
            source_path: self.source_path.clone(),
            locator: self.locator.clone(),
        }
    }
}

#[derive(Clone)]
struct JobRecord {
    snapshot: JobSnapshot,
}

#[derive(Clone, Debug)]
struct TempQueryAssetRecord {
    id: String,
    library_id: String,
    path: String,
    source_type: String,
    content_type: String,
    original_filename: Option<String>,
    duration_ms: Option<u64>,
    expires_at_ms: u128,
}

#[derive(Default)]
struct TempQueryAssetPruneSummary {
    expired_removed: usize,
    missing_removed: usize,
}

impl TempQueryAssetPruneSummary {
    fn removed_count(&self) -> usize {
        self.expired_removed + self.missing_removed
    }
}

#[derive(Clone, Debug)]
enum ResolvedImageQueryInput {
    TempAsset(TempQueryAssetRecord),
    LibraryVisualUnit(VisualUnitRecord),
}

#[derive(Clone, Debug)]
struct ResolvedVideoQueryInput {
    path: String,
    locator: Option<Value>,
}

struct PathClassification {
    source_id: String,
    normalized_path: String,
    source_type: String,
    kind: String,
    page_count: Option<usize>,
    duration_ms: Option<u64>,
}

#[derive(Debug)]
struct ImportRejection {
    normalized_path: Option<String>,
    reason_code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateLibraryRequest {
    pub name: String,
    pub config: Option<CreateLibraryConfigRequest>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLibraryConfigRequest {
    pub enabled_index_lines: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LibraryConfigPayload {
    pub enabled_index_lines: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct LibrariesListData {
    pub libraries: Vec<LibrarySnapshot>,
}

#[derive(Debug, Serialize)]
pub struct LibrarySnapshot {
    pub id: String,
    pub name: String,
    pub config: LibraryConfigPayload,
    pub index_lines: Vec<LibraryIndexLineStatus>,
    pub counts: LibraryCounts,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_job_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LibraryIndexLineStatus {
    pub index_line: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct LibraryCounts {
    pub accepted_items: usize,
    pub pending_jobs: usize,
}

#[derive(Debug, Deserialize)]
pub struct ImportPathsRequest {
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ImportPathsData {
    pub accepted: Vec<ImportAcceptedItem>,
    pub rejected: Vec<ImportRejectedItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<JobSnapshot>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ImportAcceptedItem {
    pub original_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_path: Option<String>,
    pub reason_code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    pub source_type: String,
    pub kind: String,
    pub visual_units: Vec<VisualUnitSummary>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ImportRejectedItem {
    pub original_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_path: Option<String>,
    pub reason_code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct JobSnapshot {
    pub job_id: String,
    pub library_id: String,
    pub kind: String,
    pub status: String,
    pub phase: String,
    pub progress: JobProgress,
    pub cancelable: bool,
    pub current_attempt: JobAttemptSnapshot,
}

#[derive(Debug, Serialize, Clone)]
pub struct JobProgress {
    pub completed: usize,
    pub total: usize,
    pub unit: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct JobAttemptSnapshot {
    pub attempt: u32,
    pub status: String,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub struct JobsListData {
    pub jobs: Vec<JobSnapshot>,
}

#[derive(Debug, Serialize, Clone)]
pub struct VisualUnitSummary {
    pub visual_unit_id: String,
    pub source_id: String,
    pub kind: String,
    pub source_type: String,
    pub locator: Value,
}

#[derive(Debug, Serialize, Clone)]
pub struct VisualUnitSnapshot {
    pub visual_unit_id: String,
    pub source_id: String,
    pub kind: String,
    pub source_type: String,
    pub source_path: String,
    pub locator: Value,
}

#[derive(Debug, Serialize)]
pub struct VisualUnitDetailData {
    pub visual_unit: VisualUnitSnapshot,
    pub preview: PreviewReference,
    pub neighbor_context: Value,
}

#[derive(Debug, Deserialize)]
pub struct JobsQuery {
    pub library_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TextSearchRequest {
    pub library_id: String,
    pub text: String,
    pub filters: Option<Value>,
    pub top_k: Option<usize>,
    pub cursor: Option<String>,
    pub debug: Option<bool>,
    pub target_index_lines: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct ImageSearchRequest {
    pub library_id: String,
    pub image_input: QueryImageInputRequest,
    pub filters: Option<Value>,
    pub top_k: Option<usize>,
    pub cursor: Option<String>,
    pub debug: Option<bool>,
    pub target_index_lines: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct VideoSearchRequest {
    pub library_id: String,
    pub video_input: QueryVideoInputRequest,
    pub filters: Option<Value>,
    pub top_k: Option<usize>,
    pub cursor: Option<String>,
    pub debug: Option<bool>,
    pub target_index_lines: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct QueryImageInputRequest {
    pub kind: String,
    pub temp_asset_id: Option<String>,
    pub visual_unit_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct QueryVideoInputRequest {
    pub kind: String,
    pub temp_asset_id: Option<String>,
    pub source_id: Option<String>,
    pub visual_unit_id: Option<String>,
    pub locator: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct TextSearchData {
    pub results: Vec<SearchResultItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct SearchResultItem {
    pub visual_unit_id: String,
    pub source_id: String,
    pub preview: PreviewReference,
    pub source_path: String,
    pub source_type: String,
    pub kind: String,
    pub locator: Value,
    pub cursor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct PreviewReference {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct QueryImageAssetData {
    pub temp_asset_id: String,
    pub preview: PreviewReference,
    pub source_type: String,
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_filename: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct QueryVideoAssetData {
    pub temp_asset_id: String,
    pub preview: PreviewReference,
    pub source_type: String,
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct VideoSourcesData {
    pub sources: Vec<VideoSourceSummary>,
}

#[derive(Debug, Serialize)]
pub struct VideoSourceSummary {
    pub source_id: String,
    pub source_path: String,
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    pub preview: PreviewReference,
}

struct PreparedImport {
    library_id: String,
    collection_name: String,
    accepted: Vec<ImportAcceptedItem>,
    rejected: Vec<ImportRejectedItem>,
    sources: Vec<SourceRecord>,
    visual_units: Vec<VisualUnitRecord>,
}

struct ImportJobOutcome {
    status: &'static str,
    phase: &'static str,
    completed: usize,
    activate_index: bool,
    summary: String,
}

impl ImportJobOutcome {
    fn completed(summary: String, completed: usize) -> Self {
        Self {
            status: "completed",
            phase: "activated",
            completed,
            activate_index: true,
            summary,
        }
    }

    fn failed(phase: &'static str, message: String, completed: usize) -> Self {
        Self {
            status: "failed",
            phase,
            completed,
            activate_index: false,
            summary: message,
        }
    }
}

#[derive(Debug)]
struct SearchPlan {
    library_id: String,
    collection_name: String,
    top_k: usize,
    kind_filter: Option<BTreeSet<String>>,
    source_type_filter: Option<BTreeSet<String>>,
    debug: bool,
}

struct StagedQueryAsset {
    path: String,
    source_type: String,
    content_type: String,
    original_filename: Option<String>,
    duration_ms: Option<u64>,
}

struct IncomingQueryImageUpload {
    bytes: Vec<u8>,
    content_type: String,
    original_filename: Option<String>,
    extension: String,
}

struct IncomingQueryVideoUpload {
    bytes: Vec<u8>,
    content_type: String,
    original_filename: Option<String>,
    extension: String,
}

struct IndexingError {
    phase: &'static str,
    message: String,
    completed: usize,
}

#[derive(Deserialize)]
struct SidecarEnvelope<T> {
    data: T,
}

#[derive(Deserialize)]
struct SidecarEmbedPayload {
    embeddings: Vec<SidecarEmbeddingItem>,
}

#[derive(Deserialize)]
struct SidecarEmbeddingItem {
    path: Option<String>,
    source_type: Option<String>,
    kind: Option<String>,
    locator: Option<Value>,
    vectors: Vec<Vec<f32>>,
    #[serde(default)]
    pooled_vector: Vec<f32>,
}

struct QueryEmbeddingResult {
    vectors: Vec<Vec<f32>>,
    pooled_vector: Vec<f32>,
}

#[derive(Deserialize)]
struct SidecarErrorEnvelope {
    error: SidecarErrorPayload,
}

#[derive(Deserialize)]
struct SidecarErrorPayload {
    code: String,
    message: String,
    #[allow(dead_code)]
    details: Option<Value>,
}

#[derive(Deserialize)]
struct QdrantQueryResponse {
    result: QdrantQueryResult,
}

#[derive(Deserialize)]
struct QdrantQueryResult {
    points: Vec<QdrantScoredPoint>,
}

#[derive(Deserialize)]
struct QdrantScoredPoint {
    score: f32,
    payload: Option<QdrantPointPayload>,
}

#[derive(Clone, Deserialize)]
struct QdrantPointPayload {
    visual_unit_id: String,
    source_id: String,
    source_path: String,
    source_type: String,
    kind: String,
    locator: Value,
}

#[derive(Serialize)]
struct RootPayload {
    name: &'static str,
    status: &'static str,
    stage: &'static str,
    routes: Vec<&'static str>,
}

#[derive(Serialize)]
struct HealthPayload {
    service: &'static str,
    status: &'static str,
    env: String,
    libraries: usize,
    jobs: usize,
}

#[derive(Serialize)]
struct SuccessEnvelope<T> {
    data: T,
}

#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorPayload,
}

#[derive(Debug, Serialize)]
struct ErrorPayload {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retryable: Option<bool>,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    payload: ErrorPayload,
}

impl ApiError {
    fn validation_failed(message: impl Into<String>, details: Option<Value>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            payload: ErrorPayload {
                code: "validation_failed".to_string(),
                message: message.into(),
                details,
                retryable: Some(false),
            },
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            payload: ErrorPayload {
                code: "not_found".to_string(),
                message: message.into(),
                details: None,
                retryable: Some(false),
            },
        }
    }

    fn not_enabled(message: impl Into<String>, details: Option<Value>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            payload: ErrorPayload {
                code: "not_enabled".to_string(),
                message: message.into(),
                details,
                retryable: Some(false),
            },
        }
    }

    fn not_supported(message: impl Into<String>, details: Option<Value>) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            payload: ErrorPayload {
                code: "not_supported".to_string(),
                message: message.into(),
                details,
                retryable: Some(false),
            },
        }
    }

    fn not_ready(message: impl Into<String>, details: Option<Value>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            payload: ErrorPayload {
                code: "not_ready".to_string(),
                message: message.into(),
                details,
                retryable: Some(true),
            },
        }
    }

    fn runtime_unavailable(message: impl Into<String>, details: Option<Value>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            payload: ErrorPayload {
                code: "runtime_unavailable".to_string(),
                message: message.into(),
                details,
                retryable: Some(true),
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(ErrorEnvelope {
                error: self.payload,
            }),
        )
            .into_response()
    }
}

async fn root() -> Json<RootPayload> {
    Json(RootPayload {
        name: "fauni-search",
        status: "workspace",
        stage: "search workspace",
        routes: vec![
            "GET /health",
            "GET /libraries",
            "POST /libraries",
            "POST /libraries/{library_id}/imports",
            "GET /libraries/{library_id}/video-sources",
            "POST /libraries/{library_id}/query-assets/images",
            "POST /libraries/{library_id}/query-assets/videos",
            "GET /libraries/{library_id}/video-sources/{source_id}/preview",
            "GET /libraries/{library_id}/visual-units/{visual_unit_id}",
            "GET /libraries/{library_id}/query-assets/images/{temp_asset_id}/preview",
            "GET /libraries/{library_id}/query-assets/videos/{temp_asset_id}/preview",
            "GET /jobs",
            "GET /jobs/{job_id}",
            "POST /search/text",
            "POST /search/image",
            "POST /search/video",
        ],
    })
}

async fn health(State(state): State<SharedState>) -> Json<HealthPayload> {
    let state = state.read().await;
    Json(HealthPayload {
        service: "app",
        status: "ok",
        env: std::env::var("FAUNI_ENV").unwrap_or_else(|_| "development".to_string()),
        libraries: state.libraries.len(),
        jobs: state.jobs.len(),
    })
}

async fn list_libraries(
    State(state): State<SharedState>,
) -> Json<SuccessEnvelope<LibrariesListData>> {
    let state = state.read().await;
    Json(SuccessEnvelope {
        data: state.list_libraries(),
    })
}

async fn get_library(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<LibrarySnapshot>>, ApiError> {
    let state = state.read().await;
    let snapshot = state.get_library(&library_id)?;
    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn create_library(
    State(state): State<SharedState>,
    Json(request): Json<CreateLibraryRequest>,
) -> Result<(StatusCode, Json<SuccessEnvelope<LibrarySnapshot>>), ApiError> {
    let mut state = state.write().await;
    let snapshot = state.create_library(request)?;
    Ok((
        StatusCode::CREATED,
        Json(SuccessEnvelope { data: snapshot }),
    ))
}

async fn import_paths(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
    Json(request): Json<ImportPathsRequest>,
) -> Result<Json<SuccessEnvelope<ImportPathsData>>, ApiError> {
    let (prepared, response) = {
        let mut state = state.write().await;
        let prepared = state.prepare_import(&library_id, request)?;
        let response = state.queue_import(&prepared)?;
        (prepared, response)
    };

    if let Some(job_id) = response.job_handle.clone() {
        let state = state.clone();
        tokio::spawn(async move {
            run_import_job(state, job_id, prepared).await;
        });
    }

    Ok(Json(SuccessEnvelope { data: response }))
}

async fn list_video_sources(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<VideoSourcesData>>, ApiError> {
    let state = state.read().await;
    let data = state.list_video_sources(&library_id)?;
    Ok(Json(SuccessEnvelope { data }))
}

async fn upload_query_image(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<SuccessEnvelope<QueryImageAssetData>>), ApiError> {
    let file = read_single_query_image_part(&mut multipart).await?;
    let staged = persist_query_image_asset(file)?;
    let data = {
        let mut state = state.write().await;
        state.register_temp_query_asset(&library_id, staged)?
    };

    Ok((StatusCode::CREATED, Json(SuccessEnvelope { data })))
}

async fn upload_query_video(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<SuccessEnvelope<QueryVideoAssetData>>), ApiError> {
    let file = read_single_query_video_part(&mut multipart).await?;
    let staged = persist_query_video_asset(file)?;
    let data = {
        let mut state = state.write().await;
        state.register_temp_query_video_asset(&library_id, staged)?
    };

    Ok((StatusCode::CREATED, Json(SuccessEnvelope { data })))
}

async fn get_visual_unit(
    State(state): State<SharedState>,
    Path((library_id, visual_unit_id)): Path<(String, String)>,
) -> Result<Json<SuccessEnvelope<VisualUnitDetailData>>, ApiError> {
    let state = state.read().await;
    let snapshot = state.get_visual_unit(&library_id, &visual_unit_id)?;
    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn get_visual_unit_preview(
    State(state): State<SharedState>,
    Path((library_id, visual_unit_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let visual_unit = {
        let state = state.read().await;
        let library = state
            .libraries
            .get(&library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;
        library
            .visual_units
            .get(&visual_unit_id)
            .cloned()
            .ok_or_else(|| ApiError::not_found("Visual unit was not found."))?
    };

    let bytes = fs::read(&visual_unit.source_path)
        .map_err(|_| ApiError::not_found("Preview source file is not available."))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type_for_visual_unit(&visual_unit)),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );

    Ok((headers, bytes))
}

async fn get_query_image_preview(
    State(state): State<SharedState>,
    Path((library_id, temp_asset_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let asset = {
        let mut state = state.write().await;
        state.prune_temp_query_assets();
        state.get_temp_query_asset(&library_id, &temp_asset_id)?
    };

    let bytes = fs::read(&asset.path)
        .map_err(|_| ApiError::not_found("Query image file is no longer available."))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&asset.content_type).map_err(|_| {
            ApiError::runtime_unavailable(
                "Query image preview content type is invalid.",
                Some(json!({ "temp_asset_id": temp_asset_id })),
            )
        })?,
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );

    Ok((headers, bytes))
}

async fn get_query_video_preview(
    State(state): State<SharedState>,
    Path((library_id, temp_asset_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let asset = {
        let mut state = state.write().await;
        state.prune_temp_query_assets();
        state.get_temp_query_video_asset(&library_id, &temp_asset_id)?
    };

    let bytes = fs::read(&asset.path)
        .map_err(|_| ApiError::not_found("Query video file is no longer available."))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&asset.content_type).map_err(|_| {
            ApiError::runtime_unavailable(
                "Query video preview content type is invalid.",
                Some(json!({ "temp_asset_id": temp_asset_id })),
            )
        })?,
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );

    Ok((headers, bytes))
}

async fn get_video_source_preview(
    State(state): State<SharedState>,
    Path((library_id, source_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let source = {
        let state = state.read().await;
        state.get_library_source(&library_id, &source_id)?
    };

    let bytes = fs::read(&source.source_path)
        .map_err(|_| ApiError::not_found("Video source file is no longer available."))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type_for_source(&source)),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );

    Ok((headers, bytes))
}

async fn list_jobs(
    State(state): State<SharedState>,
    Query(query): Query<JobsQuery>,
) -> Json<SuccessEnvelope<JobsListData>> {
    let state = state.read().await;
    Json(SuccessEnvelope {
        data: state.list_jobs(query.library_id.as_deref()),
    })
}

async fn get_job(
    State(state): State<SharedState>,
    Path(job_id): Path<String>,
) -> Result<Json<SuccessEnvelope<JobSnapshot>>, ApiError> {
    let state = state.read().await;
    let snapshot = state.get_job(&job_id)?;
    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn search_text(
    State(state): State<SharedState>,
    Json(request): Json<TextSearchRequest>,
) -> Result<Json<SuccessEnvelope<TextSearchData>>, ApiError> {
    let plan = {
        let state = state.read().await;
        state.prepare_text_search(&request)?
    };

    let query_embedding = embed_query_text(request.text.trim()).await?;
    let candidates = query_qdrant(&plan, &query_embedding).await?;
    let response = build_search_response(plan, query_embedding, candidates)?;
    Ok(Json(SuccessEnvelope { data: response }))
}

async fn search_image(
    State(state): State<SharedState>,
    Json(request): Json<ImageSearchRequest>,
) -> Result<Json<SuccessEnvelope<TextSearchData>>, ApiError> {
    let (plan, query_input) = {
        let mut state = state.write().await;
        state.prune_temp_query_assets();
        state.prepare_image_search(&request)?
    };

    let (query_path, query_locator) = match &query_input {
        ResolvedImageQueryInput::TempAsset(asset) => (asset.path.as_str(), None),
        ResolvedImageQueryInput::LibraryVisualUnit(visual_unit) => (
            visual_unit.source_path.as_str(),
            Some(visual_unit.locator.clone()),
        ),
    };
    let query_embedding = embed_query_image(query_path, query_locator).await?;
    let candidates = query_qdrant(&plan, &query_embedding).await?;
    let response = build_search_response(plan, query_embedding, candidates)?;
    Ok(Json(SuccessEnvelope { data: response }))
}

async fn search_video(
    State(state): State<SharedState>,
    Json(request): Json<VideoSearchRequest>,
) -> Result<Json<SuccessEnvelope<TextSearchData>>, ApiError> {
    let (plan, query_input) = {
        let mut state = state.write().await;
        state.prune_temp_query_assets();
        state.prepare_video_search(&request)?
    };

    let query_embedding =
        embed_query_video(query_input.path.as_str(), query_input.locator.clone()).await?;
    let candidates = query_qdrant(&plan, &query_embedding).await?;
    let response = build_search_response(plan, query_embedding, candidates)?;
    Ok(Json(SuccessEnvelope { data: response }))
}

async fn run_import_job(state: SharedState, job_id: String, prepared: PreparedImport) {
    {
        let mut state = state.write().await;
        state.update_job_snapshot(
            &job_id,
            "running",
            "encode",
            0,
            format!(
                "Encoding {} accepted path(s) into multivector embeddings.",
                prepared.accepted.len()
            ),
        );
    }

    let outcome = match index_visual_units(&prepared, state.clone(), &job_id).await {
        Ok(summary) => ImportJobOutcome::completed(summary, prepared.accepted.len()),
        Err(error) => ImportJobOutcome::failed(error.phase, error.message, error.completed),
    };

    let mut state = state.write().await;
    if let Err(message) = state.finalize_import_job(&job_id, prepared, outcome) {
        tracing::warn!("Failed to finalize import job {job_id}: {message}");
    }
}

fn normalize_index_lines(lines: Option<Vec<String>>) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for line in lines.unwrap_or_default() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            unique.insert(trimmed.to_string());
        }
    }
    unique.into_iter().collect()
}

fn read_string_filter(filters: Option<&Value>, key: &str) -> Option<BTreeSet<String>> {
    let value = filters?.get(key)?;
    match value {
        Value::String(item) => Some(BTreeSet::from([item.clone()])),
        Value::Array(items) => {
            let values = items
                .iter()
                .filter_map(|item| item.as_str().map(|text| text.to_string()))
                .collect::<BTreeSet<_>>();
            (!values.is_empty()).then_some(values)
        }
        _ => None,
    }
}

async fn read_single_query_image_part(
    multipart: &mut Multipart,
) -> Result<IncomingQueryImageUpload, ApiError> {
    let mut file_upload: Option<IncomingQueryImageUpload> = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        ApiError::validation_failed(
            format!("Query image upload could not be parsed: {error}"),
            Some(json!({ "field": "file" })),
        )
    })? {
        let filename = field.file_name().map(|value| value.to_string());
        let content_type = field
            .content_type()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let extension = infer_query_image_extension(filename.as_deref(), &content_type)
            .ok_or_else(|| {
                ApiError::validation_failed(
                    "Only common image files are accepted as query images right now.",
                    Some(json!({
                        "field": "file",
                        "content_type": content_type,
                        "filename": filename,
                    })),
                )
            })?;
        let bytes = field.bytes().await.map_err(|error| {
            ApiError::validation_failed(
                format!("Query image upload body could not be read: {error}"),
                Some(json!({ "field": "file" })),
            )
        })?;

        if bytes.is_empty() {
            return Err(ApiError::validation_failed(
                "Query image upload must not be empty.",
                Some(json!({ "field": "file" })),
            ));
        }
        if file_upload.is_some() {
            return Err(ApiError::validation_failed(
                "Current 110-image-search implementation accepts exactly one query image per upload.",
                Some(json!({ "field": "file" })),
            ));
        }

        file_upload = Some(IncomingQueryImageUpload {
            bytes: bytes.to_vec(),
            content_type,
            original_filename: filename,
            extension,
        });
    }

    file_upload.ok_or_else(|| {
        ApiError::validation_failed(
            "Query image upload requires one image file part.",
            Some(json!({ "field": "file" })),
        )
    })
}

async fn read_single_query_video_part(
    multipart: &mut Multipart,
) -> Result<IncomingQueryVideoUpload, ApiError> {
    let mut file_upload: Option<IncomingQueryVideoUpload> = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        ApiError::validation_failed(
            format!("Query video upload could not be parsed: {error}"),
            Some(json!({ "field": "file" })),
        )
    })? {
        let filename = field.file_name().map(|value| value.to_string());
        let content_type = field
            .content_type()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let extension = infer_query_video_extension(filename.as_deref(), &content_type)
            .ok_or_else(|| {
                ApiError::validation_failed(
                    "Only mp4, mov, or m4v files are accepted as query videos right now.",
                    Some(json!({
                        "field": "file",
                        "content_type": content_type,
                        "filename": filename,
                    })),
                )
            })?;
        let bytes = field.bytes().await.map_err(|error| {
            ApiError::validation_failed(
                format!("Query video upload body could not be read: {error}"),
                Some(json!({ "field": "file" })),
            )
        })?;

        if bytes.is_empty() {
            return Err(ApiError::validation_failed(
                "Query video upload must not be empty.",
                Some(json!({ "field": "file" })),
            ));
        }
        if file_upload.is_some() {
            return Err(ApiError::validation_failed(
                "Current 120-video-search implementation accepts exactly one query video per upload.",
                Some(json!({ "field": "file" })),
            ));
        }

        file_upload = Some(IncomingQueryVideoUpload {
            bytes: bytes.to_vec(),
            content_type,
            original_filename: filename,
            extension,
        });
    }

    file_upload.ok_or_else(|| {
        ApiError::validation_failed(
            "Query video upload requires one video file part.",
            Some(json!({ "field": "file" })),
        )
    })
}

fn infer_query_image_extension(filename: Option<&str>, content_type: &str) -> Option<String> {
    let by_filename = filename
        .and_then(|name| FsPath::new(name).extension())
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| is_supported_query_image_extension(value));
    if by_filename.is_some() {
        return by_filename;
    }

    match content_type {
        "image/png" => Some("png".to_string()),
        "image/jpeg" => Some("jpg".to_string()),
        "image/webp" => Some("webp".to_string()),
        "image/bmp" => Some("bmp".to_string()),
        "image/gif" => Some("gif".to_string()),
        _ => None,
    }
}

fn infer_query_video_extension(filename: Option<&str>, content_type: &str) -> Option<String> {
    let by_filename = filename
        .and_then(|name| FsPath::new(name).extension())
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| is_supported_query_video_extension(value));
    if by_filename.is_some() {
        return by_filename;
    }

    match content_type {
        "video/mp4" | "video/quicktime" => Some("mp4".to_string()),
        "video/x-m4v" => Some("m4v".to_string()),
        _ => None,
    }
}

fn is_supported_query_image_extension(extension: &str) -> bool {
    matches!(extension, "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif")
}

fn is_supported_query_video_extension(extension: &str) -> bool {
    matches!(extension, "mp4" | "mov" | "m4v")
}

fn persist_query_image_asset(
    upload: IncomingQueryImageUpload,
) -> Result<StagedQueryAsset, ApiError> {
    let runtime_dir = read_required_env("APP_RUNTIME_DIR")?;
    let target_dir = FsPath::new(&runtime_dir).join("temp-assets").join("images");
    fs::create_dir_all(&target_dir).map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Query image asset directory could not be created: {error}"),
            Some(json!({ "path": target_dir })),
        )
    })?;

    let filename = format!("query-image-{}.{}", runtime_token(), upload.extension);
    let path = target_dir.join(filename);
    fs::write(&path, upload.bytes).map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Query image asset could not be written: {error}"),
            Some(json!({ "path": path })),
        )
    })?;

    Ok(StagedQueryAsset {
        path: path.to_string_lossy().to_string(),
        source_type: "image".to_string(),
        content_type: upload.content_type,
        original_filename: upload.original_filename,
        duration_ms: None,
    })
}

fn persist_query_video_asset(
    upload: IncomingQueryVideoUpload,
) -> Result<StagedQueryAsset, ApiError> {
    let runtime_dir = read_required_env("APP_RUNTIME_DIR")?;
    let target_dir = FsPath::new(&runtime_dir).join("temp-assets").join("videos");
    fs::create_dir_all(&target_dir).map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Query video asset directory could not be created: {error}"),
            Some(json!({ "path": target_dir })),
        )
    })?;

    let filename = format!("query-video-{}.{}", runtime_token(), upload.extension);
    let path = target_dir.join(filename);
    fs::write(&path, upload.bytes).map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Query video asset could not be written: {error}"),
            Some(json!({ "path": path })),
        )
    })?;

    let duration_ms = video_duration_ms(&path).map_err(|message| {
        remove_temp_query_asset_file(path.to_string_lossy().as_ref());
        ApiError::validation_failed(
            message,
            Some(json!({ "field": "file" })),
        )
    })?;

    Ok(StagedQueryAsset {
        path: path.to_string_lossy().to_string(),
        source_type: "video".to_string(),
        content_type: upload.content_type,
        original_filename: upload.original_filename,
        duration_ms: Some(duration_ms),
    })
}

fn pdf_page_count(path: &FsPath) -> Result<usize, String> {
    let document =
        PdfDocument::load(path).map_err(|error| format!("PDF could not be opened: {error}"))?;
    let page_count = document.get_pages().len();
    if page_count == 0 {
        return Err("PDF has no pages.".to_string());
    }
    Ok(page_count)
}

fn video_duration_ms(path: &FsPath) -> Result<u64, String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(path)
        .output()
        .map_err(|error| format!("Video metadata could not be probed: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            "unknown ffprobe error".to_string()
        } else {
            stderr
        };
        return Err(format!("Video metadata could not be probed: {detail}"));
    }

    let duration_text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let duration_secs = duration_text
        .parse::<f64>()
        .map_err(|_| format!("Video duration was invalid: {duration_text}"))?;
    Ok((duration_secs * 1000.0).round().max(1.0) as u64)
}

fn build_video_segment_ranges(duration_ms: u64) -> Vec<(u64, u64)> {
    let duration_ms = duration_ms.max(1);
    let mut ranges = Vec::new();
    let step_ms = VIDEO_SEGMENT_WINDOW_MS.saturating_sub(VIDEO_SEGMENT_OVERLAP_MS).max(1);
    let mut start_ms = 0;

    loop {
        let end_ms = (start_ms + VIDEO_SEGMENT_WINDOW_MS).min(duration_ms);
        ranges.push((start_ms, end_ms.max(start_ms + 1)));
        if end_ms >= duration_ms {
            break;
        }
        start_ms += step_ms;
    }

    ranges
}

fn resolve_video_query_locator(
    locator: Option<&Value>,
    duration_ms: Option<u64>,
    field_name: &str,
) -> Result<Option<Value>, ApiError> {
    let duration_ms = duration_ms.ok_or_else(|| {
        ApiError::runtime_unavailable(
            "Video duration is unavailable for the selected query input.",
            Some(json!({ "field": field_name })),
        )
    })?;

    let Some(locator) = locator else {
        return Ok(None);
    };
    let start_ms = locator.get("start_ms").and_then(Value::as_u64).ok_or_else(|| {
        ApiError::validation_failed(
            "Video locator must include integer start_ms.",
            Some(json!({ "field": format!("{field_name}.start_ms") })),
        )
    })?;
    let end_ms = locator.get("end_ms").and_then(Value::as_u64).ok_or_else(|| {
        ApiError::validation_failed(
            "Video locator must include integer end_ms.",
            Some(json!({ "field": format!("{field_name}.end_ms") })),
        )
    })?;
    if start_ms >= end_ms || end_ms > duration_ms {
        return Err(ApiError::validation_failed(
            "Video locator must satisfy 0 <= start_ms < end_ms <= duration_ms.",
            Some(json!({
                "field": field_name,
                "start_ms": start_ms,
                "end_ms": end_ms,
                "duration_ms": duration_ms,
            })),
        ));
    }

    Ok(Some(json!({
        "start_ms": start_ms,
        "end_ms": end_ms,
        "duration_ms": duration_ms,
    })))
}

async fn index_visual_units(
    prepared: &PreparedImport,
    state: SharedState,
    job_id: &str,
) -> Result<String, IndexingError> {
    let embeddings = embed_documents(&prepared.visual_units).await?;
    {
        let mut state = state.write().await;
        state.update_job_snapshot(
            job_id,
            "running",
            "stage_write",
            0,
            format!(
                "Writing {} visual unit(s) into the active multivector collection.",
                prepared.visual_units.len()
            ),
        );
    }

    ensure_qdrant_collection(&prepared.collection_name, embeddings[0].vectors[0].len())
        .await
        .map_err(|message| IndexingError {
            phase: "stage_write",
            message,
            completed: prepared.accepted.len(),
        })?;
    upsert_qdrant_points(
        &prepared.collection_name,
        &prepared.visual_units,
        &embeddings,
    )
    .await
    .map_err(|message| IndexingError {
        phase: "stage_write",
        message,
        completed: prepared.accepted.len(),
    })?;

    Ok(format!(
        "Accepted {} path(s); indexed {} visual unit(s) into the active multivector collection.",
        prepared.accepted.len(),
        prepared.visual_units.len()
    ))
}

async fn embed_documents(
    visual_units: &[VisualUnitRecord],
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
    let payload = json!({
        "operation_kind": "document_embedding",
        "inputs": {
            "documents": documents,
        },
    });

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

async fn embed_query_text(text: &str) -> Result<QueryEmbeddingResult, ApiError> {
    let payload = json!({
        "operation_kind": "query_embedding",
        "inputs": {
            "queries": [text],
        },
    });
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

async fn embed_query_image(
    path: &str,
    locator: Option<Value>,
) -> Result<QueryEmbeddingResult, ApiError> {
    let payload = json!({
        "operation_kind": "image_query_embedding",
        "inputs": {
            "images": [{
                "path": path,
                "locator": locator,
            }],
        },
    });
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

async fn embed_query_video(
    path: &str,
    locator: Option<Value>,
) -> Result<QueryEmbeddingResult, ApiError> {
    let payload = json!({
        "operation_kind": "video_query_embedding",
        "inputs": {
            "videos": [{
                "path": path,
                "locator": locator,
            }],
        },
    });
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

async fn ensure_qdrant_collection(collection_name: &str, vector_size: usize) -> Result<(), String> {
    let base_url = qdrant_base_url().map_err(|error| error.payload.message)?;
    let client = qdrant_client();
    let collection_url = format!("{}/collections/{}", base_url, collection_name);
    let response = client
        .get(&collection_url)
        .send()
        .await
        .map_err(|error| format!("Qdrant collection probe failed: {error}"))?;

    if response.status().is_success() {
        return Ok(());
    }
    if response.status() != StatusCode::NOT_FOUND {
        return Err(format!(
            "Qdrant collection probe for {collection_name} failed with {}.",
            response.status()
        ));
    }

    let payload = json!({
        "vectors": {
            "mv": {
                "size": vector_size,
                "distance": "Cosine",
                "multivector_config": {
                    "comparator": "max_sim"
                }
            },
            "prefetch_dense": {
                "size": vector_size,
                "distance": "Cosine"
            }
        }
    });
    let create_response = client
        .put(&collection_url)
        .json(&payload)
        .send()
        .await
        .map_err(|error| format!("Qdrant collection creation failed: {error}"))?;

    if create_response.status().is_success() {
        Ok(())
    } else {
        Err(format!(
            "Qdrant collection creation for {collection_name} failed with {}.",
            create_response.status()
        ))
    }
}

async fn upsert_qdrant_points(
    collection_name: &str,
    visual_units: &[VisualUnitRecord],
    embeddings: &[SidecarEmbeddingItem],
) -> Result<(), String> {
    let points: Vec<_> = visual_units
        .iter()
        .zip(embeddings.iter())
        .map(build_qdrant_point)
        .collect();
    let point_chunks = chunk_qdrant_points(points, QDRANT_MAX_UPSERT_BODY_BYTES)?;
    let total_chunks = point_chunks.len();

    for (chunk_index, points_chunk) in point_chunks.into_iter().enumerate() {
        let response = qdrant_client()
            .put(format!(
                "{}/collections/{}/points?wait=true",
                qdrant_base_url().map_err(|error| error.payload.message)?,
                collection_name
            ))
            .json(&json!({ "points": points_chunk }))
            .send()
            .await
            .map_err(|error| {
                format!(
                    "Qdrant upsert request for {collection_name} chunk {}/{} failed: {error}",
                    chunk_index + 1,
                    total_chunks
                )
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let detail = qdrant_error_detail(&body);
            return Err(format!(
                "Qdrant upsert for {collection_name} chunk {}/{} failed with {}: {}.",
                chunk_index + 1,
                total_chunks,
                status,
                detail
            ));
        }
    }

    Ok(())
}

fn build_qdrant_point(
    (visual_unit, embedding): (&VisualUnitRecord, &SidecarEmbeddingItem),
) -> Value {
    json!({
        "id": visual_unit.point_id,
        "vector": {
            "mv": embedding.vectors,
            "prefetch_dense": embedding.pooled_vector,
        },
        "payload": {
            "visual_unit_id": visual_unit.id,
            "source_id": visual_unit.source_id,
            "source_path": visual_unit.source_path,
            "source_type": visual_unit.source_type,
            "kind": visual_unit.kind,
            "locator": visual_unit.locator,
        }
    })
}

fn chunk_qdrant_points(
    points: Vec<Value>,
    max_body_bytes: usize,
) -> Result<Vec<Vec<Value>>, String> {
    if max_body_bytes <= QDRANT_UPSERT_BODY_OVERHEAD_BYTES {
        return Err(
            "Qdrant upsert body limit must be larger than the request envelope.".to_string(),
        );
    }

    let mut chunks: Vec<Vec<Value>> = Vec::new();
    let mut current_chunk: Vec<Value> = Vec::new();
    let mut current_size = QDRANT_UPSERT_BODY_OVERHEAD_BYTES;

    for point in points {
        let point_size = serde_json::to_vec(&point)
            .map_err(|error| format!("Failed to serialize Qdrant point payload: {error}"))?
            .len();
        let separator_size = usize::from(!current_chunk.is_empty());
        let next_size = current_size + separator_size + point_size;

        if !current_chunk.is_empty() && next_size > max_body_bytes {
            chunks.push(current_chunk);
            current_chunk = Vec::new();
            current_size = QDRANT_UPSERT_BODY_OVERHEAD_BYTES;
        }

        current_size += usize::from(!current_chunk.is_empty()) + point_size;
        current_chunk.push(point);
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    Ok(chunks)
}

fn qdrant_error_detail(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "empty response body".to_string();
    }

    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
        if let Some(error) = parsed
            .pointer("/status/error")
            .and_then(Value::as_str)
            .or_else(|| parsed.get("error").and_then(Value::as_str))
        {
            return error.to_string();
        }
    }

    trimmed.to_string()
}

async fn query_qdrant(
    plan: &SearchPlan,
    embedding: &QueryEmbeddingResult,
) -> Result<Vec<QdrantScoredPoint>, ApiError> {
    let prefetch_limit = (plan.top_k.saturating_mul(10)).max(20);
    let payload = json!({
        "prefetch": {
            "query": embedding.pooled_vector,
            "using": "prefetch_dense",
            "limit": prefetch_limit,
        },
        "query": embedding.vectors,
        "using": "mv",
        "limit": prefetch_limit,
        "with_payload": true,
    });
    let response = qdrant_client()
        .post(format!(
            "{}/collections/{}/points/query",
            qdrant_base_url()?.trim_end_matches('/'),
            plan.collection_name
        ))
        .json(&payload)
        .send()
        .await
        .map_err(|error| {
            ApiError::runtime_unavailable(
                format!("Qdrant query request failed: {error}"),
                Some(json!({ "service": "qdrant" })),
            )
        })?;

    if !response.status().is_success() {
        return Err(ApiError::runtime_unavailable(
            format!("Qdrant query request failed with {}.", response.status()),
            Some(json!({ "service": "qdrant" })),
        ));
    }

    let parsed: QdrantQueryResponse = response.json().await.map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Qdrant query response was invalid JSON: {error}"),
            Some(json!({ "service": "qdrant" })),
        )
    })?;
    Ok(parsed.result.points)
}

fn build_search_response(
    plan: SearchPlan,
    embedding: QueryEmbeddingResult,
    candidates: Vec<QdrantScoredPoint>,
) -> Result<TextSearchData, ApiError> {
    let result_count = candidates.len();
    let top_score = candidates.first().map(|point| point.score);
    let results = candidates
        .into_iter()
        .filter_map(|point| point.payload.map(|payload| (point.score, payload)))
        .filter(|(_, payload)| {
            plan.kind_filter
                .as_ref()
                .map(|expected| expected.contains(&payload.kind))
                .unwrap_or(true)
                && plan
                    .source_type_filter
                    .as_ref()
                    .map(|expected| expected.contains(&payload.source_type))
                    .unwrap_or(true)
        })
        .take(plan.top_k)
        .map(|(score, payload)| {
            let preview = visual_unit_preview_reference(
                &plan.library_id,
                &payload.visual_unit_id,
                &payload.kind,
                &payload.locator,
            )?;
            Ok(SearchResultItem {
                visual_unit_id: payload.visual_unit_id.clone(),
                source_id: payload.source_id,
                preview,
                source_path: payload.source_path,
                source_type: payload.source_type,
                kind: payload.kind,
                locator: payload.locator,
                cursor: format!("cursor:{}", payload.visual_unit_id),
                score: Some(score),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;

    Ok(TextSearchData {
        results,
        next_cursor: None,
        debug: plan.debug.then_some(json!({
            "backend": "qdrant",
            "repr_kind": "multivector",
            "query_vector_count": embedding.vectors.len(),
            "retrieved_points": result_count,
            "top_score": top_score,
        })),
    })
}

fn mean_pool_vectors(vectors: &[Vec<f32>]) -> Option<Vec<f32>> {
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

fn visual_unit_preview_reference(
    library_id: &str,
    visual_unit_id: &str,
    kind: &str,
    locator: &Value,
) -> Result<PreviewReference, ApiError> {
    let base = format!(
        "{}/libraries/{}/visual-units/{}/preview",
        app_base_url()?.trim_end_matches('/'),
        library_id,
        visual_unit_id
    );
    let url = if kind == "document_page" {
        let page = locator.get("page").and_then(Value::as_u64).unwrap_or(1);
        format!("{base}#page={page}&view=FitH")
    } else if kind == "video_segment" {
        let start_seconds = locator
            .get("start_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0) as f64
            / 1000.0;
        let end_seconds = locator
            .get("end_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0) as f64
            / 1000.0;
        format!("{base}#t={start_seconds:.3},{end_seconds:.3}")
    } else {
        base
    };
    Ok(PreviewReference {
        url,
        handle: Some(format!("preview:{visual_unit_id}")),
    })
}

fn query_video_preview_reference(
    library_id: &str,
    temp_asset_id: &str,
) -> Result<PreviewReference, ApiError> {
    Ok(PreviewReference {
        url: format!(
            "{}/libraries/{}/query-assets/videos/{}/preview",
            app_base_url()?.trim_end_matches('/'),
            library_id,
            temp_asset_id
        ),
        handle: Some(format!("query-video-preview:{temp_asset_id}")),
    })
}

fn video_source_preview_reference(
    library_id: &str,
    source_id: &str,
) -> Result<PreviewReference, ApiError> {
    Ok(PreviewReference {
        url: format!(
            "{}/libraries/{}/video-sources/{}/preview",
            app_base_url()?.trim_end_matches('/'),
            library_id,
            source_id
        ),
        handle: Some(format!("video-source-preview:{source_id}")),
    })
}

fn query_image_preview_reference(
    library_id: &str,
    temp_asset_id: &str,
) -> Result<PreviewReference, ApiError> {
    Ok(PreviewReference {
        url: format!(
            "{}/libraries/{}/query-assets/images/{}/preview",
            app_base_url()?.trim_end_matches('/'),
            library_id,
            temp_asset_id
        ),
        handle: Some(format!("query-image-preview:{temp_asset_id}")),
    })
}

fn content_type_for_visual_unit(visual_unit: &VisualUnitRecord) -> &'static str {
    content_type_for_source_type_and_path(&visual_unit.source_type, &visual_unit.source_path)
}

fn content_type_for_source(source: &SourceRecord) -> &'static str {
    content_type_for_source_type_and_path(&source.source_type, &source.source_path)
}

fn content_type_for_source_type_and_path(source_type: &str, source_path: &str) -> &'static str {
    match source_type {
        "pdf" => "application/pdf",
        "video" => match FsPath::new(source_path)
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref()
        {
            Some("mov") => "video/quicktime",
            Some("m4v") => "video/x-m4v",
            _ => "video/mp4",
        },
        _ => match FsPath::new(source_path)
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .as_deref()
        {
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("webp") => "image/webp",
            Some("gif") => "image/gif",
            Some("bmp") => "image/bmp",
            _ => "image/png",
        },
    }
}

fn runtime_token() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn remove_temp_query_asset_file(path: &str) {
    if let Err(error) = fs::remove_file(path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!("Failed to remove expired query image asset file {path}: {error}");
        }
    }
}

fn read_required_env(name: &'static str) -> Result<String, ApiError> {
    env::var(name).map_err(|_| {
        ApiError::runtime_unavailable(
            format!("Missing required environment variable {name}; source .env or use scripts/local/run.sh"),
            Some(json!({ "field": name })),
        )
    })
}

fn sidecar_base_url() -> Result<String, ApiError> {
    Ok(format!(
        "http://{}:{}",
        read_required_env("SIDECAR_HOST")?,
        read_required_env("SIDECAR_PORT")?,
    ))
}

fn app_base_url() -> Result<String, ApiError> {
    Ok(format!(
        "http://{}:{}",
        read_required_env("APP_HOST")?,
        read_required_env("APP_PORT")?,
    ))
}

fn qdrant_base_url() -> Result<String, ApiError> {
    read_required_env("QDRANT_URL")
}

fn sidecar_client() -> Client {
    Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(SIDECAR_REQUEST_TIMEOUT_SECS))
        .build()
        .expect("sidecar client should be constructible")
}

fn qdrant_client() -> Client {
    Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("qdrant client should be constructible")
}

fn parse_sidecar_error_message(body: &str) -> Option<String> {
    serde_json::from_str::<SidecarErrorEnvelope>(body)
        .ok()
        .map(|envelope| {
            format!(
                "Sidecar {}: {}",
                envelope.error.code, envelope.error.message
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Document, Object, Stream};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn create_library_requires_multivector_only() {
        let mut state = AppState::default();

        let error = state
            .create_library(CreateLibraryRequest {
                name: "demo".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["single-vector".to_string()],
                }),
            })
            .unwrap_err();

        assert_eq!(error.payload.code, "validation_failed");

        let snapshot = state
            .create_library(CreateLibraryRequest {
                name: "demo".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();

        assert_eq!(snapshot.id, "lib_000001");
        assert_eq!(snapshot.index_lines[0].status, "not_ready");
    }

    #[test]
    fn import_paths_partially_accepts_and_queues_a_job() {
        set_test_app_env();
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "imports".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();

        let pdf_path = unique_test_file_path("fixture.pdf");
        let txt_path = unique_test_file_path("fixture.txt");
        write_test_pdf(&pdf_path, 2);
        fs::write(&txt_path, b"nope").unwrap();

        let prepared = state
            .prepare_import(
                &library.id,
                ImportPathsRequest {
                    paths: vec![
                        pdf_path.to_string_lossy().to_string(),
                        txt_path.to_string_lossy().to_string(),
                    ],
                },
            )
            .unwrap();

        assert_eq!(prepared.accepted.len(), 1);
        assert_eq!(prepared.rejected.len(), 1);

        let response = state.queue_import(&prepared).unwrap();

        assert_eq!(response.job.as_ref().unwrap().status, "queued");
        assert_eq!(response.job.as_ref().unwrap().phase, "intake");

        let job_id = response.job_handle.clone().unwrap();
        state
            .finalize_import_job(
                &job_id,
                prepared,
                ImportJobOutcome::completed(
                    "Accepted 1 path(s); indexed 2 visual unit(s) into the active multivector collection."
                        .to_string(),
                    1,
                ),
            )
            .unwrap();

        assert_eq!(response.accepted.len(), 1);
        assert_eq!(response.accepted[0].kind, "document_page");
        assert_eq!(response.accepted[0].visual_units.len(), 2);
        assert_eq!(response.accepted[0].visual_units[0].locator["page"], 1);
        assert_eq!(response.accepted[0].visual_units[1].locator["page"], 2);
        assert_eq!(response.rejected.len(), 1);
        assert_eq!(response.rejected[0].reason_code, "unsupported_type");

        let library_snapshot = state.get_library(&library.id).unwrap();
        assert_eq!(library_snapshot.counts.accepted_items, 2);
        assert_eq!(library_snapshot.counts.pending_jobs, 0);
        assert_eq!(library_snapshot.index_lines[0].status, "ready");

        let job_snapshot = state.get_job(&job_id).unwrap();
        assert_eq!(job_snapshot.status, "completed");
        assert_eq!(job_snapshot.phase, "activated");

        let detail = state
            .get_visual_unit(
                &library.id,
                &response.accepted[0].visual_units[0].visual_unit_id,
            )
            .unwrap();
        assert_eq!(detail.visual_unit.kind, "document_page");
        assert_eq!(detail.visual_unit.source_type, "pdf");
        assert_eq!(detail.visual_unit.locator["page"], 1);
        assert!(detail.preview.url.contains(&format!(
            "/libraries/{}/visual-units/{}/preview#page=1&view=FitH",
            library.id, response.accepted[0].visual_units[0].visual_unit_id
        )));
        assert_eq!(detail.neighbor_context["previous_page"], Value::Null);
        assert_eq!(detail.neighbor_context["next_page"], 2);

        let second_detail = state
            .get_visual_unit(
                &library.id,
                &response.accepted[0].visual_units[1].visual_unit_id,
            )
            .unwrap();
        assert_eq!(second_detail.visual_unit.locator["page"], 2);
        assert_eq!(second_detail.neighbor_context["previous_page"], 1);
        assert_eq!(second_detail.neighbor_context["next_page"], Value::Null);
        assert_eq!(second_detail.neighbor_context["total_pages"], 2);

        let _ = fs::remove_file(pdf_path);
        let _ = fs::remove_file(txt_path);
    }

    #[test]
    fn search_returns_not_ready_with_latest_job_details() {
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "search".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();

        let pdf_path = unique_test_file_path("pending.pdf");
        write_test_pdf(&pdf_path, 2);
        let prepared = state
            .prepare_import(
                &library.id,
                ImportPathsRequest {
                    paths: vec![pdf_path.to_string_lossy().to_string()],
                },
            )
            .unwrap();
        let queued = state.queue_import(&prepared).unwrap();

        let error = state
            .prepare_text_search(&TextSearchRequest {
                library_id: library.id.clone(),
                text: "chart".to_string(),
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(false),
                target_index_lines: None,
            })
            .unwrap_err();

        assert_eq!(error.payload.code, "not_ready");
        let details = error.payload.details.unwrap();
        let first_index_line = &details["index_lines"][0];
        assert_eq!(first_index_line["index_line"], MULTIVECTOR_INDEX_LINE);
        assert_eq!(
            first_index_line["job"]["job_id"],
            queued.job_handle.unwrap()
        );
        assert_eq!(first_index_line["job"]["status"], "queued");
        assert_eq!(first_index_line["job"]["phase"], "intake");

        let _ = fs::remove_file(pdf_path);
    }

    #[test]
    fn build_search_response_returns_qdrant_results_after_import() {
        set_test_app_env();
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "ready-search".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();

        let pdf_path = unique_test_file_path("report.pdf");
        let image_path = unique_test_file_path("report-chart.png");
        write_test_pdf(&pdf_path, 2);
        fs::write(&image_path, b"png").unwrap();

        let prepared = state
            .prepare_import(
                &library.id,
                ImportPathsRequest {
                    paths: vec![
                        pdf_path.to_string_lossy().to_string(),
                        image_path.to_string_lossy().to_string(),
                    ],
                },
            )
            .unwrap();

        let image_visual_unit_id = prepared
            .accepted
            .iter()
            .find(|item| item.kind == "image")
            .unwrap()
            .visual_units[0]
            .visual_unit_id
            .clone();
        let document_visual_unit_id = prepared
            .accepted
            .iter()
            .find(|item| item.kind == "document_page")
            .unwrap()
            .visual_units[0]
            .visual_unit_id
            .clone();

        let queued = state.queue_import(&prepared).unwrap();
        let job_id = queued.job_handle.clone().unwrap();
        state
            .finalize_import_job(
                &job_id,
                prepared,
                ImportJobOutcome::completed(
                    "Accepted 2 path(s); indexed 3 visual unit(s) into the active multivector collection."
                        .to_string(),
                    2,
                ),
            )
            .unwrap();

        let plan = state
            .prepare_text_search(&TextSearchRequest {
                library_id: library.id.clone(),
                text: "report".to_string(),
                filters: None,
                top_k: Some(10),
                cursor: None,
                debug: Some(true),
                target_index_lines: None,
            })
            .unwrap();

        let response = build_search_response(
            plan,
            QueryEmbeddingResult {
                vectors: vec![vec![0.1, 0.2, 0.3], vec![0.3, 0.2, 0.1]],
                pooled_vector: vec![0.2, 0.2, 0.2],
            },
            vec![
                QdrantScoredPoint {
                    score: 0.9,
                    payload: Some(QdrantPointPayload {
                        visual_unit_id: image_visual_unit_id,
                        source_id: "src_000002".to_string(),
                        source_path: image_path.to_string_lossy().to_string(),
                        source_type: "image".to_string(),
                        kind: "image".to_string(),
                        locator: json!({ "path": image_path.to_string_lossy().to_string() }),
                    }),
                },
                QdrantScoredPoint {
                    score: 0.8,
                    payload: Some(QdrantPointPayload {
                        visual_unit_id: document_visual_unit_id,
                        source_id: "src_000001".to_string(),
                        source_path: pdf_path.to_string_lossy().to_string(),
                        source_type: "pdf".to_string(),
                        kind: "document_page".to_string(),
                        locator: json!({ "page": 1, "page_label": "1" }),
                    }),
                },
            ],
        )
        .unwrap();

        assert_eq!(response.results.len(), 2);
        assert!(response
            .results
            .iter()
            .any(|item| item.kind == "document_page"));
        assert!(response.results.iter().any(|item| item.kind == "image"));
        assert_eq!(response.results[0].score, Some(0.9));
        assert_eq!(response.results[1].score, Some(0.8));
        assert!(response.results.iter().all(|item| item
            .preview
            .url
            .starts_with("http://127.0.0.1:53210/libraries/")));
        assert_eq!(response.debug.as_ref().unwrap()["repr_kind"], "multivector");

        let _ = fs::remove_file(pdf_path);
        let _ = fs::remove_file(image_path);
    }

    #[test]
    fn prepare_image_search_requires_existing_temp_asset() {
        set_test_app_env();
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "image-search".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .active_index_lines
            .insert(MULTIVECTOR_INDEX_LINE.to_string());

        let image_path = unique_test_file_path("query.png");
        fs::write(&image_path, b"png").unwrap();

        let staged = StagedQueryAsset {
            path: image_path.to_string_lossy().to_string(),
            source_type: "image".to_string(),
            content_type: "image/png".to_string(),
            original_filename: Some("query.png".to_string()),
            duration_ms: None,
        };
        let asset = state
            .register_temp_query_asset(&library.id, staged)
            .unwrap();

        let (plan, temp_asset) = state
            .prepare_image_search(&ImageSearchRequest {
                library_id: library.id.clone(),
                image_input: QueryImageInputRequest {
                    kind: "temp_asset".to_string(),
                    temp_asset_id: Some(asset.temp_asset_id.clone()),
                    visual_unit_id: None,
                },
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(true),
                target_index_lines: None,
            })
            .unwrap();

        assert_eq!(plan.library_id, library.id);
        match temp_asset {
            ResolvedImageQueryInput::TempAsset(temp_asset) => {
                assert_eq!(temp_asset.id, asset.temp_asset_id);
                assert_eq!(temp_asset.path, image_path.to_string_lossy().to_string());
            }
            ResolvedImageQueryInput::LibraryVisualUnit(_) => {
                panic!("expected temp query asset input")
            }
        }

        let missing = state
            .prepare_image_search(&ImageSearchRequest {
                library_id: library.id.clone(),
                image_input: QueryImageInputRequest {
                    kind: "temp_asset".to_string(),
                    temp_asset_id: Some("temp_asset_999999".to_string()),
                    visual_unit_id: None,
                },
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(false),
                target_index_lines: None,
            })
            .unwrap_err();

        assert_eq!(missing.payload.code, "not_found");

        let _ = fs::remove_file(image_path);
    }

    #[test]
    fn prepare_image_search_accepts_library_image_objects() {
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "image-search-library-object".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .active_index_lines
            .insert(MULTIVECTOR_INDEX_LINE.to_string());

        let image_path = unique_test_file_path("library-query.png");
        fs::write(&image_path, b"png").unwrap();
        let classification = state
            .inspect_import_path(&image_path.to_string_lossy())
            .unwrap();
        let visual_unit = state
            .new_visual_units_from_classification(&classification)
            .into_iter()
            .next()
            .unwrap();
        let visual_unit_id = visual_unit.id.clone();
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .visual_units
            .insert(visual_unit.id.clone(), visual_unit.clone());

        let (plan, input) = state
            .prepare_image_search(&ImageSearchRequest {
                library_id: library.id.clone(),
                image_input: QueryImageInputRequest {
                    kind: "library_object".to_string(),
                    temp_asset_id: None,
                    visual_unit_id: Some(visual_unit_id),
                },
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(false),
                target_index_lines: None,
            })
            .unwrap();

        assert_eq!(plan.library_id, library.id);
        match input {
            ResolvedImageQueryInput::LibraryVisualUnit(visual_unit) => {
                assert_eq!(visual_unit.kind, "image");
                assert_eq!(visual_unit.source_path, image_path.to_string_lossy());
            }
            ResolvedImageQueryInput::TempAsset(_) => {
                panic!("expected library visual unit query input")
            }
        }

        let _ = fs::remove_file(image_path);
    }

    #[test]
    fn prepare_image_search_accepts_library_document_page_objects() {
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "document-page-query-object".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .active_index_lines
            .insert(MULTIVECTOR_INDEX_LINE.to_string());

        let pdf_path = unique_test_file_path("library-query-page.pdf");
        write_test_pdf(&pdf_path, 1);
        let classification = state
            .inspect_import_path(&pdf_path.to_string_lossy())
            .unwrap();
        let visual_unit = state
            .new_visual_units_from_classification(&classification)
            .into_iter()
            .next()
            .unwrap();
        let visual_unit_id = visual_unit.id.clone();
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .visual_units
            .insert(visual_unit.id.clone(), visual_unit.clone());

        let (plan, input) = state
            .prepare_image_search(&ImageSearchRequest {
                library_id: library.id.clone(),
                image_input: QueryImageInputRequest {
                    kind: "library_object".to_string(),
                    temp_asset_id: None,
                    visual_unit_id: Some(visual_unit_id),
                },
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(false),
                target_index_lines: None,
            })
            .unwrap();

        assert_eq!(plan.library_id, library.id);
        match input {
            ResolvedImageQueryInput::LibraryVisualUnit(visual_unit) => {
                assert_eq!(visual_unit.kind, "document_page");
                assert_eq!(visual_unit.locator["page"], 1);
            }
            ResolvedImageQueryInput::TempAsset(_) => {
                panic!("expected library visual unit query input")
            }
        }

        let _ = fs::remove_file(pdf_path);
    }

    #[test]
    fn prepare_image_search_rejects_unsupported_library_object_query_images() {
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "unsupported-query-object".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .active_index_lines
            .insert(MULTIVECTOR_INDEX_LINE.to_string());

        let visual_unit_id = "vu_video_000001".to_string();
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .visual_units
            .insert(
                visual_unit_id.clone(),
                VisualUnitRecord {
                    id: visual_unit_id.clone(),
                    point_id: 1,
                    source_id: "src_video_000001".to_string(),
                    source_path: "/tmp/example.mp4".to_string(),
                    source_type: "video".to_string(),
                    kind: "video_segment".to_string(),
                    locator: json!({ "start_ms": 0, "end_ms": 1000 }),
                    neighbor_context: json!({}),
                },
            );

        let error = state
            .prepare_image_search(&ImageSearchRequest {
                library_id: library.id.clone(),
                image_input: QueryImageInputRequest {
                    kind: "library_object".to_string(),
                    temp_asset_id: None,
                    visual_unit_id: Some(visual_unit_id),
                },
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(false),
                target_index_lines: None,
            })
            .unwrap_err();

        assert_eq!(error.payload.code, "not_supported");
        let details = error.payload.details.unwrap();
        assert_eq!(details["supported_kinds"][0], "image");
        assert_eq!(details["supported_kinds"][1], "document_page");
    }

    #[test]
    fn get_temp_query_asset_rejects_expired_assets() {
        set_test_app_env();
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "expired-query-image".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();

        let image_path = unique_test_file_path("expired-query.png");
        fs::write(&image_path, b"png").unwrap();
        let staged = StagedQueryAsset {
            path: image_path.to_string_lossy().to_string(),
            source_type: "image".to_string(),
            content_type: "image/png".to_string(),
            original_filename: Some("expired-query.png".to_string()),
            duration_ms: None,
        };
        let asset = state
            .register_temp_query_asset(&library.id, staged)
            .unwrap();
        state
            .temp_query_assets
            .get_mut(&asset.temp_asset_id)
            .unwrap()
            .expires_at_ms = 0;

        let error = state
            .get_temp_query_asset(&library.id, &asset.temp_asset_id)
            .unwrap_err();

        assert_eq!(error.payload.code, "not_found");
        assert_eq!(
            error.payload.message,
            "Query image was not found or has expired."
        );

        let _ = fs::remove_file(image_path);
    }

    #[test]
    fn prune_temp_query_assets_removes_expired_asset_records_and_files() {
        set_test_app_env();
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "prune-expired-query-image".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();

        let image_path = unique_test_file_path("expired-prune-query.png");
        fs::write(&image_path, b"png").unwrap();
        let staged = StagedQueryAsset {
            path: image_path.to_string_lossy().to_string(),
            source_type: "image".to_string(),
            content_type: "image/png".to_string(),
            original_filename: Some("expired-prune-query.png".to_string()),
            duration_ms: None,
        };
        let asset = state
            .register_temp_query_asset(&library.id, staged)
            .unwrap();
        state
            .temp_query_assets
            .get_mut(&asset.temp_asset_id)
            .unwrap()
            .expires_at_ms = 0;

        let summary = state.prune_temp_query_assets();

        assert_eq!(summary.expired_removed, 1);
        assert_eq!(summary.missing_removed, 0);
        assert!(!state.temp_query_assets.contains_key(&asset.temp_asset_id));
        assert!(!image_path.exists());
    }

    #[test]
    fn prune_temp_query_assets_removes_missing_asset_records() {
        set_test_app_env();
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "prune-missing-query-image".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();

        let image_path = unique_test_file_path("missing-prune-query.png");
        fs::write(&image_path, b"png").unwrap();
        let staged = StagedQueryAsset {
            path: image_path.to_string_lossy().to_string(),
            source_type: "image".to_string(),
            content_type: "image/png".to_string(),
            original_filename: Some("missing-prune-query.png".to_string()),
            duration_ms: None,
        };
        let asset = state
            .register_temp_query_asset(&library.id, staged)
            .unwrap();
        fs::remove_file(&image_path).unwrap();

        let summary = state.prune_temp_query_assets();

        assert_eq!(summary.expired_removed, 0);
        assert_eq!(summary.missing_removed, 1);
        assert!(!state.temp_query_assets.contains_key(&asset.temp_asset_id));
    }

    #[test]
    fn import_paths_accepts_video_and_generates_video_segments() {
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "video-import".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();

        let video_path = unique_test_file_path("fixture.mp4");
        write_test_video(&video_path, 2.5);

        let prepared = state
            .prepare_import(
                &library.id,
                ImportPathsRequest {
                    paths: vec![video_path.to_string_lossy().to_string()],
                },
            )
            .unwrap();

        assert_eq!(prepared.sources.len(), 1);
        assert_eq!(prepared.sources[0].source_type, "video");
        assert_eq!(prepared.accepted.len(), 1);
        assert_eq!(prepared.accepted[0].source_type, "video");
        assert_eq!(prepared.accepted[0].kind, "video_segment");
        assert_eq!(prepared.accepted[0].visual_units.len(), 1);
        assert_eq!(prepared.accepted[0].visual_units[0].source_id, "src_000001");
        assert_eq!(prepared.accepted[0].visual_units[0].locator["start_ms"], 0);
        assert_eq!(prepared.accepted[0].visual_units[0].locator["duration_ms"], 2500);
        assert_eq!(prepared.accepted[0].source_id.as_deref(), Some("src_000001"));

        let _ = fs::remove_file(video_path);
    }

    #[test]
    fn prepare_video_search_accepts_temp_assets_and_library_sources() {
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "video-search".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .active_index_lines
            .insert(MULTIVECTOR_INDEX_LINE.to_string());

        let video_path = unique_test_file_path("query.mp4");
        write_test_video(&video_path, 3.0);
        let staged = StagedQueryAsset {
            path: video_path.to_string_lossy().to_string(),
            source_type: "video".to_string(),
            content_type: "video/mp4".to_string(),
            original_filename: Some("query.mp4".to_string()),
            duration_ms: Some(3000),
        };
        let asset = state
            .register_temp_query_video_asset(&library.id, staged)
            .unwrap();

        let (plan, temp_input) = state
            .prepare_video_search(&VideoSearchRequest {
                library_id: library.id.clone(),
                video_input: QueryVideoInputRequest {
                    kind: "temp_asset".to_string(),
                    temp_asset_id: Some(asset.temp_asset_id.clone()),
                    source_id: None,
                    visual_unit_id: None,
                    locator: None,
                },
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(false),
                target_index_lines: None,
            })
            .unwrap();
        assert_eq!(plan.library_id, library.id);
        assert_eq!(temp_input.path, video_path.to_string_lossy());
        assert!(temp_input.locator.is_none());

        let classification = state
            .inspect_import_path(&video_path.to_string_lossy())
            .unwrap();
        let source = state.source_record_from_classification(&classification);
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .sources
            .insert(source.id.clone(), source.clone());

        let (_, library_input) = state
            .prepare_video_search(&VideoSearchRequest {
                library_id: library.id.clone(),
                video_input: QueryVideoInputRequest {
                    kind: "library_object".to_string(),
                    temp_asset_id: None,
                    source_id: Some(source.id),
                    visual_unit_id: None,
                    locator: Some(json!({ "start_ms": 500, "end_ms": 1500 })),
                },
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(false),
                target_index_lines: None,
            })
            .unwrap();

        assert_eq!(library_input.path, video_path.to_string_lossy());
        assert_eq!(
            library_input.locator.unwrap(),
            json!({ "start_ms": 500, "end_ms": 1500, "duration_ms": 3000 })
        );

        let _ = fs::remove_file(video_path);
    }

    #[test]
    fn prepare_video_search_rejects_invalid_ranges() {
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "video-range-errors".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .active_index_lines
            .insert(MULTIVECTOR_INDEX_LINE.to_string());

        let video_path = unique_test_file_path("invalid-range.mp4");
        write_test_video(&video_path, 2.0);
        let staged = StagedQueryAsset {
            path: video_path.to_string_lossy().to_string(),
            source_type: "video".to_string(),
            content_type: "video/mp4".to_string(),
            original_filename: Some("invalid-range.mp4".to_string()),
            duration_ms: Some(2000),
        };
        let asset = state
            .register_temp_query_video_asset(&library.id, staged)
            .unwrap();

        let error = state
            .prepare_video_search(&VideoSearchRequest {
                library_id: library.id.clone(),
                video_input: QueryVideoInputRequest {
                    kind: "temp_asset".to_string(),
                    temp_asset_id: Some(asset.temp_asset_id),
                    source_id: None,
                    visual_unit_id: None,
                    locator: Some(json!({ "start_ms": 1500, "end_ms": 2500 })),
                },
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(false),
                target_index_lines: None,
            })
            .unwrap_err();

        assert_eq!(error.payload.code, "validation_failed");

        let _ = fs::remove_file(video_path);
    }

    #[test]
    fn prepare_video_search_rejects_expired_temp_assets() {
        set_test_app_env();
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "expired-query-video".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .active_index_lines
            .insert(MULTIVECTOR_INDEX_LINE.to_string());

        let video_path = unique_test_file_path("expired-query.mp4");
        write_test_video(&video_path, 2.0);
        let staged = StagedQueryAsset {
            path: video_path.to_string_lossy().to_string(),
            source_type: "video".to_string(),
            content_type: "video/mp4".to_string(),
            original_filename: Some("expired-query.mp4".to_string()),
            duration_ms: Some(2000),
        };
        let asset = state
            .register_temp_query_video_asset(&library.id, staged)
            .unwrap();
        state
            .temp_query_assets
            .get_mut(&asset.temp_asset_id)
            .unwrap()
            .expires_at_ms = 0;

        let error = state
            .prepare_video_search(&VideoSearchRequest {
                library_id: library.id.clone(),
                video_input: QueryVideoInputRequest {
                    kind: "temp_asset".to_string(),
                    temp_asset_id: Some(asset.temp_asset_id),
                    source_id: None,
                    visual_unit_id: None,
                    locator: None,
                },
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(false),
                target_index_lines: None,
            })
            .unwrap_err();

        assert_eq!(error.payload.code, "not_found");

        let _ = fs::remove_file(video_path);
    }

    #[test]
    fn prepare_video_search_rejects_non_video_library_sources() {
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "unsupported-video-query-source".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .active_index_lines
            .insert(MULTIVECTOR_INDEX_LINE.to_string());

        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .sources
            .insert(
                "src_image_000001".to_string(),
                SourceRecord {
                    id: "src_image_000001".to_string(),
                    source_path: "/tmp/example.png".to_string(),
                    source_type: "image".to_string(),
                    duration_ms: None,
                },
            );

        let error = state
            .prepare_video_search(&VideoSearchRequest {
                library_id: library.id.clone(),
                video_input: QueryVideoInputRequest {
                    kind: "library_object".to_string(),
                    temp_asset_id: None,
                    source_id: Some("src_image_000001".to_string()),
                    visual_unit_id: None,
                    locator: None,
                },
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(false),
                target_index_lines: None,
            })
            .unwrap_err();

        assert_eq!(error.payload.code, "not_supported");
        let details = error.payload.details.unwrap();
        assert_eq!(details["supported_source_type"], "video");
        assert_eq!(details["received_source_type"], "image");
    }

    #[test]
    fn prepare_video_search_accepts_library_video_segments() {
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "video-segment-query".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .active_index_lines
            .insert(MULTIVECTOR_INDEX_LINE.to_string());

        let visual_unit_id = "vu_video_000123".to_string();
        let locator = json!({ "start_ms": 600, "end_ms": 1800, "duration_ms": 3000 });
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .visual_units
            .insert(
                visual_unit_id.clone(),
                VisualUnitRecord {
                    id: visual_unit_id.clone(),
                    point_id: 1,
                    source_id: "src_video_000123".to_string(),
                    source_path: "/tmp/example.mp4".to_string(),
                    source_type: "video".to_string(),
                    kind: "video_segment".to_string(),
                    locator: locator.clone(),
                    neighbor_context: json!({}),
                },
            );

        let (_, input) = state
            .prepare_video_search(&VideoSearchRequest {
                library_id: library.id.clone(),
                video_input: QueryVideoInputRequest {
                    kind: "library_object".to_string(),
                    temp_asset_id: None,
                    source_id: None,
                    visual_unit_id: Some(visual_unit_id),
                    locator: None,
                },
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(false),
                target_index_lines: None,
            })
            .unwrap();

        assert_eq!(input.path, "/tmp/example.mp4");
        assert_eq!(input.locator.unwrap(), locator);
    }

    #[test]
    fn prepare_video_search_rejects_locator_override_for_library_video_segments() {
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "video-segment-query-locator".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .active_index_lines
            .insert(MULTIVECTOR_INDEX_LINE.to_string());

        let visual_unit_id = "vu_video_000124".to_string();
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .visual_units
            .insert(
                visual_unit_id.clone(),
                VisualUnitRecord {
                    id: visual_unit_id.clone(),
                    point_id: 1,
                    source_id: "src_video_000124".to_string(),
                    source_path: "/tmp/example.mp4".to_string(),
                    source_type: "video".to_string(),
                    kind: "video_segment".to_string(),
                    locator: json!({ "start_ms": 600, "end_ms": 1800, "duration_ms": 3000 }),
                    neighbor_context: json!({}),
                },
            );

        let error = state
            .prepare_video_search(&VideoSearchRequest {
                library_id: library.id.clone(),
                video_input: QueryVideoInputRequest {
                    kind: "library_object".to_string(),
                    temp_asset_id: None,
                    source_id: None,
                    visual_unit_id: Some(visual_unit_id),
                    locator: Some(json!({ "start_ms": 0, "end_ms": 1000 })),
                },
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(false),
                target_index_lines: None,
            })
            .unwrap_err();

        assert_eq!(error.payload.code, "validation_failed");
    }

    #[test]
    fn chunk_qdrant_points_splits_large_batches_by_request_size() {
        let point = json!({
            "id": 1,
            "vector": {
                "mv": vec![vec![0.1_f32; 32]; 8],
                "prefetch_dense": vec![0.1_f32; 32],
            },
            "payload": {
                "visual_unit_id": "vu_000001",
                "source_path": "/tmp/demo.png",
                "source_type": "image",
                "kind": "image",
                "locator": { "path": "/tmp/demo.png" },
            }
        });

        let single_size = serde_json::to_vec(&point).unwrap().len();
        let max_body_bytes = QDRANT_UPSERT_BODY_OVERHEAD_BYTES + (single_size * 2) + 1;
        let chunks = chunk_qdrant_points(
            vec![point.clone(), point.clone(), point.clone()],
            max_body_bytes,
        )
        .unwrap();

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 2);
        assert_eq!(chunks[1].len(), 1);

        for chunk in chunks {
            let body_len = serde_json::to_vec(&json!({ "points": chunk }))
                .unwrap()
                .len();
            assert!(body_len <= max_body_bytes);
        }
    }

    fn unique_test_file_path(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("fauni-search-{stamp}-{name}"))
    }

    fn write_test_pdf(path: &std::path::Path, page_count: usize) {
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let catalog_id = document.new_object_id();
        let resources_id = document.add_object(dictionary! {});

        let mut page_refs = Vec::new();
        for _ in 0..page_count {
            let content_id = document.add_object(Stream::new(dictionary! {}, Vec::new()));
            let page_id = document.new_object_id();
            let page = dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 300.into(), 300.into()],
                "Contents" => content_id,
                "Resources" => resources_id,
            };
            document.objects.insert(page_id, Object::Dictionary(page));
            page_refs.push(Object::Reference(page_id));
        }

        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => page_refs,
                "Count" => page_count as i64,
            }),
        );
        document.objects.insert(
            catalog_id,
            Object::Dictionary(dictionary! {
                "Type" => "Catalog",
                "Pages" => pages_id,
            }),
        );
        document.trailer.set("Root", catalog_id);
        document.compress();
        document.save(path).unwrap();
    }

    fn write_test_video(path: &std::path::Path, duration_secs: f64) {
        let duration_arg = format!("{duration_secs:.3}");
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=blue:s=640x360:r=30",
                "-t",
                &duration_arg,
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(path)
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn set_test_app_env() {
        std::env::set_var("APP_HOST", "127.0.0.1");
        std::env::set_var("APP_PORT", "53210");
    }
}
