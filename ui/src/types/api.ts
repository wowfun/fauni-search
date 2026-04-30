import type {
  Locator,
  ModelTestModality,
  PreviewReference,
  AssetType,
} from "./primitives";

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
  active_model?: string | null;
  base_url?: string | null;
  readonly_reason?: string | null;
  probe?: ProviderProbeSnapshot | null;
  origin: string;
  models: ProviderModelConfigSnapshot[];
}

export interface ProviderModelConfigSnapshot {
  model_id: string;
  enabled: boolean;
  version: string;
  backend?: string | null;
  embedding_capabilities: EmbeddingCapabilities;
  origin: string;
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
  origins?: Record<string, ContentTypeOriginSnapshot>;
}

export interface LibraryContentTypesData {
  content_types: ContentTypesPayload;
  origins?: Record<string, ContentTypeOriginSnapshot>;
}

export interface ContentTypeOriginSnapshot {
  origin: string;
  has_runtime_overlay: boolean;
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
  content_types: string[];
  unit_index_summary: UnitIndexSummary;
  content_e2e_index_summary: ContentE2eIndexSummary;
  provider_id?: string | null;
  provider_kind?: string | null;
  model_id?: string | null;
  model_version?: string | null;
  vector_type?: string | null;
  cleanup_summary?: Record<string, unknown> | null;
}

export interface UnitIndexSummary {
  active: number;
  retired: number;
  failed: number;
  not_ready: number;
}

export interface ContentE2eIndexSummary {
  completed: number;
  missing: number;
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
  source_uri: string;
  source_type: string;
  media_type: string;
  kind: string;
  status: string;
  status_reason?: string | null;
  relative_path?: string | null;
  source_root_id?: string | null;
  source_root_path?: string | null;
  source_root_label: string;
  asset_count: number;
  representative_asset?: AssetSummary | null;
  representative_preview?: PreviewReference | null;
}

export interface VideoSourceItem {
  source_id: string;
  source_uri: string;
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

export interface AssetSummary {
  asset_id: string;
  source_id: string;
  asset_type: AssetType;
  source_type: string;
  locator: Locator;
}

export interface AssetSnapshot extends AssetSummary {
  source_uri: string;
}

export interface AssetDetailData {
  asset: AssetSnapshot;
  preview: PreviewReference;
  neighbor_context: Record<string, unknown> | null;
  units: UnitSummary[];
}

export interface SearchResultItem extends AssetSnapshot {
  library_id: string;
  preview: PreviewReference;
  matched_units: MatchedUnitEvidence[];
  score?: number | null;
  cursor?: string | null;
}

export interface UnitSummary {
  unit_id: string;
  unit_type: string;
}

export interface MatchedUnitEvidence {
  unit_id: string;
  unit_type: string;
  vector_space_id: string;
  rank: number;
  raw_score: number;
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
  assets: AssetSummary[];
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
  asset_id: string;
  asset_type: "image" | "document_page";
  source_uri: string;
  preview: PreviewReference;
}

export interface LibraryObjectQueryVideo {
  library_id: string;
  asset_id: string;
  asset_type: "video_segment";
  source_uri: string;
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
  asset_id: string;
  source_id: string;
  asset_type: "document_page";
  source_uri: string;
  locator: DocumentQueryLocator | null;
  preview: PreviewReference;
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
