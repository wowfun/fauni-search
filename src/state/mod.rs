mod jobs;
mod search;
mod sources;
#[cfg(test)]
mod tests;

use crate::{
    api::*,
    model::*,
    persistence::*,
    qdrant::{probe_active_qdrant_namespace, stable_collection_name},
    source_roots::*,
    MULTIVECTOR_INDEX_LINE,
};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    future::Future,
    io,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;

pub type SharedState = Arc<RwLock<AppState>>;

pub async fn new_state() -> Result<SharedState, io::Error> {
    let state = AppState::load_from_runtime_env().await?;
    Ok(Arc::new(RwLock::new(state)))
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
}

impl AppState {
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
