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

export function onProviderConfigSelect(event) {
  const providerId = event.target.value;
  const provider = state.providerConfigs.find((item) => item.provider_id === providerId) ?? null;
  hydrateProviderEditor(provider);
  renderWorkspace();
}

export function onProviderEnabledChange(event) {
  state.providerEnabledDraft = event.target.checked;
}

export function onProviderBaseUrlInput(event) {
  state.providerBaseUrlDraft = event.target.value;
}

export async function onSubmitProviderConfig(event) {
  event.preventDefault();
  if (!state.editingProviderId) {
    return;
  }

  try {
    state.globalError = null;
    await apiRequest(`/settings/providers/${encodeURIComponent(state.editingProviderId)}`, {
      method: "PATCH",
      body: JSON.stringify({
        enabled: state.providerEnabledDraft,
        base_url: state.providerBaseUrlDraft.trim() || null,
      }),
    });
    await refreshProviderSettingsData();
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    renderWorkspace();
  }
}

export function onResetProviderConfigForm() {
  hydrateProviderEditor(selectedProviderConfig());
  state.globalError = null;
  renderWorkspace();
}

export async function onEditProviderConfig(event) {
  const providerId = event.currentTarget.dataset.providerEditId;
  if (!providerId) {
    return;
  }
  const provider = state.providerConfigs.find((item) => item.provider_id === providerId) ?? null;
  hydrateProviderEditor(provider);
  state.globalError = null;
  renderWorkspace();
}

export function onSettingsDiagnosticsJobsToggle(event) {
  state.settingsDiagnosticsJobsOpen = event.currentTarget.open;
}

export function updateGlobalContentTypeBinding(
  updater: (binding: ContentTypeBindingPayload) => ContentTypeBindingPayload
) {
  const contentType = selectedGlobalContentTypeKey();
  if (!contentType) {
    return;
  }
  state.globalContentTypes.content_types[contentType] = updater(selectedGlobalContentTypeBinding());
}

export function updateLibraryContentTypeBinding(
  updater: (binding: ContentTypeBindingPayload) => ContentTypeBindingPayload
) {
  const contentType = selectedLibraryContentTypeKey();
  if (!contentType) {
    return;
  }
  state.libraryContentTypes.content_types[contentType] = updater(selectedLibraryContentTypeBinding());
}

export function onGlobalContentTypeChange(event) {
  state.selectedGlobalContentType = event.target.value;
  resetGlobalModelTestState();
  renderWorkspace();
}

export function onSelectContentTypeTab(event: Event) {
  const target = event.currentTarget as HTMLElement | null;
  const scope = target?.dataset.contentTypeScope;
  const contentType = target?.dataset.contentType ?? "";
  if (!contentType) {
    return;
  }
  if (scope === "library") {
    state.selectedLibraryContentType = contentType;
    resetLibraryModelTestState();
  } else {
    state.selectedGlobalContentType = contentType;
    resetGlobalModelTestState();
  }
  renderWorkspace();
}

export function onGlobalContentTypeEnabledChange(event) {
  updateGlobalContentTypeBinding((binding) => ({
    ...binding,
    enabled: event.target.checked,
  }));
  renderWorkspace();
}

export function onGlobalContentTypeProviderChange(event) {
  updateGlobalContentTypeBinding((binding) =>
    normalizeContentTypeBindingForProvider(event.target.value, binding)
  );
  resetGlobalModelTestState();
  renderWorkspace();
}

export function onGlobalContentTypeModelIdInput(event) {
  updateGlobalContentTypeBinding((binding) => {
    const selection = selectedGlobalModelSelection();
    return {
      ...binding,
      model: composeModelReference({
        provider_id: selection.provider_id,
        model_id: event.target.value,
      }),
    };
  });
  resetGlobalModelTestState();
  renderWorkspace();
}

export function onGlobalContentTypeVectorTypeChange(event) {
  updateGlobalContentTypeBinding((binding) => ({
    ...binding,
    vector_type: event.target.value,
  }));
  renderWorkspace();
}

export async function onSubmitGlobalContentTypes(event) {
  event.preventDefault();

  try {
    state.globalError = null;
    await apiRequest("/settings/content-types", {
      method: "PATCH",
      body: JSON.stringify(state.globalContentTypes),
    });
    await refreshProviderSettingsData();
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    renderWorkspace();
  }
}

export function onLibraryContentTypeChange(event) {
  state.selectedLibraryContentType = event.target.value;
  resetLibraryModelTestState();
  renderWorkspace();
}

export function onLibraryOverrideModeChange(event: Event) {
  const mode = (event.currentTarget as HTMLElement | null)?.dataset.libraryOverrideMode;
  if (!mode) {
    return;
  }

  if (mode === "override") {
    if (!selectedLibraryContentTypeHasOverride()) {
      updateLibraryContentTypeBinding((binding) => ({ ...binding }));
      resetLibraryModelTestState();
      renderWorkspace();
    }
    return;
  }

  if (selectedLibraryContentTypeHasOverride()) {
    const contentType = selectedLibraryContentTypeKey();
    if (!contentType) {
      return;
    }
    delete state.libraryContentTypes.content_types[contentType];
    resetLibraryModelTestState();
    renderWorkspace();
  }
}

