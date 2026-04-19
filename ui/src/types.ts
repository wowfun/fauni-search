export type WorkspaceKind = "search" | "inventory";
export type SearchMode = "text" | "image" | "video" | "document";
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

export interface LibraryIndexLineStatus {
  index_line: string;
  status: string;
}

export interface LibraryCounts {
  accepted_items: number;
  pending_jobs: number;
}

export interface LibrarySnapshot {
  id: string;
  name: string;
  index_lines: LibraryIndexLineStatus[];
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
  preview: PreviewReference;
  score?: number | null;
  cursor?: string | null;
}

export interface SearchErrorIndexLineDetail {
  index_line: string;
  status: string;
  job?: {
    job_id: string;
    status: string;
    phase: string;
  } | null;
}

export interface ApiErrorDetails extends Record<string, unknown> {
  field?: string;
  index_lines?: SearchErrorIndexLineDetail[];
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

export interface ImportPathsData {
  accepted: ImportAcceptedItem[];
  rejected: ImportRejectedItem[];
  job_handle?: string | null;
  job?: JobSnapshot | null;
}

export interface LibraryObjectQueryImage {
  visual_unit_id: string;
  kind: "image" | "document_page";
  source_path: string;
  preview: PreviewReference;
}

export interface LibraryObjectQueryVideo {
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
  activeWorkspace: WorkspaceKind;
  inventoryFilters: InventoryFilters;
  searchFilters: SearchFilters;
  inventorySummary: InventorySummary;
  librarySources: SourceInventoryItem[];
  libraryNameDraft: string;
  selectedLibraryId: string;
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
  searchOutcome: SearchOutcomeState | null;
  lastSearchRequest: SearchRequestSnapshot | null;
  globalError: ApiErrorPayload | null;
  statusMessage: string | null;
}
