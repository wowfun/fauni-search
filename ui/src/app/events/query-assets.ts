import {
  activeProviderDraftForSelection,
  apiRequest,
  canExecuteSettingsModelTest,
  clearQueryDocumentState,
  clearQueryImageState,
  clearQueryVideoState,
  composeModelReference,
  currentQueryVideoEndMs,
  currentQueryVideoStartMs,
  EDITABLE_TARGET_SELECTOR,
  emptyContentTypes,
  firstClipboardImageFile,
  hasFocusedEditableControl,
  hydrateLibraryManagementDraft,
  hydrateProviderEditor,
  isTerminalJobStatus,
  JOB_POLL_INTERVAL_MS,
  JOB_POLL_TIMEOUT_MS,
  keepSearchPreparationDisclosureOpen,
  libraryDisplayName,
  libraryIsArchived,
  normalizeContentTypeBindingForProvider,
  populateSourceRootEditor,
  PROVIDER_ID_LOCAL_SIDECAR,
  probeVideoDurationFromUrl,
  queryDocumentLocatorPayload,
  queryVideoLocatorPayload,
  queryVideoRangeStep,
  refreshJob,
  refreshLibraryContentSettings,
  refreshLibrarySources,
  refreshProviderSettingsData,
  refreshWorkspace,
  resetGlobalModelTestState,
  resetInventoryFilters,
  resetInventoryState,
  resetLibraryModelTestState,
  resetSearchFilters,
  resetSearchResultLibraryFocus,
  resetSourceRootEditor,
  searchFiltersPayload,
  searchScopeRequestPayload,
  SEARCH_PAGE_SIZE,
  selectedGlobalContentTypeBinding,
  selectedGlobalContentTypeKey,
  selectedGlobalModelSelection,
  selectedInventoryRepresentativePreview,
  selectedInventoryRepresentativeAsset,
  selectedInventorySource,
  selectedLibrary,
  selectedLibraryContentTypeBinding,
  selectedLibraryContentTypeHasOverride,
  selectedLibraryContentTypeKey,
  selectedLibraryModelSelection,
  selectedProviderConfig,
  selectedAssetId,
  selectedAssetOriginLibraryId,
  setLibraryQueryDocumentAsset,
  setLibraryQueryVideoSource,
  setLibraryQueryVideoAsset,
  setPendingQueryDocumentFile,
  setPendingQueryImageFile,
  setPendingQueryVideoFile,
  setQueryDocumentPageCount,
  setQueryVideoDuration,
  sleep,
  sourceRootDisplayName,
  sourceRootPayloadFromDraft,
  state,
  supportedTestModalitiesForSelection,
  switchCurrentLibrary,
  syncQueryDocumentRangeUi,
  syncQueryVideoDurationFromVideoElement,
  toApiError,
  upsertLibrarySnapshot,
  WORKSPACE_POLL_INTERVAL_MS,
  type ApiErrorPayload,
  type ContentTypeBindingPayload,
  type ImportPathsData,
  type JobSnapshot,
  type LibraryObjectQueryDocument,
  type LibraryObjectQueryImage,
  type LibraryObjectQueryVideo,
  type LibrarySnapshot,
  type MaintenanceActionData,
  type ModelTestData,
  type QueryAssetData,
  type SearchMode,
  type SearchOutcomeState,
  type SearchRequestSnapshot,
  type SearchScopeKind,
  type SettingsSection,
  type SourceActionData,
  type AssetDetailData,
  type WorkspaceKind,
} from "../core";
import { renderWorkspace } from "../render/workspace";

export async function uploadQueryImage(file: File): Promise<QueryAssetData> {
  const formData = new FormData();
  formData.append("file", file);
  const data = await apiRequest<QueryAssetData>(
    `/libraries/${state.selectedLibraryId}/query-assets/images`,
    {
      method: "POST",
      body: formData,
    }
  );
  if (state.queryImageObjectUrl) {
    URL.revokeObjectURL(state.queryImageObjectUrl);
  }
  state.queryImageFile = null;
  state.queryImageObjectUrl = null;
  state.queryImageAsset = data;
  renderWorkspace();
  return data;
}

