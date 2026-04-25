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
  setInventoryLibraryMaintenanceOpen,
  setInventorySourceManagementOpen,
  setSourceRootAdvancedRulesOpen,
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
  startSourceRootCreate,
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
import { importPaths, triggerJobBackedAction } from "./jobs";

export function onSourceRootPathInput(event) {
  state.sourceRootPathDraft = event.target.value;
}

export function onSourceRootEnabledInput(event) {
  state.sourceRootEnabledDraft = event.target.checked;
}

export function onSourceRootIncludeGlobsInput(event) {
  state.sourceRootIncludeGlobsDraft = event.target.value;
}

export function onSourceRootExcludeGlobsInput(event) {
  state.sourceRootExcludeGlobsDraft = event.target.value;
}

export function onSourceRootIncludeExtensionsInput(event) {
  state.sourceRootIncludeExtensionsDraft = event.target.value;
}

export function onToggleInventorySourceManagement() {
  setInventorySourceManagementOpen(!state.inventorySourceManagementOpen);
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export function onToggleInventoryLibraryMaintenance() {
  setInventoryLibraryMaintenanceOpen(!state.inventoryLibraryMaintenanceOpen);
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export function onStartCreateSourceRoot() {
  startSourceRootCreate();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export function onToggleSourceRootAdvancedRules() {
  setSourceRootAdvancedRulesOpen(!state.sourceRootAdvancedRulesOpen);
  renderWorkspace();
}

export function onSourceFilterRootChange(event) {
  state.inventoryFilters.sourceRootId = event.target.value;
  refreshWorkspace({ keepSelection: true }).catch((error) => {
    state.globalError = toApiError(error);
    renderWorkspace();
  });
}

export function onSourceFilterTypeChange(event) {
  state.inventoryFilters.sourceType = event.target.value;
  refreshWorkspace({ keepSelection: true }).catch((error) => {
    state.globalError = toApiError(error);
    renderWorkspace();
  });
}

export function onSourceFilterStatusChange(event) {
  state.inventoryFilters.sourceStatus = event.target.value;
  refreshWorkspace({ keepSelection: true }).catch((error) => {
    state.globalError = toApiError(error);
    renderWorkspace();
  });
}

export function parseImportPaths(value) {
  return value
    .split("\n")
    .map((item) => item.trim())
    .filter(Boolean);
}

export async function onSubmitSourceRoot(event) {
  event.preventDefault();
  if (!state.selectedLibraryId) {
    return;
  }

  const payload = sourceRootPayloadFromDraft();
  if (!payload.root_path) {
    state.globalError = {
      code: "validation_failed",
      message: "请先填写来源根目录路径。",
    };
    renderWorkspace();
    return;
  }

  try {
    keepSearchPreparationDisclosureOpen();
    state.globalError = null;
    state.statusMessage = state.editingSourceRootId
      ? "正在保存来源根..."
      : "正在创建来源根...";
    renderWorkspace();

    const path = state.editingSourceRootId
      ? `/libraries/${state.selectedLibraryId}/source-roots/${encodeURIComponent(state.editingSourceRootId)}`
      : `/libraries/${state.selectedLibraryId}/source-roots`;
    const method = state.editingSourceRootId ? "PATCH" : "POST";
    await apiRequest(path, {
      method,
      body: JSON.stringify(payload),
    });

    resetSourceRootEditor();
    state.statusMessage = null;
    await refreshWorkspace({ keepSelection: true });
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export function onResetSourceRootEditor() {
  resetSourceRootEditor();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export function onEditSourceRoot(event) {
  const sourceRootId = event.currentTarget.dataset.sourceRootEditId;
  const sourceRoot = state.sourceRoots.find((item) => item.source_root_id === sourceRootId);
  if (!sourceRoot) {
    return;
  }
  populateSourceRootEditor(sourceRoot);
  keepSearchPreparationDisclosureOpen();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export function onSearchPreparationDisclosureToggle(event) {
  state.searchPreparationDisclosureOpen = event.currentTarget.open;
}

export async function onRefreshLibrarySources(event?) {
  if (!state.selectedLibraryId) {
    return;
  }

  try {
    if (event) {
      keepSearchPreparationDisclosureOpen();
    }
    await triggerJobBackedAction<SourceActionData>(
      `/libraries/${state.selectedLibraryId}/refresh`,
      "正在执行库级刷新..."
    );
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export async function onRescanLibrarySources(event?) {
  if (!state.selectedLibraryId) {
    return;
  }

  try {
    if (event) {
      keepSearchPreparationDisclosureOpen();
    }
    await triggerJobBackedAction<SourceActionData>(
      `/libraries/${state.selectedLibraryId}/rescan`,
      "正在执行库级重扫..."
    );
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export async function onRebuildLibrarySources() {
  if (!state.selectedLibraryId) {
    return;
  }

  try {
    await triggerJobBackedAction<SourceActionData>(
      `/libraries/${state.selectedLibraryId}/rebuild`,
      "正在执行库级重建..."
    );
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export async function onRefreshSourceRoot(event) {
  if (!state.selectedLibraryId) {
    return;
  }

  const sourceRootId = event.currentTarget.dataset.sourceRootRefreshId;
  try {
    keepSearchPreparationDisclosureOpen();
    await triggerJobBackedAction<SourceActionData>(
      `/libraries/${state.selectedLibraryId}/source-roots/${encodeURIComponent(sourceRootId)}/refresh`,
      `正在 refresh ${sourceRootDisplayName(sourceRootId)}...`
    );
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export async function onRescanSourceRoot(event) {
  if (!state.selectedLibraryId) {
    return;
  }

  const sourceRootId = event.currentTarget.dataset.sourceRootRescanId;
  try {
    keepSearchPreparationDisclosureOpen();
    await triggerJobBackedAction<SourceActionData>(
      `/libraries/${state.selectedLibraryId}/source-roots/${encodeURIComponent(sourceRootId)}/rescan`,
      `正在 rescan ${sourceRootDisplayName(sourceRootId)}...`
    );
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export async function onToggleSourceRoot(event) {
  if (!state.selectedLibraryId) {
    return;
  }

  const sourceRootId = event.currentTarget.dataset.sourceRootToggleId;
  const sourceRoot = state.sourceRoots.find((item) => item.source_root_id === sourceRootId);
  if (!sourceRoot) {
    return;
  }

  try {
    keepSearchPreparationDisclosureOpen();
    state.globalError = null;
    state.statusMessage = sourceRoot.enabled ? "正在停用来源根..." : "正在启用来源根...";
    renderWorkspace();
    await apiRequest(
      `/libraries/${state.selectedLibraryId}/source-roots/${encodeURIComponent(sourceRootId)}`,
      {
        method: "PATCH",
        body: JSON.stringify({ enabled: !sourceRoot.enabled }),
      }
    );
    state.statusMessage = null;
    await refreshWorkspace({ keepSelection: true });
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export async function onDeleteSourceRoot(event) {
  if (!state.selectedLibraryId) {
    return;
  }

  const sourceRootId = event.currentTarget.dataset.sourceRootDeleteId;
  try {
    keepSearchPreparationDisclosureOpen();
    state.globalError = null;
    state.statusMessage = `正在删除 ${sourceRootDisplayName(sourceRootId)}...`;
    renderWorkspace();
    await apiRequest(
      `/libraries/${state.selectedLibraryId}/source-roots/${encodeURIComponent(sourceRootId)}`,
      { method: "DELETE" }
    );
    if (state.editingSourceRootId === sourceRootId) {
      resetSourceRootEditor();
    }
    state.statusMessage = null;
    await refreshWorkspace({ keepSelection: true });
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export async function onImportPaths(event) {
  event.preventDefault();
  if (!state.selectedLibraryId) {
    return;
  }

  keepSearchPreparationDisclosureOpen();
  const textarea = document.querySelector<HTMLTextAreaElement>("#import-paths");
  state.importPathsDraft = textarea?.value ?? "";
  const paths = parseImportPaths(state.importPathsDraft);
  if (!paths.length) {
    state.globalError = {
      code: "validation_failed",
      message: "请至少输入一个本地路径。",
    };
    renderWorkspace();
    return;
  }

  try {
    state.globalError = null;
    state.statusMessage = "正在导入并建立索引...";
    renderWorkspace();
    await importPaths(paths);
    state.importPathsDraft = "";
    state.statusMessage = null;
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export function onSelectInventorySource(event) {
  const nextSourceId = event.currentTarget.dataset.sourceId;
  if (!nextSourceId) {
    return;
  }
  if (nextSourceId === state.selectedInventorySourceId) {
    if (!state.inventoryDetailSheetOpen) {
      state.inventoryDetailSheetOpen = true;
      renderWorkspace();
    }
    return;
  }
  state.selectedInventorySourceId = nextSourceId;
  state.inventoryDetailSheetOpen = true;
  renderWorkspace();
}
