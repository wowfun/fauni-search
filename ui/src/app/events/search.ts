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
import { uploadQueryDocument, uploadQueryImage, uploadQueryVideo } from "./query-assets";
import { loadVisualUnit } from "./workspace";
import { visibleSearchResults } from "../workspaces/search";

export function onImportPathsInput(event) {
  state.importPathsDraft = event.target.value;
}

export function onSearchTextInput(event) {
  state.searchTextDraft = event.target.value;
}

export function onSearchFilterKindChange(event) {
  state.searchFilters.visualUnitKind = event.target.value;
}

export function onSearchFilterSourceTypeChange(event) {
  state.searchFilters.sourceType = event.target.value;
}

export function onSearchFilterPathPrefixInput(event) {
  state.searchFilters.pathPrefix = event.target.value;
}

export function onSearchFilterTimeRangeStartInput(event) {
  state.searchFilters.timeRangeStartMsDraft = event.target.value;
}

export function onSearchFilterTimeRangeEndInput(event) {
  state.searchFilters.timeRangeEndMsDraft = event.target.value;
}

export function onClearSearchFilters() {
  resetSearchFilters();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export function onToggleSearchFiltersPanel() {
  state.searchFiltersPanelOpen = !state.searchFiltersPanelOpen;
  renderWorkspace();
}

export function onSelectSearchScope(event) {
  const nextScope = event.currentTarget.dataset.searchScope as SearchScopeKind | undefined;
  if (!nextScope || nextScope === state.searchScope) {
    return;
  }
  state.searchScope = nextScope;
  resetSearchResultLibraryFocus();
  state.globalError = null;
  state.statusMessage = null;
  state.searchInFlight = false;
  if (nextScope === "all_libraries" && state.searchMode !== "text") {
    state.searchMode = "text";
  }
  renderWorkspace();
}

export async function onSelectSearchResultLibraryFocus(event) {
  const nextLibraryId = event.currentTarget.dataset.searchResultLibraryFocus?.trim() ?? "";
  if (nextLibraryId === state.searchResultLibraryFocusId) {
    return;
  }

  state.searchResultLibraryFocusId = nextLibraryId;
  const results = visibleSearchResults();
  if (!results.length) {
    renderWorkspace();
    return;
  }

  const currentSelection = selectedVisualUnitId();
  const currentStillVisible = results.some(
    (item) => `${item.library_id}:${item.visual_unit_id}` === currentSelection
  );
  if (currentStillVisible) {
    renderWorkspace();
    return;
  }

  await loadVisualUnit(results[0].library_id, results[0].visual_unit_id);
}

export async function onSearchSubmit(event) {
  event.preventDefault();
  if (!state.selectedLibraryId) {
    return;
  }

  try {
    state.globalError = null;
    state.searchInFlight = true;
    renderWorkspace();
    if (state.searchMode === "image") {
      await runImageSearch();
    } else if (state.searchMode === "video") {
      await runVideoSearch();
    } else if (state.searchMode === "document") {
      await runDocumentSearch();
    } else {
      await runTextSearch();
    }
    state.searchInFlight = false;
    state.statusMessage = null;
    renderWorkspace();
  } catch (error) {
    state.searchInFlight = false;
    state.searchOutcome = { error: toApiError(error) };
    state.statusMessage = null;
    renderWorkspace();
  }
}

export async function onLoadMoreSearchResults() {
  if (!state.searchOutcome?.next_cursor || !state.lastSearchRequest) {
    return;
  }

  try {
    state.globalError = null;
    state.statusMessage = "正在加载更多搜索结果...";
    renderWorkspace();
    await executeSearchRequest(state.lastSearchRequest, {
      append: true,
      cursor: state.searchOutcome.next_cursor,
    });
    state.statusMessage = null;
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export function sharedSearchRequestFields() {
  const filters = searchFiltersPayload();
  return {
    search_scope: searchScopeRequestPayload(),
    top_k: SEARCH_PAGE_SIZE,
    debug: true,
    ...(filters ? { filters } : {}),
  };
}

export function sharedLibraryBoundSearchRequestFields() {
  return {
    ...sharedSearchRequestFields(),
    library_id: state.selectedLibraryId,
  };
}

export async function executeSearchRequest(
  request: SearchRequestSnapshot,
  options: { append?: boolean; cursor?: string | null } = {}
): Promise<SearchOutcomeState> {
  const requestBody = {
    ...request.body,
    ...(options.cursor ? { cursor: options.cursor } : {}),
  };
  const data = await apiRequest<SearchOutcomeState>(request.endpoint, {
    method: "POST",
    body: JSON.stringify(requestBody),
  });
  const fallbackLibraryId =
    typeof request.body?.library_id === "string" && request.body.library_id.trim()
      ? request.body.library_id
      : state.selectedLibraryId;
  const normalizedResults =
    data.results?.map((result) => ({
      ...result,
      library_id: result.library_id || fallbackLibraryId,
    })) ?? data.results;
  const mergedResults = options.append
    ? [...(state.searchOutcome?.results ?? []), ...(normalizedResults ?? [])]
    : normalizedResults;
  if (!options.append) {
    resetSearchResultLibraryFocus();
  }
  state.searchOutcome = {
    ...data,
    results: mergedResults,
  };
  state.lastSearchRequest = request;
  renderWorkspace();
  if (!options.append && data.results?.[0]?.visual_unit_id) {
    await loadVisualUnit(data.results[0].library_id, data.results[0].visual_unit_id);
  }
  return state.searchOutcome;
}

export async function runTextSearch() {
  const input = document.querySelector<HTMLInputElement>("#search-text");
  state.searchTextDraft = input?.value ?? "";
  const text = state.searchTextDraft.trim();
  if (!text) {
    state.searchOutcome = {
      error: {
        code: "validation_failed",
        message: "请输入查询文本。",
      },
    };
    renderWorkspace();
    return;
  }

  await searchText(text);
}

export async function runImageSearch() {
  if (!state.queryImageFile && !state.queryImageAsset && !state.queryImageLibraryObject) {
    state.searchOutcome = {
      error: {
        code: "validation_failed",
        message: "请先选择一张查询图片。",
      },
    };
    renderWorkspace();
    return;
  }

  if (state.queryImageFile) {
    await uploadQueryImage(state.queryImageFile);
  }

  if (state.queryImageAsset) {
    await searchImage({
      kind: "temp_asset",
      temp_asset_id: state.queryImageAsset.temp_asset_id,
    });
    return;
  }

  if (state.queryImageLibraryObject) {
    await searchImage({
      kind: "library_object",
      visual_unit_id: state.queryImageLibraryObject.visual_unit_id,
    });
  }
}

export async function runVideoSearch() {
  if (
    !state.queryVideoFile &&
    !state.queryVideoAsset &&
    !state.queryVideoSource &&
    !state.queryVideoLibraryObject
  ) {
    state.searchOutcome = {
      error: {
        code: "validation_failed",
        message: "请先选择一个查询视频。",
      },
    };
    renderWorkspace();
    return;
  }

  if (state.queryVideoFile) {
    await uploadQueryVideo(state.queryVideoFile);
  }

  const locator = queryVideoLocatorPayload();
  if (state.queryVideoAsset) {
    await searchVideo({
      kind: "temp_asset",
      temp_asset_id: state.queryVideoAsset.temp_asset_id,
      ...(locator ? { locator } : {}),
    });
    return;
  }

  if (state.queryVideoSource) {
    await searchVideo({
      kind: "library_object",
      source_id: state.queryVideoSource.source_id,
      ...(locator ? { locator } : {}),
    });
    return;
  }

  if (state.queryVideoLibraryObject) {
    await searchVideo({
      kind: "library_object",
      visual_unit_id: state.queryVideoLibraryObject.visual_unit_id,
    });
  }
}

export async function runDocumentSearch() {
  if (!state.queryDocumentFile && !state.queryDocumentAsset && !state.queryDocumentLibraryObject) {
    state.searchOutcome = {
      error: {
        code: "validation_failed",
        message: "请先选择一个查询文档。",
      },
    };
    renderWorkspace();
    return;
  }

  if (state.queryDocumentFile) {
    await uploadQueryDocument(state.queryDocumentFile);
  }

  const locator = queryDocumentLocatorPayload();
  if (state.queryDocumentAsset) {
    await searchDocument({
      kind: "temp_asset",
      temp_asset_id: state.queryDocumentAsset.temp_asset_id,
      ...(locator ? { locator } : {}),
    });
    return;
  }

  if (state.queryDocumentLibraryObject) {
    await searchDocument({
      kind: "library_object",
      source_id: state.queryDocumentLibraryObject.source_id,
      ...(locator ? { locator } : {}),
    });
  }
}

export async function searchText(text: string): Promise<SearchOutcomeState> {
  return executeSearchRequest({
    endpoint: "/search/text",
    body: {
      ...sharedSearchRequestFields(),
      text,
    },
  });
}

export async function searchImage(imageInput: Record<string, unknown>): Promise<SearchOutcomeState> {
  return executeSearchRequest({
    endpoint: "/search/image",
    body: {
      ...sharedLibraryBoundSearchRequestFields(),
      image_input: imageInput,
    },
  });
}

export async function searchVideo(videoInput: Record<string, unknown>): Promise<SearchOutcomeState> {
  return executeSearchRequest({
    endpoint: "/search/video",
    body: {
      ...sharedLibraryBoundSearchRequestFields(),
      video_input: videoInput,
    },
  });
}

export async function searchDocument(documentInput: Record<string, unknown>): Promise<SearchOutcomeState> {
  return executeSearchRequest({
    endpoint: "/search/document",
    body: {
      ...sharedLibraryBoundSearchRequestFields(),
      document_input: documentInput,
    },
  });
}
