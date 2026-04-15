use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use lopdf::Document as PdfDocument;
use reqwest::Client;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    future::Future,
    io,
    path::{Path as FsPath, PathBuf},
    process::Command,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;

const MULTIVECTOR_INDEX_LINE: &str = "multivector";
const DEFAULT_INDEX_EMBED_BATCH_ITEMS: usize = 8;
const DEFAULT_QDRANT_MAX_UPSERT_BODY_BYTES: usize = 8 * 1024 * 1024;
const QDRANT_UPSERT_BODY_OVERHEAD_BYTES: usize = br#"{"points":[]}"#.len();
const SIDECAR_REQUEST_TIMEOUT_SECS: u64 = 600;
const TEMP_QUERY_ASSET_TTL_MS: u128 = 60 * 60 * 1000;
const TEMP_QUERY_ASSET_REAPER_INTERVAL_SECS: u64 = 60;
const VIDEO_SEGMENT_WINDOW_MS: u64 = 8_000;
const VIDEO_SEGMENT_OVERLAP_MS: u64 = 2_000;
const APP_BODY_LIMIT_BYTES: usize = 64 * 1024 * 1024;
const SOURCE_WATCHER_POLL_INTERVAL_SECS: u64 = 2;
const SOURCE_WATCHER_DEBOUNCE_MS: u128 = 1_500;
const STATE_SNAPSHOT_ROW_ID: i64 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
enum ActiveNamespaceProbeResult {
    Ready { target_collection: String },
    Missing,
    MissingTarget { target_collection: String },
    LegacyDirectCollection,
}

pub type SharedState = Arc<RwLock<AppState>>;

pub async fn new_state() -> Result<SharedState, io::Error> {
    let state = AppState::load_from_runtime_env().await?;
    Ok(Arc::new(RwLock::new(state)))
}

pub fn build_app(state: SharedState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/libraries", get(list_libraries).post(create_library))
        .route("/libraries/:library_id", get(get_library))
        .route("/libraries/:library_id/imports", post(import_paths))
        .route(
            "/libraries/:library_id/source-roots",
            get(list_source_roots).post(create_source_root),
        )
        .route(
            "/libraries/:library_id/source-roots/:source_root_id",
            get(get_source_root)
                .patch(update_source_root)
                .delete(delete_source_root),
        )
        .route("/libraries/:library_id/sources", get(list_sources))
        .route(
            "/libraries/:library_id/refresh",
            post(refresh_library_sources),
        )
        .route(
            "/libraries/:library_id/rescan",
            post(rescan_library_sources),
        )
        .route(
            "/libraries/:library_id/source-roots/:source_root_id/refresh",
            post(refresh_source_root),
        )
        .route(
            "/libraries/:library_id/source-roots/:source_root_id/rescan",
            post(rescan_source_root),
        )
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
            "/libraries/:library_id/query-assets/documents",
            post(upload_query_document),
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
            "/libraries/:library_id/query-assets/documents/:temp_asset_id/preview",
            get(get_query_document_preview),
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
        .route("/search/document", post(search_document))
        .layer(DefaultBodyLimit::max(APP_BODY_LIMIT_BYTES))
        .with_state(state)
}