export function onLibraryContentTypeEnabledChange(event) {
  updateLibraryContentTypeBinding((binding) => ({
    ...binding,
    enabled: event.target.checked,
  }));
  renderWorkspace();
}

export function onLibraryContentTypeProviderChange(event) {
  updateLibraryContentTypeBinding((binding) =>
    normalizeContentTypeBindingForProvider(event.target.value, binding)
  );
  resetLibraryModelTestState();
  renderWorkspace();
}

export function onLibraryContentTypeModelIdInput(event) {
  updateLibraryContentTypeBinding((binding) => {
    const selection = selectedLibraryModelSelection();
    return {
      ...binding,
      model: composeModelReference({
        provider_id: selection.provider_id,
        model_id: event.target.value,
      }),
    };
  });
  resetLibraryModelTestState();
  renderWorkspace();
}

export function onLibraryContentTypeVectorTypeChange(event) {
  updateLibraryContentTypeBinding((binding) => ({
    ...binding,
    vector_type: event.target.value,
  }));
  renderWorkspace();
}

export async function onSubmitLibraryContentTypes(event) {
  event.preventDefault();
  if (!state.selectedLibraryId) {
    return;
  }

  try {
    state.globalError = null;
    await apiRequest(`/libraries/${encodeURIComponent(state.selectedLibraryId)}/content-types`, {
      method: "PATCH",
      body: JSON.stringify(state.libraryContentTypes),
    });
    await refreshLibraryContentSettings();
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    renderWorkspace();
  }
}

export async function onResetLibraryContentTypes() {
  if (!state.selectedLibraryId) {
    return;
  }

  try {
    state.globalError = null;
    const contentType = selectedLibraryContentTypeKey();
    if (contentType) {
      delete state.libraryContentTypes.content_types[contentType];
    }
    resetLibraryModelTestState();
    await apiRequest(`/libraries/${encodeURIComponent(state.selectedLibraryId)}/content-types`, {
      method: "PATCH",
      body: JSON.stringify(state.libraryContentTypes),
    });
    await refreshLibraryContentSettings();
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    renderWorkspace();
  }
}

export function onGlobalModelTestModalityChange(event) {
  state.globalModelTestModalityDraft = event.target.value;
  state.globalModelTestFile = null;
  state.globalModelTestResult = null;
  state.globalModelTestError = null;
  renderWorkspace();
}

export function onGlobalModelTestTextInput(event) {
  state.globalModelTestTextDraft = event.target.value;
  state.globalModelTestResult = null;
  state.globalModelTestError = null;
}

export function onGlobalModelTestFileInput(event) {
  state.globalModelTestFile = event.target.files?.[0] ?? null;
  state.globalModelTestResult = null;
  state.globalModelTestError = null;
  renderWorkspace();
}

export function onGlobalModelTestComparisonModalityChange(event) {
  state.globalModelTestComparisonModalityDraft = event.target.value;
  state.globalModelTestComparisonFile = null;
  state.globalModelTestResult = null;
  state.globalModelTestError = null;
  renderWorkspace();
}

export function onGlobalModelTestComparisonTextInput(event) {
  state.globalModelTestComparisonTextDraft = event.target.value;
  state.globalModelTestResult = null;
  state.globalModelTestError = null;
}

export function onGlobalModelTestComparisonFileInput(event) {
  state.globalModelTestComparisonFile = event.target.files?.[0] ?? null;
  state.globalModelTestResult = null;
  state.globalModelTestError = null;
  renderWorkspace();
}

export function onLibraryModelTestModalityChange(event) {
  state.libraryModelTestModalityDraft = event.target.value;
  state.libraryModelTestFile = null;
  state.libraryModelTestResult = null;
  state.libraryModelTestError = null;
  renderWorkspace();
}

export function onLibraryModelTestTextInput(event) {
  state.libraryModelTestTextDraft = event.target.value;
  state.libraryModelTestResult = null;
  state.libraryModelTestError = null;
}

export function onLibraryModelTestFileInput(event) {
  state.libraryModelTestFile = event.target.files?.[0] ?? null;
  state.libraryModelTestResult = null;
  state.libraryModelTestError = null;
  renderWorkspace();
}

export function onLibraryModelTestComparisonModalityChange(event) {
  state.libraryModelTestComparisonModalityDraft = event.target.value;
  state.libraryModelTestComparisonFile = null;
  state.libraryModelTestResult = null;
  state.libraryModelTestError = null;
  renderWorkspace();
}

export function onLibraryModelTestComparisonTextInput(event) {
  state.libraryModelTestComparisonTextDraft = event.target.value;
  state.libraryModelTestResult = null;
  state.libraryModelTestError = null;
}

