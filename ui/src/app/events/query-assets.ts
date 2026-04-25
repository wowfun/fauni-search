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
  selectedInventoryRepresentativeVisualUnit,
  selectedInventorySource,
  selectedLibrary,
  selectedLibraryContentTypeBinding,
  selectedLibraryContentTypeHasOverride,
  selectedLibraryContentTypeKey,
  selectedLibraryModelSelection,
  selectedProviderConfig,
  selectedVisualUnitId,
  selectedVisualUnitOriginLibraryId,
  setLibraryQueryDocumentVisualUnit,
  setLibraryQueryVideoSource,
  setLibraryQueryVideoVisualUnit,
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
  type VisualUnitDetailData,
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
  visualUnitId,
  libraryId: string | null = null
): LibraryObjectQueryImage | null {
  const resultItem =
    state.searchOutcome?.results?.find(
      (item) =>
        item.visual_unit_id === visualUnitId && (!libraryId || item.library_id === libraryId)
    ) ?? null;
  if (resultItem?.kind === "image") {
    return {
      library_id: resultItem.library_id,
      visual_unit_id: resultItem.visual_unit_id,
      kind: resultItem.kind,
      source_path: resultItem.source_path,
      preview: resultItem.preview,
    };
  }
  if (resultItem?.kind === "document_page") {
    return {
      library_id: resultItem.library_id,
      visual_unit_id: resultItem.visual_unit_id,
      kind: resultItem.kind,
      source_path: resultItem.source_path,
      preview: resultItem.preview,
    };
  }

  const detailVisualUnit = state.selectedVisualUnit?.visual_unit;
  if (
    detailVisualUnit?.visual_unit_id === visualUnitId &&
    (detailVisualUnit.kind === "image" || detailVisualUnit.kind === "document_page")
  ) {
    return {
      library_id: selectedVisualUnitOriginLibraryId(),
      visual_unit_id: detailVisualUnit.visual_unit_id,
      kind: detailVisualUnit.kind,
      source_path: detailVisualUnit.source_path,
      preview: state.selectedVisualUnit.preview,
    };
  }

  const inventorySource = selectedInventorySource();
  const representativeVisual = selectedInventoryRepresentativeVisualUnit(inventorySource);
  const representativePreview = selectedInventoryRepresentativePreview(inventorySource);
  if (
    representativeVisual?.visual_unit_id === visualUnitId &&
    representativePreview &&
    (representativeVisual.kind === "image" || representativeVisual.kind === "document_page")
  ) {
    return {
      library_id: state.selectedLibraryId,
      visual_unit_id: representativeVisual.visual_unit_id,
      kind: representativeVisual.kind,
      source_path: inventorySource?.source_path ?? "",
      preview: representativePreview,
    };
  }

  return null;
}

export function resolveLibraryObjectQueryVideo(
  visualUnitId,
  libraryId: string | null = null
): LibraryObjectQueryVideo | null {
  const resultItem =
    state.searchOutcome?.results?.find(
      (item) =>
        item.visual_unit_id === visualUnitId && (!libraryId || item.library_id === libraryId)
    ) ?? null;
  if (resultItem?.kind === "video_segment") {
    return {
      library_id: resultItem.library_id,
      visual_unit_id: resultItem.visual_unit_id,
      kind: resultItem.kind,
      source_path: resultItem.source_path,
      locator: resultItem.locator,
      preview: resultItem.preview,
    };
  }

  const detailVisualUnit = state.selectedVisualUnit?.visual_unit;
  if (detailVisualUnit?.visual_unit_id === visualUnitId && detailVisualUnit.kind === "video_segment") {
    return {
      library_id: selectedVisualUnitOriginLibraryId(),
      visual_unit_id: detailVisualUnit.visual_unit_id,
      kind: detailVisualUnit.kind,
      source_path: detailVisualUnit.source_path,
      locator: detailVisualUnit.locator,
      preview: state.selectedVisualUnit.preview,
    };
  }

  const inventorySource = selectedInventorySource();
  const representativeVisual = selectedInventoryRepresentativeVisualUnit(inventorySource);
  const representativePreview = selectedInventoryRepresentativePreview(inventorySource);
  if (
    representativeVisual?.visual_unit_id === visualUnitId &&
    representativePreview &&
    representativeVisual.kind === "video_segment"
  ) {
    return {
      library_id: state.selectedLibraryId,
      visual_unit_id: representativeVisual.visual_unit_id,
      kind: representativeVisual.kind,
      source_path: inventorySource?.source_path ?? "",
      locator: representativeVisual.locator,
      preview: representativePreview,
    };
  }

  return null;
}

