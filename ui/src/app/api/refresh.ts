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
import { renderWorkspace } from "../render/workspace";
import {
  clearQueryDocumentState,
  clearQueryImageState,
  clearQueryVideoState,
  ensureValidModelTestDrafts,
  hydrateLibraryManagementDraft,
  resetInventoryFilters,
  resetInventoryState,
  resetLibraryModelTestState,
  resetProviderEditor,
  resetSearchFilters,
  resetSearchResultLibraryFocus,
  resetSourceRootEditor,
  setQueryVideoDuration,
} from "../state/mutations";
import { emptyContentTypes, state } from "../state/store";
import { ensureSelectedInventorySource, summarizeInventorySources } from "../selectors/inventory";
import { apiRequest } from "./request";

export async function refreshLibraries({ keepSelection = true } = {}) {
  const data = await apiRequest<LibrariesListData>("/libraries");
  state.libraries = data.libraries;

  if (!keepSelection || !state.libraries.some((item) => item.id === state.selectedLibraryId)) {
    state.selectedLibraryId = state.libraries[0]?.id ?? "";
  }

  const currentLibrary =
    state.libraries.find((item) => item.id === state.selectedLibraryId) ?? null;
  if (!currentLibrary || state.libraryManagementDraftLibraryId !== currentLibrary.id) {
    state.manageLibraryPopoverOpen = false;
  }
  if (!currentLibrary || state.libraryManagementDraftLibraryId !== currentLibrary.id) {
    hydrateLibraryManagementDraft(currentLibrary);
  }
}

export async function refreshSourceRoots() {
  if (!state.selectedLibraryId) {
    state.sourceRoots = [];
    resetSourceRootEditor();
    return;
  }

  const data = await apiRequest<SourceRootsListData>(
    `/libraries/${encodeURIComponent(state.selectedLibraryId)}/source-roots`
  );
  state.sourceRoots = data.source_roots;

  if (
    state.editingSourceRootId &&
    !state.sourceRoots.some(
      (sourceRoot) => sourceRoot.source_root_id === state.editingSourceRootId
    )
  ) {
    resetSourceRootEditor();
  }
}

export async function refreshLibrarySources() {
  if (!state.selectedLibraryId) {
    resetInventoryState();
    state.selectedInventorySourceId = "";
    return;
  }

  const unfilteredData = await apiRequest<SourcesListData>(
    `/libraries/${encodeURIComponent(state.selectedLibraryId)}/sources`
  );
  state.inventorySummary = summarizeInventorySources(unfilteredData.sources);

  const params = new URLSearchParams();
  if (state.inventoryFilters.sourceRootId && state.inventoryFilters.sourceRootId !== "manual") {
    params.set("source_root_id", state.inventoryFilters.sourceRootId);
  }
  if (state.inventoryFilters.sourceType) {
    params.set("source_type", state.inventoryFilters.sourceType);
  }
  if (state.inventoryFilters.sourceStatus) {
    params.set("status", state.inventoryFilters.sourceStatus);
  }

  const query = params.toString();
  const data =
    query.length > 0
      ? await apiRequest(
          `/libraries/${encodeURIComponent(state.selectedLibraryId)}/sources?${query}`
        )
      : unfilteredData;
  state.librarySources =
    state.inventoryFilters.sourceRootId === "manual"
      ? data.sources.filter((source) => !source.source_root_id)
      : data.sources;
  ensureSelectedInventorySource();
}

export async function refreshJobs() {
  if (!state.selectedLibraryId) {
    state.jobs = [];
    return;
  }

  const data = await apiRequest<JobsListData>(
    `/jobs?library_id=${encodeURIComponent(state.selectedLibraryId)}`
  );
  state.jobs = data.jobs;
}

export async function refreshGlobalJobs() {
  const data = await apiRequest<JobsListData>("/jobs");
  state.globalJobs = data.jobs;
}

export async function refreshVideoSources() {
  if (!state.selectedLibraryId) {
    state.videoSources = [];
    if (!state.queryVideoFile && !state.queryVideoAsset) {
      clearQueryVideoState();
    }
    return;
  }

  const data = await apiRequest<VideoSourcesData>(
    `/libraries/${encodeURIComponent(state.selectedLibraryId)}/video-sources`
  );
  state.videoSources = data.sources;

  if (state.queryVideoSource) {
    const refreshed = state.videoSources.find(
      (source) => source.source_id === state.queryVideoSource.source_id
    );
    if (refreshed) {
      state.queryVideoSource = refreshed;
      setQueryVideoDuration(refreshed.duration_ms ?? null);
    } else if (!state.queryVideoFile && !state.queryVideoAsset) {
      clearQueryVideoState();
    }
  }
}