export function onLibraryModelTestComparisonFileInput(event) {
  state.libraryModelTestComparisonFile = event.target.files?.[0] ?? null;
  state.libraryModelTestResult = null;
  state.libraryModelTestError = null;
  renderWorkspace();
}

export async function submitSettingsModelTest(scope: "global" | "library") {
  const selection =
    scope === "global" ? selectedGlobalModelSelection() : selectedLibraryModelSelection();
  const modalityDraft =
    scope === "global" ? state.globalModelTestModalityDraft : state.libraryModelTestModalityDraft;
  const inputModality =
    modalityDraft ||
    supportedTestModalitiesForSelection(selection.provider_id, selection.model_id)[0] ||
    "";
  const textDraft =
    scope === "global" ? state.globalModelTestTextDraft : state.libraryModelTestTextDraft;
  const file = scope === "global" ? state.globalModelTestFile : state.libraryModelTestFile;
  const comparisonModalityDraft =
    scope === "global"
      ? state.globalModelTestComparisonModalityDraft
      : state.libraryModelTestComparisonModalityDraft;
  const comparisonTextDraft =
    scope === "global"
      ? state.globalModelTestComparisonTextDraft
      : state.libraryModelTestComparisonTextDraft;
  const comparisonFile =
    scope === "global"
      ? state.globalModelTestComparisonFile
      : state.libraryModelTestComparisonFile;
  const providerDraft = activeProviderDraftForSelection(selection.provider_id);
  const setPending = (value: boolean) => {
    if (scope === "global") {
      state.globalModelTestPending = value;
    } else {
      state.libraryModelTestPending = value;
    }
  };
  const setResult = (result: ModelTestData | null) => {
    if (scope === "global") {
      state.globalModelTestResult = result;
    } else {
      state.libraryModelTestResult = result;
    }
  };
  const setError = (error: ApiErrorPayload | null) => {
    if (scope === "global") {
      state.globalModelTestError = error;
    } else {
      state.libraryModelTestError = error;
    }
  };

  if (!inputModality) {
    setError({
      code: "not_supported",
      message: "当前 provider + model 组合不支持执行设置页模型测试。",
    });
    renderWorkspace();
    return;
  }

  if (!canExecuteSettingsModelTest(selection)) {
    setError({
      code: "not_supported",
      message: "当前 provider + model 组合在这个切片里不可执行设置页模型测试。",
    });
    renderWorkspace();
    return;
  }

  if (inputModality === "text" && !textDraft.trim()) {
    setError({
      code: "validation_failed",
      message: "请先输入测试文本。",
    });
    renderWorkspace();
    return;
  }

  if (inputModality !== "text" && !file) {
    setError({
      code: "validation_failed",
      message: "请先选择一个测试文件。",
    });
    renderWorkspace();
    return;
  }

  if (comparisonModalityDraft === "text" && !comparisonTextDraft.trim()) {
    setError({
      code: "validation_failed",
      message: "请先输入用于比较的第二个测试文本。",
    });
    renderWorkspace();
    return;
  }

  if (comparisonModalityDraft === "image" && !comparisonFile) {
    setError({
      code: "validation_failed",
      message: "请先选择一个用于比较的第二个测试文件。",
    });
    renderWorkspace();
    return;
  }

  try {
    setPending(true);
    setError(null);
    renderWorkspace();

    const formData = new FormData();
    formData.append("provider_id", selection.provider_id);
    formData.append("model_id", selection.model_id);
    formData.append("input_modality", inputModality);
    if (providerDraft.enabled !== undefined) {
      formData.append("provider_enabled", String(providerDraft.enabled));
    }
    if (selection.provider_id !== PROVIDER_ID_LOCAL_SIDECAR && providerDraft.baseUrl) {
      formData.append("provider_base_url", providerDraft.baseUrl);
    }
    if (inputModality === "text") {
      formData.append("text", textDraft.trim());
    } else if (file) {
      formData.append("file", file);
    }
    if (comparisonModalityDraft) {
      formData.append("comparison_input_modality", comparisonModalityDraft);
      if (comparisonModalityDraft === "text") {
        formData.append("comparison_text", comparisonTextDraft.trim());
      } else if (comparisonFile) {
        formData.append("comparison_file", comparisonFile);
      }
    }

    const result = await apiRequest<ModelTestData>("/settings/model-tests", {
      method: "POST",
      body: formData,
    });
    setResult(result);
    setError(null);
  } catch (error) {
    setResult(null);
    setError(toApiError(error));
  } finally {
    setPending(false);
    renderWorkspace();
  }
}

export async function onSubmitGlobalModelTest(event) {
  event.preventDefault();
  await submitSettingsModelTest("global");
}

export async function onSubmitLibraryModelTest(event) {
  event.preventDefault();
  await submitSettingsModelTest("library");
}
