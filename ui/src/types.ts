export type WorkspaceKind = "search" | "inventory" | "settings";
export type SearchMode = "text" | "image" | "video" | "document";
export type SearchScopeKind = "library" | "all_libraries";
export type UtilityDrawerSection = "status" | "jobs" | "source-prep" | "maintenance";
export type SettingsSection =
  | "content-types"
  | "library-overrides"
  | "providers"
  | "model-tests"
  | "diagnostics";
export type VisualUnitKind = "image" | "document_page" | "video_segment" | string;
export type Locator = Record<string, string | number | boolean | null | undefined>;

export interface PreviewReference {
  url: string;
}

export interface InventoryFilters {
  sourceRootId: string;
  sourceType: string;
  sourceStatus: string;
}

export interface SearchFilters {
  visualUnitKind: string;
  sourceType: string;
  pathPrefix: string;
  timeRangeStartMsDraft: string;
  timeRangeEndMsDraft: string;
}

export interface InventorySummary {
  total: number;
  active: number;
  invalidated: number;
  out_of_scope: number;
}

export interface LibraryCounts {
  accepted_items: number;
  pending_jobs: number;
}

export interface ProviderProbeSnapshot {
  status: string;
  message: string;
  last_probed_at?: string | null;
}

export interface ProviderConfigSnapshot {
  provider_id: string;
  display_name: string;
  provider_kind: string;
  enabled: boolean;
  base_url?: string | null;
  readonly_reason?: string | null;
  probe?: ProviderProbeSnapshot | null;
}

export interface ProvidersListData {
  providers: ProviderConfigSnapshot[];
}

export interface UpdateProviderConfigRequest {
  enabled?: boolean;
  base_url?: string;
}

export interface ModelCatalogEntry {
  provider_id: string;
  provider_kind: string;
  model_id: string;
  model_version: string;
  model_revision?: string | null;
  embedding_capabilities: EmbeddingCapabilities;
  editable: boolean;
  status: string;
  message: string;
}

export interface ModelCatalogData {
  entries: ModelCatalogEntry[];
}

export interface ContentTypeBindingPayload {
  enabled: boolean;
  model: string;
  vector_type: string;
}

export interface ContentTypesPayload {
  content_types: Record<string, ContentTypeBindingPayload>;
}

export interface ModelSelectionPayload {
  provider_id: string;
  model_id: string;
}

export type BindingSource =
  | "global_content_type"
  | "library_content_type"
  | "settings_model_test"
  | string;

export interface GlobalContentTypesData {
  content_types: ContentTypesPayload;
}

export interface LibraryContentTypesData {
  content_types: ContentTypesPayload;
}

export interface ResolvedModelSelectionPayload {
  binding_source: BindingSource;
  provider_id: string;
  provider_kind: string;
  model_id: string;
  model_version: string;
  model_revision?: string | null;
  embedding_capabilities: EmbeddingCapabilities;
  status: string;
  message: string;
  last_probed_at?: string | null;
}

export interface ResolvedContentModelSelectionPayload {
  binding_source: BindingSource;
  content_type: string;
  provider_id: string;
  provider_kind: string;
  model_id: string;
  model_version: string;
  model_revision?: string | null;
  vector_type: string;
  vector_space_id?: string | null;
  embedding_capabilities: EmbeddingCapabilities;
  status: string;
  message: string;
  last_probed_at?: string | null;
}

export interface ResolvedContentModelsData {
  content_types: Record<string, ResolvedContentModelSelectionPayload>;
}

export interface VectorSpaceDiagnosticSnapshot {
  vector_space_id: string;
  lifecycle_state: string;
  content_types: string[];
  provider_id?: string | null;
  provider_kind?: string | null;
  model_id?: string | null;
  model_version?: string | null;
  vector_type?: string | null;
  retired_at_ms?: number | null;
}

export interface VectorSpaceDiagnosticsData {
  vector_spaces: VectorSpaceDiagnosticSnapshot[];
}

export interface RuntimeProcessHealthSnapshot {
  component_id: string;
  display_name: string;
  status: string;
  message: string;
  last_checked_at: string;
  details?: Record<string, unknown> | null;
}

