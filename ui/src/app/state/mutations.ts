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
  ProviderModelConfigSnapshot,
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
  AssetDetailData,
  WorkspaceKind,
} from "../../types";
import {
  emptyInventorySummary,
} from "../selectors/inventory";
import {
  selectedGlobalContentTypeKey,
  selectedGlobalTestModalities,
  selectedLibraryContentTypeKey,
  selectedLibraryTestModalities,
  selectedProviderTestModalities,
} from "../selectors/settings";
import { queryDocumentRangeSummary } from "../selectors/query-assets";
import { state } from "./store";

export function resetInventoryFilters() {
  state.inventoryFilters = {
    sourceRootId: "",
    sourceType: "",
    sourceStatus: "",
  };
}

export function hydrateLibraryManagementDraft(library: LibrarySnapshot | null) {
  if (!library) {
    state.libraryManagementDraftLibraryId = "";
    state.libraryManagementDisplayNameDraft = "";
    state.manageLibraryPopoverOpen = false;
    return;
  }

  state.libraryManagementDraftLibraryId = library.id;
  state.libraryManagementDisplayNameDraft = library.display_name;
}

export function upsertLibrarySnapshot(library: LibrarySnapshot) {
  const index = state.libraries.findIndex((item) => item.id === library.id);
  if (index >= 0) {
    state.libraries.splice(index, 1, library);
    return;
  }
  state.libraries.unshift(library);
}

export function resetSearchFilters() {
  state.searchFilters = {
    assetType: "",
    sourceType: "",
    pathPrefix: "",
    timeRangeStartMsDraft: "",
    timeRangeEndMsDraft: "",
  };
}

export function resetSearchResultLibraryFocus() {
  state.searchResultLibraryFocusId = "";
}

export function resetProviderEditor() {
  state.editingProviderId = "";
  state.providerDisplayNameDraft = "";
  state.providerKindDraft = "";
  state.providerEnabledDraft = true;
  state.providerBaseUrlDraft = "";
  state.providerActiveModelDraft = "";
  resetProviderModelEditor();
}

export function resetProviderModelEditor() {
  state.editingProviderModelId = "";
  state.providerModelIdDraft = "";
  state.providerModelEnabledDraft = true;
  state.providerModelVersionDraft = "main";
  state.providerModelBackendDraft = "";
  state.providerModelInputTypesDraft = "text, image";
  state.providerModelVectorTypesDraft = "single_vector";
  state.providerModelSupportsMixedInputsDraft = false;
}

export function resetInventoryState() {
  state.librarySources = [];
  state.inventorySummary = emptyInventorySummary();
  state.selectedInventorySourceId = "";
  state.inventoryDetailSheetOpen = false;
  state.inventoryImportOpen = false;
  state.inventorySourceManagementOpen = false;
  state.inventoryLibraryMaintenanceOpen = false;
}

export function resetSourceRootEditor() {
  state.inventorySourceRootEditorOpen = false;
  state.editingSourceRootId = "";
  state.sourceRootPathDraft = "";
  state.sourceRootEnabledDraft = true;
  state.sourceRootIncludeGlobsDraft = "";
  state.sourceRootExcludeGlobsDraft = "";
  state.sourceRootIncludeExtensionsDraft = "";
  state.sourceRootAdvancedRulesOpen = false;
}

export function keepSearchPreparationDisclosureOpen() {
  state.inventoryImportOpen = true;
}

export function setInventoryImportOpen(open: boolean) {
  state.inventoryImportOpen = open;
}

export function setInventorySourceManagementOpen(open: boolean) {
  state.inventorySourceManagementOpen = open;
}

export function setInventoryLibraryMaintenanceOpen(open: boolean) {
  state.inventoryLibraryMaintenanceOpen = open;
}

export function setSettingsDiagnosticsJobsOpen(open: boolean) {
  state.settingsDiagnosticsJobsOpen = open;
}

export function startSourceRootCreate() {
  resetSourceRootEditor();
  state.inventorySourceManagementOpen = true;
  state.inventorySourceRootEditorOpen = true;
}

