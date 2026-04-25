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
import { state } from "../state/store";
import { libraryDisplayName } from "./common";

export function selectedLibrary(): LibrarySnapshot | null {
  return state.libraries.find((library) => library.id === state.selectedLibraryId) ?? null;
}

export function libraryById(libraryId: string | null | undefined): LibrarySnapshot | null {
  if (!libraryId) {
    return null;
  }
  return state.libraries.find((library) => library.id === libraryId) ?? null;
}

export function selectedVisualUnitOriginLibraryId(): string {
  return state.selectedVisualUnitLibraryId || state.selectedLibraryId || "";
}

export function allLibrariesTextScopeActive() {
  return state.searchScope === "all_libraries" && state.searchMode === "text";
}

export function searchScopeLabel(): string {
  if (state.searchScope === "all_libraries") {
    return `所有库 · ${state.libraries.length} 个库`;
  }
  const library = selectedLibrary();
  return library ? `当前库 · ${libraryDisplayName(library)}` : "当前库";
}

export function searchScopeRequestPayload() {
  if (state.searchScope === "all_libraries") {
    return { kind: "all_libraries" };
  }
  return {
    kind: "library",
    library_id: state.selectedLibraryId,
  };
}
