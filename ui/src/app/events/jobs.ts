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
import { loadAsset } from "./workspace";

export async function triggerJobBackedAction<T extends { job?: JobSnapshot | null }>(
  path,
  statusMessage,
  options: RequestInit = { method: "POST" }
): Promise<T> {
  state.globalError = null;
  state.statusMessage = statusMessage;
  renderWorkspace();

  const receipt = await apiRequest<T>(path, options);
  await refreshWorkspace({ keepSelection: true });

  const job = receipt.job;
  if (job && !isTerminalJobStatus(job.status)) {
    await waitForJobTerminal(job.job_id);
  }

  state.statusMessage = null;
  await refreshWorkspace({ keepSelection: true });
  return receipt;
}

export async function onCleanupRetiredVectorSpaces() {
  if (!state.selectedLibraryId) {
    return;
  }

  try {
    await triggerJobBackedAction<MaintenanceActionData>(
      `/libraries/${state.selectedLibraryId}/maintenance`,
      "正在清理检索命名空间...",
      {
        method: "POST",
        body: JSON.stringify({ action: "cleanup_retired_vector_spaces" }),
      }
    );
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export async function onCancelJob(event) {
  const jobId = event.currentTarget.dataset.jobCancelId;
  if (!jobId) {
    return;
  }

  try {
    state.globalError = null;
    state.statusMessage = `正在取消任务 ${jobId}...`;
    renderWorkspace();
    const snapshot = await apiRequest<JobSnapshot>(`/jobs/${encodeURIComponent(jobId)}/cancel`, {
      method: "POST",
    });
    await refreshWorkspace({ keepSelection: true });

    if (!isTerminalJobStatus(snapshot.status)) {
      await waitForJobTerminal(snapshot.job_id);
    }

    state.statusMessage = null;
    await refreshWorkspace({ keepSelection: true });
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export async function onRetryJob(event) {
  const jobId = event.currentTarget.dataset.jobRetryId;
  if (!jobId) {
    return;
  }

  try {
    state.globalError = null;
    state.statusMessage = `正在重试任务 ${jobId}...`;
    renderWorkspace();
    const snapshot = await apiRequest<JobSnapshot>(`/jobs/${encodeURIComponent(jobId)}/retry`, {
      method: "POST",
    });
    await refreshWorkspace({ keepSelection: true });

    if (!isTerminalJobStatus(snapshot.status)) {
      await waitForJobTerminal(snapshot.job_id);
    }

    state.statusMessage = null;
    await refreshWorkspace({ keepSelection: true });
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export async function onResumeJob(event) {
  const jobId = event.currentTarget.dataset.jobResumeId;
  if (!jobId) {
    return;
  }

  try {
    state.globalError = null;
    state.statusMessage = `正在继续任务 ${jobId}...`;
    renderWorkspace();
    const snapshot = await apiRequest<JobSnapshot>(`/jobs/${encodeURIComponent(jobId)}/resume`, {
      method: "POST",
    });
    await refreshWorkspace({ keepSelection: true });

    if (!isTerminalJobStatus(snapshot.status)) {
      await waitForJobTerminal(snapshot.job_id);
    }

    state.statusMessage = null;
    await refreshWorkspace({ keepSelection: true });
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

export async function importPaths(paths: string[]): Promise<ImportPathsData> {
  state.importReceipt = await apiRequest<ImportPathsData>(
    `/libraries/${state.selectedLibraryId}/imports`,
    {
      method: "POST",
      body: JSON.stringify({ paths }),
    }
  );
  state.searchOutcome = null;
  state.searchInFlight = false;
  await refreshWorkspace({ keepSelection: true });

  const job = state.importReceipt.job;
  if (job && !isTerminalJobStatus(job.status)) {
    const terminalJob = await waitForJobTerminal(job.job_id);
    state.importReceipt.job = terminalJob;
    if (terminalJob.status === "failed" || terminalJob.status === "canceled") {
      state.globalError = {
        code: terminalJob.status,
        message: terminalJob.current_attempt.summary,
      };
      renderWorkspace();
      return state.importReceipt;
    }
  }

  const firstAsset = state.importReceipt.accepted
    .flatMap((item) => item.assets ?? [])
    .at(0);
  if (firstAsset && state.selectedLibraryId) {
    await loadAsset(state.selectedLibraryId, firstAsset.asset_id);
  }
  return state.importReceipt;
}

export async function waitForJobTerminal(jobId: string): Promise<JobSnapshot> {
  const startedAt = Date.now();

  while (Date.now() - startedAt < JOB_POLL_TIMEOUT_MS) {
    const job = await refreshJob(jobId);
    await refreshWorkspace({ keepSelection: true });

    if (isTerminalJobStatus(job.status)) {
      state.statusMessage = null;
      renderWorkspace();
      return job;
    }

    state.statusMessage = `后台任务 ${job.job_id} 正在 ${job.phase}...`;
    renderWorkspace();
    await sleep(JOB_POLL_INTERVAL_MS);
  }

  throw {
    code: "job_timeout",
    message: `后台任务 ${jobId} 在预期时间内没有进入终态。`,
  };
}