export interface RuntimeProviderHealthSnapshot {
  provider_id: string;
  display_name: string;
  provider_kind: string;
  enabled: boolean;
  status: string;
  message: string;
  last_probed_at?: string | null;
  model_id?: string | null;
  model_version?: string | null;
  model_revision?: string | null;
  embedding_capabilities?: EmbeddingCapabilities | null;
  execution_input_types: string[];
  runtime_adapters: string[];
}

export interface RuntimeHealthData {
  app: RuntimeProcessHealthSnapshot;
  qdrant: RuntimeProcessHealthSnapshot;
  providers: RuntimeProviderHealthSnapshot[];
}

export interface EmbeddingCapabilities {
  input_types: string[];
  vector_types: string[];
  supports_mixed_inputs: boolean;
}

export type ModelTestModality = "text" | "image";

export interface ModelTestInputSummary {
  kind: string;
  text_preview?: string | null;
  original_filename?: string | null;
  content_type?: string | null;
  size_bytes?: number | null;
}

export interface ModelTestData {
  resolved_model: ResolvedModelSelectionPayload;
  input_modality: ModelTestModality | string;
  operation_kind: string;
  vector_shape: number[];
  vectors: number[][];
  pooled_vector?: number[] | null;
  input_summary: ModelTestInputSummary;
  comparison?: ModelTestComparisonData | null;
}

export interface ModelTestComparisonData {
  input_modality: ModelTestModality | string;
  operation_kind: string;
  vector_shape: number[];
  vectors: number[][];
  pooled_vector?: number[] | null;
  input_summary: ModelTestInputSummary;
  similarity_to_primary: number;
}

export interface LibrarySnapshot {
  id: string;
  display_name: string;
  lifecycle_state: string;
  archived_at_ms?: number | null;
  counts: LibraryCounts;
  latest_job_id?: string | null;
}

export interface SourceRootRulesPayload {
  include_globs: string[];
  exclude_globs: string[];
  include_extensions: string[];
}

export interface SourceRootCoverageSummary {
  observed_file_count: number;
  matched_file_count: number;
  active_source_count: number;
  inactive_source_count: number;
  last_scan_at_ms?: number | null;
}

export interface SourceRootLastAction {
  action: string;
  status: string;
  summary: string;
  job_id?: string | null;
}

export interface SourceRootSnapshot {
  source_root_id: string;
  root_path: string;
  enabled: boolean;
  status: string;
  watch_state: string;
  coverage_summary: SourceRootCoverageSummary;
  rules: SourceRootRulesPayload;
  last_action?: SourceRootLastAction | null;
}

export interface SourceInventoryItem {
  source_id: string;
  source_path: string;
  source_type: string;
  kind: string;
  status: string;
  status_reason?: string | null;
  relative_path?: string | null;
  source_root_id?: string | null;
  source_root_path?: string | null;
  source_root_label: string;
  visual_unit_count: number;
  representative_visual_unit?: VisualUnitSummary | null;
  representative_preview?: PreviewReference | null;
}

export interface VideoSourceItem {
  source_id: string;
  source_path: string;
  source_type: string;
  duration_ms?: number | null;
  preview: PreviewReference;
}

export interface JobProgress {
  completed: number;
  total: number;
  unit: string;
}

export interface JobAttemptSnapshot {
  attempt: number;
  status: string;
  summary: string;
}

export interface JobSnapshot {
  job_id: string;
  library_id: string;
  kind: string;
  status: string;
  phase: string;
  progress: JobProgress;
  cancelable: boolean;
  retryable: boolean;
  retried_from_job_id?: string | null;
  current_attempt: JobAttemptSnapshot;
}

export interface VisualUnitSummary {
  visual_unit_id: string;
  source_id: string;
  kind: VisualUnitKind;
  source_type: string;
  locator: Locator;
}

export interface VisualUnitSnapshot extends VisualUnitSummary {
  source_path: string;
}

export interface VisualUnitDetailData {
  visual_unit: VisualUnitSnapshot;
  preview: PreviewReference;
  neighbor_context: Record<string, unknown> | null;
}

export interface SearchResultItem extends VisualUnitSnapshot {
  library_id: string;
  preview: PreviewReference;
  score?: number | null;
  cursor?: string | null;
}

