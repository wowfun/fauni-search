use crate::api::{
    AssetSnapshot, AssetSummary, ImportAcceptedItem, ImportPathsRequest, ImportRejectedItem,
    JobSnapshot, ProviderConfigSnapshot, ResolvedContentModelSelectionPayload,
    ResolvedModelSelectionPayload, SourceRootCoverageSummary, SourceRootLastAction,
    SourceRootRulesPayload, UnitSummary, UnsupportedContentTypeSnapshot,
};
use crate::config::ContentTypeOverrideRecord;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ActiveNamespaceProbeResult {
    Ready { target_collection: String },
    Missing,
    MissingTarget { target_collection: String },
    LegacyDirectCollection,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ProviderConfigRecord {
    pub(crate) provider_id: String,
    pub(crate) display_name: String,
    pub(crate) provider_kind: String,
    pub(crate) enabled: bool,
    pub(crate) base_url: Option<String>,
    pub(crate) readonly_reason: Option<String>,
}

impl ProviderConfigRecord {
    pub(crate) fn snapshot(&self) -> ProviderConfigSnapshot {
        ProviderConfigSnapshot {
            provider_id: self.provider_id.clone(),
            display_name: self.display_name.clone(),
            provider_kind: self.provider_kind.clone(),
            enabled: self.enabled,
            active_model: None,
            base_url: self.base_url.clone(),
            readonly_reason: self.readonly_reason.clone(),
            probe: None,
            origin: "baseline".to_string(),
            models: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedExecutionModelSelection {
    pub(crate) summary: ResolvedModelSelectionPayload,
    pub(crate) vector_type: String,
    pub(crate) vector_space_id: String,
    pub(crate) execution_input_types: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct VectorSpaceExecutionGroup {
    pub(crate) library_id: String,
    pub(crate) vector_space_id: String,
    pub(crate) active_unit_count: usize,
    pub(crate) eligible_point_ids: BTreeSet<u64>,
    pub(crate) content_types: Vec<String>,
    pub(crate) resolved_model: ResolvedExecutionModelSelection,
}

#[derive(Clone, Debug)]
pub(crate) struct SearchContentTypeDebugEntry {
    pub(crate) library_id: String,
    pub(crate) content_type: String,
    pub(crate) resolved_model: ResolvedContentModelSelectionPayload,
}

#[derive(Clone, Debug)]
pub(crate) struct LibraryRecord {
    pub(crate) id: String,
    pub(crate) display_name: String,
    pub(crate) lifecycle_state: String,
    pub(crate) archived_at_ms: Option<u128>,
    pub(crate) content_type_overrides: BTreeMap<String, ContentTypeOverrideRecord>,
    pub(crate) source_roots: BTreeMap<String, SourceRootRecord>,
    pub(crate) source_root_order: Vec<String>,
    pub(crate) contents: BTreeMap<String, ContentRecord>,
    pub(crate) sources: BTreeMap<String, SourceRecord>,
    pub(crate) source_order: Vec<String>,
    pub(crate) source_asset_locations: BTreeMap<String, SourceAssetLocationRecord>,
    pub(crate) source_asset_location_order: Vec<String>,
    pub(crate) assets: BTreeMap<String, AssetRecord>,
    pub(crate) asset_order: Vec<String>,
    pub(crate) units: BTreeMap<String, UnitRecord>,
    pub(crate) unit_order: Vec<String>,
    pub(crate) vector_spaces: BTreeMap<String, VectorSpaceRecord>,
    pub(crate) unit_indexes: BTreeMap<String, UnitIndexRecord>,
    pub(crate) content_e2e_index_states: BTreeMap<String, ContentE2eIndexStateRecord>,
    pub(crate) latest_job_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ContentRecord {
    pub(crate) id: String,
    pub(crate) size_bytes: Option<u64>,
    pub(crate) fast_fingerprint: Option<String>,
    pub(crate) sha256: Option<String>,
    pub(crate) created_at_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct VectorSpaceRecord {
    pub(crate) id: String,
    pub(crate) provider_id: String,
    pub(crate) model_id: String,
    pub(crate) model_version: String,
    pub(crate) model_revision: Option<String>,
    pub(crate) vector_type: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct UnitIndexRecord {
    pub(crate) unit_id: String,
    pub(crate) vector_space_id: String,
    pub(crate) status: String,
    pub(crate) visibility: String,
    pub(crate) vector_ref: Option<Value>,
    pub(crate) job_id: Option<String>,
    pub(crate) error_summary: Option<String>,
}

impl UnitIndexRecord {
    pub(crate) fn key(unit_id: &str, vector_space_id: &str) -> String {
        format!("{unit_id}::{vector_space_id}")
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ContentE2eIndexStateRecord {
    pub(crate) content_id: String,
    pub(crate) pipe_signature: String,
    pub(crate) vector_space_id: String,
    pub(crate) indexed_at_ms: u128,
}

impl ContentE2eIndexStateRecord {
    pub(crate) fn key(content_id: &str, pipe_signature: &str, vector_space_id: &str) -> String {
        format!("{content_id}::{pipe_signature}::{vector_space_id}")
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SourceRootRecord {
    pub(crate) id: String,
    pub(crate) root_path: String,
    pub(crate) enabled: bool,
    pub(crate) status: String,
    pub(crate) watch_state: String,
    pub(crate) rules: SourceRootRulesPayload,
    pub(crate) coverage_summary: SourceRootCoverageSummary,
    pub(crate) observed_entries: BTreeMap<String, ObservedSourceFile>,
    pub(crate) pending_watch_paths: BTreeSet<String>,
    pub(crate) pending_watch_deadline_ms: Option<u128>,
    pub(crate) pending_watch_error: Option<String>,
    pub(crate) last_action: Option<SourceRootLastAction>,
}

#[derive(Clone, Debug)]
pub(crate) struct ObservedSourceFile {
    pub(crate) absolute_path: String,
    pub(crate) relative_path: String,
    pub(crate) size_bytes: u64,
    pub(crate) modified_at_ms: Option<u128>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct SourceRecord {
    pub(crate) id: String,
    pub(crate) source_root_id: Option<String>,
    pub(crate) source_root_path: Option<String>,
    #[serde(default)]
    pub(crate) source_path: String,
    pub(crate) source_uri: String,
    pub(crate) relative_path: Option<String>,
    pub(crate) source_type: String,
    pub(crate) media_type: String,
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) status_reason: Option<String>,
    pub(crate) page_count: Option<usize>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) observed_size_bytes: Option<u64>,
    pub(crate) observed_modified_at_ms: Option<u128>,
    pub(crate) source_content_id: String,
    // Derived from SourceAssetLocation records for efficient in-memory traversal.
    pub(crate) asset_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct SourceAssetLocationRecord {
    pub(crate) id: String,
    pub(crate) source_id: String,
    pub(crate) asset_id: String,
    pub(crate) locator: Value,
    pub(crate) visibility: String,
}

impl SourceAssetLocationRecord {
    pub(crate) fn key(source_id: &str, asset_id: &str) -> String {
        format!("{source_id}::{asset_id}")
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct AssetRecord {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) source_id: String,
    #[serde(default)]
    pub(crate) content_id: String,
    #[serde(default)]
    pub(crate) source_path: String,
    #[serde(default)]
    pub(crate) source_type: String,
    pub(crate) source_content_id: String,
    pub(crate) asset_type: String,
    pub(crate) locator: Value,
    pub(crate) derivation_signature: String,
    pub(crate) neighbor_context: Value,
    // Derived from Unit records for efficient in-memory traversal.
    pub(crate) unit_ids: Vec<String>,
}

impl AssetRecord {
    pub(crate) fn summary(&self, source_id: &str, source_type: &str) -> AssetSummary {
        AssetSummary {
            asset_id: self.id.clone(),
            source_id: source_id.to_string(),
            asset_type: self.asset_type.clone(),
            source_type: source_type.to_string(),
            locator: self.locator.clone(),
        }
    }

    pub(crate) fn snapshot(
        &self,
        source_id: &str,
        source_type: &str,
        source_uri: &str,
    ) -> AssetSnapshot {
        AssetSnapshot {
            asset_id: self.id.clone(),
            source_id: source_id.to_string(),
            asset_type: self.asset_type.clone(),
            source_type: source_type.to_string(),
            source_uri: source_uri.to_string(),
            locator: self.locator.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct UnitRecord {
    pub(crate) id: String,
    pub(crate) asset_id: String,
    #[serde(default)]
    pub(crate) content_id: String,
    #[serde(default)]
    pub(crate) point_id: u64,
    #[serde(default)]
    pub(crate) source_id: String,
    #[serde(default)]
    pub(crate) source_path: String,
    #[serde(default)]
    pub(crate) source_type: String,
    #[serde(default)]
    pub(crate) asset_type: String,
    pub(crate) unit_type: String,
    pub(crate) derivation_signature: String,
    pub(crate) locator: Value,
    pub(crate) neighbor_context: Value,
}

impl UnitRecord {
    pub(crate) fn summary(&self) -> UnitSummary {
        UnitSummary {
            unit_id: self.id.clone(),
            unit_type: self.unit_type.clone(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct JobRecord {
    pub(crate) snapshot: JobSnapshot,
    pub(crate) cancellation_requested: bool,
    pub(crate) replay: Option<JobReplayAction>,
}

#[derive(Clone, Debug)]
pub(crate) struct JobQueueContext {
    pub(crate) attempt: u32,
    pub(crate) retried_from_job_id: Option<String>,
}

impl Default for JobQueueContext {
    fn default() -> Self {
        Self {
            attempt: 1,
            retried_from_job_id: None,
        }
    }
}

#[derive(Clone)]
pub(crate) enum JobReplayAction {
    Import {
        request: ImportPathsRequest,
    },
    SourceAction {
        scope: SourceActionScope,
        action: SourceActionKind,
    },
    Maintenance {
        action: MaintenanceActionKind,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct TempQueryAssetRecord {
    pub(crate) id: String,
    pub(crate) owner_scope: String,
    pub(crate) library_id: Option<String>,
    pub(crate) path: String,
    pub(crate) source_type: String,
    pub(crate) content_type: String,
    pub(crate) original_filename: Option<String>,
    pub(crate) page_count: Option<usize>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) size_bytes: usize,
    pub(crate) created_at_ms: u128,
    pub(crate) expires_at_ms: u128,
}

impl TempQueryAssetRecord {
    pub(crate) fn is_global(&self) -> bool {
        self.owner_scope == "global"
    }

    pub(crate) fn is_library_scoped_to(&self, library_id: &str) -> bool {
        self.owner_scope == "library" && self.library_id.as_deref() == Some(library_id)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct QueryHistoryRecord {
    pub(crate) id: String,
    pub(crate) created_at_ms: u128,
    pub(crate) source: String,
    pub(crate) query_kind: String,
    pub(crate) input_kind: String,
    pub(crate) input_summary: String,
    pub(crate) input_json: Value,
    pub(crate) search_scope_json: Value,
    pub(crate) filters_json: Option<Value>,
    pub(crate) target_content_types_json: Option<Value>,
    pub(crate) top_k: Option<usize>,
    pub(crate) status: String,
    pub(crate) result_count: Option<usize>,
    pub(crate) error_code: Option<String>,
    pub(crate) error_message: Option<String>,
    pub(crate) duration_ms: u128,
}

#[derive(Clone, Debug)]
pub(crate) struct QueryHistoryDraft {
    pub(crate) source: String,
    pub(crate) query_kind: String,
    pub(crate) input_kind: String,
    pub(crate) input_summary: String,
    pub(crate) input_json: Value,
    pub(crate) search_scope_json: Value,
    pub(crate) filters_json: Option<Value>,
    pub(crate) target_content_types_json: Option<Value>,
    pub(crate) top_k: Option<usize>,
    pub(crate) status: String,
    pub(crate) result_count: Option<usize>,
    pub(crate) error_code: Option<String>,
    pub(crate) error_message: Option<String>,
    pub(crate) duration_ms: u128,
}

#[derive(Default)]
pub(crate) struct TempQueryAssetPruneSummary {
    pub(crate) expired_removed: usize,
    pub(crate) missing_removed: usize,
}

impl TempQueryAssetPruneSummary {
    pub(crate) fn removed_count(&self) -> usize {
        self.expired_removed + self.missing_removed
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ResolvedImageQueryInput {
    TempAsset(TempQueryAssetRecord),
    LibraryAsset(AssetRecord),
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedVideoQueryInput {
    pub(crate) path: String,
    pub(crate) locator: Option<Value>,
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedDocumentQueryInput {
    pub(crate) path: String,
    pub(crate) locator: Option<Value>,
}

pub(crate) struct PathClassification {
    pub(crate) source_id: String,
    pub(crate) normalized_path: String,
    pub(crate) source_type: String,
    pub(crate) kind: String,
    pub(crate) page_count: Option<usize>,
    pub(crate) duration_ms: Option<u64>,
}

#[derive(Debug)]
pub(crate) struct ImportRejection {
    pub(crate) normalized_path: Option<String>,
    pub(crate) reason_code: String,
    pub(crate) message: String,
}

#[derive(Debug)]
pub(crate) struct PreparedImport {
    pub(crate) library_id: String,
    pub(crate) request: ImportPathsRequest,
    pub(crate) accepted: Vec<ImportAcceptedItem>,
    pub(crate) rejected: Vec<ImportRejectedItem>,
    pub(crate) contents: Vec<ContentRecord>,
    pub(crate) sources: Vec<SourceRecord>,
    pub(crate) source_asset_locations: Vec<SourceAssetLocationRecord>,
    pub(crate) assets: Vec<AssetRecord>,
    pub(crate) units: Vec<UnitRecord>,
    pub(crate) vector_space_batches: Vec<PreparedImportVectorSpaceBatch>,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedImportVectorSpaceBatch {
    pub(crate) vector_space_id: String,
    pub(crate) units: Vec<UnitRecord>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct RetiredVectorSpaceCleanupCandidate {
    pub(crate) library_id: String,
    pub(crate) vector_space_id: String,
}

#[derive(Clone, Debug)]
pub(crate) struct SourceActionPlan {
    pub(crate) library_id: String,
    pub(crate) action: SourceActionKind,
    pub(crate) target_root_ids: Vec<String>,
    pub(crate) changed_paths_by_root: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceActionKind {
    Refresh,
    Rescan,
    Rebuild,
}

impl SourceActionKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Refresh => "refresh",
            Self::Rescan => "rescan",
            Self::Rebuild => "rebuild",
        }
    }

    pub(crate) fn requires_full_scan(self) -> bool {
        matches!(self, Self::Rescan | Self::Rebuild)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MaintenanceActionKind {
    CleanupRetiredVectorSpaces,
}

impl MaintenanceActionKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::CleanupRetiredVectorSpaces => "cleanup_retired_vector_spaces",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum SourceActionScope {
    Library,
    SourceRoot(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceActionTrigger {
    Manual,
    Watcher,
}

impl SourceActionTrigger {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Watcher => "watcher",
        }
    }
}

pub(crate) struct PreparedSourceAction {
    pub(crate) library_id: String,
    pub(crate) action: SourceActionKind,
    pub(crate) accepted_root_count: usize,
    pub(crate) root_updates: Vec<PreparedSourceRootUpdate>,
    pub(crate) source_mutations: Vec<PreparedSourceMutation>,
    pub(crate) summary: SourceActionSummary,
    pub(crate) vector_space_batches: Vec<PreparedSourceActionVectorSpaceBatch>,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedSourceActionVectorSpaceBatch {
    pub(crate) vector_space_id: String,
    pub(crate) units_to_index: Vec<UnitRecord>,
}

pub(crate) struct PreparedSourceRootUpdate {
    pub(crate) source_root_id: String,
    pub(crate) status: String,
    pub(crate) watch_state: String,
    pub(crate) coverage_summary: SourceRootCoverageSummary,
    pub(crate) observed_entries: BTreeMap<String, ObservedSourceFile>,
}

pub(crate) struct PreparedSourceMutation {
    pub(crate) contents: Vec<ContentRecord>,
    pub(crate) source: SourceRecord,
    pub(crate) source_asset_locations: Vec<SourceAssetLocationRecord>,
    pub(crate) assets: Vec<AssetRecord>,
    pub(crate) units: Vec<UnitRecord>,
}

#[derive(Default)]
pub(crate) struct SourceActionSummary {
    pub(crate) scanned_roots: usize,
    pub(crate) observed_files: usize,
    pub(crate) matched_files: usize,
    pub(crate) activated_sources: usize,
    pub(crate) invalidated_sources: usize,
    pub(crate) out_of_scope_sources: usize,
    pub(crate) indexing_units: usize,
    pub(crate) degraded_roots: usize,
}

pub(crate) struct SourceActionJobOutcome {
    pub(crate) status: &'static str,
    pub(crate) phase: &'static str,
    pub(crate) completed: usize,
    pub(crate) activated_vector_spaces: BTreeSet<String>,
    pub(crate) apply_structured_changes: bool,
    pub(crate) summary: String,
}

impl SourceActionJobOutcome {
    pub(crate) fn completed(prepared: &PreparedSourceAction) -> Self {
        let summary = format!(
            "{} {} source root(s); observed {} file(s), matched {} file(s), activated {}, invalidated {}, out_of_scope {}, indexed {} unit(s).",
            prepared.action.as_str(),
            prepared.summary.scanned_roots,
            prepared.summary.observed_files,
            prepared.summary.matched_files,
            prepared.summary.activated_sources,
            prepared.summary.invalidated_sources,
            prepared.summary.out_of_scope_sources,
            prepared.summary.indexing_units,
        );
        Self {
            status: "completed",
            phase: "activated",
            completed: prepared.accepted_root_count,
            activated_vector_spaces: prepared
                .vector_space_batches
                .iter()
                .filter(|batch| !batch.units_to_index.is_empty())
                .map(|batch| batch.vector_space_id.clone())
                .collect(),
            apply_structured_changes: true,
            summary,
        }
    }

    pub(crate) fn failed(action: SourceActionKind, completed: usize, message: String) -> Self {
        Self {
            status: "failed",
            phase: "failed",
            completed,
            activated_vector_spaces: BTreeSet::new(),
            apply_structured_changes: false,
            summary: format!("{} failed: {message}", action.as_str()),
        }
    }

    pub(crate) fn failed_with_structured_changes(
        action: SourceActionKind,
        completed: usize,
        activated_vector_spaces: BTreeSet<String>,
        message: String,
    ) -> Self {
        Self {
            status: "failed",
            phase: "failed",
            completed,
            activated_vector_spaces,
            apply_structured_changes: true,
            summary: format!("{} partially failed: {message}", action.as_str()),
        }
    }

    pub(crate) fn canceled(action: SourceActionKind, completed: usize, message: String) -> Self {
        Self {
            status: "canceled",
            phase: "canceled",
            completed,
            activated_vector_spaces: BTreeSet::new(),
            apply_structured_changes: false,
            summary: format!("{} canceled: {message}", action.as_str()),
        }
    }

    pub(crate) fn canceled_with_structured_changes(
        action: SourceActionKind,
        completed: usize,
        activated_vector_spaces: BTreeSet<String>,
        message: String,
    ) -> Self {
        Self {
            status: "canceled",
            phase: "canceled",
            completed,
            activated_vector_spaces,
            apply_structured_changes: true,
            summary: format!("{} canceled: {message}", action.as_str()),
        }
    }
}

#[derive(Debug)]
pub(crate) struct QueuedSourceAction {
    pub(crate) job_id: String,
    pub(crate) plan: SourceActionPlan,
}

#[derive(Debug)]
pub(crate) enum RetryJobDispatch {
    Import(PreparedImport),
    SourceAction(QueuedSourceAction),
    Maintenance(QueuedMaintenanceAction),
}

#[derive(Debug)]
pub(crate) enum ResumeJobDispatch {
    Import(PreparedImport),
    SourceAction(SourceActionPlan),
    Maintenance(MaintenanceActionPlan),
}

#[derive(Clone, Debug)]
pub(crate) struct MaintenanceActionPlan {
    pub(crate) library_id: String,
    pub(crate) action: MaintenanceActionKind,
    pub(crate) target_vector_space_ids: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct QueuedMaintenanceAction {
    pub(crate) job_id: String,
    pub(crate) plan: MaintenanceActionPlan,
}

pub(crate) struct SourceRootScanResult {
    pub(crate) status: String,
    pub(crate) observed_entries: BTreeMap<String, ObservedSourceFile>,
    pub(crate) error: Option<String>,
}

impl SourceRootScanResult {
    pub(crate) fn disabled() -> Self {
        Self {
            status: "disabled".to_string(),
            observed_entries: BTreeMap::new(),
            error: None,
        }
    }
}

pub(crate) struct ImportJobOutcome {
    pub(crate) status: &'static str,
    pub(crate) phase: &'static str,
    pub(crate) completed: usize,
    pub(crate) activated_vector_spaces: BTreeSet<String>,
    pub(crate) summary: String,
}

impl ImportJobOutcome {
    pub(crate) fn completed(
        summary: String,
        completed: usize,
        activated_vector_spaces: BTreeSet<String>,
    ) -> Self {
        Self {
            status: "completed",
            phase: "activated",
            completed,
            activated_vector_spaces,
            summary,
        }
    }

    pub(crate) fn failed(phase: &'static str, message: String, completed: usize) -> Self {
        Self {
            status: "failed",
            phase,
            completed,
            activated_vector_spaces: BTreeSet::new(),
            summary: message,
        }
    }

    pub(crate) fn failed_with_activations(
        phase: &'static str,
        message: String,
        completed: usize,
        activated_vector_spaces: BTreeSet<String>,
    ) -> Self {
        Self {
            status: "failed",
            phase,
            completed,
            activated_vector_spaces,
            summary: message,
        }
    }

    pub(crate) fn canceled(message: String, completed: usize) -> Self {
        Self {
            status: "canceled",
            phase: "canceled",
            completed,
            activated_vector_spaces: BTreeSet::new(),
            summary: message,
        }
    }

    pub(crate) fn canceled_with_activations(
        message: String,
        completed: usize,
        activated_vector_spaces: BTreeSet<String>,
    ) -> Self {
        Self {
            status: "canceled",
            phase: "canceled",
            completed,
            activated_vector_spaces,
            summary: message,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SearchPlan {
    // Retained as the internal semantic hook for search-scope diagnostics; not currently emitted.
    #[allow(dead_code)]
    pub(crate) search_scope_kind: String,
    pub(crate) library_id: String,
    pub(crate) top_k: usize,
    pub(crate) cursor_offset: usize,
    pub(crate) kind_filter: Option<BTreeSet<String>>,
    pub(crate) path_prefix_filter: Option<BTreeSet<String>>,
    pub(crate) source_type_filter: Option<BTreeSet<String>>,
    pub(crate) time_range_filter: Option<SearchTimeRangeFilter>,
    pub(crate) target_content_types: Vec<String>,
    pub(crate) unsupported_content_types: Vec<UnsupportedContentTypeSnapshot>,
    pub(crate) active_asset_refs: BTreeSet<String>,
    pub(crate) active_unit_index_refs: BTreeSet<String>,
    pub(crate) asset_locations: BTreeMap<String, SearchPlanAssetLocation>,
    pub(crate) execution_groups: Vec<VectorSpaceExecutionGroup>,
    pub(crate) debug_content_types: Vec<SearchContentTypeDebugEntry>,
    pub(crate) debug: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct SearchPlanAssetLocation {
    pub(crate) source_id: String,
    pub(crate) source_uri: String,
    pub(crate) source_type: String,
    pub(crate) locator: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SearchTimeRangeFilter {
    pub(crate) start_ms: u64,
    pub(crate) end_ms: u64,
}

pub(crate) struct StagedQueryAsset {
    pub(crate) path: String,
    pub(crate) source_type: String,
    pub(crate) content_type: String,
    pub(crate) original_filename: Option<String>,
    pub(crate) page_count: Option<usize>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) size_bytes: usize,
}

pub(crate) struct StagedSettingsModelTestFile {
    pub(crate) path: String,
    pub(crate) content_type: String,
    pub(crate) original_filename: Option<String>,
    pub(crate) size_bytes: usize,
}

pub(crate) struct IncomingQueryImageUpload {
    pub(crate) bytes: Vec<u8>,
    pub(crate) content_type: String,
    pub(crate) original_filename: Option<String>,
    pub(crate) extension: String,
}

pub(crate) struct IncomingQueryVideoUpload {
    pub(crate) bytes: Vec<u8>,
    pub(crate) content_type: String,
    pub(crate) original_filename: Option<String>,
    pub(crate) extension: String,
}

pub(crate) struct IncomingQueryDocumentUpload {
    pub(crate) bytes: Vec<u8>,
    pub(crate) content_type: String,
    pub(crate) original_filename: Option<String>,
    pub(crate) extension: String,
}