export async function uploadQueryVideo(file: File): Promise<QueryAssetData> {
  const formData = new FormData();
  formData.append("file", file);
  const data = await apiRequest<QueryAssetData>(
    `/libraries/${state.selectedLibraryId}/query-assets/videos`,
    {
      method: "POST",
      body: formData,
    }
  );
  if (state.queryVideoObjectUrl) {
    URL.revokeObjectURL(state.queryVideoObjectUrl);
  }
  state.queryVideoFile = null;
  state.queryVideoObjectUrl = null;
  state.queryVideoAsset = data;
  state.queryVideoSource = null;
  setQueryVideoDuration(data.duration_ms ?? state.queryVideoDurationMs);
  renderWorkspace();
  return data;
}

export async function uploadQueryDocument(file: File): Promise<QueryAssetData> {
  const formData = new FormData();
  formData.append("file", file);
  const data = await apiRequest<QueryAssetData>(
    `/libraries/${state.selectedLibraryId}/query-assets/documents`,
    {
      method: "POST",
      body: formData,
    }
  );
  if (state.queryDocumentObjectUrl) {
    URL.revokeObjectURL(state.queryDocumentObjectUrl);
  }
  state.queryDocumentFile = null;
  state.queryDocumentObjectUrl = null;
  state.queryDocumentAsset = data;
  state.queryDocumentLibraryObject = null;
  setQueryDocumentPageCount(data.page_count ?? null);
  renderWorkspace();
  return data;
}

export function onSelectSearchMode(event) {
  const nextMode = event.currentTarget.dataset.searchMode as SearchMode | undefined;
  if (!nextMode) {
    return;
  }
  state.searchMode = nextMode;
  resetSearchResultLibraryFocus();
  if (nextMode !== "text" && state.searchScope === "all_libraries") {
    state.searchScope = "library";
  }
  state.globalError = null;
  state.statusMessage = null;
  state.searchInFlight = false;
  renderWorkspace();
}

