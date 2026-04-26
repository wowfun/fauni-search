import type {
  ApiErrorPayload,
  AppState,
  BindingSource,
  ContentTypeBindingPayload,
  ContentTypesPayload,
  EmbeddingCapabilities,
  GlobalContentTypesData,
  ImportPathsData,
  InventorySummary,
  JobSnapshot,
  JobsListData,
  LibrariesListData,
  LibraryContentTypesData,
  LibraryObjectQueryDocument,
  LibraryObjectQueryImage,
  LibraryObjectQueryVideo,
  LibrarySnapshot,
  MaintenanceActionData,
  ModelCatalogData,
  ModelCatalogEntry,
  ModelTestData,
  ModelTestModality,
  ModelSelectionPayload,
  PreviewReference,
  ProviderConfigSnapshot,
  ProvidersListData,
  QueryAssetData,
  ResolvedContentModelSelectionPayload,
  ResolvedContentModelsData,
  ResolvedModelSelectionPayload,
  RuntimeHealthData,
  SearchMode,
  SearchOutcomeState,
  SearchRequestSnapshot,
  SearchScopeKind,
  SourceActionData,
  SourceInventoryItem,
  SourceRootSnapshot,
  SourceRootsListData,
  SettingsSection,
  SourcesListData,
  VectorSpaceDiagnosticsData,
  VideoSourceItem,
  VideoSourcesData,
  VisualUnitDetailData,
  WorkspaceKind,
} from "../../types";

export interface FocusedEditableState {
  id: string;
  value: string | null;
  selectionStart: number | null;
  selectionEnd: number | null;
}

export const JOB_POLL_INTERVAL_MS = 1000;

export const JOB_POLL_TIMEOUT_MS = 5 * 60 * 1000;

export const WORKSPACE_POLL_INTERVAL_MS = 3000;

export const SEARCH_PAGE_SIZE = 5;

export const PROVIDER_ID_LOCAL_SIDECAR = "local_sidecar";

export const MODEL_TEST_MODALITIES: readonly ModelTestModality[] = ["text", "image"];

export const CONTENT_TYPE_ORDER = ["image", "document", "video", "text"] as const;

export function emptyContentTypes(): ContentTypesPayload {
  return {
    content_types: {},
  };
}

export const state: AppState = {
  libraries: [],
  globalJobs: [],
  jobs: [],
  videoSources: [],
  sourceRoots: [],
  providerConfigs: [],
  modelCatalog: [],
  globalContentTypes: emptyContentTypes(),
  libraryContentTypes: emptyContentTypes(),
  resolvedContentModels: null,
  vectorSpaceDiagnostics: null,
  runtimeHealth: null,
  activeWorkspace: "search",
  selectedSettingsSection: "content-types",
  inventoryFilters: {
    sourceRootId: "",
    sourceType: "",
    sourceStatus: "",
  },
  searchFilters: {
    visualUnitKind: "",
    sourceType: "",
    pathPrefix: "",
    timeRangeStartMsDraft: "",
    timeRangeEndMsDraft: "",
  },
  inventorySummary: {
    total: 0,
    active: 0,
    invalidated: 0,
    out_of_scope: 0,
  },
  librarySources: [],
  selectedInventorySourceId: "",
  libraryDisplayNameDraft: "",
  libraryManagementDisplayNameDraft: "",
  libraryManagementDraftLibraryId: "",
  libraryIdDraft: "",
  selectedLibraryId: "",
  searchScope: "library",
  createLibraryPopoverOpen: false,
  manageLibraryPopoverOpen: false,
  searchFiltersPanelOpen: false,
  settingsDiagnosticsJobsOpen: false,
  searchDetailSheetOpen: false,
  inventoryDetailSheetOpen: false,
  inventoryImportOpen: false,
  inventorySourceManagementOpen: false,
  inventoryLibraryMaintenanceOpen: false,
  inventorySourceRootEditorOpen: false,
  editingSourceRootId: "",
  sourceRootPathDraft: "",
  sourceRootEnabledDraft: true,
  sourceRootIncludeGlobsDraft: "",
  sourceRootExcludeGlobsDraft: "",
  sourceRootIncludeExtensionsDraft: "",
  sourceRootAdvancedRulesOpen: false,
  importPathsDraft: "",
  searchMode: "text",
  searchTextDraft: "",
  queryImageFile: null,
  queryImageObjectUrl: null,
  queryImageAsset: null,
  queryImageLibraryObject: null,
  queryVideoFile: null,
  queryVideoObjectUrl: null,
  queryVideoAsset: null,
  queryVideoSource: null,
  queryVideoLibraryObject: null,
  queryVideoDurationMs: null,
  queryVideoRange: null,
  queryDocumentFile: null,
  queryDocumentObjectUrl: null,
  queryDocumentAsset: null,
  queryDocumentLibraryObject: null,
  queryDocumentPageCount: null,
  queryDocumentStartPageDraft: "",
  queryDocumentEndPageDraft: "",
  importReceipt: null,
  selectedVisualUnit: null,
  selectedVisualUnitLibraryId: "",
  searchOutcome: null,
  searchInFlight: false,
  searchResultLibraryFocusId: "",
  lastSearchRequest: null,
  editingProviderId: "",
  providerEnabledDraft: true,
  providerBaseUrlDraft: "",
  selectedGlobalContentType: "",
  selectedLibraryContentType: "",
  globalModelTestModalityDraft: "",
  globalModelTestTextDraft: "",
  globalModelTestFile: null,
  globalModelTestComparisonModalityDraft: "",
  globalModelTestComparisonTextDraft: "",
  globalModelTestComparisonFile: null,
  globalModelTestResult: null,
  globalModelTestError: null,
  globalModelTestPending: false,
  libraryModelTestModalityDraft: "",
  libraryModelTestTextDraft: "",
  libraryModelTestFile: null,
  libraryModelTestComparisonModalityDraft: "",
  libraryModelTestComparisonTextDraft: "",
  libraryModelTestComparisonFile: null,
  libraryModelTestResult: null,
  libraryModelTestError: null,
  libraryModelTestPending: false,
  globalError: null,
  statusMessage: null,
};

export const EDITABLE_TARGET_SELECTOR = 'input, textarea, [contenteditable="true"], [contenteditable=""], select';

export let lastRenderedDetailPanelKey: string | null = null;

export function setLastRenderedDetailPanelKey(nextKey: string | null) {
  lastRenderedDetailPanelKey = nextKey;
}

export const root = document.querySelector<HTMLElement>("#app");
