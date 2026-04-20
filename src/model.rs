use crate::api::{
    ImportAcceptedItem, ImportRejectedItem, JobSnapshot, ProviderConfigSnapshot,
    ResolvedContentModelSelectionPayload, ResolvedModelSelectionPayload, SourceRootCoverageSummary,
    SourceRootLastAction, SourceRootRulesPayload, UnsupportedContentTypeSnapshot,
    VisualUnitSnapshot, VisualUnitSummary,
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
            base_url: self.base_url.clone(),
            readonly_reason: self.readonly_reason.clone(),
            probe: None,
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
    pub(crate) vector_space_id: String,
    pub(crate) content_types: Vec<String>,
    pub(crate) resolved_model: ResolvedExecutionModelSelection,
}

#[derive(Clone, Debug)]
pub(crate) struct LibraryRecord {
    pub(crate) id: String,
    pub(crate) display_name: String,
    pub(crate) content_type_overrides: BTreeMap<String, ContentTypeOverrideRecord>,
    pub(crate) source_roots: BTreeMap<String, SourceRootRecord>,
    pub(crate) source_root_order: Vec<String>,
    pub(crate) sources: BTreeMap<String, SourceRecord>,
    pub(crate) source_order: Vec<String>,
    pub(crate) visual_units: BTreeMap<String, VisualUnitRecord>,
    pub(crate) visual_unit_order: Vec<String>,
    pub(crate) latest_job_id: Option<String>,
    pub(crate) active_vector_spaces: BTreeSet<String>,
    pub(crate) retired_vector_spaces: BTreeMap<String, RetiredVectorSpaceRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct RetiredVectorSpaceRecord {
    pub(crate) retired_at_ms: u128,
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
    pub(crate) source_path: String,
    pub(crate) relative_path: Option<String>,
    pub(crate) source_type: String,
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) status_reason: Option<String>,
    pub(crate) page_count: Option<usize>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) observed_size_bytes: Option<u64>,
    pub(crate) observed_modified_at_ms: Option<u128>,
    pub(crate) visual_unit_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct VisualUnitRecord {
    pub(crate) id: String,
    pub(crate) point_id: u64,
    pub(crate) source_id: String,
    pub(crate) source_path: String,
    pub(crate) source_type: String,
    pub(crate) kind: String,
    pub(crate) locator: Value,
    pub(crate) neighbor_context: Value,
}

impl VisualUnitRecord {
    pub(crate) fn summary(&self) -> VisualUnitSummary {
        VisualUnitSummary {
            visual_unit_id: self.id.clone(),
            source_id: self.source_id.clone(),
            kind: self.kind.clone(),
            source_type: self.source_type.clone(),
            locator: self.locator.clone(),
        }
    }

    pub(crate) fn snapshot(&self) -> VisualUnitSnapshot {
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
pub(crate) struct JobRecord {
    pub(crate) snapshot: JobSnapshot,
}

#[derive(Clone, Debug)]
pub(crate) struct TempQueryAssetRecord {
    pub(crate) id: String,
    pub(crate) library_id: String,
    pub(crate) path: String,
    pub(crate) source_type: String,
    pub(crate) content_type: String,
    pub(crate) original_filename: Option<String>,
    pub(crate) page_count: Option<usize>,
    pub(crate) duration_ms: Option<u64>,
    pub(crate) expires_at_ms: u128,
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
    LibraryVisualUnit(VisualUnitRecord),
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

pub(crate) struct PreparedImport {
    pub(crate) library_id: String,
    pub(crate) accepted: Vec<ImportAcceptedItem>,
    pub(crate) rejected: Vec<ImportRejectedItem>,
    pub(crate) sources: Vec<SourceRecord>,
    pub(crate) visual_units: Vec<VisualUnitRecord>,
    pub(crate) vector_space_batches: Vec<PreparedImportVectorSpaceBatch>,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedImportVectorSpaceBatch {
    pub(crate) vector_space_id: String,
    pub(crate) content_types: Vec<String>,
    pub(crate) had_existing_index: bool,
    pub(crate) visual_units: Vec<VisualUnitRecord>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct RetiredVectorSpaceCleanupCandidate {
    pub(crate) library_id: String,
    pub(crate) vector_space_id: String,
}

#[derive(Clone)]
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
}

impl SourceActionKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Refresh => "refresh",
            Self::Rescan => "rescan",
        }
    }

    pub(crate) fn is_rescan(self) -> bool {
        matches!(self, Self::Rescan)
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
    pub(crate) content_types: Vec<String>,
    pub(crate) can_rebuild_from_scratch: bool,
    pub(crate) had_existing_index: bool,
    pub(crate) stale_point_ids: Vec<u64>,
    pub(crate) visual_units_to_index: Vec<VisualUnitRecord>,
}

pub(crate) struct PreparedSourceRootUpdate {
    pub(crate) source_root_id: String,
    pub(crate) status: String,
    pub(crate) watch_state: String,
    pub(crate) coverage_summary: SourceRootCoverageSummary,
    pub(crate) observed_entries: BTreeMap<String, ObservedSourceFile>,
}

pub(crate) struct PreparedSourceMutation {
    pub(crate) source: SourceRecord,
    pub(crate) visual_units: Vec<VisualUnitRecord>,
}

impl PreparedSourceAction {
    pub(crate) fn requires_index_update(&self) -> bool {
        self.vector_space_batches.iter().any(|batch| {
            !batch.visual_units_to_index.is_empty() || !batch.stale_point_ids.is_empty()
        })
    }
}

#[derive(Default)]
pub(crate) struct SourceActionSummary {
    pub(crate) scanned_roots: usize,
    pub(crate) observed_files: usize,
    pub(crate) matched_files: usize,
    pub(crate) activated_sources: usize,
    pub(crate) invalidated_sources: usize,
    pub(crate) out_of_scope_sources: usize,
    pub(crate) indexing_visual_units: usize,
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
            activated_vector_spaces: prepared
                .vector_space_batches
                .iter()
                .filter(|batch| {
                    !batch.visual_units_to_index.is_empty() || !batch.stale_point_ids.is_empty()
                })
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
}

pub(crate) struct QueuedSourceAction {
    pub(crate) job_id: String,
    pub(crate) plan: SourceActionPlan,
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
}

#[derive(Clone, Debug)]
pub(crate) struct SearchPlan {
    pub(crate) library_id: String,
    pub(crate) top_k: usize,
    pub(crate) cursor_offset: usize,
    pub(crate) kind_filter: Option<BTreeSet<String>>,
    pub(crate) path_prefix_filter: Option<BTreeSet<String>>,
    pub(crate) source_type_filter: Option<BTreeSet<String>>,
    pub(crate) time_range_filter: Option<SearchTimeRangeFilter>,
    pub(crate) target_content_types: Vec<String>,
    pub(crate) unsupported_content_types: Vec<UnsupportedContentTypeSnapshot>,
    pub(crate) active_visual_unit_ids: BTreeSet<String>,
    pub(crate) execution_groups: Vec<VectorSpaceExecutionGroup>,
    pub(crate) resolved_content_models: BTreeMap<String, ResolvedContentModelSelectionPayload>,
    pub(crate) debug: bool,
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