function sourceRootHasCustomRules(sourceRoot) {
  return Boolean(
    sourceRoot?.rules?.include_globs?.length ||
      sourceRoot?.rules?.exclude_globs?.length ||
      sourceRoot?.rules?.include_extensions?.length
  );
}

export function populateSourceRootEditor(sourceRoot) {
  const hasCustomRules = sourceRootHasCustomRules(sourceRoot);
  state.inventorySourceManagementOpen = true;
  state.inventorySourceRootEditorOpen = true;
  state.editingSourceRootId = sourceRoot.source_root_id;
  state.sourceRootPathDraft = sourceRoot.root_path ?? "";
  state.sourceRootEnabledDraft = Boolean(sourceRoot.enabled);
  state.sourceRootIncludeGlobsDraft = (sourceRoot.rules?.include_globs ?? []).join("\n");
  state.sourceRootExcludeGlobsDraft = (sourceRoot.rules?.exclude_globs ?? []).join("\n");
  state.sourceRootIncludeExtensionsDraft = (sourceRoot.rules?.include_extensions ?? []).join(", ");
  state.sourceRootAdvancedRulesOpen = hasCustomRules;
}

export function setSourceRootAdvancedRulesOpen(open: boolean) {
  state.sourceRootAdvancedRulesOpen = open;
}

export function multilineDraftToList(value) {
  return String(value ?? "")
    .split("\n")
    .map((item) => item.trim())
    .filter(Boolean);
}