export interface SearchErrorContentTypeDetail {
  content_type: string;
  status: string;
  job?: {
    job_id: string;
    status: string;
    phase: string;
  } | null;
}

export interface ApiErrorDetails extends Record<string, unknown> {
  field?: string;
  content_types?: SearchErrorContentTypeDetail[];
}

export interface UnsupportedContentTypeSnapshot {
  content_type: string;
  model: string;
  vector_type: string;
  reason: string;
}

export interface ApiErrorPayload {
  code: string;
  message: string;
  details?: ApiErrorDetails | null;
  retryable?: boolean | null;
}

export interface SearchOutcomeState {
  results?: SearchResultItem[];
  next_cursor?: string | null;
  unsupported_content_types?: UnsupportedContentTypeSnapshot[];
  debug?: Record<string, unknown>;
  error?: ApiErrorPayload;
}

export interface SearchRequestSnapshot {
  endpoint: "/search/text" | "/search/image" | "/search/video" | "/search/document";
  body: Record<string, unknown>;
}

export interface QueryAssetData {
  temp_asset_id: string;
  preview: PreviewReference;
  source_type: string;
  content_type: string;
  original_filename: string;
  duration_ms?: number | null;
  page_count?: number | null;
}

export interface ImportAcceptedItem {
  original_path: string;
  normalized_path?: string | null;
  reason_code: string;
  message: string;
  source_id?: string | null;
  source_type: string;
  kind: string;
  visual_units: VisualUnitSummary[];
}

export interface ImportRejectedItem {
  original_path: string;
  normalized_path?: string | null;
  reason_code: string;
  message: string;
}

export interface SourceActionAcceptedItem {
  source_root_id: string;
  root_path: string;
  action: string;
}

export interface SourceActionRejectedItem {
  source_root_id?: string | null;
  root_path?: string | null;
  reason_code: string;
  message: string;
}

export interface SourceActionData {
  accepted: SourceActionAcceptedItem[];
  rejected: SourceActionRejectedItem[];
  job_handle?: string | null;
  job?: JobSnapshot | null;
}

export interface MaintenanceActionAcceptedItem {
  target_kind: string;
  target_id: string;
  message: string;
}

export interface MaintenanceActionRejectedItem {
  reason_code: string;
  message: string;
}

export interface MaintenanceActionData {
  action: string;
  accepted: MaintenanceActionAcceptedItem[];
  rejected: MaintenanceActionRejectedItem[];
  job_handle?: string | null;
  job?: JobSnapshot | null;
}

export interface ImportPathsData {
  accepted: ImportAcceptedItem[];
  rejected: ImportRejectedItem[];
  job_handle?: string | null;
  job?: JobSnapshot | null;
}

export interface LibraryObjectQueryImage {
  library_id: string;
  visual_unit_id: string;
  kind: "image" | "document_page";
  source_path: string;
  preview: PreviewReference;
}

export interface LibraryObjectQueryVideo {
  library_id: string;
  visual_unit_id: string;
  kind: "video_segment";
  source_path: string;
  locator: VideoQueryLocator;
  preview: PreviewReference;
}

export interface DocumentQueryLocator {
  start_page: number;
  end_page: number;
}

export interface VideoQueryLocator {
  start_ms?: number;
  end_ms?: number;
  duration_ms?: number;
}

export interface LibraryObjectQueryDocument {
  library_id: string;
  visual_unit_id: string;
  source_id: string;
  kind: "document_page";
  source_path: string;
  locator: DocumentQueryLocator | null;
  preview: PreviewReference;
}

export interface VideoRangeState {
  start_ms: number;
  end_ms: number;
}

export interface LibrariesListData {
  libraries: LibrarySnapshot[];
}

export interface SourceRootsListData {
  source_roots: SourceRootSnapshot[];
}

export interface SourcesListData {
  sources: SourceInventoryItem[];
}

export interface JobsListData {
  jobs: JobSnapshot[];
}

export interface VideoSourcesData {
  sources: VideoSourceItem[];
}

