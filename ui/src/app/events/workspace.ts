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
import { onCleanupRetiredVectorSpaces } from "./jobs";
import { onRefreshLibrarySources, onRebuildLibrarySources, onRescanLibrarySources } from "./inventory";

async function openSettingsDiagnostics(options: { openJobs?: boolean } = {}) {
  state.selectedSettingsSection = "diagnostics";
  state.settingsDiagnosticsJobsOpen = Boolean(options.openJobs);
  state.activeWorkspace = "settings";
  state.searchDetailSheetOpen = false;
  state.inventoryDetailSheetOpen = false;
  state.globalError = null;
  state.statusMessage = null;

  try {
    await refreshProviderSettingsData();
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    renderWorkspace();
  }
}

export async function onSelectWorkspace(event) {
  const nextWorkspace = event.currentTarget.dataset.workspace as WorkspaceKind | undefined;
  if (!nextWorkspace || nextWorkspace === state.activeWorkspace) {
    return;
  }

  state.activeWorkspace = nextWorkspace;
  if (nextWorkspace !== "search") {
    state.searchDetailSheetOpen = false;
  }
  if (nextWorkspace !== "inventory") {
    state.inventoryDetailSheetOpen = false;
  }
  state.globalError = null;
  state.statusMessage = null;

  try {
    if (nextWorkspace === "inventory") {
      await refreshLibrarySources();
    } else if (nextWorkspace === "settings") {
      await refreshProviderSettingsData();
    }
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    renderWorkspace();
  }
}

export async function onOpenHitLibraryContext(event) {
  const libraryId = event.currentTarget.dataset.openHitLibraryId?.trim();
  if (!libraryId) {
    return;
  }

  state.globalError = null;
  state.statusMessage = null;
  state.activeWorkspace = "inventory";
  state.searchDetailSheetOpen = false;

  try {
    if (libraryId === state.selectedLibraryId) {
      await refreshLibrarySources();
      renderWorkspace();
      return;
    }
    await switchCurrentLibrary(libraryId);
  } catch (error) {
    state.globalError = toApiError(error);
    renderWorkspace();
  }
}

export function onSelectSettingsSection(event) {
  const nextSection = event.currentTarget.dataset.settingsSection as SettingsSection | undefined;
  if (!nextSection || nextSection === state.selectedSettingsSection) {
    return;
  }
  state.selectedSettingsSection = nextSection;
  state.settingsDiagnosticsJobsOpen = false;
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

export async function onOpenSettingsSection(event) {
  const nextSection = event.currentTarget.dataset.openSettingsSection as SettingsSection | undefined;
  if (!nextSection) {
    return;
  }

  if (nextSection === "diagnostics") {
    await openSettingsDiagnostics();
    return;
  }

  state.selectedSettingsSection = nextSection;
  state.settingsDiagnosticsJobsOpen = false;
  state.activeWorkspace = "settings";
  state.searchDetailSheetOpen = false;
  state.inventoryDetailSheetOpen = false;
  state.globalError = null;
  state.statusMessage = null;

  try {
    await refreshProviderSettingsData();
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    renderWorkspace();
  }
}

export function onCloseMobileSheet(event) {
  const sheet = event.currentTarget.dataset.mobileSheetClose;
  if (sheet === "inventory") {
    state.inventoryDetailSheetOpen = false;
  } else {
    state.searchDetailSheetOpen = false;
  }
  renderWorkspace();
}

export async function onUtilitiesAction(event) {
  const action = event.currentTarget.dataset.utilitiesAction;
  if (!action) {
    return;
  }

  state.globalError = null;
  state.statusMessage = null;

  if (action === "focus-source-prep") {
    state.activeWorkspace = "search";
    state.searchPreparationDisclosureOpen = true;
    renderWorkspace();
    return;
  }

  if (action === "focus-search-jobs") {
    await openSettingsDiagnostics({ openJobs: true });
    return;
  }

  if (action === "refresh-library") {
    await onRefreshLibrarySources();
    return;
  }

  if (action === "rescan-library") {
    await onRescanLibrarySources();
    return;
  }

  if (action === "rebuild-library") {
    await onRebuildLibrarySources();
    return;
  }

  if (action === "cleanup-retired-vector-spaces") {
    await onCleanupRetiredVectorSpaces();
  }
}

export async function loadVisualUnit(libraryId: string, visualUnitId: string): Promise<void> {
  if (!libraryId) {
    return;
  }

  try {
    state.globalError = null;
    state.selectedVisualUnit = await apiRequest<VisualUnitDetailData>(
      `/libraries/${libraryId}/visual-units/${encodeURIComponent(visualUnitId)}`
    );
    state.selectedVisualUnitLibraryId = libraryId;
    state.searchDetailSheetOpen = true;
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    renderWorkspace();
  }
}

export async function onSelectVisualUnit(event) {
  const visualUnitId = event.currentTarget.dataset.visualUnitId;
  const libraryId =
    event.currentTarget.dataset.visualUnitLibraryId || state.selectedLibraryId || "";
  if (
    visualUnitId &&
    `${libraryId}:${visualUnitId}` === selectedVisualUnitId() &&
    !state.searchDetailSheetOpen
  ) {
    state.searchDetailSheetOpen = true;
    renderWorkspace();
    return;
  }
  await loadVisualUnit(libraryId, visualUnitId);
}
