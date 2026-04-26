mod jobs;
mod providers;
mod search;
mod sources;
#[cfg(test)]
mod tests;

use crate::{
    api::*,
    config::{
        load_merged_runtime_config, load_merged_runtime_config_from_paths, ContentTypeConfigRecord,
        FauniConfig, ProviderModelConfigRecord,
    },
    model::*,
    persistence::*,
    provider::*,
    qdrant::{probe_active_qdrant_namespace, stable_vector_space_name},
    source_roots::*,
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
    next_job_seq: u64,
    next_visual_unit_seq: u64,
    next_source_seq: u64,
    next_source_root_seq: u64,
    next_temp_asset_seq: u64,
    provider_configs: BTreeMap<String, ProviderConfigRecord>,
    provider_models: BTreeMap<String, BTreeMap<String, ProviderModelConfigRecord>>,
    global_content_types: BTreeMap<String, ContentTypeConfigRecord>,
    provider_probe_cache: BTreeMap<String, ProviderProbeSnapshot>,
    provider_runtime_models: BTreeMap<String, ProviderRuntimeModelSnapshot>,
    provider_embedding_capabilities: BTreeMap<String, EmbeddingCapabilities>,
    provider_execution_input_types: BTreeMap<String, Vec<String>>,
    provider_runtime_adapters: BTreeMap<String, Vec<String>>,
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
            next_job_seq: 0,
            next_visual_unit_seq: 0,
            next_source_seq: 0,
            next_source_root_seq: 0,
            next_temp_asset_seq: 0,
            provider_configs: default_provider_configs(),
            provider_models: BTreeMap::new(),
            global_content_types: BTreeMap::new(),
            provider_probe_cache: BTreeMap::new(),
            provider_runtime_models: BTreeMap::new(),
            provider_embedding_capabilities: BTreeMap::new(),
            provider_execution_input_types: BTreeMap::new(),
            provider_runtime_adapters: BTreeMap::new(),
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
        state.sync_config_backed_model_state()?;
        state
            .reconcile_active_vector_spaces_on_boot_with_probe(probe_collection)
            .await;
        state.refresh_boot_provider_probe_cache().await;

        Ok(state)
    }

    fn with_durable_store_path(durable_store_path: Option<PathBuf>) -> Self {
        let mut state = Self {
            durable_store_path,
            ..Self::default()
        };
        let _ = state.sync_config_backed_model_state();
        state
    }

    fn from_durable_snapshot(
        snapshot: LoadedDurableStateSnapshot,
        durable_store_path: Option<PathBuf>,
    ) -> Self {
        let mut state = Self::with_durable_store_path(durable_store_path);
        state.apply_durable_snapshot(snapshot);
        state
    }

    fn durable_snapshot(&self) -> DurableAppStateSnapshot {
        DurableAppStateSnapshot {
            version: 3,
            library_order: self.library_order.clone(),
            libraries: self
                .libraries
                .iter()
                .map(|(library_id, library)| {
                    (
                        library_id.clone(),
                        DurableLibraryRecord {
                            id: library.id.clone(),
                            display_name: library.display_name.clone(),
                            lifecycle_state: library.lifecycle_state.clone(),
                            archived_at_ms: library.archived_at_ms,
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
                            active_vector_spaces: library.active_vector_spaces.clone(),
                            retired_vector_spaces: library.retired_vector_spaces.clone(),
                        },
                    )
                })
                .collect(),
        }
    }

    fn apply_durable_snapshot(&mut self, snapshot: LoadedDurableStateSnapshot) {
        let DurableAppStateSnapshot {
            library_order,
            libraries,
            ..
        } = snapshot.snapshot;

        self.library_order = library_order;
        self.libraries = libraries
            .into_iter()
            .map(|(library_id, library)| {
                (
                    library_id,
                    LibraryRecord {
                        id: library.id.clone(),
                        display_name: library.display_name,
                        lifecycle_state: library.lifecycle_state,
                        archived_at_ms: library.archived_at_ms,
                        content_type_overrides: BTreeMap::new(),
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
                        active_vector_spaces: library.active_vector_spaces,
                        retired_vector_spaces: library.retired_vector_spaces,
                    },
                )
            })
            .collect();
        self.rebuild_durable_sequences();
    }

    fn sync_config_backed_model_state(&mut self) -> Result<(), io::Error> {
        let loaded = match load_merged_runtime_config() {
            Ok(loaded) => loaded,
            Err(error)
                if error.kind() == io::ErrorKind::InvalidInput
                    && self.durable_store_path.is_some() =>
            {
                let repo_path = env::var("FAUNI_CONFIG_PATH")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("fauni.config.json"));
                let runtime_path = self
                    .durable_store_path
                    .as_ref()
                    .and_then(|path| {
                        path.parent()
                            .map(|parent| parent.join("runtime-config.json"))
                    })
                    .ok_or(error)?;
                load_merged_runtime_config_from_paths(&repo_path, &runtime_path)?
            }
            Err(error) => return Err(error),
        };
        self.apply_config_backed_model_state(&loaded.config)
    }

    fn apply_config_backed_model_state(&mut self, config: &FauniConfig) -> Result<(), io::Error> {
        let mut durable_changed = false;
        let mut provider_configs = default_provider_configs();
        for (provider_id, provider) in &config.provider {
            let Some(existing) = provider_configs.get_mut(provider_id) else {
                continue;
            };
            existing.enabled = provider.enabled;
            if let Some(display_name) = provider
                .display_name
                .as_ref()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
            {
                existing.display_name = display_name.to_string();
            }
            if provider_id == DASHSCOPE_PROVIDER_ID {
                existing.base_url = provider.base_url.clone();
            }
        }
        self.provider_configs = provider_configs;
        self.provider_models = config
            .provider
            .iter()
            .map(|(provider_id, provider)| (provider_id.clone(), provider.models.clone()))
            .collect();
        self.global_content_types = config.content_types.clone();

        let library_ids = self.library_order.clone();
        for library_id in library_ids {
            if let Some(display_name) = config
                .libraries
                .get(&library_id)
                .and_then(|record| record.display_name.as_ref())
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
            {
                if let Some(library) = self.libraries.get_mut(&library_id) {
                    if library.display_name != display_name {
                        library.display_name = display_name.to_string();
                        durable_changed = true;
                    }
                }
            }
            let overrides = config
                .libraries
                .get(&library_id)
                .map(|record| record.content_types.clone())
                .unwrap_or_default();
            if let Some(library) = self.libraries.get_mut(&library_id) {
                library.content_type_overrides = overrides;
            }
        }

        let library_ids = self.library_order.clone();
        for library_id in library_ids {
            let configured_vector_spaces = self
                .configured_vector_space_bindings_for_library(&library_id)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.payload.message))?
                .into_iter()
                .map(|binding| binding.vector_space_id)
                .collect::<BTreeSet<_>>();
            if let Some(library) = self.libraries.get_mut(&library_id) {
                let removed_vector_spaces = library
                    .active_vector_spaces
                    .iter()
                    .filter(|vector_space_id| !configured_vector_spaces.contains(*vector_space_id))
                    .cloned()
                    .collect::<Vec<_>>();
                library
                    .active_vector_spaces
                    .retain(|vector_space_id| configured_vector_spaces.contains(vector_space_id));
                for vector_space_id in removed_vector_spaces {
                    library.retired_vector_spaces.insert(
                        vector_space_id,
                        RetiredVectorSpaceRecord {
                            retired_at_ms: current_unix_ms(),
                        },
                    );
                    durable_changed = true;
                }
            }
        }

        if durable_changed {
            self.persist_durable_state().map_err(|message| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("Failed to persist durable state after config sync: {message}"),
                )
            })?;
        }

        Ok(())
    }

    fn rebuild_durable_sequences(&mut self) {
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
        self.provider_probe_cache.clear();
        self.provider_runtime_models.clear();
        self.provider_execution_input_types.clear();
        self.provider_runtime_adapters.clear();
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

    async fn reconcile_active_vector_spaces_on_boot_with_probe<F, Fut>(
        &mut self,
        mut probe_collection: F,
    ) where
        F: FnMut(&str) -> Fut,
        Fut: Future<Output = Result<ActiveNamespaceProbeResult, String>>,
    {
        let mut changed = false;
        let library_ids = self.library_order.clone();
        for library_id in library_ids {
            let active_vector_spaces = self
                .libraries
                .get(&library_id)
                .map(|library| {
                    library
                        .active_vector_spaces
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            for vector_space_id in active_vector_spaces {
                let collection_name = stable_vector_space_name(&library_id, &vector_space_id);
                match probe_collection(&collection_name).await {
                    Ok(ActiveNamespaceProbeResult::Ready { .. }) => {}
                    Ok(ActiveNamespaceProbeResult::Missing) => {
                        if let Some(library) = self.libraries.get_mut(&library_id) {
                            changed |= library.active_vector_spaces.remove(&vector_space_id);
                        }
                    }
                    Ok(ActiveNamespaceProbeResult::MissingTarget { target_collection }) => {
                        tracing::warn!(
                            library_id = %library_id,
                            vector_space_id = %vector_space_id,
                            target_collection = %target_collection,
                            "Active Qdrant namespace alias points to a missing collection during restart restore"
                        );
                        if let Some(library) = self.libraries.get_mut(&library_id) {
                            changed |= library.active_vector_spaces.remove(&vector_space_id);
                        }
                    }
                    Ok(ActiveNamespaceProbeResult::LegacyDirectCollection) => {
                        tracing::warn!(
                            library_id = %library_id,
                            vector_space_id = %vector_space_id,
                            "Direct Qdrant collection collides with the active alias namespace during restart restore; manual cleanup is required"
                        );
                        if let Some(library) = self.libraries.get_mut(&library_id) {
                            changed |= library.active_vector_spaces.remove(&vector_space_id);
                        }
                    }
                    Err(error) => {
                        tracing::warn!(
                            library_id = %library_id,
                            vector_space_id = %vector_space_id,
                            "Failed to probe Qdrant collection during restart restore: {error}"
                        );
                        if let Some(library) = self.libraries.get_mut(&library_id) {
                            changed |= library.active_vector_spaces.remove(&vector_space_id);
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

    pub(crate) fn eligible_retired_vector_spaces_for_cleanup(
        &self,
        now_ms: u128,
    ) -> Vec<RetiredVectorSpaceCleanupCandidate> {
        self.library_order
            .iter()
            .filter_map(|library_id| {
                self.libraries
                    .get(library_id)
                    .map(|library| (library_id, library))
            })
            .flat_map(|(library_id, library)| {
                library
                    .retired_vector_spaces
                    .iter()
                    .filter(move |(vector_space_id, retired)| {
                        !library.active_vector_spaces.contains(*vector_space_id)
                            && now_ms.saturating_sub(retired.retired_at_ms)
                                >= crate::RETIRED_VECTOR_SPACE_RETENTION_MS
                    })
                    .map(
                        move |(vector_space_id, _)| RetiredVectorSpaceCleanupCandidate {
                            library_id: library_id.clone(),
                            vector_space_id: vector_space_id.clone(),
                        },
                    )
            })
            .collect()
    }

    pub(crate) fn forget_cleaned_retired_vector_spaces(
        &mut self,
        cleaned: &[RetiredVectorSpaceCleanupCandidate],
    ) -> Result<(), String> {
        if cleaned.is_empty() {
            return Ok(());
        }

        let before = self.clone();
        for cleaned_candidate in cleaned {
            if let Some(library) = self.libraries.get_mut(&cleaned_candidate.library_id) {
                library
                    .retired_vector_spaces
                    .remove(&cleaned_candidate.vector_space_id);
            }
        }

        if let Err(message) = self.persist_durable_state() {
            *self = before;
            return Err(format!(
                "Failed to persist retired vector-space cleanup progress: {message}"
            ));
        }

        Ok(())
    }
}

impl AppState {
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

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}