export async function refreshProviderConfigs() {
  const data = await apiRequest<ProvidersListData>("/settings/providers");
  state.providerConfigs = data.providers;
  if (
    state.editingProviderId &&
    !state.providerConfigs.some((provider) => provider.provider_id === state.editingProviderId)
  ) {
    resetProviderEditor();
  }
}

export async function refreshModelCatalog() {
  const data = await apiRequest<ModelCatalogData>("/settings/model-catalog");
  state.modelCatalog = data.entries;
  ensureValidModelTestDrafts();
}

export async function refreshGlobalContentTypes() {
  const data = await apiRequest<GlobalContentTypesData>("/settings/content-types");
  state.globalContentTypes = data.content_types;
  ensureValidModelTestDrafts();
}

export async function refreshRuntimeHealth() {
  const data = await apiRequest<RuntimeHealthData>("/runtime/status");
  state.runtimeHealth = data;
}

export async function refreshLibraryContentSettings() {
  if (!state.selectedLibraryId) {
    state.libraryContentTypes = emptyContentTypes();
    state.resolvedContentModels = null;
    state.vectorSpaceDiagnostics = null;
    resetLibraryModelTestState();
    return;
  }

  const [contentTypesData, resolvedData, diagnosticsData] = await Promise.all([
    apiRequest<LibraryContentTypesData>(
      `/libraries/${encodeURIComponent(state.selectedLibraryId)}/content-types`
    ),
    apiRequest<ResolvedContentModelsData>(
      `/libraries/${encodeURIComponent(state.selectedLibraryId)}/resolved-content-models`
    ),
    apiRequest<VectorSpaceDiagnosticsData>(
      `/libraries/${encodeURIComponent(state.selectedLibraryId)}/vector-space-diagnostics`
    ),
  ]);
  state.libraryContentTypes = contentTypesData.content_types;
  state.resolvedContentModels = resolvedData;
  state.vectorSpaceDiagnostics = diagnosticsData;
  ensureValidModelTestDrafts();
}

export async function refreshProviderSettingsData() {
  await refreshProviderConfigs();
  await refreshLibraryContentSettings();
  if (state.activeWorkspace === "settings") {
    await refreshRuntimeHealth();
    await refreshModelCatalog();
    await refreshGlobalContentTypes();
  }
}

export async function refreshJob(jobId) {
  return apiRequest<JobSnapshot>(`/jobs/${encodeURIComponent(jobId)}`);
}

export async function refreshWorkspace(options) {
  await refreshLibraries(options);
  await refreshSourceRoots();
  await refreshProviderConfigs();
  await refreshLibraryContentSettings();
  await refreshRuntimeHealth();
  if (state.activeWorkspace === "inventory") {
    await refreshLibrarySources();
  } else if (state.activeWorkspace === "settings") {
    await refreshModelCatalog();
    await refreshGlobalContentTypes();
  }
  await refreshGlobalJobs();
  await refreshJobs();
  await refreshVideoSources();
  renderWorkspace();
}

export async function switchCurrentLibrary(libraryId: string) {
  state.selectedLibraryId = libraryId;
  resetSourceRootEditor();
  resetInventoryFilters();
  resetSearchFilters();
  resetSearchResultLibraryFocus();
  resetInventoryState();
  state.libraryContentTypes = emptyContentTypes();
  state.resolvedContentModels = null;
  state.vectorSpaceDiagnostics = null;
  clearQueryImageState();
  clearQueryVideoState();
  clearQueryDocumentState();
  resetLibraryModelTestState();
  state.importReceipt = null;
  state.selectedVisualUnit = null;
  state.selectedVisualUnitLibraryId = "";
  state.searchOutcome = null;
  state.searchInFlight = false;
  state.lastSearchRequest = null;
  state.settingsDiagnosticsJobsOpen = false;
  state.searchDetailSheetOpen = false;
  state.createLibraryPopoverOpen = false;
  state.manageLibraryPopoverOpen = false;
  state.globalError = null;
  state.statusMessage = null;
  await refreshWorkspace({ keepSelection: true });
}