export function onQueryImageInput(event) {
  const [file] = event.target.files ?? [];
  if (file) {
    setPendingQueryImageFile(file);
  }
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export function onQueryDocumentInput(event) {
  const [file] = event.target.files ?? [];
  if (file) {
    setPendingQueryDocumentFile(file);
  }
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export async function onQueryVideoInput(event) {
  const [file] = event.target.files ?? [];
  if (file) {
    setPendingQueryVideoFile(file);
  }
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();

  if (!file || !state.queryVideoObjectUrl) {
    return;
  }

  const previewUrl = state.queryVideoObjectUrl;
  try {
    const durationMs = await probeVideoDurationFromUrl(previewUrl);
    if (state.queryVideoObjectUrl === previewUrl) {
      setQueryVideoDuration(durationMs);
      renderWorkspace();
    }
  } catch {
    if (state.queryVideoObjectUrl === previewUrl) {
      state.globalError = {
        code: "validation_failed",
        message: "当前查询视频的元数据无法读取。",
      };
      renderWorkspace();
    }
  }
}

export function onQueryImagePaste(event) {
  if (state.searchMode !== "image" || !state.selectedLibraryId) {
    return;
  }

  const clipboardImage = firstClipboardImageFile(event.clipboardData);
  if (!clipboardImage) {
    const target = event.target;
    if (target instanceof Element && target.matches(EDITABLE_TARGET_SELECTOR)) {
      return;
    }
    state.globalError = {
      code: "validation_failed",
      message: "剪贴板中没有可用的图片。",
    };
    state.statusMessage = null;
    renderWorkspace();
    return;
  }

  event.preventDefault();
  setPendingQueryImageFile(clipboardImage);
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export function onClearQueryImage() {
  clearQueryImageState();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export function onClearQueryDocument() {
  clearQueryDocumentState();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export function onQueryVideoSourceSelect(event) {
  const sourceId = event.target.value;
  if (!sourceId) {
    if (!state.queryVideoLibraryObject) {
      clearQueryVideoState();
    }
  } else {
    const source = state.videoSources.find((item) => item.source_id === sourceId);
    if (source) {
      setLibraryQueryVideoSource(source);
    }
  }
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export function onClearQueryVideo() {
  clearQueryVideoState();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export function onClearQueryVideoRange() {
  state.queryVideoRange = null;
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export function onClearQueryDocumentRange() {
  state.queryDocumentStartPageDraft = "";
  state.queryDocumentEndPageDraft = "";
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export function onQueryVideoRangeStartInput(event) {
  if (!state.queryVideoDurationMs) {
    return;
  }

  const startMs = Math.max(0, Math.round(Number(event.target.value) || 0));
  const currentEndMs = Math.max(currentQueryVideoEndMs(), startMs + 1);
  state.queryVideoRange = {
    start_ms: Math.min(startMs, state.queryVideoDurationMs - 1),
    end_ms: Math.min(currentEndMs, state.queryVideoDurationMs),
  };
  if (state.queryVideoRange.end_ms <= state.queryVideoRange.start_ms) {
    state.queryVideoRange.end_ms = Math.min(
      state.queryVideoDurationMs,
      state.queryVideoRange.start_ms + queryVideoRangeStep()
    );
  }
  renderWorkspace();
}

export function onQueryVideoRangeEndInput(event) {
  if (!state.queryVideoDurationMs) {
    return;
  }

  const endMs = Math.min(
    state.queryVideoDurationMs,
    Math.max(1, Math.round(Number(event.target.value) || state.queryVideoDurationMs))
  );
  const currentStartMs = Math.min(currentQueryVideoStartMs(), endMs - 1);
  state.queryVideoRange = {
    start_ms: Math.max(0, currentStartMs),
    end_ms: endMs,
  };
  if (state.queryVideoRange.start_ms >= state.queryVideoRange.end_ms) {
    state.queryVideoRange.start_ms = Math.max(0, state.queryVideoRange.end_ms - queryVideoRangeStep());
  }
  renderWorkspace();
}

export function onQueryVideoPreviewLoadedMetadata(event) {
  syncQueryVideoDurationFromVideoElement(event.currentTarget);
}

export function onQueryDocumentRangeStartInput(event) {
  state.queryDocumentStartPageDraft = event.target.value.trim();
  state.globalError = null;
  state.statusMessage = null;
  syncQueryDocumentRangeUi();
}

export function onQueryDocumentRangeEndInput(event) {
  state.queryDocumentEndPageDraft = event.target.value.trim();
  state.globalError = null;
  state.statusMessage = null;
  syncQueryDocumentRangeUi();
}

export function resolveLibraryObjectQueryImage(
  assetId,
  libraryId: string | null = null
): LibraryObjectQueryImage | null {
  const resultItem =
    state.searchOutcome?.results?.find(
      (item) =>
        item.asset_id === assetId && (!libraryId || item.library_id === libraryId)
    ) ?? null;
  if (resultItem?.asset_type === "image") {
    return {
      library_id: resultItem.library_id,
      asset_id: resultItem.asset_id,
      asset_type: resultItem.asset_type,
      source_uri: resultItem.source_uri,
      preview: resultItem.preview,
    };
  }
  if (resultItem?.asset_type === "document_page") {
    return {
      library_id: resultItem.library_id,
      asset_id: resultItem.asset_id,
      asset_type: resultItem.asset_type,
      source_uri: resultItem.source_uri,
      preview: resultItem.preview,
    };
  }

  const detailAsset = state.selectedAsset?.asset;
  if (
    detailAsset?.asset_id === assetId &&
    (detailAsset.asset_type === "image" || detailAsset.asset_type === "document_page")
  ) {
    return {
      library_id: selectedAssetOriginLibraryId(),
      asset_id: detailAsset.asset_id,
      asset_type: detailAsset.asset_type,
      source_uri: detailAsset.source_uri,
      preview: state.selectedAsset.preview,
    };
  }

  const inventorySource = selectedInventorySource();
  const representativeVisual = selectedInventoryRepresentativeAsset(inventorySource);
  const representativePreview = selectedInventoryRepresentativePreview(inventorySource);
  if (
    representativeVisual?.asset_id === assetId &&
    representativePreview &&
    (representativeVisual.asset_type === "image" || representativeVisual.asset_type === "document_page")
  ) {
    return {
      library_id: state.selectedLibraryId,
      asset_id: representativeVisual.asset_id,
      asset_type: representativeVisual.asset_type,
      source_uri: inventorySource?.source_uri ?? "",
      preview: representativePreview,
    };
  }

  return null;
}

export function resolveLibraryObjectQueryVideo(
  assetId,
  libraryId: string | null = null
): LibraryObjectQueryVideo | null {
  const resultItem =
    state.searchOutcome?.results?.find(
      (item) =>
        item.asset_id === assetId && (!libraryId || item.library_id === libraryId)
    ) ?? null;
  if (resultItem?.asset_type === "video_segment") {
    return {
      library_id: resultItem.library_id,
      asset_id: resultItem.asset_id,
      asset_type: resultItem.asset_type,
      source_uri: resultItem.source_uri,
      locator: resultItem.locator,
      preview: resultItem.preview,
    };
  }

  const detailAsset = state.selectedAsset?.asset;
  if (detailAsset?.asset_id === assetId && detailAsset.asset_type === "video_segment") {
    return {
      library_id: selectedAssetOriginLibraryId(),
      asset_id: detailAsset.asset_id,
      asset_type: detailAsset.asset_type,
      source_uri: detailAsset.source_uri,
      locator: detailAsset.locator,
      preview: state.selectedAsset.preview,
    };
  }

  const inventorySource = selectedInventorySource();
  const representativeVisual = selectedInventoryRepresentativeAsset(inventorySource);
  const representativePreview = selectedInventoryRepresentativePreview(inventorySource);
  if (
    representativeVisual?.asset_id === assetId &&
    representativePreview &&
    representativeVisual.asset_type === "video_segment"
  ) {
    return {
      library_id: state.selectedLibraryId,
      asset_id: representativeVisual.asset_id,
      asset_type: representativeVisual.asset_type,
      source_uri: inventorySource?.source_uri ?? "",
      locator: representativeVisual.locator,
      preview: representativePreview,
    };
  }

  return null;
}

export function resolveLibraryObjectQueryDocument(
  assetId,
  libraryId: string | null = null
): LibraryObjectQueryDocument | null {
  const resultItem =
    state.searchOutcome?.results?.find(
      (item) =>
        item.asset_id === assetId && (!libraryId || item.library_id === libraryId)
    ) ?? null;
  if (resultItem?.asset_type === "document_page") {
    const page = Number(resultItem.locator?.page ?? 0);
    return {
      library_id: resultItem.library_id,
      asset_id: resultItem.asset_id,
      source_id: resultItem.source_id,
      asset_type: resultItem.asset_type,
      source_uri: resultItem.source_uri,
      locator:
        page > 0
          ? {
              start_page: page,
              end_page: page,
            }
          : null,
      preview: resultItem.preview,
    };
  }

  const detailAsset = state.selectedAsset?.asset;
  if (detailAsset?.asset_id === assetId && detailAsset.asset_type === "document_page") {
    const page = Number(detailAsset.locator?.page ?? 0);
    return {
      library_id: selectedAssetOriginLibraryId(),
      asset_id: detailAsset.asset_id,
      source_id: detailAsset.source_id,
      asset_type: detailAsset.asset_type,
      source_uri: detailAsset.source_uri,
      locator:
        page > 0
          ? {
              start_page: page,
              end_page: page,
            }
          : null,
      preview: state.selectedAsset.preview,
    };
  }

  const inventorySource = selectedInventorySource();
  const representativeVisual = selectedInventoryRepresentativeAsset(inventorySource);
  const representativePreview = selectedInventoryRepresentativePreview(inventorySource);
  if (
    representativeVisual?.asset_id === assetId &&
    representativePreview &&
    representativeVisual.asset_type === "document_page"
  ) {
    const page = Number(representativeVisual.locator?.page ?? 0);
    return {
      library_id: state.selectedLibraryId,
      asset_id: representativeVisual.asset_id,
      source_id: representativeVisual.source_id,
      asset_type: representativeVisual.asset_type,
      source_uri: inventorySource?.source_uri ?? "",
      locator:
        page > 0
          ? {
              start_page: page,
              end_page: page,
            }
          : null,
      preview: representativePreview,
    };
  }

  return null;
}

export async function onUseAsQueryImage(event) {
  const assetId = event.currentTarget.dataset.useQueryAssetId;
  const libraryId = event.currentTarget.dataset.useQueryLibraryId ?? null;
  const libraryObject = resolveLibraryObjectQueryImage(assetId, libraryId);
  if (!libraryObject) {
    state.globalError = {
      code: "not_supported",
      message: "当前只能把库内 image 或 document_page 对象作为查询图片。",
    };
    renderWorkspace();
    return;
  }

  if (libraryObject.library_id && libraryObject.library_id !== state.selectedLibraryId) {
    try {
      await switchCurrentLibrary(libraryObject.library_id);
    } catch (error) {
      state.globalError = toApiError(error);
      renderWorkspace();
      return;
    }
  }
  clearQueryImageState();
  state.queryImageLibraryObject = libraryObject;
  state.activeWorkspace = "search";
  state.searchMode = "image";
  state.searchScope = "library";
  state.inventoryDetailSheetOpen = false;
  state.searchDetailSheetOpen = false;
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export async function onUseAsQueryVideo(event) {
  const assetId = event.currentTarget.dataset.useQueryVideoAssetId;
  const libraryId = event.currentTarget.dataset.useQueryLibraryId ?? null;
  const libraryObject = resolveLibraryObjectQueryVideo(assetId, libraryId);
  if (!libraryObject) {
    state.globalError = {
      code: "not_supported",
      message: "当前只能把库内 video_segment 对象作为查询视频片段。",
    };
    renderWorkspace();
    return;
  }

  if (libraryObject.library_id && libraryObject.library_id !== state.selectedLibraryId) {
    try {
      await switchCurrentLibrary(libraryObject.library_id);
    } catch (error) {
      state.globalError = toApiError(error);
      renderWorkspace();
      return;
    }
  }
  setLibraryQueryVideoAsset(libraryObject);
  state.activeWorkspace = "search";
  state.searchMode = "video";
  state.searchScope = "library";
  state.inventoryDetailSheetOpen = false;
  state.searchDetailSheetOpen = false;
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export async function onUseAsQueryDocument(event) {
  const assetId = event.currentTarget.dataset.useQueryDocumentAssetId;
  const libraryId = event.currentTarget.dataset.useQueryLibraryId ?? null;
  const libraryObject = resolveLibraryObjectQueryDocument(assetId, libraryId);
  if (!libraryObject) {
    state.globalError = {
      code: "not_supported",
      message: "当前只能把库内 document_page 对象作为查询文档。",
    };
    renderWorkspace();
    return;
  }

  if (libraryObject.library_id && libraryObject.library_id !== state.selectedLibraryId) {
    try {
      await switchCurrentLibrary(libraryObject.library_id);
    } catch (error) {
      state.globalError = toApiError(error);
      renderWorkspace();
      return;
    }
  }
  setLibraryQueryDocumentAsset(libraryObject);
  state.activeWorkspace = "search";
  state.searchMode = "document";
  state.searchScope = "library";
  state.inventoryDetailSheetOpen = false;
  state.searchDetailSheetOpen = false;
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}