export function resolveLibraryObjectQueryDocument(
  visualUnitId,
  libraryId: string | null = null
): LibraryObjectQueryDocument | null {
  const resultItem =
    state.searchOutcome?.results?.find(
      (item) =>
        item.visual_unit_id === visualUnitId && (!libraryId || item.library_id === libraryId)
    ) ?? null;
  if (resultItem?.kind === "document_page") {
    const page = Number(resultItem.locator?.page ?? 0);
    return {
      library_id: resultItem.library_id,
      visual_unit_id: resultItem.visual_unit_id,
      source_id: resultItem.source_id,
      kind: resultItem.kind,
      source_path: resultItem.source_path,
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

  const detailVisualUnit = state.selectedVisualUnit?.visual_unit;
  if (detailVisualUnit?.visual_unit_id === visualUnitId && detailVisualUnit.kind === "document_page") {
    const page = Number(detailVisualUnit.locator?.page ?? 0);
    return {
      library_id: selectedVisualUnitOriginLibraryId(),
      visual_unit_id: detailVisualUnit.visual_unit_id,
      source_id: detailVisualUnit.source_id,
      kind: detailVisualUnit.kind,
      source_path: detailVisualUnit.source_path,
      locator:
        page > 0
          ? {
              start_page: page,
              end_page: page,
            }
          : null,
      preview: state.selectedVisualUnit.preview,
    };
  }

  const inventorySource = selectedInventorySource();
  const representativeVisual = selectedInventoryRepresentativeVisualUnit(inventorySource);
  const representativePreview = selectedInventoryRepresentativePreview(inventorySource);
  if (
    representativeVisual?.visual_unit_id === visualUnitId &&
    representativePreview &&
    representativeVisual.kind === "document_page"
  ) {
    const page = Number(representativeVisual.locator?.page ?? 0);
    return {
      library_id: state.selectedLibraryId,
      visual_unit_id: representativeVisual.visual_unit_id,
      source_id: representativeVisual.source_id,
      kind: representativeVisual.kind,
      source_path: inventorySource?.source_path ?? "",
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
  const visualUnitId = event.currentTarget.dataset.useQueryVisualUnitId;
  const libraryId = event.currentTarget.dataset.useQueryLibraryId ?? null;
  const libraryObject = resolveLibraryObjectQueryImage(visualUnitId, libraryId);
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
  const visualUnitId = event.currentTarget.dataset.useQueryVideoVisualUnitId;
  const libraryId = event.currentTarget.dataset.useQueryLibraryId ?? null;
  const libraryObject = resolveLibraryObjectQueryVideo(visualUnitId, libraryId);
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
  setLibraryQueryVideoVisualUnit(libraryObject);
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
  const visualUnitId = event.currentTarget.dataset.useQueryDocumentVisualUnitId;
  const libraryId = event.currentTarget.dataset.useQueryLibraryId ?? null;
  const libraryObject = resolveLibraryObjectQueryDocument(visualUnitId, libraryId);
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
  setLibraryQueryDocumentVisualUnit(libraryObject);
  state.activeWorkspace = "search";
  state.searchMode = "document";
  state.searchScope = "library";
  state.inventoryDetailSheetOpen = false;
  state.searchDetailSheetOpen = false;
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}