pub fn spawn_runtime_maintenance(state: SharedState) {
    let reaper_state = state.clone();
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(Duration::from_secs(TEMP_QUERY_ASSET_REAPER_INTERVAL_SECS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;

        loop {
            interval.tick().await;
            let summary = {
                let mut state = reaper_state.write().await;
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

    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(Duration::from_secs(SOURCE_WATCHER_POLL_INTERVAL_SECS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;

        loop {
            interval.tick().await;

            let queued = {
                let mut state = state.write().await;
                state.poll_source_root_watchers()
            };

            for queued_action in queued {
                let background_state = state.clone();
                tokio::spawn(async move {
                    run_source_action_job(
                        background_state,
                        queued_action.job_id,
                        queued_action.plan,
                    )
                    .await;
                });
            }
        }
    });
}

#[derive(Clone)]
pub struct AppState {
    durable_store_path: Option<PathBuf>,
    next_library_seq: u64,
    next_job_seq: u64,
    next_visual_unit_seq: u64,
    next_source_seq: u64,
    next_source_root_seq: u64,
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
            durable_store_path: None,
            next_library_seq: 0,
            next_job_seq: 0,
            next_visual_unit_seq: 0,
            next_source_seq: 0,
            next_source_root_seq: 0,
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
    async fn load_from_runtime_env() -> Result<Self, io::Error> {
        let durable_store_path = env::var("APP_RUNTIME_DIR")
            .ok()
            .map(|runtime_dir| PathBuf::from(runtime_dir).join("state.sqlite"));
        Self::load_from_durable_store_path_with_probe(durable_store_path, |namespace_name| {
            let namespace_name = namespace_name.to_string();
            async move { probe_active_qdrant_namespace(&namespace_name).await }
        })
        .await
    }

    async fn load_from_durable_store_path_with_probe<F, Fut>(
        durable_store_path: Option<PathBuf>,
        probe_collection: F,
    ) -> Result<Self, io::Error>
    where
        F: FnMut(&str) -> Fut,
        Fut: Future<Output = Result<ActiveNamespaceProbeResult, String>>,
    {
        let mut state = match durable_store_path.as_ref() {
            Some(path) => match load_durable_state_snapshot(path)? {
                Some(snapshot) => Self::from_durable_snapshot(snapshot, durable_store_path.clone()),
                None => Self::with_durable_store_path(durable_store_path.clone()),
            },
            None => Self::with_durable_store_path(None),
        };

        state.clear_ephemeral_restart_state();
        state.reseed_source_root_runtime_fields();
        state
            .reconcile_active_index_lines_on_boot_with_probe(probe_collection)
            .await;

        Ok(state)
    }

    fn with_durable_store_path(durable_store_path: Option<PathBuf>) -> Self {
        Self {
            durable_store_path,
            ..Self::default()
        }
    }

    fn from_durable_snapshot(
        snapshot: DurableAppStateSnapshot,
        durable_store_path: Option<PathBuf>,
    ) -> Self {
        let mut state = Self::with_durable_store_path(durable_store_path);
        state.apply_durable_snapshot(snapshot);
        state
    }

    fn durable_snapshot(&self) -> DurableAppStateSnapshot {
        DurableAppStateSnapshot {
            version: 1,
            library_order: self.library_order.clone(),
            libraries: self
                .libraries
                .iter()
                .map(|(library_id, library)| {
                    (
                        library_id.clone(),
                        DurableLibraryRecord {
                            id: library.id.clone(),
                            name: library.name.clone(),
                            config: library.config.clone(),
                            source_roots: library
                                .source_roots
                                .iter()
                                .map(|(source_root_id, root)| {
                                    (
                                        source_root_id.clone(),
                                        DurableSourceRootRecord {
                                            id: root.id.clone(),
                                            root_path: root.root_path.clone(),
                                            enabled: root.enabled,
                                            rules: root.rules.clone(),
                                        },
                                    )
                                })
                                .collect(),
                            source_root_order: library.source_root_order.clone(),
                            sources: library.sources.clone(),
                            source_order: library.source_order.clone(),
                            visual_units: library.visual_units.clone(),
                            visual_unit_order: library.visual_unit_order.clone(),
                            active_index_lines: library.active_index_lines.clone(),
                        },
                    )
                })
                .collect(),
        }
    }

    fn apply_durable_snapshot(&mut self, snapshot: DurableAppStateSnapshot) {
        self.library_order = snapshot.library_order;
        self.libraries = snapshot
            .libraries
            .into_iter()
            .map(|(library_id, library)| {
                (
                    library_id,
                    LibraryRecord {
                        id: library.id.clone(),
                        name: library.name,
                        collection_name: stable_collection_name(
                            &library.id,
                            MULTIVECTOR_INDEX_LINE,
                        ),
                        config: library.config,
                        source_roots: library
                            .source_roots
                            .into_iter()
                            .map(|(source_root_id, root)| {
                                (
                                    source_root_id,
                                    SourceRootRecord {
                                        id: root.id,
                                        root_path: root.root_path,
                                        enabled: root.enabled,
                                        status: "disabled".to_string(),
                                        watch_state: "disabled".to_string(),
                                        rules: root.rules,
                                        coverage_summary: SourceRootCoverageSummary::default(),
                                        observed_entries: BTreeMap::new(),
                                        pending_watch_paths: BTreeSet::new(),
                                        pending_watch_deadline_ms: None,
                                        pending_watch_error: None,
                                        last_action: None,
                                    },
                                )
                            })
                            .collect(),
                        source_root_order: library.source_root_order,
                        sources: library.sources,
                        source_order: library.source_order,
                        visual_units: library.visual_units,
                        visual_unit_order: library.visual_unit_order,
                        latest_job_id: None,
                        active_index_lines: library.active_index_lines,
                    },
                )
            })
            .collect();
        self.rebuild_durable_sequences();
    }

    fn rebuild_durable_sequences(&mut self) {
        self.next_library_seq = max_id_seq(self.libraries.keys(), "lib_");
        self.next_source_root_seq = self
            .libraries
            .values()
            .flat_map(|library| library.source_roots.keys())
            .map(|id| parse_id_seq(id, "root_"))
            .max()
            .unwrap_or(0);
        self.next_source_seq = self
            .libraries
            .values()
            .flat_map(|library| library.sources.keys())
            .map(|id| parse_id_seq(id, "src_"))
            .max()
            .unwrap_or(0);
        self.next_visual_unit_seq = self
            .libraries
            .values()
            .flat_map(|library| library.visual_units.keys())
            .map(|id| parse_id_seq(id, "vu_"))
            .max()
            .unwrap_or(0);
    }

    fn clear_ephemeral_restart_state(&mut self) {
        self.next_job_seq = 0;
        self.next_temp_asset_seq = 0;
        self.jobs.clear();
        self.job_order.clear();
        self.temp_query_assets.clear();
        for library in self.libraries.values_mut() {
            library.latest_job_id = None;
            for root in library.source_roots.values_mut() {
                root.last_action = None;
            }
        }
    }

    fn reseed_source_root_runtime_fields(&mut self) {
        for library in self.libraries.values_mut() {
            let source_root_ids = library.source_root_order.clone();
            for source_root_id in source_root_ids {
                let (active_source_count, inactive_source_count) =
                    count_sources_for_root(library, &source_root_id);
                let Some(root) = library.source_roots.get_mut(&source_root_id) else {
                    continue;
                };
                let scan = if root.enabled {
                    scan_source_root_directory(&root.root_path)
                } else {
                    SourceRootScanResult::disabled()
                };
                root.status = source_root_status_from_scan(root.enabled, &scan);
                root.watch_state = source_root_watch_state(root.enabled, &scan, false);
                root.coverage_summary = SourceRootCoverageSummary {
                    observed_file_count: scan.observed_entries.len(),
                    matched_file_count: count_matched_observed_entries(
                        &scan.observed_entries,
                        &root.rules,
                    ),
                    active_source_count,
                    inactive_source_count,
                    last_scan_at_ms: None,
                };
                root.observed_entries = scan.observed_entries;
                root.pending_watch_paths.clear();
                root.pending_watch_deadline_ms = None;
                root.pending_watch_error = scan.error;
            }
        }
    }

    fn persist_durable_state(&self) -> Result<(), String> {
        let Some(path) = self.durable_store_path.as_ref() else {
            return Ok(());
        };
        write_durable_state_snapshot(path, &self.durable_snapshot())
    }

    fn commit_durable_api<T, F>(&mut self, mutation: F) -> Result<T, ApiError>
    where
        F: FnOnce(&mut Self) -> Result<T, ApiError>,
    {
        let before = self.clone();
        let value = match mutation(self) {
            Ok(value) => value,
            Err(error) => {
                *self = before;
                return Err(error);
            }
        };

        if let Err(message) = self.persist_durable_state() {
            *self = before;
            return Err(ApiError::runtime_unavailable(
                format!("Failed to persist durable app state: {message}"),
                Some(json!({ "store": "state.sqlite" })),
            ));
        }

        Ok(value)
    }

    async fn reconcile_active_index_lines_on_boot_with_probe<F, Fut>(
        &mut self,
        mut probe_collection: F,
    ) where
        F: FnMut(&str) -> Fut,
        Fut: Future<Output = Result<ActiveNamespaceProbeResult, String>>,
    {
        let mut changed = false;
        let library_ids = self.library_order.clone();
        for library_id in library_ids {
            let active_index_lines = self
                .libraries
                .get(&library_id)
                .map(|library| {
                    library
                        .active_index_lines
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            for index_line in active_index_lines {
                let collection_name = stable_collection_name(&library_id, &index_line);
                match probe_collection(&collection_name).await {
                    Ok(ActiveNamespaceProbeResult::Ready { .. }) => {}
                    Ok(ActiveNamespaceProbeResult::Missing) => {
                        if let Some(library) = self.libraries.get_mut(&library_id) {
                            changed |= library.active_index_lines.remove(&index_line);
                        }
                    }
                    Ok(ActiveNamespaceProbeResult::MissingTarget { target_collection }) => {
                        tracing::warn!(
                            library_id = %library_id,
                            index_line = %index_line,
                            target_collection = %target_collection,
                            "Active Qdrant namespace alias points to a missing collection during restart restore"
                        );
                        if let Some(library) = self.libraries.get_mut(&library_id) {
                            changed |= library.active_index_lines.remove(&index_line);
                        }
                    }
                    Ok(ActiveNamespaceProbeResult::LegacyDirectCollection) => {
                        tracing::warn!(
                            library_id = %library_id,
                            index_line = %index_line,
                            "Direct Qdrant collection collides with the active alias namespace during restart restore; manual cleanup is required"
                        );
                        if let Some(library) = self.libraries.get_mut(&library_id) {
                            changed |= library.active_index_lines.remove(&index_line);
                        }
                    }
                    Err(error) => {
                        tracing::warn!(
                            library_id = %library_id,
                            index_line = %index_line,
                            "Failed to probe Qdrant collection during restart restore: {error}"
                        );
                        if let Some(library) = self.libraries.get_mut(&library_id) {
                            changed |= library.active_index_lines.remove(&index_line);
                        }
                    }
                }
            }
        }

        if changed {
            if let Err(error) = self.persist_durable_state() {
                tracing::warn!("Failed to persist boot-time active index reconciliation: {error}");
            }
        }
    }

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

    fn list_source_roots(&self, library_id: &str) -> Result<SourceRootsListData, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        let source_roots = library
            .source_root_order
            .iter()
            .filter_map(|source_root_id| library.source_roots.get(source_root_id))
            .map(Self::source_root_snapshot)
            .collect();

        Ok(SourceRootsListData { source_roots })
    }

    fn get_source_root(
        &self,
        library_id: &str,
        source_root_id: &str,
    ) -> Result<SourceRootDetailData, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;
        let source_root = library
            .source_roots
            .get(source_root_id)
            .ok_or_else(|| ApiError::not_found("Source root was not found."))?;

        Ok(SourceRootDetailData {
            source_root: Self::source_root_snapshot(source_root),
        })
    }

    fn create_source_root(
        &mut self,
        library_id: &str,
        request: CreateSourceRootRequest,
    ) -> Result<SourceRootSnapshot, ApiError> {
        self.commit_durable_api(|state| {
            if !state.libraries.contains_key(library_id) {
                return Err(ApiError::not_found("Library was not found."));
            }

            let root_path = normalize_source_root_path(&request.root_path)?;
            let rules = normalize_source_root_rules(request.rules.unwrap_or_default());
            let enabled = request.enabled.unwrap_or(true);
            let root_id = state.next_source_root_id();
            let scan = if enabled {
                scan_source_root_directory(&root_path)
            } else {
                SourceRootScanResult::disabled()
            };
            let coverage_summary = SourceRootCoverageSummary {
                observed_file_count: scan.observed_entries.len(),
                matched_file_count: count_matched_observed_entries(&scan.observed_entries, &rules),
                active_source_count: 0,
                inactive_source_count: 0,
                last_scan_at_ms: None,
            };
            let record = SourceRootRecord {
                id: root_id.clone(),
                root_path,
                enabled,
                status: source_root_status_from_scan(enabled, &scan),
                watch_state: source_root_watch_state(enabled, &scan, false),
                rules,
                coverage_summary,
                observed_entries: scan.observed_entries,
                pending_watch_paths: BTreeSet::new(),
                pending_watch_deadline_ms: None,
                pending_watch_error: scan.error,
                last_action: None,
            };

            let snapshot = Self::source_root_snapshot(&record);
            let library = state
                .libraries
                .get_mut(library_id)
                .ok_or_else(|| ApiError::not_found("Library was not found."))?;
            library.source_root_order.push(root_id.clone());
            library.source_roots.insert(root_id, record);
            Ok(snapshot)
        })
    }

    fn update_source_root(
        &mut self,
        library_id: &str,
        source_root_id: &str,
        request: UpdateSourceRootRequest,
    ) -> Result<SourceRootSnapshot, ApiError> {
        self.commit_durable_api(|state| {
            let library = state
                .libraries
                .get_mut(library_id)
                .ok_or_else(|| ApiError::not_found("Library was not found."))?;
            if !library.source_roots.contains_key(source_root_id) {
                return Err(ApiError::not_found("Source root was not found."));
            }

            if let Some(false) = request.enabled {
                mark_source_root_sources_state(
                    library,
                    source_root_id,
                    "out_of_scope",
                    Some("source_root_disabled".to_string()),
                );
            }

            {
                let root = library
                    .source_roots
                    .get_mut(source_root_id)
                    .ok_or_else(|| ApiError::not_found("Source root was not found."))?;

                if let Some(root_path) = request.root_path.as_ref() {
                    root.root_path = normalize_source_root_path(root_path)?;
                    root.pending_watch_paths.clear();
                    root.pending_watch_deadline_ms = None;
                }
                if let Some(enabled) = request.enabled {
                    root.enabled = enabled;
                }
                if let Some(rules) = request.rules {
                    root.rules = normalize_source_root_rules(rules);
                }

                let scan = if root.enabled {
                    scan_source_root_directory(&root.root_path)
                } else {
                    SourceRootScanResult::disabled()
                };
                root.status = source_root_status_from_scan(root.enabled, &scan);
                root.watch_state = source_root_watch_state(root.enabled, &scan, false);
                root.pending_watch_error = scan.error.clone();
                root.observed_entries = scan.observed_entries;
                root.coverage_summary.observed_file_count = root.observed_entries.len();
                root.coverage_summary.matched_file_count =
                    count_matched_observed_entries(&root.observed_entries, &root.rules);
            }

            let (active_source_count, inactive_source_count) =
                count_sources_for_root(library, source_root_id);
            let root = library
                .source_roots
                .get_mut(source_root_id)
                .ok_or_else(|| ApiError::not_found("Source root was not found."))?;
            root.coverage_summary.active_source_count = active_source_count;
            root.coverage_summary.inactive_source_count = inactive_source_count;

            Ok(Self::source_root_snapshot(root))
        })
    }

    fn delete_source_root(
        &mut self,
        library_id: &str,
        source_root_id: &str,
    ) -> Result<SourceRootSnapshot, ApiError> {
        self.commit_durable_api(|state| {
            let library = state
                .libraries
                .get_mut(library_id)
                .ok_or_else(|| ApiError::not_found("Library was not found."))?;
            if !library.source_roots.contains_key(source_root_id) {
                return Err(ApiError::not_found("Source root was not found."));
            }

            mark_source_root_sources_state(
                library,
                source_root_id,
                "out_of_scope",
                Some("source_root_deleted".to_string()),
            );

            let root = library
                .source_roots
                .remove(source_root_id)
                .ok_or_else(|| ApiError::not_found("Source root was not found."))?;
            library
                .source_root_order
                .retain(|candidate| candidate != source_root_id);
            Ok(Self::source_root_snapshot(&root))
        })
    }

    fn list_sources(
        &self,
        library_id: &str,
        query: SourcesQuery,
    ) -> Result<SourcesListData, ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        let sources = library
            .source_order
            .iter()
            .filter_map(|source_id| library.sources.get(source_id))
            .filter(|source| {
                query
                    .source_root_id
                    .as_ref()
                    .map(|expected| source.source_root_id.as_ref() == Some(expected))
                    .unwrap_or(true)
                    && query
                        .source_type
                        .as_ref()
                        .map(|expected| &source.source_type == expected)
                        .unwrap_or(true)
                    && query
                        .status
                        .as_ref()
                        .map(|expected| &source.status == expected)
                        .unwrap_or(true)
            })
            .map(|source| Self::source_inventory_item(library, source))
            .collect();

        Ok(SourcesListData { sources })
    }

    fn queue_source_action(
        &mut self,
        library_id: &str,
        scope: SourceActionScope,
        action: SourceActionKind,
        trigger: SourceActionTrigger,
        changed_paths_by_root: BTreeMap<String, BTreeSet<String>>,
    ) -> Result<(SourceActionData, Option<QueuedSourceAction>), ApiError> {
        let library = self
            .libraries
            .get(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        let mut accepted_root_ids = Vec::new();

        match &scope {
            SourceActionScope::Library => {
                for source_root_id in &library.source_root_order {
                    let root = library
                        .source_roots
                        .get(source_root_id)
                        .expect("source_root_order should reference a valid source root");
                    if !root.enabled {
                        rejected.push(SourceActionRejectedItem {
                            source_root_id: Some(root.id.clone()),
                            root_path: Some(root.root_path.clone()),
                            reason_code: "not_enabled".to_string(),
                            message: "Source root is disabled.".to_string(),
                        });
                        continue;
                    }
                    if source_root_action_in_flight(root) {
                        rejected.push(SourceActionRejectedItem {
                            source_root_id: Some(root.id.clone()),
                            root_path: Some(root.root_path.clone()),
                            reason_code: "job_in_progress".to_string(),
                            message:
                                "Source root already has an in-flight source-management action."
                                    .to_string(),
                        });
                        continue;
                    }
                    accepted_root_ids.push(root.id.clone());
                    accepted.push(SourceActionAcceptedItem {
                        source_root_id: root.id.clone(),
                        root_path: root.root_path.clone(),
                        action: action.as_str().to_string(),
                    });
                }
            }
            SourceActionScope::SourceRoot(source_root_id) => {
                let root = library
                    .source_roots
                    .get(source_root_id)
                    .ok_or_else(|| ApiError::not_found("Source root was not found."))?;
                if !root.enabled {
                    return Ok((
                        SourceActionData {
                            accepted,
                            rejected: vec![SourceActionRejectedItem {
                                source_root_id: Some(root.id.clone()),
                                root_path: Some(root.root_path.clone()),
                                reason_code: "not_enabled".to_string(),
                                message: "Source root is disabled.".to_string(),
                            }],
                            job_handle: None,
                            job: None,
                        },
                        None,
                    ));
                }
                if source_root_action_in_flight(root) {
                    return Ok((
                        SourceActionData {
                            accepted,
                            rejected: vec![SourceActionRejectedItem {
                                source_root_id: Some(root.id.clone()),
                                root_path: Some(root.root_path.clone()),
                                reason_code: "job_in_progress".to_string(),
                                message:
                                    "Source root already has an in-flight source-management action."
                                        .to_string(),
                            }],
                            job_handle: None,
                            job: None,
                        },
                        None,
                    ));
                }
                accepted_root_ids.push(root.id.clone());
                accepted.push(SourceActionAcceptedItem {
                    source_root_id: root.id.clone(),
                    root_path: root.root_path.clone(),
                    action: action.as_str().to_string(),
                });
            }
        }

        if accepted_root_ids.is_empty() {
            return Ok((
                SourceActionData {
                    accepted,
                    rejected,
                    job_handle: None,
                    job: None,
                },
                None,
            ));
        }

        let job_id = self.next_job_id();
        let snapshot = JobSnapshot {
            job_id: job_id.clone(),
            library_id: library_id.to_string(),
            kind: action.as_str().to_string(),
            status: "queued".to_string(),
            phase: "intake".to_string(),
            progress: JobProgress {
                completed: 0,
                total: accepted_root_ids.len(),
                unit: "source_root".to_string(),
            },
            cancelable: false,
            current_attempt: JobAttemptSnapshot {
                attempt: 1,
                status: "queued".to_string(),
                summary: format!(
                    "Queued {} across {} source root(s) via {} trigger.",
                    action.as_str(),
                    accepted_root_ids.len(),
                    trigger.as_str(),
                ),
            },
        };

        let plan = SourceActionPlan {
            library_id: library_id.to_string(),
            action,
            target_root_ids: accepted_root_ids.clone(),
            changed_paths_by_root,
        };

        let library = self
            .libraries
            .get_mut(library_id)
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;
        for source_root_id in &accepted_root_ids {
            if let Some(root) = library.source_roots.get_mut(source_root_id) {
                root.watch_state = queued_watch_state_for_action(action).to_string();
            }
        }
        library.latest_job_id = Some(job_id.clone());

        self.jobs.insert(
            job_id.clone(),
            JobRecord {
                snapshot: snapshot.clone(),
            },
        );
        self.job_order.push(job_id.clone());

        Ok((
            SourceActionData {
                accepted,
                rejected,
                job_handle: Some(job_id.clone()),
                job: Some(snapshot),
            },
            Some(QueuedSourceAction { job_id, plan }),
        ))
    }

    fn poll_source_root_watchers(&mut self) -> Vec<QueuedSourceAction> {
        let now = current_unix_ms();
        let library_ids = self.library_order.clone();
        let mut due_actions = Vec::new();

        for library_id in &library_ids {
            let Some(library) = self.libraries.get_mut(library_id) else {
                continue;
            };
            let source_root_ids = library.source_root_order.clone();

            for source_root_id in source_root_ids {
                let Some(root) = library.source_roots.get_mut(&source_root_id) else {
                    continue;
                };
                if !root.enabled {
                    root.status = "disabled".to_string();
                    root.watch_state = "disabled".to_string();
                    continue;
                }
                let watcher_refresh_pending = root.watch_state == "queued_refresh"
                    && root.pending_watch_deadline_ms.is_some()
                    && !root.pending_watch_paths.is_empty();
                if source_root_action_in_flight(root) && !watcher_refresh_pending {
                    continue;
                }

                let scan = scan_source_root_directory(&root.root_path);
                let changed_paths =
                    diff_observed_entries(&root.observed_entries, &scan.observed_entries);
                if !changed_paths.is_empty() {
                    root.pending_watch_paths.extend(changed_paths);
                    if root.pending_watch_deadline_ms.is_none() {
                        root.pending_watch_deadline_ms =
                            Some(now.saturating_add(SOURCE_WATCHER_DEBOUNCE_MS));
                    }
                }
                root.observed_entries = scan.observed_entries.clone();
                root.pending_watch_error = scan.error.clone();
                root.status = source_root_status_from_scan(true, &scan);
                root.watch_state = if root.pending_watch_paths.is_empty() {
                    source_root_watch_state(true, &scan, false)
                } else {
                    "queued_refresh".to_string()
                };

                if root
                    .pending_watch_deadline_ms
                    .map(|deadline| now >= deadline)
                    .unwrap_or(false)
                    && !root.pending_watch_paths.is_empty()
                {
                    root.watch_state = source_root_watch_state(true, &scan, false);
                    due_actions.push((
                        library_id.clone(),
                        source_root_id.clone(),
                        std::mem::take(&mut root.pending_watch_paths),
                    ));
                    root.pending_watch_deadline_ms = None;
                }
            }
        }

        let mut queued = Vec::new();
        for (library_id, source_root_id, changed_paths) in due_actions {
            let mut changed_paths_by_root = BTreeMap::new();
            changed_paths_by_root.insert(source_root_id.clone(), changed_paths);
            if let Ok((_, Some(queued_action))) = self.queue_source_action(
                &library_id,
                SourceActionScope::SourceRoot(source_root_id),
                SourceActionKind::Refresh,
                SourceActionTrigger::Watcher,
                changed_paths_by_root,
            ) {
                queued.push(queued_action);
            }
        }

        queued
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
            .filter(|source| source.source_type == "video" && source.status == "active")
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
        self.commit_durable_api(|state| {
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

            let library_id = state.next_library_id();
            let record = LibraryRecord {
                id: library_id.clone(),
                name: name.to_string(),
                collection_name: stable_collection_name(&library_id, MULTIVECTOR_INDEX_LINE),
                config: LibraryConfigPayload {
                    enabled_index_lines,
                },
                source_roots: BTreeMap::new(),
                source_root_order: Vec::new(),
                sources: BTreeMap::new(),
                source_order: Vec::new(),
                visual_units: BTreeMap::new(),
                visual_unit_order: Vec::new(),
                latest_job_id: None,
                active_index_lines: BTreeSet::new(),
            };

            let snapshot = state.library_snapshot(&record);
            state.library_order.push(library_id.clone());
            state.libraries.insert(library_id, record);
            Ok(snapshot)
        })
    }

    fn prepare_import(
        &mut self,
        library_id: &str,
        request: ImportPathsRequest,
    ) -> Result<PreparedImport, ApiError> {
        let (collection_name, had_existing_visual_units) = self
            .libraries
            .get(library_id)
            .map(|library| {
                (
                    library.collection_name.clone(),
                    !library.visual_units.is_empty(),
                )
            })
            .ok_or_else(|| ApiError::not_found("Library was not found."))?;

        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        let mut new_sources = Vec::new();
        let mut new_visual_units = Vec::new();

        for original in request.paths {
            match self.inspect_import_path(&original) {
                Ok(classification) => {
                    let visual_units = self.new_visual_units_from_classification(&classification);
                    let source = self.source_record_from_classification(
                        &classification,
                        visual_units.iter().map(|item| item.id.clone()).collect(),
                    );
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
            had_existing_visual_units,
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
        if !self.jobs.contains_key(job_id) {
            return Err("Job was not found.".to_string());
        }
        if !self.libraries.contains_key(&prepared.library_id) {
            return Err("Library was not found.".to_string());
        }

        let before = self.clone();
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

        if let Err(message) = self.persist_durable_state() {
            *self = before;
            if let Some(job) = self.jobs.get_mut(job_id) {
                job.snapshot.status = "failed".to_string();
                job.snapshot.phase = "failed".to_string();
                job.snapshot.current_attempt.status = "failed".to_string();
                job.snapshot.current_attempt.summary =
                    format!("Persisting durable app state failed: {message}");
            }
            return Err(format!("Failed to persist durable app state: {message}"));
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

    fn mark_source_action_running(&mut self, plan: &SourceActionPlan, job_id: &str) {
        self.update_job_snapshot(
            job_id,
            "running",
            "scan",
            0,
            format!(
                "{} is evaluating {} source root(s).",
                plan.action.as_str(),
                plan.target_root_ids.len()
            ),
        );

        if let Some(library) = self.libraries.get_mut(&plan.library_id) {
            for source_root_id in &plan.target_root_ids {
                if let Some(root) = library.source_roots.get_mut(source_root_id) {
                    root.watch_state = running_watch_state_for_action(plan.action).to_string();
                }
            }
        }
    }

    fn prepare_source_action_execution(
        &mut self,
        plan: &SourceActionPlan,
    ) -> Result<PreparedSourceAction, String> {
        let (collection_name, had_existing_visual_units, can_rebuild_from_scratch) = self
            .libraries
            .get(&plan.library_id)
            .map(|library| {
                (
                    library.collection_name.clone(),
                    !library.visual_units.is_empty(),
                    plan.action.is_rescan()
                        && plan.target_root_ids.len() == library.source_root_order.len(),
                )
            })
            .ok_or_else(|| "Library was not found.".to_string())?;

        let mut root_updates = Vec::new();
        let mut source_mutations = Vec::new();
        let mut stale_point_ids = Vec::new();
        let mut visual_units_to_index = Vec::new();
        let mut summary = SourceActionSummary::default();

        for source_root_id in &plan.target_root_ids {
            let (root, existing_sources, existing_point_ids_by_source) = {
                let library = self
                    .libraries
                    .get(&plan.library_id)
                    .ok_or_else(|| "Library was not found.".to_string())?;
                let root = library
                    .source_roots
                    .get(source_root_id)
                    .cloned()
                    .ok_or_else(|| format!("Source root {source_root_id} was not found."))?;
                let existing_sources = library
                    .sources
                    .values()
                    .filter(|source| {
                        source.source_root_id.as_deref() == Some(source_root_id.as_str())
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                let existing_point_ids_by_source = existing_sources
                    .iter()
                    .map(|source| {
                        let point_ids = source
                            .visual_unit_ids
                            .iter()
                            .filter_map(|visual_unit_id| {
                                library
                                    .visual_units
                                    .get(visual_unit_id)
                                    .map(|visual_unit| visual_unit.point_id)
                            })
                            .collect::<Vec<_>>();
                        (source.id.clone(), point_ids)
                    })
                    .collect::<BTreeMap<_, _>>();
                (root, existing_sources, existing_point_ids_by_source)
            };

            let scan = scan_source_root_directory(&root.root_path);
            let candidate_entries = scan
                .observed_entries
                .values()
                .filter(|entry| observed_entry_is_in_scope(entry, &root.rules))
                .cloned()
                .collect::<Vec<_>>();
            let candidate_by_relative_path = candidate_entries
                .iter()
                .map(|entry| (entry.relative_path.clone(), entry.clone()))
                .collect::<BTreeMap<_, _>>();
            let existing_by_relative_path = existing_sources
                .iter()
                .filter_map(|source| {
                    source
                        .relative_path
                        .as_ref()
                        .map(|relative_path| (relative_path.clone(), source.clone()))
                })
                .collect::<BTreeMap<_, _>>();

            let affected_paths = planned_source_action_paths(
                plan,
                &root,
                &candidate_by_relative_path,
                &existing_by_relative_path,
            );
            let mut root_status_by_source_id = existing_sources
                .iter()
                .map(|source| (source.id.clone(), source.status.clone()))
                .collect::<BTreeMap<_, _>>();

            for relative_path in affected_paths {
                let current_entry = candidate_by_relative_path.get(&relative_path);
                let existing_source = existing_by_relative_path.get(&relative_path);

                if let Some(entry) = current_entry {
                    match self.inspect_import_path(&entry.absolute_path) {
                        Ok(mut classification) => {
                            if let Some(existing_source) = existing_source {
                                classification.source_id = existing_source.id.clone();
                                stale_point_ids.extend(
                                    existing_point_ids_by_source
                                        .get(&existing_source.id)
                                        .into_iter()
                                        .flatten()
                                        .copied(),
                                );
                            }
                            let visual_units =
                                self.new_visual_units_from_classification(&classification);
                            let mut source = self.source_record_from_classification(
                                &classification,
                                visual_units.iter().map(|item| item.id.clone()).collect(),
                            );
                            source.source_root_id = Some(root.id.clone());
                            source.source_root_path = Some(root.root_path.clone());
                            source.relative_path = Some(relative_path.clone());
                            source.status = "active".to_string();
                            source.status_reason = None;
                            source.observed_size_bytes = Some(entry.size_bytes);
                            source.observed_modified_at_ms = entry.modified_at_ms;

                            root_status_by_source_id
                                .insert(source.id.clone(), source.status.clone());
                            summary.activated_sources += 1;
                            summary.indexing_visual_units += visual_units.len();
                            visual_units_to_index.extend(visual_units.iter().cloned());
                            source_mutations.push(PreparedSourceMutation {
                                source,
                                visual_units,
                            });
                        }
                        Err(rejection) => {
                            if let Some(existing_source) = existing_source.cloned() {
                                stale_point_ids.extend(
                                    existing_point_ids_by_source
                                        .get(&existing_source.id)
                                        .into_iter()
                                        .flatten()
                                        .copied(),
                                );
                                let mutated = invalidated_source_record(
                                    existing_source,
                                    "invalidated",
                                    Some(rejection.reason_code),
                                    Some(entry.size_bytes),
                                    entry.modified_at_ms,
                                );
                                root_status_by_source_id
                                    .insert(mutated.id.clone(), mutated.status.clone());
                                summary.invalidated_sources += 1;
                                source_mutations.push(PreparedSourceMutation {
                                    source: mutated,
                                    visual_units: Vec::new(),
                                });
                            }
                        }
                    }
                    continue;
                }

                let Some(existing_source) = existing_source.cloned() else {
                    continue;
                };

                let (status, reason) = match scan.observed_entries.get(&relative_path) {
                    Some(observed) => out_of_scope_status_reason(observed, &root.rules),
                    None if scan.error.is_some() => (
                        "invalidated".to_string(),
                        Some("source_root_unreachable".to_string()),
                    ),
                    None => ("invalidated".to_string(), Some("not_found".to_string())),
                };
                if status == "invalidated" {
                    summary.invalidated_sources += 1;
                } else {
                    summary.out_of_scope_sources += 1;
                }
                stale_point_ids.extend(
                    existing_point_ids_by_source
                        .get(&existing_source.id)
                        .into_iter()
                        .flatten()
                        .copied(),
                );
                let mutated =
                    invalidated_source_record(existing_source, &status, reason, None, None);
                root_status_by_source_id.insert(mutated.id.clone(), mutated.status.clone());
                source_mutations.push(PreparedSourceMutation {
                    source: mutated,
                    visual_units: Vec::new(),
                });
            }

            summary.scanned_roots += 1;
            summary.observed_files += scan.observed_entries.len();
            summary.matched_files += candidate_entries.len();
            if scan.status == "degraded" {
                summary.degraded_roots += 1;
            }

            let active_source_count = root_status_by_source_id
                .values()
                .filter(|status| status.as_str() == "active")
                .count();
            root_updates.push(PreparedSourceRootUpdate {
                source_root_id: root.id.clone(),
                status: source_root_status_from_scan(root.enabled, &scan),
                watch_state: source_root_watch_state(root.enabled, &scan, false),
                coverage_summary: SourceRootCoverageSummary {
                    observed_file_count: scan.observed_entries.len(),
                    matched_file_count: candidate_entries.len(),
                    active_source_count,
                    inactive_source_count: root_status_by_source_id
                        .len()
                        .saturating_sub(active_source_count),
                    last_scan_at_ms: Some(current_unix_ms()),
                },
                observed_entries: scan.observed_entries,
            });
        }

        stale_point_ids.sort_unstable();
        stale_point_ids.dedup();

        Ok(PreparedSourceAction {
            library_id: plan.library_id.clone(),
            collection_name,
            action: plan.action,
            accepted_root_count: plan.target_root_ids.len(),
            can_rebuild_from_scratch,
            had_existing_visual_units,
            root_updates,
            source_mutations,
            stale_point_ids,
            visual_units_to_index,
            summary,
        })
    }

    fn finalize_source_action_job(
        &mut self,
        job_id: &str,
        prepared: PreparedSourceAction,
        outcome: SourceActionJobOutcome,
    ) -> Result<(), String> {
        if !self.jobs.contains_key(job_id) {
            return Err("Job was not found.".to_string());
        }
        if !self.libraries.contains_key(&prepared.library_id) {
            return Err("Library was not found.".to_string());
        }

        if outcome.status == "completed" {
            let before = self.clone();
            let library = self
                .libraries
                .get_mut(&prepared.library_id)
                .ok_or_else(|| "Library was not found.".to_string())?;

            for mutation in &prepared.source_mutations {
                let old_visual_unit_ids = library
                    .sources
                    .get(&mutation.source.id)
                    .map(|source| source.visual_unit_ids.clone())
                    .unwrap_or_default();
                if !old_visual_unit_ids.is_empty() {
                    let stale_ids = old_visual_unit_ids.iter().cloned().collect::<BTreeSet<_>>();
                    library
                        .visual_unit_order
                        .retain(|visual_unit_id| !stale_ids.contains(visual_unit_id));
                    for visual_unit_id in old_visual_unit_ids {
                        library.visual_units.remove(&visual_unit_id);
                    }
                }

                if !library.sources.contains_key(&mutation.source.id) {
                    library.source_order.push(mutation.source.id.clone());
                }
                library
                    .sources
                    .insert(mutation.source.id.clone(), mutation.source.clone());

                for visual_unit in &mutation.visual_units {
                    library.visual_unit_order.push(visual_unit.id.clone());
                    library
                        .visual_units
                        .insert(visual_unit.id.clone(), visual_unit.clone());
                }
            }

            for update in &prepared.root_updates {
                if let Some(root) = library.source_roots.get_mut(&update.source_root_id) {
                    root.status = update.status.clone();
                    root.watch_state = update.watch_state.clone();
                    root.coverage_summary = update.coverage_summary.clone();
                    root.observed_entries = update.observed_entries.clone();
                    root.pending_watch_error = None;
                }
            }

            if outcome.activate_index {
                library
                    .active_index_lines
                    .insert(MULTIVECTOR_INDEX_LINE.to_string());
            }

            if let Err(message) = self.persist_durable_state() {
                *self = before;
                if let Some(job) = self.jobs.get_mut(job_id) {
                    job.snapshot.status = "failed".to_string();
                    job.snapshot.phase = "failed".to_string();
                    job.snapshot.current_attempt.status = "failed".to_string();
                    job.snapshot.current_attempt.summary =
                        format!("Persisting durable app state failed: {message}");
                }
                return Err(format!("Failed to persist durable app state: {message}"));
            }
        }

        let library = self
            .libraries
            .get_mut(&prepared.library_id)
            .ok_or_else(|| "Library was not found.".to_string())?;

        for update in &prepared.root_updates {
            if let Some(root) = library.source_roots.get_mut(&update.source_root_id) {
                root.watch_state = if outcome.status == "completed" {
                    update.watch_state.clone()
                } else {
                    source_root_watch_state(
                        root.enabled,
                        &SourceRootScanResult {
                            status: root.status.clone(),
                            observed_entries: root.observed_entries.clone(),
                            error: root.pending_watch_error.clone(),
                        },
                        false,
                    )
                };
                root.last_action = Some(SourceRootLastAction {
                    action: prepared.action.as_str().to_string(),
                    status: outcome.status.to_string(),
                    summary: outcome.summary.clone(),
                    job_id: Some(job_id.to_string()),
                });
            }
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

    fn source_root_snapshot(root: &SourceRootRecord) -> SourceRootSnapshot {
        SourceRootSnapshot {
            source_root_id: root.id.clone(),
            root_path: root.root_path.clone(),
            enabled: root.enabled,
            status: root.status.clone(),
            watch_state: root.watch_state.clone(),
            coverage_summary: root.coverage_summary.clone(),
            rules: root.rules.clone(),
            last_action: root.last_action.clone(),
        }
    }

    fn source_inventory_item(
        library: &LibraryRecord,
        source: &SourceRecord,
    ) -> SourceInventoryItem {
        let source_root_path = source
            .source_root_id
            .as_ref()
            .and_then(|source_root_id| {
                library
                    .source_roots
                    .get(source_root_id)
                    .map(|root| root.root_path.clone())
            })
            .or_else(|| source.source_root_path.clone());
        let source_root_label = match (&source.source_root_id, &source_root_path) {
            (Some(_), Some(root_path)) => root_path.clone(),
            (Some(source_root_id), None) => format!("deleted:{source_root_id}"),
            (None, _) => "manual import".to_string(),
        };

        SourceInventoryItem {
            source_id: source.id.clone(),
            source_path: source.source_path.clone(),
            source_type: source.source_type.clone(),
            kind: source.kind.clone(),
            status: source.status.clone(),
            status_reason: source.status_reason.clone(),
            relative_path: source.relative_path.clone(),
            source_root_id: source.source_root_id.clone(),
            source_root_path,
            source_root_label,
            visual_unit_count: source.visual_unit_ids.len(),
        }
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

    fn prepare_document_search(
        &self,
        request: &DocumentSearchRequest,
    ) -> Result<(SearchPlan, ResolvedDocumentQueryInput), ApiError> {
        let plan = self.prepare_search_scope(
            request.library_id.trim(),
            request.filters.as_ref(),
            request.top_k,
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
            active_visual_unit_ids: library.visual_units.keys().cloned().collect(),
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

    fn register_temp_query_document_asset(
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
            page_count: staged.page_count,
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

    fn get_temp_query_document_asset(
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
                message:
                    "Only PDF, common image files, and mp4/mov video files are accepted right now."
                        .to_string(),
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
                accepted_items: library
                    .sources
                    .values()
                    .filter(|source| source.status == "active")
                    .map(|source| source.visual_unit_ids.len())
                    .sum(),
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

    fn next_source_root_id(&mut self) -> String {
        self.next_source_root_seq += 1;
        format!("root_{:06}", self.next_source_root_seq)
    }

    fn next_temp_asset_id(&mut self) -> String {
        self.next_temp_asset_seq += 1;
        format!("temp_asset_{:06}", self.next_temp_asset_seq)
    }

    fn source_record_from_classification(
        &self,
        classification: &PathClassification,
        visual_unit_ids: Vec<String>,
    ) -> SourceRecord {
        SourceRecord {
            id: classification.source_id.clone(),
            source_root_id: None,
            source_root_path: None,
            source_path: classification.normalized_path.clone(),
            relative_path: None,
            source_type: classification.source_type.clone(),
            kind: classification.kind.clone(),
            status: "active".to_string(),
            status_reason: None,
            page_count: classification.page_count,
            duration_ms: classification.duration_ms,
            observed_size_bytes: None,
            observed_modified_at_ms: None,
            visual_unit_ids,
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
                        .map(
                            |(start_ms, end_ms)| json!({ "start_ms": start_ms, "end_ms": end_ms }),
                        );
                    let next = segments.get(segment_index + 1).map(
                        |(start_ms, end_ms)| json!({ "start_ms": start_ms, "end_ms": end_ms }),
                    );
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
    source_roots: BTreeMap<String, SourceRootRecord>,
    source_root_order: Vec<String>,
    sources: BTreeMap<String, SourceRecord>,
    source_order: Vec<String>,
    visual_units: BTreeMap<String, VisualUnitRecord>,
    visual_unit_order: Vec<String>,
    latest_job_id: Option<String>,
    active_index_lines: BTreeSet<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct DurableAppStateSnapshot {
    version: u32,
    library_order: Vec<String>,
    libraries: BTreeMap<String, DurableLibraryRecord>,
}

#[derive(Debug, Deserialize, Serialize)]
struct DurableLibraryRecord {
    id: String,
    name: String,
    config: LibraryConfigPayload,
    source_roots: BTreeMap<String, DurableSourceRootRecord>,
    source_root_order: Vec<String>,
    sources: BTreeMap<String, SourceRecord>,
    source_order: Vec<String>,
    visual_units: BTreeMap<String, VisualUnitRecord>,
    visual_unit_order: Vec<String>,
    active_index_lines: BTreeSet<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct DurableSourceRootRecord {
    id: String,
    root_path: String,
    enabled: bool,
    rules: SourceRootRulesPayload,
}

#[derive(Clone, Debug)]
struct SourceRootRecord {
    id: String,
    root_path: String,
    enabled: bool,
    status: String,
    watch_state: String,
    rules: SourceRootRulesPayload,
    coverage_summary: SourceRootCoverageSummary,
    observed_entries: BTreeMap<String, ObservedSourceFile>,
    pending_watch_paths: BTreeSet<String>,
    pending_watch_deadline_ms: Option<u128>,
    pending_watch_error: Option<String>,
    last_action: Option<SourceRootLastAction>,
}

#[derive(Clone, Debug)]
struct ObservedSourceFile {
    absolute_path: String,
    relative_path: String,
    size_bytes: u64,
    modified_at_ms: Option<u128>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SourceRecord {
    id: String,
    source_root_id: Option<String>,
    source_root_path: Option<String>,
    source_path: String,
    relative_path: Option<String>,
    source_type: String,
    kind: String,
    status: String,
    status_reason: Option<String>,
    page_count: Option<usize>,
    duration_ms: Option<u64>,
    observed_size_bytes: Option<u64>,
    observed_modified_at_ms: Option<u128>,
    visual_unit_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
    page_count: Option<usize>,
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

#[derive(Clone, Debug)]
struct ResolvedDocumentQueryInput {
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

#[derive(Clone, Debug, Deserialize, Serialize)]
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
pub struct CreateSourceRootRequest {
    pub root_path: String,
    pub enabled: Option<bool>,
    pub rules: Option<SourceRootRulesPayload>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSourceRootRequest {
    pub root_path: Option<String>,
    pub enabled: Option<bool>,
    pub rules: Option<SourceRootRulesPayload>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SourceRootRulesPayload {
    #[serde(default)]
    pub include_globs: Vec<String>,
    #[serde(default)]
    pub exclude_globs: Vec<String>,
    #[serde(default)]
    pub include_extensions: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SourceRootCoverageSummary {
    pub observed_file_count: usize,
    pub matched_file_count: usize,
    pub active_source_count: usize,
    pub inactive_source_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_scan_at_ms: Option<u128>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceRootLastAction {
    pub action: String,
    pub status: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceRootSnapshot {
    pub source_root_id: String,
    pub root_path: String,
    pub enabled: bool,
    pub status: String,
    pub watch_state: String,
    pub coverage_summary: SourceRootCoverageSummary,
    pub rules: SourceRootRulesPayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_action: Option<SourceRootLastAction>,
}

#[derive(Debug, Serialize)]
pub struct SourceRootsListData {
    pub source_roots: Vec<SourceRootSnapshot>,
}

#[derive(Debug, Serialize)]
pub struct SourceRootDetailData {
    pub source_root: SourceRootSnapshot,
}

#[derive(Debug, Deserialize)]
pub struct SourcesQuery {
    pub source_root_id: Option<String>,
    pub source_type: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SourcesListData {
    pub sources: Vec<SourceInventoryItem>,
}

#[derive(Debug, Serialize)]
pub struct SourceInventoryItem {
    pub source_id: String,
    pub source_path: String,
    pub source_type: String,
    pub kind: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_root_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_root_path: Option<String>,
    pub source_root_label: String,
    pub visual_unit_count: usize,
}

#[derive(Debug, Serialize, Clone)]
pub struct SourceActionAcceptedItem {
    pub source_root_id: String,
    pub root_path: String,
    pub action: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct SourceActionRejectedItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_root_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_path: Option<String>,
    pub reason_code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SourceActionData {
    pub accepted: Vec<SourceActionAcceptedItem>,
    pub rejected: Vec<SourceActionRejectedItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<JobSnapshot>,
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
pub struct DocumentSearchRequest {
    pub library_id: String,
    pub document_input: QueryDocumentInputRequest,
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

#[derive(Debug, Deserialize)]
pub struct QueryDocumentInputRequest {
    pub kind: String,
    pub temp_asset_id: Option<String>,
    pub source_id: Option<String>,
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
pub struct QueryDocumentAssetData {
    pub temp_asset_id: String,
    pub preview: PreviewReference,
    pub source_type: String,
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<usize>,
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
    had_existing_visual_units: bool,
    accepted: Vec<ImportAcceptedItem>,
    rejected: Vec<ImportRejectedItem>,
    sources: Vec<SourceRecord>,
    visual_units: Vec<VisualUnitRecord>,
}

#[derive(Clone)]
struct SourceActionPlan {
    library_id: String,
    action: SourceActionKind,
    target_root_ids: Vec<String>,
    changed_paths_by_root: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceActionKind {
    Refresh,
    Rescan,
}

impl SourceActionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Refresh => "refresh",
            Self::Rescan => "rescan",
        }
    }

    fn is_rescan(self) -> bool {
        matches!(self, Self::Rescan)
    }
}

#[derive(Clone, Debug)]
enum SourceActionScope {
    Library,
    SourceRoot(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceActionTrigger {
    Manual,
    Watcher,
}

impl SourceActionTrigger {
    fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Watcher => "watcher",
        }
    }
}

struct PreparedSourceAction {
    library_id: String,
    collection_name: String,
    action: SourceActionKind,
    accepted_root_count: usize,
    can_rebuild_from_scratch: bool,
    had_existing_visual_units: bool,
    root_updates: Vec<PreparedSourceRootUpdate>,
    source_mutations: Vec<PreparedSourceMutation>,
    stale_point_ids: Vec<u64>,
    visual_units_to_index: Vec<VisualUnitRecord>,
    summary: SourceActionSummary,
}

struct PreparedSourceRootUpdate {
    source_root_id: String,
    status: String,
    watch_state: String,
    coverage_summary: SourceRootCoverageSummary,
    observed_entries: BTreeMap<String, ObservedSourceFile>,
}

struct PreparedSourceMutation {
    source: SourceRecord,
    visual_units: Vec<VisualUnitRecord>,
}

impl PreparedSourceAction {
    fn requires_index_update(&self) -> bool {
        !self.visual_units_to_index.is_empty() || !self.stale_point_ids.is_empty()
    }
}

#[derive(Default)]
struct SourceActionSummary {
    scanned_roots: usize,
    observed_files: usize,
    matched_files: usize,
    activated_sources: usize,
    invalidated_sources: usize,
    out_of_scope_sources: usize,
    indexing_visual_units: usize,
    degraded_roots: usize,
}

struct SourceActionJobOutcome {
    status: &'static str,
    phase: &'static str,
    completed: usize,
    activate_index: bool,
    summary: String,
}

impl SourceActionJobOutcome {
    fn completed(prepared: &PreparedSourceAction) -> Self {
        let summary = format!(
            "{} {} source root(s); observed {} file(s), matched {} file(s), activated {}, invalidated {}, out_of_scope {}, indexed {} visual unit(s).",
            prepared.action.as_str(),
            prepared.summary.scanned_roots,
            prepared.summary.observed_files,
            prepared.summary.matched_files,
            prepared.summary.activated_sources,
            prepared.summary.invalidated_sources,
            prepared.summary.out_of_scope_sources,
            prepared.summary.indexing_visual_units,
        );
        Self {
            status: "completed",
            phase: "activated",
            completed: prepared.accepted_root_count,
            activate_index: prepared.summary.indexing_visual_units > 0,
            summary,
        }
    }

    fn failed(action: SourceActionKind, completed: usize, message: String) -> Self {
        Self {
            status: "failed",
            phase: "failed",
            completed,
            activate_index: false,
            summary: format!("{} failed: {message}", action.as_str()),
        }
    }
}

struct QueuedSourceAction {
    job_id: String,
    plan: SourceActionPlan,
}

struct SourceRootScanResult {
    status: String,
    observed_entries: BTreeMap<String, ObservedSourceFile>,
    error: Option<String>,
}

impl SourceRootScanResult {
    fn disabled() -> Self {
        Self {
            status: "disabled".to_string(),
            observed_entries: BTreeMap::new(),
            error: None,
        }
    }
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
    active_visual_unit_ids: BTreeSet<String>,
    debug: bool,
}

struct StagedQueryAsset {
    path: String,
    source_type: String,
    content_type: String,
    original_filename: Option<String>,
    page_count: Option<usize>,
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

struct IncomingQueryDocumentUpload {
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
            "GET /libraries/{library_id}/source-roots",
            "POST /libraries/{library_id}/source-roots",
            "GET /libraries/{library_id}/source-roots/{source_root_id}",
            "PATCH /libraries/{library_id}/source-roots/{source_root_id}",
            "DELETE /libraries/{library_id}/source-roots/{source_root_id}",
            "GET /libraries/{library_id}/sources",
            "POST /libraries/{library_id}/imports",
            "POST /libraries/{library_id}/refresh",
            "POST /libraries/{library_id}/rescan",
            "POST /libraries/{library_id}/source-roots/{source_root_id}/refresh",
            "POST /libraries/{library_id}/source-roots/{source_root_id}/rescan",
            "GET /libraries/{library_id}/video-sources",
            "POST /libraries/{library_id}/query-assets/images",
            "POST /libraries/{library_id}/query-assets/videos",
            "POST /libraries/{library_id}/query-assets/documents",
            "GET /libraries/{library_id}/video-sources/{source_id}/preview",
            "GET /libraries/{library_id}/visual-units/{visual_unit_id}",
            "GET /libraries/{library_id}/query-assets/images/{temp_asset_id}/preview",
            "GET /libraries/{library_id}/query-assets/videos/{temp_asset_id}/preview",
            "GET /libraries/{library_id}/query-assets/documents/{temp_asset_id}/preview",
            "GET /jobs",
            "GET /jobs/{job_id}",
            "POST /search/text",
            "POST /search/image",
            "POST /search/video",
            "POST /search/document",
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

async fn list_source_roots(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<SourceRootsListData>>, ApiError> {
    let state = state.read().await;
    let data = state.list_source_roots(&library_id)?;
    Ok(Json(SuccessEnvelope { data }))
}

async fn get_source_root(
    State(state): State<SharedState>,
    Path((library_id, source_root_id)): Path<(String, String)>,
) -> Result<Json<SuccessEnvelope<SourceRootDetailData>>, ApiError> {
    let state = state.read().await;
    let data = state.get_source_root(&library_id, &source_root_id)?;
    Ok(Json(SuccessEnvelope { data }))
}

async fn create_source_root(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
    Json(request): Json<CreateSourceRootRequest>,
) -> Result<(StatusCode, Json<SuccessEnvelope<SourceRootSnapshot>>), ApiError> {
    let mut state = state.write().await;
    let snapshot = state.create_source_root(&library_id, request)?;
    Ok((
        StatusCode::CREATED,
        Json(SuccessEnvelope { data: snapshot }),
    ))
}

async fn update_source_root(
    State(state): State<SharedState>,
    Path((library_id, source_root_id)): Path<(String, String)>,
    Json(request): Json<UpdateSourceRootRequest>,
) -> Result<Json<SuccessEnvelope<SourceRootSnapshot>>, ApiError> {
    let mut state = state.write().await;
    let snapshot = state.update_source_root(&library_id, &source_root_id, request)?;
    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn delete_source_root(
    State(state): State<SharedState>,
    Path((library_id, source_root_id)): Path<(String, String)>,
) -> Result<Json<SuccessEnvelope<SourceRootSnapshot>>, ApiError> {
    let mut state = state.write().await;
    let snapshot = state.delete_source_root(&library_id, &source_root_id)?;
    Ok(Json(SuccessEnvelope { data: snapshot }))
}

async fn list_sources(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
    Query(query): Query<SourcesQuery>,
) -> Result<Json<SuccessEnvelope<SourcesListData>>, ApiError> {
    let state = state.read().await;
    let data = state.list_sources(&library_id, query)?;
    Ok(Json(SuccessEnvelope { data }))
}

async fn refresh_library_sources(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<SourceActionData>>, ApiError> {
    let (response, queued_action) = {
        let mut state = state.write().await;
        state.queue_source_action(
            &library_id,
            SourceActionScope::Library,
            SourceActionKind::Refresh,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )?
    };

    if let Some(queued_action) = queued_action {
        let background_state = state.clone();
        tokio::spawn(async move {
            run_source_action_job(background_state, queued_action.job_id, queued_action.plan).await;
        });
    }

    Ok(Json(SuccessEnvelope { data: response }))
}

async fn rescan_library_sources(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
) -> Result<Json<SuccessEnvelope<SourceActionData>>, ApiError> {
    let (response, queued_action) = {
        let mut state = state.write().await;
        state.queue_source_action(
            &library_id,
            SourceActionScope::Library,
            SourceActionKind::Rescan,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )?
    };

    if let Some(queued_action) = queued_action {
        let background_state = state.clone();
        tokio::spawn(async move {
            run_source_action_job(background_state, queued_action.job_id, queued_action.plan).await;
        });
    }

    Ok(Json(SuccessEnvelope { data: response }))
}

async fn refresh_source_root(
    State(state): State<SharedState>,
    Path((library_id, source_root_id)): Path<(String, String)>,
) -> Result<Json<SuccessEnvelope<SourceActionData>>, ApiError> {
    let (response, queued_action) = {
        let mut state = state.write().await;
        state.queue_source_action(
            &library_id,
            SourceActionScope::SourceRoot(source_root_id),
            SourceActionKind::Refresh,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )?
    };

    if let Some(queued_action) = queued_action {
        let background_state = state.clone();
        tokio::spawn(async move {
            run_source_action_job(background_state, queued_action.job_id, queued_action.plan).await;
        });
    }

    Ok(Json(SuccessEnvelope { data: response }))
}

async fn rescan_source_root(
    State(state): State<SharedState>,
    Path((library_id, source_root_id)): Path<(String, String)>,
) -> Result<Json<SuccessEnvelope<SourceActionData>>, ApiError> {
    let (response, queued_action) = {
        let mut state = state.write().await;
        state.queue_source_action(
            &library_id,
            SourceActionScope::SourceRoot(source_root_id),
            SourceActionKind::Rescan,
            SourceActionTrigger::Manual,
            BTreeMap::new(),
        )?
    };

    if let Some(queued_action) = queued_action {
        let background_state = state.clone();
        tokio::spawn(async move {
            run_source_action_job(background_state, queued_action.job_id, queued_action.plan).await;
        });
    }

    Ok(Json(SuccessEnvelope { data: response }))
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

async fn upload_query_document(
    State(state): State<SharedState>,
    Path(library_id): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<SuccessEnvelope<QueryDocumentAssetData>>), ApiError> {
    let file = read_single_query_document_part(&mut multipart).await?;
    let staged = persist_query_document_asset(file)?;
    let data = {
        let mut state = state.write().await;
        state.register_temp_query_document_asset(&library_id, staged)?
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

async fn get_query_document_preview(
    State(state): State<SharedState>,
    Path((library_id, temp_asset_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let asset = {
        let mut state = state.write().await;
        state.prune_temp_query_assets();
        state.get_temp_query_document_asset(&library_id, &temp_asset_id)?
    };

    let bytes = fs::read(&asset.path)
        .map_err(|_| ApiError::not_found("Query document file is no longer available."))?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&asset.content_type).map_err(|_| {
            ApiError::runtime_unavailable(
                "Query document preview content type is invalid.",
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

async fn search_document(
    State(state): State<SharedState>,
    Json(request): Json<DocumentSearchRequest>,
) -> Result<Json<SuccessEnvelope<TextSearchData>>, ApiError> {
    let (plan, query_input) = {
        let mut state = state.write().await;
        state.prune_temp_query_assets();
        state.prepare_document_search(&request)?
    };

    let query_embedding =
        embed_query_document(query_input.path.as_str(), query_input.locator).await?;
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

async fn run_source_action_job(state: SharedState, job_id: String, plan: SourceActionPlan) {
    {
        let mut state = state.write().await;
        state.mark_source_action_running(&plan, &job_id);
    }

    let prepared = {
        let mut state = state.write().await;
        state.prepare_source_action_execution(&plan)
    };

    let outcome = match prepared {
        Ok(prepared) => {
            if prepared.requires_index_update() {
                {
                    let mut state = state.write().await;
                    state.update_job_snapshot(
                        &job_id,
                        "running",
                        "encode",
                        0,
                        format!(
                            "Encoding {} visual unit(s) for {}.",
                            prepared.visual_units_to_index.len(),
                            plan.action.as_str(),
                        ),
                    );
                }

                let indexing =
                    index_source_action_visual_units(&prepared, state.clone(), &job_id).await;
                match indexing {
                    Ok(()) => {
                        let outcome = SourceActionJobOutcome::completed(&prepared);
                        let mut state = state.write().await;
                        if let Err(message) =
                            state.finalize_source_action_job(&job_id, prepared, outcome)
                        {
                            tracing::warn!(
                                "Failed to finalize source action job {job_id}: {message}"
                            );
                        }
                        return;
                    }
                    Err(message) => {
                        let mut state = state.write().await;
                        if let Err(finalize_error) = state.finalize_source_action_job(
                            &job_id,
                            prepared,
                            SourceActionJobOutcome::failed(plan.action, 0, message),
                        ) {
                            tracing::warn!(
                                "Failed to finalize failed source action job {job_id}: {finalize_error}"
                            );
                        }
                        return;
                    }
                }
            }

            let outcome = SourceActionJobOutcome::completed(&prepared);
            let mut state = state.write().await;
            if let Err(message) = state.finalize_source_action_job(&job_id, prepared, outcome) {
                tracing::warn!("Failed to finalize source action job {job_id}: {message}");
            }
            return;
        }
        Err(message) => SourceActionJobOutcome::failed(plan.action, 0, message),
    };

    let mut state = state.write().await;
    let prepared = PreparedSourceAction {
        library_id: plan.library_id,
        collection_name: String::new(),
        action: plan.action,
        accepted_root_count: plan.target_root_ids.len(),
        can_rebuild_from_scratch: false,
        had_existing_visual_units: false,
        root_updates: Vec::new(),
        source_mutations: Vec::new(),
        stale_point_ids: Vec::new(),
        visual_units_to_index: Vec::new(),
        summary: SourceActionSummary::default(),
    };
    if let Err(message) = state.finalize_source_action_job(&job_id, prepared, outcome) {
        tracing::warn!("Failed to finalize source action job {job_id}: {message}");
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

fn normalize_source_root_rules(rules: SourceRootRulesPayload) -> SourceRootRulesPayload {
    SourceRootRulesPayload {
        include_globs: normalize_rule_globs(rules.include_globs),
        exclude_globs: normalize_rule_globs(rules.exclude_globs),
        include_extensions: normalize_rule_extensions(rules.include_extensions),
    }
}

fn normalize_rule_globs(globs: Vec<String>) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for glob in globs {
        let normalized = glob.trim().replace('\\', "/");
        let normalized = normalized.trim_start_matches("./").trim_matches('/');
        if !normalized.is_empty() {
            unique.insert(normalized.to_string());
        }
    }
    unique.into_iter().collect()
}

fn normalize_rule_extensions(extensions: Vec<String>) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for extension in extensions {
        let normalized = extension
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase();
        if !normalized.is_empty() {
            unique.insert(normalized);
        }
    }
    unique.into_iter().collect()
}

fn normalize_source_root_path(root_path: &str) -> Result<String, ApiError> {
    let trimmed = root_path.trim();
    if trimmed.is_empty() {
        return Err(ApiError::validation_failed(
            "Source root path must not be empty.",
            Some(json!({ "field": "root_path" })),
        ));
    }

    let path = FsPath::new(trimmed);
    if path.exists() {
        let metadata = fs::metadata(path).map_err(|error| {
            ApiError::validation_failed(
                format!("Source root metadata could not be read: {error}"),
                Some(json!({ "field": "root_path", "root_path": trimmed })),
            )
        })?;
        if !metadata.is_dir() {
            return Err(ApiError::validation_failed(
                "Current 140-library-source-management implementation only accepts local directory source roots.",
                Some(json!({ "field": "root_path", "root_path": trimmed })),
            ));
        }
        return Ok(fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string());
    }

    Ok(trimmed.to_string())
}

fn source_root_status_from_scan(enabled: bool, scan: &SourceRootScanResult) -> String {
    if !enabled {
        "disabled".to_string()
    } else if scan.status == "degraded" {
        "degraded".to_string()
    } else {
        "ready".to_string()
    }
}

fn source_root_watch_state(enabled: bool, scan: &SourceRootScanResult, queued: bool) -> String {
    if !enabled {
        "disabled".to_string()
    } else if queued {
        "queued_refresh".to_string()
    } else if scan.status == "degraded" {
        "error".to_string()
    } else {
        "watching".to_string()
    }
}

fn queued_watch_state_for_action(action: SourceActionKind) -> &'static str {
    match action {
        SourceActionKind::Refresh => "queued_refresh",
        SourceActionKind::Rescan => "queued_rescan",
    }
}

fn running_watch_state_for_action(action: SourceActionKind) -> &'static str {
    match action {
        SourceActionKind::Refresh => "refreshing",
        SourceActionKind::Rescan => "rescanning",
    }
}

fn source_root_action_in_flight(root: &SourceRootRecord) -> bool {
    matches!(
        root.watch_state.as_str(),
        "queued_refresh" | "queued_rescan" | "refreshing" | "rescanning"
    )
}

fn count_sources_for_root(library: &LibraryRecord, source_root_id: &str) -> (usize, usize) {
    library
        .sources
        .values()
        .filter(|source| source.source_root_id.as_deref() == Some(source_root_id))
        .fold((0usize, 0usize), |(active, inactive), source| {
            if source.status == "active" {
                (active + 1, inactive)
            } else {
                (active, inactive + 1)
            }
        })
}

fn mark_source_root_sources_state(
    library: &mut LibraryRecord,
    source_root_id: &str,
    status: &str,
    reason: Option<String>,
) {
    let affected_source_ids = library
        .source_order
        .iter()
        .filter_map(|source_id| {
            library
                .sources
                .get(source_id)
                .filter(|source| source.source_root_id.as_deref() == Some(source_root_id))
                .map(|source| source.id.clone())
        })
        .collect::<Vec<_>>();
    let mut removed_visual_unit_ids = BTreeSet::new();

    for source_id in affected_source_ids {
        if let Some(source) = library.sources.get_mut(&source_id) {
            removed_visual_unit_ids.extend(source.visual_unit_ids.iter().cloned());
            source.status = status.to_string();
            source.status_reason = reason.clone();
            source.visual_unit_ids.clear();
            source.observed_size_bytes = None;
            source.observed_modified_at_ms = None;
        }
    }

    if !removed_visual_unit_ids.is_empty() {
        library
            .visual_unit_order
            .retain(|visual_unit_id| !removed_visual_unit_ids.contains(visual_unit_id));
        for visual_unit_id in removed_visual_unit_ids {
            library.visual_units.remove(&visual_unit_id);
        }
    }
}

fn diff_observed_entries(
    previous: &BTreeMap<String, ObservedSourceFile>,
    current: &BTreeMap<String, ObservedSourceFile>,
) -> BTreeSet<String> {
    let mut changed = BTreeSet::new();
    for relative_path in previous.keys().chain(current.keys()) {
        let before = previous.get(relative_path);
        let after = current.get(relative_path);
        if before.map(observed_signature) != after.map(observed_signature) {
            changed.insert(relative_path.clone());
        }
    }
    changed
}

fn observed_signature(entry: &ObservedSourceFile) -> (u64, Option<u128>) {
    (entry.size_bytes, entry.modified_at_ms)
}

fn count_matched_observed_entries(
    observed_entries: &BTreeMap<String, ObservedSourceFile>,
    rules: &SourceRootRulesPayload,
) -> usize {
    observed_entries
        .values()
        .filter(|entry| observed_entry_is_in_scope(entry, rules))
        .count()
}

fn observed_entry_is_in_scope(entry: &ObservedSourceFile, rules: &SourceRootRulesPayload) -> bool {
    source_root_rules_allow_path(&entry.relative_path, rules)
        && observed_entry_extension_allowed(entry, rules)
}

fn observed_entry_extension_allowed(
    entry: &ObservedSourceFile,
    rules: &SourceRootRulesPayload,
) -> bool {
    let extension = FsPath::new(&entry.absolute_path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let Some(extension) = extension else {
        return false;
    };
    if !is_supported_source_extension(&extension) {
        return false;
    }
    rules.include_extensions.is_empty() || rules.include_extensions.contains(&extension)
}

fn source_root_rules_allow_path(relative_path: &str, rules: &SourceRootRulesPayload) -> bool {
    let normalized_path = relative_path.replace('\\', "/");
    let included = rules.include_globs.is_empty()
        || rules
            .include_globs
            .iter()
            .any(|pattern| glob_pattern_matches(pattern, &normalized_path));
    if !included {
        return false;
    }

    !rules
        .exclude_globs
        .iter()
        .any(|pattern| glob_pattern_matches(pattern, &normalized_path))
}

fn glob_pattern_matches(pattern: &str, relative_path: &str) -> bool {
    let pattern_segments = pattern
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let path_segments = relative_path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    glob_segments_match(&pattern_segments, &path_segments)
}

fn glob_segments_match(pattern_segments: &[&str], path_segments: &[&str]) -> bool {
    if pattern_segments.is_empty() {
        return path_segments.is_empty();
    }
    if pattern_segments[0] == "**" {
        return glob_segments_match(&pattern_segments[1..], path_segments)
            || (!path_segments.is_empty()
                && glob_segments_match(pattern_segments, &path_segments[1..]));
    }
    !path_segments.is_empty()
        && wildcard_segment_matches(pattern_segments[0], path_segments[0])
        && glob_segments_match(&pattern_segments[1..], &path_segments[1..])
}

fn wildcard_segment_matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.chars().collect::<Vec<_>>();
    let value = value.chars().collect::<Vec<_>>();
    let mut dp = vec![vec![false; value.len() + 1]; pattern.len() + 1];
    dp[0][0] = true;

    for pattern_index in 0..pattern.len() {
        match pattern[pattern_index] {
            '*' => {
                for value_index in 0..=value.len() {
                    if dp[pattern_index][value_index] {
                        dp[pattern_index + 1][value_index] = true;
                        if value_index < value.len() {
                            dp[pattern_index][value_index + 1] = true;
                        }
                    }
                }
            }
            '?' => {
                for value_index in 0..value.len() {
                    if dp[pattern_index][value_index] {
                        dp[pattern_index + 1][value_index + 1] = true;
                    }
                }
            }
            expected => {
                for value_index in 0..value.len() {
                    if dp[pattern_index][value_index] && value[value_index] == expected {
                        dp[pattern_index + 1][value_index + 1] = true;
                    }
                }
            }
        }
    }

    dp[pattern.len()][value.len()]
}

fn planned_source_action_paths(
    plan: &SourceActionPlan,
    root: &SourceRootRecord,
    candidate_by_relative_path: &BTreeMap<String, ObservedSourceFile>,
    existing_by_relative_path: &BTreeMap<String, SourceRecord>,
) -> BTreeSet<String> {
    if plan.action.is_rescan() {
        return candidate_by_relative_path
            .keys()
            .chain(existing_by_relative_path.keys())
            .cloned()
            .collect();
    }

    if let Some(paths) = plan.changed_paths_by_root.get(&root.id) {
        return paths.clone();
    }

    let mut affected = BTreeSet::new();
    for (relative_path, entry) in candidate_by_relative_path {
        let current_source = existing_by_relative_path.get(relative_path);
        let unchanged = current_source
            .map(|source| {
                source.status == "active"
                    && source.observed_size_bytes == Some(entry.size_bytes)
                    && source.observed_modified_at_ms == entry.modified_at_ms
            })
            .unwrap_or(false);
        if !unchanged {
            affected.insert(relative_path.clone());
        }
    }

    for relative_path in existing_by_relative_path.keys() {
        if !candidate_by_relative_path.contains_key(relative_path) {
            affected.insert(relative_path.clone());
        }
    }

    affected
}

fn invalidated_source_record(
    mut source: SourceRecord,
    status: &str,
    reason: Option<String>,
    observed_size_bytes: Option<u64>,
    observed_modified_at_ms: Option<u128>,
) -> SourceRecord {
    source.status = status.to_string();
    source.status_reason = reason;
    source.visual_unit_ids.clear();
    source.observed_size_bytes = observed_size_bytes;
    source.observed_modified_at_ms = observed_modified_at_ms;
    source
}

fn out_of_scope_status_reason(
    observed: &ObservedSourceFile,
    rules: &SourceRootRulesPayload,
) -> (String, Option<String>) {
    if !source_root_rules_allow_path(&observed.relative_path, rules) {
        return (
            "out_of_scope".to_string(),
            Some("rule_excluded".to_string()),
        );
    }

    let extension = FsPath::new(&observed.absolute_path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let Some(extension) = extension else {
        return (
            "out_of_scope".to_string(),
            Some("unsupported_type".to_string()),
        );
    };
    if !is_supported_source_extension(&extension) {
        return (
            "out_of_scope".to_string(),
            Some("unsupported_type".to_string()),
        );
    }
    if !rules.include_extensions.is_empty() && !rules.include_extensions.contains(&extension) {
        return (
            "out_of_scope".to_string(),
            Some("extension_filtered".to_string()),
        );
    }

    (
        "out_of_scope".to_string(),
        Some("outside_coverage".to_string()),
    )
}

fn is_supported_source_extension(extension: &str) -> bool {
    matches!(
        extension,
        "pdf" | "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" | "mp4" | "mov" | "m4v"
    )
}

fn scan_source_root_directory(root_path: &str) -> SourceRootScanResult {
    let trimmed = root_path.trim();
    if trimmed.is_empty() {
        return SourceRootScanResult {
            status: "degraded".to_string(),
            observed_entries: BTreeMap::new(),
            error: Some("Source root path must not be empty.".to_string()),
        };
    }

    let root = FsPath::new(trimmed);
    let metadata = match fs::metadata(root) {
        Ok(metadata) => metadata,
        Err(error) => {
            return SourceRootScanResult {
                status: "degraded".to_string(),
                observed_entries: BTreeMap::new(),
                error: Some(format!("Source root metadata could not be read: {error}")),
            }
        }
    };
    if !metadata.is_dir() {
        return SourceRootScanResult {
            status: "degraded".to_string(),
            observed_entries: BTreeMap::new(),
            error: Some("Source root path is not a directory.".to_string()),
        };
    }

    let canonical_root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut observed_entries = BTreeMap::new();
    match collect_source_root_files(&canonical_root, &canonical_root, &mut observed_entries) {
        Ok(()) => SourceRootScanResult {
            status: "ready".to_string(),
            observed_entries,
            error: None,
        },
        Err(error) => SourceRootScanResult {
            status: "degraded".to_string(),
            observed_entries: BTreeMap::new(),
            error: Some(error),
        },
    }
}

fn collect_source_root_files(
    root: &FsPath,
    current: &FsPath,
    observed_entries: &mut BTreeMap<String, ObservedSourceFile>,
) -> Result<(), String> {
    let entries = fs::read_dir(current)
        .map_err(|error| format!("Source root directory could not be read: {error}"))?;

    for entry in entries {
        let entry =
            entry.map_err(|error| format!("Source root entry could not be read: {error}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Source root entry type could not be read: {error}"))?;

        if file_type.is_dir() {
            collect_source_root_files(root, &path, observed_entries)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let metadata = entry
            .metadata()
            .map_err(|error| format!("Source root file metadata could not be read: {error}"))?;
        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        observed_entries.insert(
            relative_path.clone(),
            ObservedSourceFile {
                absolute_path: fs::canonicalize(&path)
                    .unwrap_or(path.clone())
                    .to_string_lossy()
                    .to_string(),
                relative_path,
                size_bytes: metadata.len(),
                modified_at_ms: metadata.modified().ok().and_then(system_time_to_unix_ms),
            },
        );
    }

    Ok(())
}

fn system_time_to_unix_ms(value: SystemTime) -> Option<u128> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis())
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

async fn read_single_query_document_part(
    multipart: &mut Multipart,
) -> Result<IncomingQueryDocumentUpload, ApiError> {
    let mut file_upload: Option<IncomingQueryDocumentUpload> = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        ApiError::validation_failed(
            format!("Query document upload could not be parsed: {error}"),
            Some(json!({ "field": "file" })),
        )
    })? {
        let filename = field.file_name().map(|value| value.to_string());
        let content_type = field
            .content_type()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let extension = infer_query_document_extension(filename.as_deref(), &content_type)
            .ok_or_else(|| {
                ApiError::validation_failed(
                    "Only PDF files are accepted as query documents right now.",
                    Some(json!({
                        "field": "file",
                        "content_type": content_type,
                        "filename": filename,
                    })),
                )
            })?;
        let bytes = field.bytes().await.map_err(|error| {
            ApiError::validation_failed(
                format!("Query document upload body could not be read: {error}"),
                Some(json!({ "field": "file" })),
            )
        })?;

        if bytes.is_empty() {
            return Err(ApiError::validation_failed(
                "Query document upload must not be empty.",
                Some(json!({ "field": "file" })),
            ));
        }
        if file_upload.is_some() {
            return Err(ApiError::validation_failed(
                "Current 130-document-search implementation accepts exactly one query document per upload.",
                Some(json!({ "field": "file" })),
            ));
        }

        file_upload = Some(IncomingQueryDocumentUpload {
            bytes: bytes.to_vec(),
            content_type,
            original_filename: filename,
            extension,
        });
    }

    file_upload.ok_or_else(|| {
        ApiError::validation_failed(
            "Query document upload requires one document file part.",
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

fn infer_query_document_extension(filename: Option<&str>, content_type: &str) -> Option<String> {
    let by_filename = filename
        .and_then(|name| FsPath::new(name).extension())
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| is_supported_query_document_extension(value));
    if by_filename.is_some() {
        return by_filename;
    }

    match content_type {
        "application/pdf" => Some("pdf".to_string()),
        _ => None,
    }
}

fn is_supported_query_image_extension(extension: &str) -> bool {
    matches!(extension, "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif")
}

fn is_supported_query_video_extension(extension: &str) -> bool {
    matches!(extension, "mp4" | "mov" | "m4v")
}

fn is_supported_query_document_extension(extension: &str) -> bool {
    matches!(extension, "pdf")
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
        page_count: None,
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
        ApiError::validation_failed(message, Some(json!({ "field": "file" })))
    })?;

    Ok(StagedQueryAsset {
        path: path.to_string_lossy().to_string(),
        source_type: "video".to_string(),
        content_type: upload.content_type,
        original_filename: upload.original_filename,
        page_count: None,
        duration_ms: Some(duration_ms),
    })
}

fn persist_query_document_asset(
    upload: IncomingQueryDocumentUpload,
) -> Result<StagedQueryAsset, ApiError> {
    let runtime_dir = read_required_env("APP_RUNTIME_DIR")?;
    let target_dir = FsPath::new(&runtime_dir)
        .join("temp-assets")
        .join("documents");
    fs::create_dir_all(&target_dir).map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Query document asset directory could not be created: {error}"),
            Some(json!({ "path": target_dir })),
        )
    })?;

    let filename = format!("query-document-{}.{}", runtime_token(), upload.extension);
    let path = target_dir.join(filename);
    fs::write(&path, upload.bytes).map_err(|error| {
        ApiError::runtime_unavailable(
            format!("Query document asset could not be written: {error}"),
            Some(json!({ "path": path })),
        )
    })?;

    let page_count = pdf_page_count(&path).map_err(|message| {
        remove_temp_query_asset_file(path.to_string_lossy().as_ref());
        ApiError::validation_failed(message, Some(json!({ "field": "file" })))
    })?;

    Ok(StagedQueryAsset {
        path: path.to_string_lossy().to_string(),
        source_type: "pdf".to_string(),
        content_type: upload.content_type,
        original_filename: upload.original_filename,
        page_count: Some(page_count),
        duration_ms: None,
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
    let step_ms = VIDEO_SEGMENT_WINDOW_MS
        .saturating_sub(VIDEO_SEGMENT_OVERLAP_MS)
        .max(1);
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
    let start_ms = locator
        .get("start_ms")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ApiError::validation_failed(
                "Video locator must include integer start_ms.",
                Some(json!({ "field": format!("{field_name}.start_ms") })),
            )
        })?;
    let end_ms = locator
        .get("end_ms")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
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

fn resolve_document_query_locator(
    locator: Option<&Value>,
    page_count: Option<usize>,
    field_name: &str,
) -> Result<Option<Value>, ApiError> {
    let page_count = page_count.ok_or_else(|| {
        ApiError::runtime_unavailable(
            "Document page count is unavailable for the selected query input.",
            Some(json!({ "field": field_name })),
        )
    })?;

    let Some(locator) = locator else {
        return Ok(None);
    };

    let start_page = locator
        .get("start_page")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ApiError::validation_failed(
                "Document locator must include integer start_page.",
                Some(json!({ "field": format!("{field_name}.start_page") })),
            )
        })?;
    let end_page = locator
        .get("end_page")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ApiError::validation_failed(
                "Document locator must include integer end_page.",
                Some(json!({ "field": format!("{field_name}.end_page") })),
            )
        })?;
    if start_page < 1 || end_page < start_page || end_page > page_count as u64 {
        return Err(ApiError::validation_failed(
            "Document locator must satisfy 1 <= start_page <= end_page <= page_count.",
            Some(json!({
                "field": field_name,
                "start_page": start_page,
                "end_page": end_page,
                "page_count": page_count,
            })),
        ));
    }

    Ok(Some(json!({
        "start_page": start_page,
        "end_page": end_page,
        "page_count": page_count,
    })))
}

async fn index_visual_units(
    prepared: &PreparedImport,
    state: SharedState,
    job_id: &str,
) -> Result<String, IndexingError> {
    let batch_items = index_embed_batch_items();
    let total_batches = batch_count(prepared.visual_units.len(), batch_items);
    let stage_collection_name =
        staging_collection_name(&prepared.library_id, MULTIVECTOR_INDEX_LINE, job_id);
    let write_plan = resolve_qdrant_namespace_write_plan(
        &prepared.collection_name,
        &stage_collection_name,
        !prepared.had_existing_visual_units,
    )
    .await
    .map_err(|message| IndexingError {
        phase: "stage_write",
        message,
        completed: 0,
    })?;
    let mut stage_initialized = false;

    if matches!(
        write_plan.stage_strategy,
        StageCollectionStrategy::CloneFromActive { .. }
    ) {
        create_qdrant_stage_collection(&write_plan, None)
            .await
            .map_err(|message| IndexingError {
                phase: "stage_write",
                message,
                completed: 0,
            })?;
        stage_initialized = true;
    }

    for (batch_index, visual_unit_batch) in prepared.visual_units.chunks(batch_items).enumerate() {
        {
            let mut state = state.write().await;
            state.update_job_snapshot(
                job_id,
                "running",
                "encode",
                0,
                format!(
                    "Encoding batch {}/{} ({} visual unit(s)) for staged multivector indexing.",
                    batch_index + 1,
                    total_batches,
                    visual_unit_batch.len()
                ),
            );
        }

        let embeddings = match embed_documents(visual_unit_batch).await {
            Ok(embeddings) => embeddings,
            Err(error) => {
                if stage_initialized {
                    best_effort_delete_qdrant_collection(&stage_collection_name).await;
                }
                return Err(error);
            }
        };

        {
            let mut state = state.write().await;
            state.update_job_snapshot(
                job_id,
                "running",
                "stage_write",
                0,
                format!(
                    "Writing batch {}/{} ({} visual unit(s)) into staged multivector storage.",
                    batch_index + 1,
                    total_batches,
                    visual_unit_batch.len()
                ),
            );
        }

        if !stage_initialized {
            let vector_size = embeddings
                .first()
                .and_then(|embedding| embedding.vectors.first())
                .map(Vec::len)
                .unwrap_or_default();
            if let Err(message) =
                create_qdrant_stage_collection(&write_plan, Some(vector_size)).await
            {
                return Err(IndexingError {
                    phase: "stage_write",
                    message,
                    completed: 0,
                });
            }
            stage_initialized = true;
        }

        if let Err(message) =
            upsert_qdrant_points(&stage_collection_name, visual_unit_batch, &embeddings).await
        {
            best_effort_delete_qdrant_collection(&stage_collection_name).await;
            return Err(IndexingError {
                phase: "stage_write",
                message,
                completed: 0,
            });
        }
    }

    if !stage_initialized {
        return Err(IndexingError {
            phase: "stage_write",
            message: "No staged Qdrant collection was created for the import job.".to_string(),
            completed: 0,
        });
    }

    if let Err(message) = validate_qdrant_collection(&stage_collection_name).await {
        best_effort_delete_qdrant_collection(&stage_collection_name).await;
        return Err(IndexingError {
            phase: "stage_write",
            message,
            completed: 0,
        });
    }

    if let Err(message) = switch_qdrant_active_alias(&write_plan).await {
        best_effort_delete_qdrant_collection(&stage_collection_name).await;
        return Err(IndexingError {
            phase: "activated",
            message,
            completed: prepared.accepted.len(),
        });
    }
    best_effort_cleanup_retired_stage_collections(&write_plan).await;

    Ok(format!(
        "Accepted {} path(s); indexed {} visual unit(s) into staged multivector storage and atomically activated the active namespace.",
        prepared.accepted.len(),
        prepared.visual_units.len()
    ))
}

async fn index_source_action_visual_units(
    prepared: &PreparedSourceAction,
    state: SharedState,
    job_id: &str,
) -> Result<(), String> {
    let batch_items = index_embed_batch_items();
    let total_batches = batch_count(prepared.visual_units_to_index.len(), batch_items);
    let stage_collection_name =
        staging_collection_name(&prepared.library_id, MULTIVECTOR_INDEX_LINE, job_id);
    let write_plan = resolve_qdrant_namespace_write_plan(
        &prepared.collection_name,
        &stage_collection_name,
        !prepared.had_existing_visual_units || prepared.can_rebuild_from_scratch,
    )
    .await?;
    let mut stage_initialized = false;

    if matches!(
        write_plan.stage_strategy,
        StageCollectionStrategy::CloneFromActive { .. }
    ) {
        create_qdrant_stage_collection(&write_plan, None).await?;
        stage_initialized = true;
    }

    if stage_initialized && !prepared.stale_point_ids.is_empty() {
        {
            let mut state = state.write().await;
            state.update_job_snapshot(
                job_id,
                "running",
                "stage_write",
                0,
                format!(
                    "Deleting {} stale point(s) from staged multivector storage.",
                    prepared.stale_point_ids.len()
                ),
            );
        }
        if let Err(message) =
            delete_qdrant_points(&stage_collection_name, &prepared.stale_point_ids).await
        {
            best_effort_delete_qdrant_collection(&stage_collection_name).await;
            return Err(message);
        }
    }

    for (batch_index, visual_unit_batch) in prepared
        .visual_units_to_index
        .chunks(batch_items)
        .enumerate()
    {
        {
            let mut state = state.write().await;
            state.update_job_snapshot(
                job_id,
                "running",
                "encode",
                0,
                format!(
                    "Encoding batch {}/{} ({} visual unit(s)) for {}.",
                    batch_index + 1,
                    total_batches,
                    visual_unit_batch.len(),
                    prepared.action.as_str(),
                ),
            );
        }

        let embeddings = match embed_documents(visual_unit_batch).await {
            Ok(embeddings) => embeddings,
            Err(error) => {
                if stage_initialized {
                    best_effort_delete_qdrant_collection(&stage_collection_name).await;
                }
                return Err(error.message);
            }
        };

        {
            let mut state = state.write().await;
            state.update_job_snapshot(
                job_id,
                "running",
                "stage_write",
                0,
                format!(
                    "Writing batch {}/{} ({} visual unit(s)) into staged multivector storage.",
                    batch_index + 1,
                    total_batches,
                    visual_unit_batch.len()
                ),
            );
        }

        if !stage_initialized {
            let vector_size = embeddings
                .first()
                .and_then(|embedding| embedding.vectors.first())
                .map(Vec::len)
                .unwrap_or_default();
            create_qdrant_stage_collection(&write_plan, Some(vector_size)).await?;
            stage_initialized = true;
        }

        if let Err(message) =
            upsert_qdrant_points(&stage_collection_name, visual_unit_batch, &embeddings).await
        {
            best_effort_delete_qdrant_collection(&stage_collection_name).await;
            return Err(message);
        }
    }

    if !stage_initialized {
        return Ok(());
    }

    validate_qdrant_collection(&stage_collection_name).await?;
    if let Err(message) = switch_qdrant_active_alias(&write_plan).await {
        best_effort_delete_qdrant_collection(&stage_collection_name).await;
        return Err(message);
    }
    best_effort_cleanup_retired_stage_collections(&write_plan).await;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StageCollectionStrategy {
    Fresh,
    CloneFromActive { target_collection: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QdrantNamespaceWritePlan {
    alias_name: String,
    alias_exists: bool,
    previous_target_collection: Option<String>,
    stage_collection_name: String,
    stage_strategy: StageCollectionStrategy,
}

fn index_embed_batch_items() -> usize {
    read_optional_usize_env("INDEX_EMBED_BATCH_ITEMS", DEFAULT_INDEX_EMBED_BATCH_ITEMS).max(1)
}

fn qdrant_max_upsert_body_bytes() -> usize {
    read_optional_usize_env(
        "INDEX_QDRANT_UPSERT_BODY_BYTES",
        DEFAULT_QDRANT_MAX_UPSERT_BODY_BYTES,
    )
}

fn read_optional_usize_env(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

fn batch_count(total_items: usize, batch_items: usize) -> usize {
    if total_items == 0 {
        0
    } else {
        ((total_items - 1) / batch_items) + 1
    }
}

fn staging_collection_name(library_id: &str, index_line: &str, job_id: &str) -> String {
    format!("index_stage_{library_id}_{index_line}_{job_id}")
}

async fn resolve_qdrant_namespace_write_plan(
    alias_name: &str,
    stage_collection_name: &str,
    allow_fresh_without_active: bool,
) -> Result<QdrantNamespaceWritePlan, String> {
    match probe_active_qdrant_namespace(alias_name).await? {
        ActiveNamespaceProbeResult::Ready { target_collection } => Ok(QdrantNamespaceWritePlan {
            alias_name: alias_name.to_string(),
            alias_exists: true,
            previous_target_collection: Some(target_collection.clone()),
            stage_collection_name: stage_collection_name.to_string(),
            stage_strategy: StageCollectionStrategy::CloneFromActive { target_collection },
        }),
        ActiveNamespaceProbeResult::Missing => {
            if !allow_fresh_without_active {
                return Err(
                    "The active multivector namespace is missing. Run a full library rescan to rebuild the index before applying incremental updates."
                        .to_string(),
                );
            }
            Ok(QdrantNamespaceWritePlan {
                alias_name: alias_name.to_string(),
                alias_exists: false,
                previous_target_collection: None,
                stage_collection_name: stage_collection_name.to_string(),
                stage_strategy: StageCollectionStrategy::Fresh,
            })
        }
        ActiveNamespaceProbeResult::MissingTarget { target_collection } => {
            if !allow_fresh_without_active {
                return Err(format!(
                    "The active multivector namespace alias points to missing collection {target_collection}. Run a full library rescan to rebuild the index before applying incremental updates."
                ));
            }
            Ok(QdrantNamespaceWritePlan {
                alias_name: alias_name.to_string(),
                alias_exists: true,
                previous_target_collection: Some(target_collection),
                stage_collection_name: stage_collection_name.to_string(),
                stage_strategy: StageCollectionStrategy::Fresh,
            })
        }
        ActiveNamespaceProbeResult::LegacyDirectCollection => Err(format!(
            "Legacy direct Qdrant collection {alias_name} blocks the active alias namespace. Remove the old physical index_* collection manually, then retry."
        )),
    }
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

async fn embed_query_document(
    path: &str,
    locator: Option<Value>,
) -> Result<QueryEmbeddingResult, ApiError> {
    let payload = json!({
        "operation_kind": "document_query_embedding",
        "inputs": {
            "documents": [{
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
                format!("Sidecar document query embedding request failed: {error}"),
                Some(json!({ "service": "sidecar" })),
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let message = parse_sidecar_error_message(&body).unwrap_or_else(|| {
            format!("Sidecar document query embedding request failed with {status}.")
        });
        return Err(ApiError::runtime_unavailable(
            message,
            Some(json!({ "service": "sidecar" })),
        ));
    }

    let envelope: SidecarEnvelope<SidecarEmbedPayload> =
        response.json().await.map_err(|error| {
            ApiError::runtime_unavailable(
                format!("Sidecar document query embedding response was invalid JSON: {error}"),
                Some(json!({ "service": "sidecar" })),
            )
        })?;
    let embedding = envelope.data.embeddings.into_iter().next().ok_or_else(|| {
        ApiError::runtime_unavailable(
            "Sidecar document query embedding response did not include any embeddings.",
            Some(json!({ "service": "sidecar" })),
        )
    })?;

    let pooled_vector = if embedding.pooled_vector.is_empty() {
        mean_pool_vectors(&embedding.vectors).ok_or_else(|| {
            ApiError::runtime_unavailable(
                "Sidecar document query embedding response did not include usable vectors.",
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

fn build_qdrant_collection_create_payload(vector_size: usize, init_from: Option<&str>) -> Value {
    let mut payload = json!({
        "vectors": {
            "mv": {
                "size": vector_size,
                "distance": "Cosine",
                "on_disk": true,
                "multivector_config": {
                    "comparator": "max_sim"
                }
            },
            "prefetch_dense": {
                "size": vector_size,
                "distance": "Cosine",
                "on_disk": true
            }
        }
    });
    if let Some(source_collection) = init_from {
        payload["init_from"] = json!({ "collection": source_collection });
    }
    payload
}

async fn create_qdrant_collection(
    collection_name: &str,
    vector_size: usize,
    init_from: Option<&str>,
) -> Result<(), String> {
    let base_url = qdrant_base_url().map_err(|error| error.payload.message)?;
    let client = qdrant_client();
    let collection_url = format!("{}/collections/{}", base_url, collection_name);
    let payload = build_qdrant_collection_create_payload(vector_size, init_from);
    let create_response = client
        .put(&collection_url)
        .json(&payload)
        .send()
        .await
        .map_err(|error| format!("Qdrant collection creation failed: {error}"))?;

    if create_response.status().is_success() {
        Ok(())
    } else {
        let status = create_response.status();
        let body = create_response.text().await.unwrap_or_default();
        Err(format!(
            "Qdrant collection creation for {collection_name} failed with {}: {}.",
            status,
            qdrant_error_detail(&body)
        ))
    }
}

async fn create_qdrant_stage_collection(
    write_plan: &QdrantNamespaceWritePlan,
    vector_size: Option<usize>,
) -> Result<(), String> {
    match &write_plan.stage_strategy {
        StageCollectionStrategy::Fresh => {
            create_qdrant_collection(
                &write_plan.stage_collection_name,
                vector_size.ok_or_else(|| {
                    format!(
                        "Qdrant stage {} requires a known vector size before creation.",
                        write_plan.stage_collection_name
                    )
                })?,
                None,
            )
            .await
        }
        StageCollectionStrategy::CloneFromActive { target_collection } => {
            let vector_size = match vector_size {
                Some(vector_size) => vector_size,
                None => qdrant_collection_vector_size(target_collection).await?,
            };
            create_qdrant_collection(
                &write_plan.stage_collection_name,
                vector_size,
                Some(target_collection),
            )
            .await
        }
    }
}

async fn qdrant_collection_exists(collection_name: &str) -> Result<bool, String> {
    let collection_url = format!(
        "{}/collections/{}",
        qdrant_base_url().map_err(|error| error.payload.message)?,
        collection_name
    );
    let response = qdrant_client()
        .get(&collection_url)
        .send()
        .await
        .map_err(|error| format!("Qdrant collection probe failed: {error}"))?;

    if response.status().is_success() {
        Ok(true)
    } else if response.status() == StatusCode::NOT_FOUND {
        Ok(false)
    } else {
        Err(format!(
            "Qdrant collection probe for {collection_name} failed with {}.",
            response.status()
        ))
    }
}

async fn validate_qdrant_collection(collection_name: &str) -> Result<(), String> {
    match qdrant_collection_exists(collection_name).await? {
        true => Ok(()),
        false => Err(format!(
            "Qdrant staged collection {collection_name} was not found after write completion."
        )),
    }
}

async fn qdrant_collection_vector_size(collection_name: &str) -> Result<usize, String> {
    let response = qdrant_client()
        .get(format!(
            "{}/collections/{}",
            qdrant_base_url().map_err(|error| error.payload.message)?,
            collection_name
        ))
        .send()
        .await
        .map_err(|error| format!("Qdrant collection info probe failed: {error}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Qdrant collection info probe for {collection_name} failed with {}: {}.",
            status,
            qdrant_error_detail(&body)
        ));
    }

    let payload: Value = response
        .json()
        .await
        .map_err(|error| format!("Qdrant collection info response was invalid JSON: {error}"))?;

    payload
        .pointer("/result/config/params/vectors/mv/size")
        .and_then(Value::as_u64)
        .or_else(|| {
            payload
                .pointer("/result/config/params/vectors/size")
                .and_then(Value::as_u64)
        })
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            format!(
                "Qdrant collection info for {collection_name} did not expose a usable vector size."
            )
        })
}

async fn list_qdrant_aliases() -> Result<Vec<Value>, String> {
    let response = qdrant_client()
        .get(format!(
            "{}/aliases",
            qdrant_base_url().map_err(|error| error.payload.message)?
        ))
        .send()
        .await
        .map_err(|error| format!("Qdrant alias listing failed: {error}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Qdrant alias listing failed with {}: {}.",
            status,
            qdrant_error_detail(&body)
        ));
    }

    let payload: Value = response
        .json()
        .await
        .map_err(|error| format!("Qdrant alias listing response was invalid JSON: {error}"))?;
    Ok(payload
        .pointer("/result/aliases")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

async fn list_qdrant_collections() -> Result<Vec<String>, String> {
    let response = qdrant_client()
        .get(format!(
            "{}/collections",
            qdrant_base_url().map_err(|error| error.payload.message)?
        ))
        .send()
        .await
        .map_err(|error| format!("Qdrant collection listing failed: {error}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Qdrant collection listing failed with {}: {}.",
            status,
            qdrant_error_detail(&body)
        ));
    }

    let payload: Value = response
        .json()
        .await
        .map_err(|error| format!("Qdrant collection listing response was invalid JSON: {error}"))?;
    Ok(payload
        .pointer("/result/collections")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("name").and_then(Value::as_str))
        .map(str::to_string)
        .collect())
}

async fn qdrant_alias_target(alias_name: &str) -> Result<Option<String>, String> {
    for alias in list_qdrant_aliases().await? {
        let matches = alias
            .get("alias_name")
            .and_then(Value::as_str)
            .map(|value| value == alias_name)
            .unwrap_or(false);
        if matches {
            return Ok(alias
                .get("collection_name")
                .and_then(Value::as_str)
                .map(str::to_string));
        }
    }
    Ok(None)
}

async fn probe_active_qdrant_namespace(
    alias_name: &str,
) -> Result<ActiveNamespaceProbeResult, String> {
    if let Some(target_collection) = qdrant_alias_target(alias_name).await? {
        return match qdrant_collection_exists(&target_collection).await? {
            true => Ok(ActiveNamespaceProbeResult::Ready { target_collection }),
            false => Ok(ActiveNamespaceProbeResult::MissingTarget { target_collection }),
        };
    }

    if qdrant_collection_exists(alias_name).await? {
        Ok(ActiveNamespaceProbeResult::LegacyDirectCollection)
    } else {
        Ok(ActiveNamespaceProbeResult::Missing)
    }
}

async fn switch_qdrant_active_alias(write_plan: &QdrantNamespaceWritePlan) -> Result<(), String> {
    let mut actions = Vec::new();
    if write_plan.alias_exists {
        actions.push(json!({
            "delete_alias": {
                "alias_name": write_plan.alias_name,
            }
        }));
    }
    actions.push(json!({
        "create_alias": {
            "collection_name": write_plan.stage_collection_name,
            "alias_name": write_plan.alias_name,
        }
    }));
    let response = qdrant_client()
        .post(format!(
            "{}/collections/aliases",
            qdrant_base_url().map_err(|error| error.payload.message)?
        ))
        .json(&json!({ "actions": actions }))
        .send()
        .await
        .map_err(|error| format!("Qdrant alias activation failed: {error}"))?;

    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(format!(
            "Qdrant alias activation for {} failed with {}: {}.",
            write_plan.alias_name,
            status,
            qdrant_error_detail(&body)
        ))
    }
}

async fn delete_qdrant_points(collection_name: &str, point_ids: &[u64]) -> Result<(), String> {
    if point_ids.is_empty() {
        return Ok(());
    }

    let response = qdrant_client()
        .post(format!(
            "{}/collections/{}/points/delete?wait=true",
            qdrant_base_url().map_err(|error| error.payload.message)?,
            collection_name
        ))
        .json(&json!({ "points": point_ids }))
        .send()
        .await
        .map_err(|error| format!("Qdrant delete request failed: {error}"))?;

    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(format!(
            "Qdrant delete request for {collection_name} failed with {}: {}.",
            status,
            qdrant_error_detail(&body)
        ))
    }
}

async fn delete_qdrant_collection(collection_name: &str) -> Result<(), String> {
    let response = qdrant_client()
        .delete(format!(
            "{}/collections/{}",
            qdrant_base_url().map_err(|error| error.payload.message)?,
            collection_name
        ))
        .send()
        .await
        .map_err(|error| format!("Qdrant collection deletion failed: {error}"))?;

    if response.status().is_success() || response.status() == StatusCode::NOT_FOUND {
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(format!(
            "Qdrant collection deletion for {collection_name} failed with {}: {}.",
            status,
            qdrant_error_detail(&body)
        ))
    }
}

async fn best_effort_delete_qdrant_collection(collection_name: &str) {
    if let Err(error) = delete_qdrant_collection(collection_name).await {
        tracing::warn!(
            collection_name = %collection_name,
            "Failed to delete staged Qdrant collection during cleanup: {error}"
        );
    }
}

async fn best_effort_cleanup_retired_stage_collections(write_plan: &QdrantNamespaceWritePlan) {
    let mut keep = BTreeSet::from([write_plan.stage_collection_name.clone()]);
    if let Some(previous_target_collection) = write_plan.previous_target_collection.clone() {
        keep.insert(previous_target_collection);
    }

    let Some(namespace_tail) = write_plan.alias_name.strip_prefix("index_") else {
        return;
    };
    let prefix = format!("index_stage_{namespace_tail}_");
    let collections = match list_qdrant_collections().await {
        Ok(collections) => collections,
        Err(error) => {
            tracing::warn!("Failed to list Qdrant collections for staging cleanup: {error}");
            return;
        }
    };

    for collection_name in collections {
        if !collection_name.starts_with(&prefix) || keep.contains(&collection_name) {
            continue;
        }
        if let Err(error) = delete_qdrant_collection(&collection_name).await {
            tracing::warn!(
                collection_name = %collection_name,
                "Failed to delete retired staged Qdrant collection: {error}"
            );
        }
    }
}

async fn upsert_qdrant_points(
    collection_name: &str,
    visual_units: &[VisualUnitRecord],
    embeddings: &[SidecarEmbeddingItem],
) -> Result<(), String> {
    let max_body_bytes = qdrant_max_upsert_body_bytes();
    if max_body_bytes <= QDRANT_UPSERT_BODY_OVERHEAD_BYTES {
        return Err(
            "Qdrant upsert body limit must be larger than the request envelope.".to_string(),
        );
    }

    let mut chunk_index = 0usize;
    let mut current_chunk = Vec::new();
    let mut current_size = QDRANT_UPSERT_BODY_OVERHEAD_BYTES;
    for (visual_unit, embedding) in visual_units.iter().zip(embeddings.iter()) {
        let point = build_qdrant_point((visual_unit, embedding));
        let point_size = serde_json::to_vec(&point)
            .map_err(|error| format!("Failed to serialize Qdrant point payload: {error}"))?
            .len();
        let separator_size = usize::from(!current_chunk.is_empty());
        let next_size = current_size + separator_size + point_size;

        if !current_chunk.is_empty() && next_size > max_body_bytes {
            chunk_index += 1;
            send_qdrant_point_chunk(collection_name, chunk_index, &current_chunk).await?;
            current_chunk.clear();
            current_size = QDRANT_UPSERT_BODY_OVERHEAD_BYTES;
        }

        current_size += usize::from(!current_chunk.is_empty()) + point_size;
        current_chunk.push(point);
    }

    if !current_chunk.is_empty() {
        chunk_index += 1;
        send_qdrant_point_chunk(collection_name, chunk_index, &current_chunk).await?;
    }

    Ok(())
}

async fn send_qdrant_point_chunk(
    collection_name: &str,
    chunk_index: usize,
    points_chunk: &[Value],
) -> Result<(), String> {
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
                "Qdrant upsert request for {collection_name} chunk {chunk_index} failed: {error}"
            )
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let detail = qdrant_error_detail(&body);
        return Err(format!(
            "Qdrant upsert for {collection_name} chunk {chunk_index} failed with {}: {}.",
            status, detail
        ));
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

#[allow(dead_code)]
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
            plan.active_visual_unit_ids
                .contains(&payload.visual_unit_id)
                && plan
                    .kind_filter
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
        let start_seconds =
            locator.get("start_ms").and_then(Value::as_u64).unwrap_or(0) as f64 / 1000.0;
        let end_seconds =
            locator.get("end_ms").and_then(Value::as_u64).unwrap_or(0) as f64 / 1000.0;
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

fn query_document_preview_reference(
    library_id: &str,
    temp_asset_id: &str,
) -> Result<PreviewReference, ApiError> {
    Ok(PreviewReference {
        url: format!(
            "{}/libraries/{}/query-assets/documents/{}/preview#page=1&view=FitH",
            app_base_url()?.trim_end_matches('/'),
            library_id,
            temp_asset_id
        ),
        handle: Some(format!("query-document-preview:{temp_asset_id}")),
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

fn stable_collection_name(library_id: &str, index_line: &str) -> String {
    format!("index_{library_id}_{index_line}")
}

fn parse_id_seq(id: &str, prefix: &str) -> u64 {
    id.strip_prefix(prefix)
        .and_then(|suffix| suffix.parse::<u64>().ok())
        .unwrap_or(0)
}

fn max_id_seq<'a>(ids: impl Iterator<Item = &'a String>, prefix: &str) -> u64 {
    ids.map(|id| parse_id_seq(id, prefix)).max().unwrap_or(0)
}

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn load_durable_state_snapshot(
    path: &FsPath,
) -> Result<Option<DurableAppStateSnapshot>, io::Error> {
    if !path.exists() {
        return Ok(None);
    }

    let connection = Connection::open(path).map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Failed to open durable state store {}: {error}",
                path.display()
            ),
        )
    })?;
    initialize_durable_state_store(&connection).map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Failed to initialize durable state store {}: {error}",
                path.display()
            ),
        )
    })?;

    let payload = connection
        .query_row(
            "SELECT payload_json FROM durable_state_snapshots WHERE id = ?1",
            params![STATE_SNAPSHOT_ROW_ID],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "Failed to read durable state snapshot {}: {error}",
                    path.display()
                ),
            )
        })?;

    payload
        .map(|payload| {
            serde_json::from_str::<DurableAppStateSnapshot>(&payload).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Failed to decode durable state snapshot {}: {error}",
                        path.display()
                    ),
                )
            })
        })
        .transpose()
}

fn write_durable_state_snapshot(
    path: &FsPath,
    snapshot: &DurableAppStateSnapshot,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create durable state store directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let mut connection = Connection::open(path).map_err(|error| {
        format!(
            "Failed to open durable state store {}: {error}",
            path.display()
        )
    })?;
    initialize_durable_state_store(&connection).map_err(|error| {
        format!(
            "Failed to initialize durable state store {}: {error}",
            path.display()
        )
    })?;

    let payload = serde_json::to_string(snapshot)
        .map_err(|error| format!("Failed to encode durable state snapshot: {error}"))?;
    let updated_at_ms = i64::try_from(current_unix_ms()).unwrap_or(i64::MAX);
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("Failed to begin durable state transaction: {error}"))?;
    transaction
        .execute("DELETE FROM durable_state_snapshots", [])
        .map_err(|error| format!("Failed to clear durable state snapshot: {error}"))?;
    transaction
        .execute(
            "INSERT INTO durable_state_snapshots (id, payload_json, updated_at_ms) VALUES (?1, ?2, ?3)",
            params![STATE_SNAPSHOT_ROW_ID, payload, updated_at_ms],
        )
        .map_err(|error| format!("Failed to write durable state snapshot: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("Failed to commit durable state snapshot: {error}"))?;
    Ok(())
}

fn initialize_durable_state_store(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS durable_state_snapshots (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            payload_json TEXT NOT NULL,
            updated_at_ms INTEGER NOT NULL
        )",
        [],
    )?;
    Ok(())
}

fn remove_temp_query_asset_file(path: &str) {
    if let Err(error) = fs::remove_file(path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!("Failed to remove expired query asset file {path}: {error}");
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
    fn durable_state_roundtrip_restores_library_source_roots_sources_visual_units_and_active_index()
    {
        let store_path = unique_test_file_path("durable-roundtrip.sqlite");
        let root_dir = unique_test_dir_path("durable-roundtrip");
        fs::create_dir_all(&root_dir).unwrap();
        fs::write(root_dir.join("chart.png"), b"png").unwrap();
        write_test_pdf(&root_dir.join("report.pdf"), 2);

        let mut state = AppState::with_durable_store_path(Some(store_path.clone()));
        let library = state
            .create_library(CreateLibraryRequest {
                name: "durable-roundtrip".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        let source_root = state
            .create_source_root(
                &library.id,
                CreateSourceRootRequest {
                    root_path: root_dir.to_string_lossy().to_string(),
                    enabled: Some(true),
                    rules: Some(SourceRootRulesPayload {
                        include_globs: Vec::new(),
                        exclude_globs: Vec::new(),
                        include_extensions: vec!["png".to_string(), "pdf".to_string()],
                    }),
                },
            )
            .unwrap();
        let (_, queued) = state
            .queue_source_action(
                &library.id,
                SourceActionScope::SourceRoot(source_root.source_root_id.clone()),
                SourceActionKind::Refresh,
                SourceActionTrigger::Manual,
                BTreeMap::new(),
            )
            .unwrap();
        let queued = queued.unwrap();
        let prepared = state.prepare_source_action_execution(&queued.plan).unwrap();
        let outcome = SourceActionJobOutcome::completed(&prepared);
        state
            .finalize_source_action_job(&queued.job_id, prepared, outcome)
            .unwrap();

        let active_alias = stable_collection_name(&library.id, MULTIVECTOR_INDEX_LINE);
        let active_target =
            staging_collection_name(&library.id, MULTIVECTOR_INDEX_LINE, "job_000001");
        let loaded = load_state_with_qdrant_namespaces(
            &store_path,
            &[(active_alias, active_target.clone())],
            &[active_target],
        );
        let loaded_library = loaded.libraries.get(&library.id).unwrap();
        let loaded_root = loaded_library
            .source_roots
            .get(&source_root.source_root_id)
            .unwrap();

        assert_eq!(loaded.library_order, vec![library.id.clone()]);
        assert_eq!(
            loaded_library.source_root_order,
            vec![source_root.source_root_id]
        );
        assert_eq!(loaded_library.sources.len(), 2);
        assert_eq!(loaded_library.visual_units.len(), 3);
        assert!(loaded_library
            .active_index_lines
            .contains(MULTIVECTOR_INDEX_LINE));
        assert_eq!(
            loaded_root.rules.include_extensions,
            vec!["pdf".to_string(), "png".to_string()]
        );
        assert_eq!(loaded_root.watch_state, "watching");
        assert!(loaded.jobs.is_empty());
        assert!(loaded.job_order.is_empty());
        assert_eq!(loaded_library.latest_job_id, None);

        let _ = fs::remove_file(&store_path);
        let _ = fs::remove_dir_all(root_dir);
    }

    #[test]
    fn restart_load_continues_id_sequences_and_clears_jobs() {
        let store_path = unique_test_file_path("restart-sequences.sqlite");
        let first_image = unique_test_file_path("restart-sequences-first.png");
        let second_image = unique_test_file_path("restart-sequences-second.png");
        fs::write(&first_image, b"png").unwrap();
        fs::write(&second_image, b"png").unwrap();
        let root_dir = unique_test_dir_path("restart-sequences-root");
        fs::create_dir_all(&root_dir).unwrap();

        let mut state = AppState::with_durable_store_path(Some(store_path.clone()));
        let library = state
            .create_library(CreateLibraryRequest {
                name: "restart-sequences".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        let root = state
            .create_source_root(
                &library.id,
                CreateSourceRootRequest {
                    root_path: root_dir.to_string_lossy().to_string(),
                    enabled: Some(true),
                    rules: Some(SourceRootRulesPayload::default()),
                },
            )
            .unwrap();
        let prepared = state
            .prepare_import(
                &library.id,
                ImportPathsRequest {
                    paths: vec![first_image.to_string_lossy().to_string()],
                },
            )
            .unwrap();
        let import_data = state.queue_import(&prepared).unwrap();
        let job_id = import_data.job_handle.clone().unwrap();
        state
            .finalize_import_job(
                &job_id,
                prepared,
                ImportJobOutcome::completed("indexed first image".to_string(), 1),
            )
            .unwrap();

        let active_alias = stable_collection_name(&library.id, MULTIVECTOR_INDEX_LINE);
        let active_target =
            staging_collection_name(&library.id, MULTIVECTOR_INDEX_LINE, "job_000001");
        let mut loaded = load_state_with_qdrant_namespaces(
            &store_path,
            &[(active_alias, active_target.clone())],
            &[active_target],
        );
        let loaded_library = loaded.libraries.get(&library.id).unwrap();
        assert!(loaded.jobs.is_empty());
        assert!(loaded.job_order.is_empty());
        assert_eq!(loaded_library.latest_job_id, None);

        let second_library = loaded
            .create_library(CreateLibraryRequest {
                name: "restart-sequences-2".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        assert_eq!(second_library.id, "lib_000002");

        let second_root = loaded
            .create_source_root(
                &second_library.id,
                CreateSourceRootRequest {
                    root_path: root_dir.to_string_lossy().to_string(),
                    enabled: Some(false),
                    rules: Some(SourceRootRulesPayload::default()),
                },
            )
            .unwrap();
        assert_eq!(root.source_root_id, "root_000001");
        assert_eq!(second_root.source_root_id, "root_000002");

        let prepared = loaded
            .prepare_import(
                &second_library.id,
                ImportPathsRequest {
                    paths: vec![second_image.to_string_lossy().to_string()],
                },
            )
            .unwrap();
        assert_eq!(prepared.sources[0].id, "src_000002");
        assert_eq!(prepared.visual_units[0].id, "vu_000002");

        let _ = fs::remove_file(&store_path);
        let _ = fs::remove_file(first_image);
        let _ = fs::remove_file(second_image);
        let _ = fs::remove_dir_all(root_dir);
    }

    #[test]
    fn restart_load_missing_collection_marks_index_not_ready() {
        let store_path = unique_test_file_path("restart-missing-collection.sqlite");
        let image_path = unique_test_file_path("restart-missing-collection.png");
        fs::write(&image_path, b"png").unwrap();

        let mut state = AppState::with_durable_store_path(Some(store_path.clone()));
        let library = state
            .create_library(CreateLibraryRequest {
                name: "restart-missing-collection".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        let prepared = state
            .prepare_import(
                &library.id,
                ImportPathsRequest {
                    paths: vec![image_path.to_string_lossy().to_string()],
                },
            )
            .unwrap();
        let import_data = state.queue_import(&prepared).unwrap();
        let job_id = import_data.job_handle.clone().unwrap();
        state
            .finalize_import_job(
                &job_id,
                prepared,
                ImportJobOutcome::completed("indexed first image".to_string(), 1),
            )
            .unwrap();

        let loaded = load_state_with_qdrant_namespaces(&store_path, &[], &[]);
        let loaded_library = loaded.libraries.get(&library.id).unwrap();
        assert!(!loaded_library
            .active_index_lines
            .contains(MULTIVECTOR_INDEX_LINE));

        let error = loaded
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

        let active_alias = stable_collection_name(&library.id, MULTIVECTOR_INDEX_LINE);
        let active_target =
            staging_collection_name(&library.id, MULTIVECTOR_INDEX_LINE, "job_000001");
        let reloaded = load_state_with_qdrant_namespaces(
            &store_path,
            &[(active_alias, active_target.clone())],
            &[active_target],
        );
        assert!(!reloaded
            .libraries
            .get(&library.id)
            .unwrap()
            .active_index_lines
            .contains(MULTIVECTOR_INDEX_LINE));

        let _ = fs::remove_file(&store_path);
        let _ = fs::remove_file(image_path);
    }

    #[test]
    fn restart_load_legacy_direct_collection_marks_index_not_ready() {
        let store_path = unique_test_file_path("restart-legacy-direct.sqlite");
        let image_path = unique_test_file_path("restart-legacy-direct.png");
        fs::write(&image_path, b"png").unwrap();

        let mut state = AppState::with_durable_store_path(Some(store_path.clone()));
        let library = state
            .create_library(CreateLibraryRequest {
                name: "restart-legacy-direct".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        let prepared = state
            .prepare_import(
                &library.id,
                ImportPathsRequest {
                    paths: vec![image_path.to_string_lossy().to_string()],
                },
            )
            .unwrap();
        let import_data = state.queue_import(&prepared).unwrap();
        let job_id = import_data.job_handle.clone().unwrap();
        state
            .finalize_import_job(
                &job_id,
                prepared,
                ImportJobOutcome::completed("indexed first image".to_string(), 1),
            )
            .unwrap();

        let legacy_direct_collection = stable_collection_name(&library.id, MULTIVECTOR_INDEX_LINE);
        let loaded =
            load_state_with_qdrant_namespaces(&store_path, &[], &[legacy_direct_collection]);
        assert!(!loaded
            .libraries
            .get(&library.id)
            .unwrap()
            .active_index_lines
            .contains(MULTIVECTOR_INDEX_LINE));

        let _ = fs::remove_file(&store_path);
        let _ = fs::remove_file(image_path);
    }

    #[test]
    fn restart_load_reseeds_watcher_runtime_fields_without_auto_queueing_jobs() {
        let store_path = unique_test_file_path("restart-watcher.sqlite");
        let root_dir = unique_test_dir_path("restart-watcher-root");
        fs::create_dir_all(&root_dir).unwrap();
        fs::write(root_dir.join("watch.png"), b"png").unwrap();

        let mut state = AppState::with_durable_store_path(Some(store_path.clone()));
        let library = state
            .create_library(CreateLibraryRequest {
                name: "restart-watcher".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();
        let source_root = state
            .create_source_root(
                &library.id,
                CreateSourceRootRequest {
                    root_path: root_dir.to_string_lossy().to_string(),
                    enabled: Some(true),
                    rules: Some(SourceRootRulesPayload::default()),
                },
            )
            .unwrap();

        {
            let root = state
                .libraries
                .get_mut(&library.id)
                .unwrap()
                .source_roots
                .get_mut(&source_root.source_root_id)
                .unwrap();
            root.watch_state = "queued_refresh".to_string();
            root.pending_watch_paths.insert("watch.png".to_string());
            root.pending_watch_deadline_ms = Some(0);
            root.pending_watch_error = Some("stale watcher error".to_string());
            root.last_action = Some(SourceRootLastAction {
                action: "refresh".to_string(),
                status: "completed".to_string(),
                summary: "stale".to_string(),
                job_id: Some("job_999999".to_string()),
            });
        }
        state.persist_durable_state().unwrap();

        let loaded = load_state_with_qdrant_namespaces(&store_path, &[], &[]);
        let loaded_root = loaded
            .libraries
            .get(&library.id)
            .unwrap()
            .source_roots
            .get(&source_root.source_root_id)
            .unwrap();
        assert_eq!(loaded_root.watch_state, "watching");
        assert!(loaded_root.pending_watch_paths.is_empty());
        assert_eq!(loaded_root.pending_watch_deadline_ms, None);
        assert_eq!(loaded_root.pending_watch_error, None);
        assert!(loaded_root.last_action.is_none());
        assert!(loaded.jobs.is_empty());
        assert!(loaded.job_order.is_empty());

        let _ = fs::remove_file(&store_path);
        let _ = fs::remove_dir_all(root_dir);
    }

    #[test]
    fn source_root_refresh_activates_files_and_rule_update_moves_sources_out_of_scope() {
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "source-root-refresh".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();

        let root_dir = unique_test_dir_path("source-root-refresh");
        fs::create_dir_all(&root_dir).unwrap();
        let image_path = root_dir.join("chart.png");
        let pdf_path = root_dir.join("report.pdf");
        fs::write(&image_path, b"png").unwrap();
        write_test_pdf(&pdf_path, 2);

        let source_root = state
            .create_source_root(
                &library.id,
                CreateSourceRootRequest {
                    root_path: root_dir.to_string_lossy().to_string(),
                    enabled: Some(true),
                    rules: Some(SourceRootRulesPayload::default()),
                },
            )
            .unwrap();

        let (action, queued) = state
            .queue_source_action(
                &library.id,
                SourceActionScope::SourceRoot(source_root.source_root_id.clone()),
                SourceActionKind::Refresh,
                SourceActionTrigger::Manual,
                BTreeMap::new(),
            )
            .unwrap();
        assert_eq!(action.accepted.len(), 1);
        let queued = queued.unwrap();
        let prepared = state.prepare_source_action_execution(&queued.plan).unwrap();
        assert_eq!(prepared.summary.activated_sources, 2);
        assert_eq!(prepared.summary.indexing_visual_units, 3);
        let outcome = SourceActionJobOutcome::completed(&prepared);
        state
            .finalize_source_action_job(&queued.job_id, prepared, outcome)
            .unwrap();

        let sources = state
            .list_sources(
                &library.id,
                SourcesQuery {
                    source_root_id: Some(source_root.source_root_id.clone()),
                    source_type: None,
                    status: None,
                },
            )
            .unwrap();
        assert_eq!(sources.sources.len(), 2);
        assert!(sources
            .sources
            .iter()
            .all(|source| source.status == "active"));

        state
            .update_source_root(
                &library.id,
                &source_root.source_root_id,
                UpdateSourceRootRequest {
                    root_path: None,
                    enabled: None,
                    rules: Some(SourceRootRulesPayload {
                        include_globs: Vec::new(),
                        exclude_globs: vec!["chart.png".to_string()],
                        include_extensions: Vec::new(),
                    }),
                },
            )
            .unwrap();

        let (action, queued) = state
            .queue_source_action(
                &library.id,
                SourceActionScope::SourceRoot(source_root.source_root_id.clone()),
                SourceActionKind::Refresh,
                SourceActionTrigger::Manual,
                BTreeMap::new(),
            )
            .unwrap();
        assert_eq!(action.accepted.len(), 1);
        let queued = queued.unwrap();
        let prepared = state.prepare_source_action_execution(&queued.plan).unwrap();
        assert_eq!(prepared.summary.out_of_scope_sources, 1);
        let outcome = SourceActionJobOutcome::completed(&prepared);
        state
            .finalize_source_action_job(&queued.job_id, prepared, outcome)
            .unwrap();

        let active_sources = state
            .list_sources(
                &library.id,
                SourcesQuery {
                    source_root_id: Some(source_root.source_root_id.clone()),
                    source_type: None,
                    status: Some("active".to_string()),
                },
            )
            .unwrap();
        assert_eq!(active_sources.sources.len(), 1);
        assert_eq!(active_sources.sources[0].source_type, "pdf");

        let out_of_scope_sources = state
            .list_sources(
                &library.id,
                SourcesQuery {
                    source_root_id: Some(source_root.source_root_id),
                    source_type: None,
                    status: Some("out_of_scope".to_string()),
                },
            )
            .unwrap();
        assert_eq!(out_of_scope_sources.sources.len(), 1);
        assert_eq!(out_of_scope_sources.sources[0].source_type, "image");

        let _ = fs::remove_dir_all(root_dir);
    }

    #[test]
    fn source_root_refresh_marks_deleted_files_invalidated() {
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "source-root-invalidation".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();

        let root_dir = unique_test_dir_path("source-root-invalidation");
        fs::create_dir_all(&root_dir).unwrap();
        let image_path = root_dir.join("chart.png");
        fs::write(&image_path, b"png").unwrap();

        let source_root = state
            .create_source_root(
                &library.id,
                CreateSourceRootRequest {
                    root_path: root_dir.to_string_lossy().to_string(),
                    enabled: Some(true),
                    rules: Some(SourceRootRulesPayload::default()),
                },
            )
            .unwrap();

        let (_, queued) = state
            .queue_source_action(
                &library.id,
                SourceActionScope::SourceRoot(source_root.source_root_id.clone()),
                SourceActionKind::Refresh,
                SourceActionTrigger::Manual,
                BTreeMap::new(),
            )
            .unwrap();
        let queued = queued.unwrap();
        let prepared = state.prepare_source_action_execution(&queued.plan).unwrap();
        let outcome = SourceActionJobOutcome::completed(&prepared);
        state
            .finalize_source_action_job(&queued.job_id, prepared, outcome)
            .unwrap();

        fs::remove_file(&image_path).unwrap();

        let (_, queued) = state
            .queue_source_action(
                &library.id,
                SourceActionScope::SourceRoot(source_root.source_root_id.clone()),
                SourceActionKind::Refresh,
                SourceActionTrigger::Manual,
                BTreeMap::new(),
            )
            .unwrap();
        let queued = queued.unwrap();
        let prepared = state.prepare_source_action_execution(&queued.plan).unwrap();
        assert_eq!(prepared.summary.invalidated_sources, 1);
        let outcome = SourceActionJobOutcome::completed(&prepared);
        state
            .finalize_source_action_job(&queued.job_id, prepared, outcome)
            .unwrap();

        let invalidated_sources = state
            .list_sources(
                &library.id,
                SourcesQuery {
                    source_root_id: Some(source_root.source_root_id.clone()),
                    source_type: None,
                    status: Some("invalidated".to_string()),
                },
            )
            .unwrap();
        assert_eq!(invalidated_sources.sources.len(), 1);
        assert_eq!(
            invalidated_sources.sources[0].status_reason.as_deref(),
            Some("not_found")
        );

        let search_plan = state
            .prepare_text_search(&TextSearchRequest {
                library_id: library.id.clone(),
                text: "chart".to_string(),
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(false),
                target_index_lines: None,
            })
            .unwrap();
        assert!(search_plan.active_visual_unit_ids.is_empty());

        let _ = fs::remove_dir_all(root_dir);
    }

    #[test]
    fn watcher_poll_debounces_into_incremental_refresh_queue() {
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "watcher-refresh".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();

        let root_dir = unique_test_dir_path("watcher-refresh");
        fs::create_dir_all(&root_dir).unwrap();
        let image_path = root_dir.join("watch.png");

        let source_root = state
            .create_source_root(
                &library.id,
                CreateSourceRootRequest {
                    root_path: root_dir.to_string_lossy().to_string(),
                    enabled: Some(true),
                    rules: Some(SourceRootRulesPayload::default()),
                },
            )
            .unwrap();

        fs::write(&image_path, b"png").unwrap();

        let queued = state.poll_source_root_watchers();
        assert!(queued.is_empty());
        let root = state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .source_roots
            .get_mut(&source_root.source_root_id)
            .unwrap();
        assert_eq!(root.watch_state, "queued_refresh");
        root.pending_watch_deadline_ms = Some(0);

        let queued = state.poll_source_root_watchers();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].plan.action.as_str(), "refresh");
        assert_eq!(
            queued[0].plan.target_root_ids,
            vec![source_root.source_root_id]
        );

        let _ = fs::remove_dir_all(root_dir);
    }

    #[test]
    fn disabled_source_root_skips_watcher_and_rejects_manual_refresh() {
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "disabled-source-root".to_string(),
                config: Some(CreateLibraryConfigRequest {
                    enabled_index_lines: vec!["multivector".to_string()],
                }),
            })
            .unwrap();

        let root_dir = unique_test_dir_path("disabled-source-root");
        fs::create_dir_all(&root_dir).unwrap();
        fs::write(root_dir.join("disabled.png"), b"png").unwrap();

        let source_root = state
            .create_source_root(
                &library.id,
                CreateSourceRootRequest {
                    root_path: root_dir.to_string_lossy().to_string(),
                    enabled: Some(false),
                    rules: Some(SourceRootRulesPayload::default()),
                },
            )
            .unwrap();

        let queued = state.poll_source_root_watchers();
        assert!(queued.is_empty());

        let root = state
            .libraries
            .get(&library.id)
            .unwrap()
            .source_roots
            .get(&source_root.source_root_id)
            .unwrap();
        assert_eq!(root.status, "disabled");
        assert_eq!(root.watch_state, "disabled");

        let (action, queued) = state
            .queue_source_action(
                &library.id,
                SourceActionScope::SourceRoot(source_root.source_root_id),
                SourceActionKind::Refresh,
                SourceActionTrigger::Manual,
                BTreeMap::new(),
            )
            .unwrap();
        assert!(queued.is_none());
        assert!(action.accepted.is_empty());
        assert_eq!(action.rejected.len(), 1);
        assert_eq!(action.rejected[0].reason_code, "not_enabled");

        let _ = fs::remove_dir_all(root_dir);
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
            page_count: None,
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
            page_count: None,
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
            page_count: None,
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
            page_count: None,
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
        assert_eq!(
            prepared.accepted[0].visual_units[0].locator["duration_ms"],
            2500
        );
        assert_eq!(
            prepared.accepted[0].source_id.as_deref(),
            Some("src_000001")
        );

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
            page_count: None,
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
        let source = state.source_record_from_classification(&classification, Vec::new());
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
            page_count: None,
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
            page_count: None,
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
                    source_root_id: None,
                    source_root_path: None,
                    source_path: "/tmp/example.png".to_string(),
                    relative_path: None,
                    source_type: "image".to_string(),
                    kind: "image".to_string(),
                    status: "active".to_string(),
                    status_reason: None,
                    page_count: None,
                    duration_ms: None,
                    observed_size_bytes: None,
                    observed_modified_at_ms: None,
                    visual_unit_ids: Vec::new(),
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
    fn prepare_document_search_accepts_temp_assets_and_library_sources() {
        set_test_app_env();
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "document-search".to_string(),
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

        let pdf_path = unique_test_file_path("query-document.pdf");
        write_test_pdf(&pdf_path, 3);
        let staged = StagedQueryAsset {
            path: pdf_path.to_string_lossy().to_string(),
            source_type: "pdf".to_string(),
            content_type: "application/pdf".to_string(),
            original_filename: Some("query-document.pdf".to_string()),
            page_count: Some(3),
            duration_ms: None,
        };
        let asset = state
            .register_temp_query_document_asset(&library.id, staged)
            .unwrap();

        let (plan, temp_input) = state
            .prepare_document_search(&DocumentSearchRequest {
                library_id: library.id.clone(),
                document_input: QueryDocumentInputRequest {
                    kind: "temp_asset".to_string(),
                    temp_asset_id: Some(asset.temp_asset_id.clone()),
                    source_id: None,
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
        assert_eq!(temp_input.path, pdf_path.to_string_lossy());
        assert!(temp_input.locator.is_none());

        let classification = state
            .inspect_import_path(&pdf_path.to_string_lossy())
            .unwrap();
        let source = state.source_record_from_classification(&classification, Vec::new());
        state
            .libraries
            .get_mut(&library.id)
            .unwrap()
            .sources
            .insert(source.id.clone(), source.clone());

        let (_, library_input) = state
            .prepare_document_search(&DocumentSearchRequest {
                library_id: library.id.clone(),
                document_input: QueryDocumentInputRequest {
                    kind: "library_object".to_string(),
                    temp_asset_id: None,
                    source_id: Some(source.id),
                    locator: Some(json!({ "start_page": 2, "end_page": 3 })),
                },
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(false),
                target_index_lines: None,
            })
            .unwrap();

        assert_eq!(library_input.path, pdf_path.to_string_lossy());
        assert_eq!(
            library_input.locator.unwrap(),
            json!({ "start_page": 2, "end_page": 3, "page_count": 3 })
        );

        let _ = fs::remove_file(pdf_path);
    }

    #[test]
    fn prepare_document_search_rejects_invalid_ranges() {
        set_test_app_env();
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "document-range-errors".to_string(),
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

        let pdf_path = unique_test_file_path("invalid-document-range.pdf");
        write_test_pdf(&pdf_path, 2);
        let staged = StagedQueryAsset {
            path: pdf_path.to_string_lossy().to_string(),
            source_type: "pdf".to_string(),
            content_type: "application/pdf".to_string(),
            original_filename: Some("invalid-document-range.pdf".to_string()),
            page_count: Some(2),
            duration_ms: None,
        };
        let asset = state
            .register_temp_query_document_asset(&library.id, staged)
            .unwrap();

        let error = state
            .prepare_document_search(&DocumentSearchRequest {
                library_id: library.id.clone(),
                document_input: QueryDocumentInputRequest {
                    kind: "temp_asset".to_string(),
                    temp_asset_id: Some(asset.temp_asset_id),
                    source_id: None,
                    locator: Some(json!({ "start_page": 2, "end_page": 5 })),
                },
                filters: None,
                top_k: Some(5),
                cursor: None,
                debug: Some(false),
                target_index_lines: None,
            })
            .unwrap_err();

        assert_eq!(error.payload.code, "validation_failed");

        let _ = fs::remove_file(pdf_path);
    }

    #[test]
    fn prepare_document_search_rejects_expired_temp_assets() {
        set_test_app_env();
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "expired-query-document".to_string(),
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

        let pdf_path = unique_test_file_path("expired-query-document.pdf");
        write_test_pdf(&pdf_path, 2);
        let staged = StagedQueryAsset {
            path: pdf_path.to_string_lossy().to_string(),
            source_type: "pdf".to_string(),
            content_type: "application/pdf".to_string(),
            original_filename: Some("expired-query-document.pdf".to_string()),
            page_count: Some(2),
            duration_ms: None,
        };
        let asset = state
            .register_temp_query_document_asset(&library.id, staged)
            .unwrap();
        state
            .temp_query_assets
            .get_mut(&asset.temp_asset_id)
            .unwrap()
            .expires_at_ms = 0;

        let error = state
            .prepare_document_search(&DocumentSearchRequest {
                library_id: library.id.clone(),
                document_input: QueryDocumentInputRequest {
                    kind: "temp_asset".to_string(),
                    temp_asset_id: Some(asset.temp_asset_id),
                    source_id: None,
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

        let _ = fs::remove_file(pdf_path);
    }

    #[test]
    fn prepare_document_search_rejects_non_pdf_library_sources() {
        let mut state = AppState::default();
        let library = state
            .create_library(CreateLibraryRequest {
                name: "unsupported-document-query-source".to_string(),
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
                "src_image_000002".to_string(),
                SourceRecord {
                    id: "src_image_000002".to_string(),
                    source_root_id: None,
                    source_root_path: None,
                    source_path: "/tmp/example.png".to_string(),
                    relative_path: None,
                    source_type: "image".to_string(),
                    kind: "image".to_string(),
                    status: "active".to_string(),
                    status_reason: None,
                    page_count: None,
                    duration_ms: None,
                    observed_size_bytes: None,
                    observed_modified_at_ms: None,
                    visual_unit_ids: Vec::new(),
                },
            );

        let error = state
            .prepare_document_search(&DocumentSearchRequest {
                library_id: library.id.clone(),
                document_input: QueryDocumentInputRequest {
                    kind: "library_object".to_string(),
                    temp_asset_id: None,
                    source_id: Some("src_image_000002".to_string()),
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
        assert_eq!(details["supported_source_type"], "pdf");
        assert_eq!(details["received_source_type"], "image");
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

    #[test]
    fn build_qdrant_collection_create_payload_sets_on_disk_and_init_from() {
        let payload = build_qdrant_collection_create_payload(96, Some("index_stage_src"));

        assert_eq!(payload["vectors"]["mv"]["size"], 96);
        assert_eq!(payload["vectors"]["mv"]["on_disk"], true);
        assert_eq!(payload["vectors"]["prefetch_dense"]["size"], 96);
        assert_eq!(payload["vectors"]["prefetch_dense"]["on_disk"], true);
        assert_eq!(payload["init_from"]["collection"], "index_stage_src");
    }

    fn unique_test_file_path(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("fauni-search-{stamp}-{name}"))
    }

    fn unique_test_dir_path(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("fauni-search-{stamp}-{name}"))
    }

    fn simulated_active_namespace_probe(
        alias_name: &str,
        alias_targets: &BTreeMap<String, String>,
        existing_collections: &BTreeSet<String>,
    ) -> ActiveNamespaceProbeResult {
        if let Some(target_collection) = alias_targets.get(alias_name) {
            if existing_collections.contains(target_collection) {
                return ActiveNamespaceProbeResult::Ready {
                    target_collection: target_collection.clone(),
                };
            }
            return ActiveNamespaceProbeResult::MissingTarget {
                target_collection: target_collection.clone(),
            };
        }
        if existing_collections.contains(alias_name) {
            ActiveNamespaceProbeResult::LegacyDirectCollection
        } else {
            ActiveNamespaceProbeResult::Missing
        }
    }

    fn load_state_with_qdrant_namespaces(
        store_path: &std::path::Path,
        alias_targets: &[(String, String)],
        existing_collections: &[String],
    ) -> AppState {
        let alias_targets = alias_targets.iter().cloned().collect::<BTreeMap<_, _>>();
        let existing_collections = existing_collections
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(AppState::load_from_durable_store_path_with_probe(
                Some(store_path.to_path_buf()),
                move |collection_name| {
                    let probe = simulated_active_namespace_probe(
                        collection_name,
                        &alias_targets,
                        &existing_collections,
                    );
                    async move { Ok(probe) }
                },
            ))
            .unwrap()
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
