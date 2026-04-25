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

export let workspacePollInFlight = false;

export function bootstrapApp() {
  window.setInterval(() => {
    if (workspacePollInFlight || !state.selectedLibraryId || hasFocusedEditableControl()) {
      return;
    }

    workspacePollInFlight = true;
    refreshWorkspace({ keepSelection: true })
      .catch((error) => {
        state.globalError = toApiError(error);
        renderWorkspace();
      })
      .finally(() => {
        workspacePollInFlight = false;
      });
  }, WORKSPACE_POLL_INTERVAL_MS);

  refreshWorkspace({ keepSelection: false }).catch((error) => {
    state.globalError = toApiError(error);
    renderWorkspace();
  });
}
