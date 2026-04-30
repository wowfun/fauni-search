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
  hydrateProviderModelEditor,
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
  selectedProviderModelSelection,
  selectedProviderTestModalities,
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

export function onProviderConfigSelect(event) {
  const providerId = event.target.value;
  const provider = state.providerConfigs.find((item) => item.provider_id === providerId) ?? null;
  if (provider) {
    hydrateProviderEditor(provider);
  } else {
    const previousProviderId = state.editingProviderId;
    state.editingProviderId = providerId;
    if (previousProviderId !== providerId) {
      state.providerDisplayNameDraft = providerId;
      state.providerKindDraft = providerId;
    }
    state.providerActiveModelDraft = "";
    hydrateProviderModelEditor(null);
  }
  renderWorkspace();
}

export function onProviderDisplayNameInput(event) {
  state.providerDisplayNameDraft = event.target.value;
}

export function onProviderKindInput(event) {
  state.providerKindDraft = event.target.value;
}

export function onProviderEnabledChange(event) {
  state.providerEnabledDraft = event.target.checked;
}

export function onProviderBaseUrlInput(event) {
  state.providerBaseUrlDraft = event.target.value;
}

export function onProviderActiveModelChange(event) {
  state.providerActiveModelDraft = event.target.value;
  const provider = selectedProviderConfig();
  hydrateProviderModelEditor(
    provider?.models.find((model) => model.model_id === state.providerActiveModelDraft) ?? null
  );
  resetGlobalModelTestState();
  renderWorkspace();
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
        display_name: state.providerDisplayNameDraft.trim() || state.editingProviderId,
        provider_kind: state.providerKindDraft.trim() || state.editingProviderId,
        enabled: state.providerEnabledDraft,
        base_url: state.providerBaseUrlDraft.trim() || null,
        active_model: state.providerActiveModelDraft.trim() || null,
      }),
    });
    await refreshProviderSettingsData();
    hydrateProviderEditor(
      state.providerConfigs.find((provider) => provider.provider_id === state.editingProviderId) ??
        null
    );
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    renderWorkspace();
  }
}