export function commaDraftToList(value) {
  return String(value ?? "")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

export function sourceRootPayloadFromDraft() {
  return {
    root_path: state.sourceRootPathDraft.trim(),
    enabled: state.sourceRootEnabledDraft,
    rules: {
      include_globs: multilineDraftToList(state.sourceRootIncludeGlobsDraft),
      exclude_globs: multilineDraftToList(state.sourceRootExcludeGlobsDraft),
      include_extensions: commaDraftToList(state.sourceRootIncludeExtensionsDraft),
    },
  };
}

export function hydrateProviderEditor(provider: ProviderConfigSnapshot | null) {
  if (!provider) {
    resetProviderEditor();
    return;
  }

  state.editingProviderId = provider.provider_id;
  state.providerDisplayNameDraft = provider.display_name;
  state.providerKindDraft = provider.provider_kind;
  state.providerEnabledDraft = provider.enabled;
  state.providerBaseUrlDraft = provider.base_url ?? "";
  state.providerActiveModelDraft = provider.active_model ?? provider.models[0]?.model_id ?? "";
  hydrateProviderModelEditor(
    provider.models.find((model) => model.model_id === state.providerActiveModelDraft) ??
      provider.models[0] ??
      null
  );
}

export function hydrateProviderModelEditor(model: ProviderModelConfigSnapshot | null) {
  if (!model) {
    resetProviderModelEditor();
    return;
  }
  state.editingProviderModelId = model.model_id;
  state.providerModelIdDraft = model.model_id;
  state.providerModelEnabledDraft = model.enabled;
  state.providerModelVersionDraft = model.version || "main";
  state.providerModelBackendDraft = model.backend ?? "";
  state.providerModelInputTypesDraft = model.embedding_capabilities.input_types.join(", ");
  state.providerModelVectorTypesDraft = model.embedding_capabilities.vector_types.join(", ");
  state.providerModelSupportsMixedInputsDraft = Boolean(
    model.embedding_capabilities.supports_mixed_inputs
  );
}

export function ensureValidModelTestDrafts() {
  const globalContentType = selectedGlobalContentTypeKey();
  if (state.selectedGlobalContentType !== globalContentType) {
    state.selectedGlobalContentType = globalContentType;
  }
  const libraryContentType = selectedLibraryContentTypeKey();
  if (state.selectedLibraryContentType !== libraryContentType) {
    state.selectedLibraryContentType = libraryContentType;
  }

  const globalModalities =
    state.selectedSettingsSection === "providers"
      ? selectedProviderTestModalities()
      : selectedGlobalTestModalities();
  if (!globalModalities.includes(state.globalModelTestModalityDraft as ModelTestModality)) {
    state.globalModelTestModalityDraft =
      (globalModalities.includes("text") ? "text" : globalModalities[0]) ?? "";
    state.globalModelTestFile = null;
    state.globalModelTestResult = null;
    state.globalModelTestError = null;
  }
  if (
    state.globalModelTestComparisonModalityDraft &&
    !globalModalities.includes(state.globalModelTestComparisonModalityDraft as ModelTestModality)
  ) {
    state.globalModelTestComparisonModalityDraft = "";
    state.globalModelTestComparisonFile = null;
    state.globalModelTestResult = null;
    state.globalModelTestError = null;
  }

  const libraryModalities = selectedLibraryTestModalities();
  if (!libraryModalities.includes(state.libraryModelTestModalityDraft as ModelTestModality)) {
    state.libraryModelTestModalityDraft =
      (libraryModalities.includes("text") ? "text" : libraryModalities[0]) ?? "";
    state.libraryModelTestFile = null;
    state.libraryModelTestResult = null;
    state.libraryModelTestError = null;
  }
  if (
    state.libraryModelTestComparisonModalityDraft &&
    !libraryModalities.includes(state.libraryModelTestComparisonModalityDraft as ModelTestModality)
  ) {
    state.libraryModelTestComparisonModalityDraft = "";
    state.libraryModelTestComparisonFile = null;
    state.libraryModelTestResult = null;
    state.libraryModelTestError = null;
  }
}

export function resetGlobalModelTestState() {
  state.globalModelTestFile = null;
  state.globalModelTestComparisonFile = null;
  state.globalModelTestResult = null;
  state.globalModelTestError = null;
  state.globalModelTestPending = false;
  ensureValidModelTestDrafts();
}

export function resetLibraryModelTestState() {
  state.libraryModelTestFile = null;
  state.libraryModelTestComparisonFile = null;
  state.libraryModelTestResult = null;
  state.libraryModelTestError = null;
  state.libraryModelTestPending = false;
  ensureValidModelTestDrafts();
}

export function clearQueryImageState() {
  if (state.queryImageObjectUrl) {
    URL.revokeObjectURL(state.queryImageObjectUrl);
  }
  state.queryImageFile = null;
  state.queryImageObjectUrl = null;
  state.queryImageAsset = null;
  state.queryImageLibraryObject = null;
}

export function normalizeQueryImageFile(file, fallbackName = "pasted-image.png") {
  if (file.name && file.name.trim()) {
    return file;
  }

  return new File([file], fallbackName, {
    type: file.type || "image/png",
    lastModified: Date.now(),
  });
}

export function setPendingQueryImageFile(file) {
  clearQueryImageState();
  state.queryImageFile = normalizeQueryImageFile(file);
  state.queryImageObjectUrl = URL.createObjectURL(state.queryImageFile);
}

export function clearQueryVideoState() {
  if (state.queryVideoObjectUrl) {
    URL.revokeObjectURL(state.queryVideoObjectUrl);
  }
  state.queryVideoFile = null;
  state.queryVideoObjectUrl = null;
  state.queryVideoAsset = null;
  state.queryVideoSource = null;
  state.queryVideoLibraryObject = null;
  state.queryVideoDurationMs = null;
  state.queryVideoRange = null;
}

export function normalizeQueryVideoFile(file, fallbackName = "query-video.mp4") {
  if (file.name && file.name.trim()) {
    return file;
  }

  return new File([file], fallbackName, {
    type: file.type || "video/mp4",
    lastModified: Date.now(),
  });
}

export function setQueryVideoDuration(durationMs) {
  if (typeof durationMs !== "number" || Number.isNaN(durationMs) || durationMs <= 0) {
    return;
  }

  const normalizedDurationMs = Math.max(Math.round(durationMs), 1);
  state.queryVideoDurationMs = normalizedDurationMs;
  if (!state.queryVideoRange) {
    return;
  }

  const startMs = Math.max(
    0,
    Math.min(state.queryVideoRange.start_ms ?? 0, normalizedDurationMs - 1)
  );
  const endMs = Math.max(
    startMs + 1,
    Math.min(state.queryVideoRange.end_ms ?? normalizedDurationMs, normalizedDurationMs)
  );

  if (startMs === 0 && endMs === normalizedDurationMs) {
    state.queryVideoRange = null;
    return;
  }

  state.queryVideoRange = {
    start_ms: startMs,
    end_ms: endMs,
  };
}

export function setPendingQueryVideoFile(file) {
  clearQueryVideoState();
  state.queryVideoFile = normalizeQueryVideoFile(file);
  state.queryVideoObjectUrl = URL.createObjectURL(state.queryVideoFile);
}

export function clearQueryDocumentState() {
  if (state.queryDocumentObjectUrl) {
    URL.revokeObjectURL(state.queryDocumentObjectUrl);
  }
  state.queryDocumentFile = null;
  state.queryDocumentObjectUrl = null;
  state.queryDocumentAsset = null;
  state.queryDocumentLibraryObject = null;
  state.queryDocumentPageCount = null;
  state.queryDocumentStartPageDraft = "";
  state.queryDocumentEndPageDraft = "";
}

export function normalizeQueryDocumentFile(file, fallbackName = "query-document.pdf") {
  if (file.name && file.name.trim()) {
    return file;
  }

  return new File([file], fallbackName, {
    type: file.type || "application/pdf",
    lastModified: Date.now(),
  });
}

export function setQueryDocumentPageCount(pageCount) {
  if (typeof pageCount === "number" && Number.isFinite(pageCount) && pageCount > 0) {
    state.queryDocumentPageCount = Math.max(1, Math.round(pageCount));
    return;
  }
  state.queryDocumentPageCount = null;
}

export function setPendingQueryDocumentFile(file) {
  clearQueryDocumentState();
  state.queryDocumentFile = normalizeQueryDocumentFile(file);
  state.queryDocumentObjectUrl = URL.createObjectURL(state.queryDocumentFile);
}

export function setLibraryQueryDocumentAsset(asset: LibraryObjectQueryDocument) {
  clearQueryDocumentState();
  state.queryDocumentLibraryObject = asset;
}

export function setLibraryQueryVideoSource(source: VideoSourceItem) {
  clearQueryVideoState();
  state.queryVideoSource = source;
  setQueryVideoDuration(source?.duration_ms ?? null);
}

export function setLibraryQueryVideoAsset(asset: LibraryObjectQueryVideo) {
  clearQueryVideoState();
  state.queryVideoLibraryObject = asset;
  setQueryVideoDuration(
    asset?.locator?.duration_ms ??
      (typeof asset?.locator?.end_ms === "number" ? asset.locator.end_ms : null)
  );
}

export function probeVideoDurationFromUrl(url) {
  return new Promise((resolve, reject) => {
    const video = document.createElement("video");
    video.preload = "metadata";
    video.src = url;
    video.onloadedmetadata = () => {
      if (!Number.isFinite(video.duration) || video.duration <= 0) {
        reject(new Error("video_duration_unavailable"));
        return;
      }
      resolve(Math.round(video.duration * 1000));
    };
    video.onerror = () => reject(new Error("video_metadata_load_failed"));
  });
}

export function firstClipboardImageFile(clipboardData: DataTransfer | null | undefined) {
  if (!clipboardData) {
    return null;
  }

  const fileList = Array.from(clipboardData.files ?? []);
  const directFile = fileList.find((file) => file?.type?.startsWith("image/"));
  if (directFile) {
    return directFile;
  }

  for (const item of Array.from(clipboardData.items ?? [])) {
    if (item.kind === "file" && item.type?.startsWith("image/")) {
      const file = item.getAsFile();
      if (file) {
        return file;
      }
    }
  }

  return null;
}

export function syncQueryDocumentRangeUi() {
  const summary = document.querySelector("#query-document-range-summary");
  if (summary) {
    summary.textContent = queryDocumentRangeSummary();
  }

  const clearButton = document.querySelector("#clear-query-document-range-button");
  if (clearButton instanceof HTMLButtonElement) {
    clearButton.disabled =
      Boolean(state.queryDocumentLibraryObject) ||
      (!state.queryDocumentStartPageDraft && !state.queryDocumentEndPageDraft);
  }
}
