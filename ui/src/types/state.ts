import type {
  ApiErrorPayload,
  ContentTypesPayload,
  ImportPathsData,
  JobSnapshot,
  LibraryObjectQueryDocument,
  LibraryObjectQueryImage,
  LibraryObjectQueryVideo,
  LibrarySnapshot,
  ModelCatalogEntry,
  ModelTestData,
  ProviderConfigSnapshot,
  QueryAssetData,
  ResolvedContentModelsData,
  RuntimeHealthData,
  SearchOutcomeState,
  SearchRequestSnapshot,
  SourceInventoryItem,
  SourceRootSnapshot,
  VectorSpaceDiagnosticsData,
  VideoSourceItem,
  VisualUnitDetailData,
} from "./api";
import type {
  InventoryFilters,
  InventorySummary,
  ModelTestModality,
  SearchFilters,
  SearchMode,
  SearchScopeKind,
  SettingsSection,
  VideoRangeState,
  WorkspaceKind,
} from "./primitives";

export interface AppState {
  libraries: LibrarySnapshot[];
  globalJobs: JobSnapshot[];
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
  searchFiltersPanelOpen: boolean;
  settingsDiagnosticsJobsOpen: boolean;
  searchDetailSheetOpen: boolean;
  inventoryDetailSheetOpen: boolean;
  inventoryImportOpen: boolean;
  inventorySourceManagementOpen: boolean;
  inventoryLibraryMaintenanceOpen: boolean;
  inventorySourceRootEditorOpen: boolean;
  editingSourceRootId: string;
  sourceRootPathDraft: string;
  sourceRootEnabledDraft: boolean;
  sourceRootIncludeGlobsDraft: string;
  sourceRootExcludeGlobsDraft: string;
  sourceRootIncludeExtensionsDraft: string;
  sourceRootAdvancedRulesOpen: boolean;
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