export async function onDeleteProviderConfig() {
  if (!state.editingProviderId) {
    return;
  }
  try {
    state.globalError = null;
    await apiRequest(`/settings/providers/${encodeURIComponent(state.editingProviderId)}`, {
      method: "DELETE",
    });
    await refreshProviderSettingsData();
    hydrateProviderEditor(
      state.providerConfigs.find((provider) => provider.provider_id === state.editingProviderId) ??
        null
    );
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

export function onProviderModelSelect(event) {
  const modelId = event.target.value;
  const provider = selectedProviderConfig();
  if (provider) {
    hydrateProviderModelEditor(
      provider.models.find((model) => model.model_id === modelId) ?? null
    );
  }
  resetGlobalModelTestState();
  renderWorkspace();
}

export function onProviderModelIdInput(event) {
  state.providerModelIdDraft = event.target.value;
}

export function onProviderModelEnabledChange(event) {
  state.providerModelEnabledDraft = event.target.checked;
}

export function onProviderModelVersionInput(event) {
  state.providerModelVersionDraft = event.target.value;
}

export function onProviderModelBackendInput(event) {
  state.providerModelBackendDraft = event.target.value;
}

export function onProviderModelInputTypesInput(event) {
  state.providerModelInputTypesDraft = event.target.value;
}

export function onProviderModelVectorTypesInput(event) {
  state.providerModelVectorTypesDraft = event.target.value;
}

export function onProviderModelSupportsMixedInputsChange(event) {
  state.providerModelSupportsMixedInputsDraft = event.target.checked;
}

function commaList(value: string) {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

export async function onSubmitProviderModelConfig(event) {
  event.preventDefault();
  const providerId = state.editingProviderId.trim();
  const modelId = state.providerModelIdDraft.trim();
  if (!providerId || !modelId) {
    return;
  }
  try {
    state.globalError = null;
    await apiRequest(
      `/settings/providers/${encodeURIComponent(providerId)}/models/${encodeURIComponent(modelId)}`,
      {
        method: "PATCH",
        body: JSON.stringify({
          enabled: state.providerModelEnabledDraft,
          version: state.providerModelVersionDraft.trim() || "main",
          backend: state.providerModelBackendDraft.trim() || null,
          embedding_capabilities: {
            input_types: commaList(state.providerModelInputTypesDraft),
            vector_types: commaList(state.providerModelVectorTypesDraft),
            supports_mixed_inputs: state.providerModelSupportsMixedInputsDraft,
          },
        }),
      }
    );
    await refreshProviderSettingsData();
    const provider = state.providerConfigs.find((item) => item.provider_id === providerId) ?? null;
    hydrateProviderEditor(provider);
    hydrateProviderModelEditor(provider?.models.find((model) => model.model_id === modelId) ?? null);
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    renderWorkspace();
  }
}

export async function onDeleteProviderModelConfig() {
  const providerId = state.editingProviderId.trim();
  const modelId = state.providerModelIdDraft.trim();
  if (!providerId || !modelId) {
    return;
  }
  try {
    state.globalError = null;
    await apiRequest(
      `/settings/providers/${encodeURIComponent(providerId)}/models/${encodeURIComponent(modelId)}`,
      { method: "DELETE" }
    );
    await refreshProviderSettingsData();
    hydrateProviderEditor(state.providerConfigs.find((item) => item.provider_id === providerId) ?? null);
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    renderWorkspace();
  }
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
    const contentType = selectedGlobalContentTypeKey();
    await apiRequest(`/settings/content-types/${encodeURIComponent(contentType)}`, {
      method: "PATCH",
      body: JSON.stringify(selectedGlobalContentTypeBinding()),
    });
    await refreshProviderSettingsData();
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    renderWorkspace();
  }
}

export async function onResetGlobalContentType() {
  const contentType = selectedGlobalContentTypeKey();
  if (!contentType) {
    return;
  }
  try {
    state.globalError = null;
    await apiRequest(`/settings/content-types/${encodeURIComponent(contentType)}`, {
      method: "DELETE",
    });
    resetGlobalModelTestState();
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
    const contentType = selectedLibraryContentTypeKey();
    await apiRequest(
      `/libraries/${encodeURIComponent(state.selectedLibraryId)}/content-types/${encodeURIComponent(contentType)}`,
      {
        method: "PATCH",
        body: JSON.stringify(selectedLibraryContentTypeBinding()),
      }
    );
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
    resetLibraryModelTestState();
    await apiRequest(
      `/libraries/${encodeURIComponent(state.selectedLibraryId)}/content-types/${encodeURIComponent(contentType)}`,
      {
        method: "DELETE",
      }
    );
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

export async function submitSettingsModelTest(scope: "global" | "library" | "provider") {
  const selection =
    scope === "provider"
      ? selectedProviderModelSelection()
      : scope === "global"
        ? selectedGlobalModelSelection()
        : selectedLibraryModelSelection();
  const modalityDraft =
    scope === "library" ? state.libraryModelTestModalityDraft : state.globalModelTestModalityDraft;
  const inputModality =
    modalityDraft ||
    (scope === "provider"
      ? selectedProviderTestModalities()
      : supportedTestModalitiesForSelection(selection.provider_id, selection.model_id))[0] ||
    "";
  const textDraft =
    scope === "library" ? state.libraryModelTestTextDraft : state.globalModelTestTextDraft;
  const file = scope === "library" ? state.libraryModelTestFile : state.globalModelTestFile;
  const comparisonModalityDraft =
    scope === "library"
      ? state.libraryModelTestComparisonModalityDraft
      : state.globalModelTestComparisonModalityDraft;
  const comparisonTextDraft =
    scope === "library"
      ? state.libraryModelTestComparisonTextDraft
      : state.globalModelTestComparisonTextDraft;
  const comparisonFile =
    scope === "library"
      ? state.libraryModelTestComparisonFile
      : state.globalModelTestComparisonFile;
  const providerDraft = activeProviderDraftForSelection(selection.provider_id);
  const setPending = (value: boolean) => {
    if (scope === "library") {
      state.libraryModelTestPending = value;
    } else {
      state.globalModelTestPending = value;
    }
  };
  const setResult = (result: ModelTestData | null) => {
    if (scope === "library") {
      state.libraryModelTestResult = result;
    } else {
      state.globalModelTestResult = result;
    }
  };
  const setError = (error: ApiErrorPayload | null) => {
    if (scope === "library") {
      state.libraryModelTestError = error;
    } else {
      state.globalModelTestError = error;
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

export async function onSubmitProviderModelTest(event) {
  event.preventDefault();
  await submitSettingsModelTest("provider");
}
