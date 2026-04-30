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

export async function onCreateLibrary(event) {
  event.preventDefault();
  const displayName = state.libraryDisplayNameDraft.trim();
  const libraryId = state.libraryIdDraft.trim();
  if (!displayName) {
    return;
  }

  try {
    state.globalError = null;
    const library = await apiRequest("/libraries", {
      method: "POST",
      body: JSON.stringify({
        display_name: displayName,
        ...(libraryId ? { library_id: libraryId } : {}),
      }),
    });
    state.selectedLibraryId = library.id;
    resetSourceRootEditor();
    resetInventoryFilters();
    resetSearchFilters();
    resetSearchResultLibraryFocus();
    resetInventoryState();
    state.libraryContentTypes = emptyContentTypes();
    state.resolvedContentModels = null;
    state.vectorSpaceDiagnostics = null;
    state.importPathsDraft = "";
    state.searchTextDraft = "";
    clearQueryImageState();
    clearQueryVideoState();
    clearQueryDocumentState();
    resetLibraryModelTestState();
    state.importReceipt = null;
    state.selectedAsset = null;
    state.selectedAssetLibraryId = "";
    state.searchOutcome = null;
    state.searchInFlight = false;
    state.lastSearchRequest = null;
    state.inventoryImportOpen = false;
    state.settingsDiagnosticsJobsOpen = false;
    state.searchDetailSheetOpen = false;
    state.statusMessage = null;
    state.libraryDisplayNameDraft = "";
    state.libraryIdDraft = "";
    state.createLibraryPopoverOpen = false;
    state.manageLibraryPopoverOpen = false;
    await refreshWorkspace({ keepSelection: true });
  } catch (error) {
    state.globalError = toApiError(error);
    renderWorkspace();
  }
}

export function onLibraryNameInput(event) {
  state.libraryDisplayNameDraft = event.target.value;
}

export function onLibraryIdInput(event) {
  state.libraryIdDraft = event.target.value;
}

export function onManageLibraryNameInput(event) {
  state.libraryManagementDisplayNameDraft = event.target.value;
}

export function onCreateLibraryPopoverToggle(event) {
  const popover = event.currentTarget as HTMLDetailsElement | null;
  if (!(popover instanceof HTMLDetailsElement)) {
    return;
  }
  state.createLibraryPopoverOpen = popover.open;
  if (popover.open) {
    state.manageLibraryPopoverOpen = false;
  }
}

export function onManageLibraryPopoverToggle(event) {
  const popover = event.currentTarget as HTMLDetailsElement | null;
  if (!(popover instanceof HTMLDetailsElement)) {
    return;
  }
  state.manageLibraryPopoverOpen = popover.open;
  if (popover.open) {
    state.createLibraryPopoverOpen = false;
  }
}

export async function onRenameLibrary(event) {
  event.preventDefault();
  if (!state.selectedLibraryId) {
    return;
  }

  const displayName = state.libraryManagementDisplayNameDraft.trim();
  const previousLibrary = selectedLibrary();
  if (!displayName) {
    state.globalError = {
      code: "validation_failed",
      message: "库显示名称不能为空。",
      details: {
        field: "display_name",
      },
    };
    renderWorkspace();
    return;
  }

  try {
    state.globalError = null;
    state.statusMessage = "正在更新当前库名称...";
    if (previousLibrary) {
      upsertLibrarySnapshot({
        ...previousLibrary,
        display_name: displayName,
      });
    }
    renderWorkspace();
    const library = await apiRequest<LibrarySnapshot>(
      `/libraries/${encodeURIComponent(state.selectedLibraryId)}`,
      {
        method: "PATCH",
        body: JSON.stringify({
          display_name: displayName,
        }),
      }
    );
    const nextLibrary = {
      ...(selectedLibrary() ?? library),
      ...library,
      display_name: displayName,
    };
    upsertLibrarySnapshot(nextLibrary);
    hydrateLibraryManagementDraft(nextLibrary);
    state.statusMessage = null;
    renderWorkspace();
  } catch (error) {
    if (previousLibrary) {
      upsertLibrarySnapshot(previousLibrary);
    }
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export async function onToggleLibraryArchive() {
  if (!state.selectedLibraryId) {
    return;
  }

  const library = selectedLibrary();
  if (!library) {
    return;
  }

  const archived = libraryIsArchived(library);
  const displayName = libraryDisplayName(library);
  if (
    !archived &&
    !window.confirm(`确认归档“${displayName}”吗？归档会保留内容、来源和设置，之后仍可恢复。`)
  ) {
    return;
  }

  try {
    state.globalError = null;
    state.statusMessage = archived
      ? `正在恢复 ${displayName}...`
      : `正在归档 ${displayName}...`;
    renderWorkspace();
    const nextLibrary = await apiRequest<LibrarySnapshot>(
      `/libraries/${encodeURIComponent(state.selectedLibraryId)}/${archived ? "restore" : "archive"}`,
      {
        method: "POST",
      }
    );
    upsertLibrarySnapshot(nextLibrary);
    hydrateLibraryManagementDraft(nextLibrary);
    state.statusMessage = null;
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export async function onDeleteLibrary() {
  if (!state.selectedLibraryId) {
    return;
  }

  const library = selectedLibrary();
  const displayName = library ? libraryDisplayName(library) : state.selectedLibraryId;
  if (
    !window.confirm(
      `确认删除“${displayName}”吗？这会移除当前库的本地状态、任务引用和查询资产记录。`
    )
  ) {
    return;
  }

  try {
    state.globalError = null;
    state.statusMessage = `正在删除 ${displayName}...`;
    renderWorkspace();
    await apiRequest<LibrarySnapshot>(`/libraries/${encodeURIComponent(state.selectedLibraryId)}`, {
      method: "DELETE",
    });
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
    state.selectedAsset = null;
    state.selectedAssetLibraryId = "";
    state.searchOutcome = null;
    state.searchInFlight = false;
    state.lastSearchRequest = null;
    state.inventoryImportOpen = false;
    state.settingsDiagnosticsJobsOpen = false;
    state.searchDetailSheetOpen = false;
    state.createLibraryPopoverOpen = false;
    state.manageLibraryPopoverOpen = false;
    state.statusMessage = null;
    await refreshWorkspace({ keepSelection: false });
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export async function onSelectLibrary(event) {
  await switchCurrentLibrary(event.target.value);
}