export interface AppState {
  libraries: LibrarySnapshot[];
  jobs: JobSnapshot[];
  videoSources: VideoSourceItem[];
  sourceRoots: SourceRootSnapshot[];
  providerConfigs: ProviderConfigSnapshot[];
  modelCatalog: ModelCatalogEntry[];
  globalContentTypes: ContentTypesPayload;
  libraryContentTypes: ContentTypesPayload;
  resolvedContentModels: ResolvedContentModelsData | null;
  vectorSpaceDiagnostics: VectorSpaceDiagnosticsData | null;
  runtimeHealth: RuntimeHealthData | null;
  activeWorkspace: WorkspaceKind;
  selectedSettingsSection: SettingsSection;
  inventoryFilters: InventoryFilters;
  searchFilters: SearchFilters;
  inventorySummary: InventorySummary;
  librarySources: SourceInventoryItem[];
  selectedInventorySourceId: string;
  libraryDisplayNameDraft: string;
  libraryManagementDisplayNameDraft: string;
  libraryManagementDraftLibraryId: string;
  libraryIdDraft: string;
  selectedLibraryId: string;
  searchScope: SearchScopeKind;
  createLibraryPopoverOpen: boolean;
  manageLibraryPopoverOpen: boolean;
  utilityDrawerOpen: boolean;
  utilityDrawerSection: UtilityDrawerSection;
  searchFiltersPanelOpen: boolean;
  searchPreparationDisclosureOpen: boolean;
  searchJobsDisclosureOpen: boolean;
  searchDetailSheetOpen: boolean;
  inventoryDetailSheetOpen: boolean;
  editingSourceRootId: string;
  sourceRootPathDraft: string;
  sourceRootEnabledDraft: boolean;
  sourceRootIncludeGlobsDraft: string;
  sourceRootExcludeGlobsDraft: string;
  sourceRootIncludeExtensionsDraft: string;
  importPathsDraft: string;
  searchMode: SearchMode;
  searchTextDraft: string;
  queryImageFile: File | null;
  queryImageObjectUrl: string | null;
  queryImageAsset: QueryAssetData | null;
  queryImageLibraryObject: LibraryObjectQueryImage | null;
  queryVideoFile: File | null;
  queryVideoObjectUrl: string | null;
  queryVideoAsset: QueryAssetData | null;
  queryVideoSource: VideoSourceItem | null;
  queryVideoLibraryObject: LibraryObjectQueryVideo | null;
  queryVideoDurationMs: number | null;
  queryVideoRange: VideoRangeState | null;
  queryDocumentFile: File | null;
  queryDocumentObjectUrl: string | null;
  queryDocumentAsset: QueryAssetData | null;
  queryDocumentLibraryObject: LibraryObjectQueryDocument | null;
  queryDocumentPageCount: number | null;
  queryDocumentStartPageDraft: string;
  queryDocumentEndPageDraft: string;
  importReceipt: ImportPathsData | null;
  selectedVisualUnit: VisualUnitDetailData | null;
  selectedVisualUnitLibraryId: string;
  searchOutcome: SearchOutcomeState | null;
  searchInFlight: boolean;
  searchResultLibraryFocusId: string;
  lastSearchRequest: SearchRequestSnapshot | null;
  editingProviderId: string;
  providerEnabledDraft: boolean;
  providerBaseUrlDraft: string;
  selectedGlobalContentType: string;
  selectedLibraryContentType: string;
  globalModelTestModalityDraft: ModelTestModality | "";
  globalModelTestTextDraft: string;
  globalModelTestFile: File | null;
  globalModelTestComparisonModalityDraft: ModelTestModality | "";
  globalModelTestComparisonTextDraft: string;
  globalModelTestComparisonFile: File | null;
  globalModelTestResult: ModelTestData | null;
  globalModelTestError: ApiErrorPayload | null;
  globalModelTestPending: boolean;
  libraryModelTestModalityDraft: ModelTestModality | "";
  libraryModelTestTextDraft: string;
  libraryModelTestFile: File | null;
  libraryModelTestComparisonModalityDraft: ModelTestModality | "";
  libraryModelTestComparisonTextDraft: string;
  libraryModelTestComparisonFile: File | null;
  libraryModelTestResult: ModelTestData | null;
  libraryModelTestError: ApiErrorPayload | null;
  libraryModelTestPending: boolean;
  globalError: ApiErrorPayload | null;
  statusMessage: string | null;
}
