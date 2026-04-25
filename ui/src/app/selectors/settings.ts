import type {
  ApiErrorPayload,
  AppState,
  BindingSource,
  ContentTypeBindingPayload,
  ContentTypesPayload,
  EmbeddingCapabilities,
  GlobalContentTypesData,
  ImportPathsData,
  InventorySummary,
  JobSnapshot,
  JobsListData,
  LibrariesListData,
  LibraryContentTypesData,
  LibraryObjectQueryDocument,
  LibraryObjectQueryImage,
  LibraryObjectQueryVideo,
  LibrarySnapshot,
  MaintenanceActionData,
  ModelCatalogData,
  ModelCatalogEntry,
  ModelTestData,
  ModelTestModality,
  ModelSelectionPayload,
  PreviewReference,
  ProviderConfigSnapshot,
  ProvidersListData,
  QueryAssetData,
  ResolvedContentModelSelectionPayload,
  ResolvedContentModelsData,
  ResolvedModelSelectionPayload,
  RuntimeHealthData,
  SearchMode,
  SearchOutcomeState,
  SearchRequestSnapshot,
  SearchScopeKind,
  SourceActionData,
  SourceInventoryItem,
  SourceRootSnapshot,
  SourceRootsListData,
  SettingsSection,
  SourcesListData,
  VectorSpaceDiagnosticsData,
  VideoSourceItem,
  VideoSourcesData,
  VisualUnitDetailData,
  WorkspaceKind,
} from "../../types";
import {
  CONTENT_TYPE_ORDER,
  MODEL_TEST_MODALITIES,
  PROVIDER_ID_LOCAL_SIDECAR,
  state,
} from "../state/store";
import {
  contentTypeDisplayName,
  formatBindingSource,
  formatExecutionInputTypes,
  formatResolvedContentModel,
  formatResolvedContentModelContext,
  formatResolvedModel,
  formatResolvedModelContext,
  libraryDisplayName,
  modelTestModalityDisplayName,
} from "./common";
import { retiredVectorSpaceDiagnostics, runtimeHealthOverview } from "./runtime";

export function settingsSectionLabel(section: SettingsSection) {
  switch (section) {
    case "library-overrides":
      return "当前库覆盖";
    case "providers":
      return "连接";
    case "model-tests":
      return "模型测试";
    case "diagnostics":
      return "诊断";
    default:
      return "内容类型";
  }
}

export function settingsSectionIcon(section: SettingsSection) {
  switch (section) {
    case "library-overrides":
      return "override";
    case "providers":
      return "providers";
    case "model-tests":
      return "experiment";
    case "diagnostics":
      return "diagnostics";
    default:
      return "content-types";
  }
}

export function settingsSectionDescription(section: SettingsSection, library: LibrarySnapshot | null) {
  switch (section) {
    case "library-overrides":
      return library
        ? `先判断 ${libraryDisplayName(library)} 是沿用默认，还是需要切到库级覆盖。`
        : "先选择一个库，再判断这一章是沿用默认还是切到库级覆盖。";
    case "providers":
      return "把连接状态、当前精确模型和最小可编辑字段收口到同一章里。";
    case "model-tests":
      return "模型测试只面向当前草稿，用来验证输入模态、向量形状和相似度。";
    case "diagnostics":
      return "先看运行时与连接摘要，再下钻到维护动作和执行空间诊断。";
    default:
      return "先配置全局默认的内容类型绑定，再让搜索和库级覆盖复用它。";
  }
}

export function settingsSectionNavSummary(section: SettingsSection, library: LibrarySnapshot | null) {
  switch (section) {
    case "library-overrides":
      return library ? "判断当前库是否需要脱离默认。" : "先选库，再进入库级差异。";
    case "providers":
      return "查看连接状态并编辑当前地址。";
    case "model-tests":
      return "基于当前草稿验证输入和结果。";
    case "diagnostics":
      return "汇总运行时、维护与执行空间。";
    default:
      return "先配置全局默认内容类型绑定。";
  }
}

export function settingsSectionPill(section: SettingsSection, library: LibrarySnapshot | null) {
  if (section === "library-overrides") {
    if (!library) {
      return {
        label: "等待库",
        pillClass: "pending",
      };
    }
    return selectedLibraryContentTypeHasOverride()
      ? { label: "存在覆盖", pillClass: "ready" }
      : { label: "沿用默认", pillClass: "muted" };
  }

  if (section === "providers") {
    const runtimeOverview = runtimeHealthOverview();
    if (!runtimeOverview) {
      return {
        label: "待刷新",
        pillClass: "pending",
      };
    }
    return runtimeOverview.processIssues.length || runtimeOverview.providerIssues.length
      ? { label: "部分受限", pillClass: "error" }
      : { label: "连接正常", pillClass: "ready" };
  }

  if (section === "diagnostics") {
    const runtimeOverview = runtimeHealthOverview();
    if (!runtimeOverview) {
      return {
        label: "待刷新",
        pillClass: "pending",
      };
    }
    return runtimeOverview.processIssues.length || runtimeOverview.providerIssues.length
      ? { label: "需要关注", pillClass: "error" }
      : { label: "运行正常", pillClass: "ready" };
  }

  if (section === "model-tests") {
    return {
      label: "基于草稿",
      pillClass: "muted",
    };
  }

  return {
    label: "全局默认",
    pillClass: "ready",
  };
}

export function settingsMetricsForSection(section: SettingsSection, library: LibrarySnapshot | null) {
  if (section === "library-overrides") {
    const contentType = selectedLibraryContentTypeKey();
    const binding = selectedLibraryContentTypeBinding();
    const resolved = state.resolvedContentModels?.content_types?.[contentType];
    return [
      {
        label: "当前库",
        value: library ? libraryDisplayName(library) : "未选择",
      },
      {
        label: "当前类型",
        value: contentType ? contentTypeDisplayName(contentType) : "未选择",
      },
      {
        label: "覆盖状态",
        value: library ? (selectedLibraryContentTypeHasOverride() ? "覆盖当前库" : "继承全局默认") : "等待库",
      },
      {
        label: "当前生效",
        value: resolved ? `${resolved.model_id}@${resolved.model_version}` : binding.model || "未配置",
      },
    ];
  }

  if (section === "providers") {
    const enabledProviders = state.providerConfigs.filter((provider) => provider.enabled);
    const localRuntime = state.runtimeHealth?.providers.find(
      (provider) => provider.provider_id === PROVIDER_ID_LOCAL_SIDECAR
    );
    const editableRemoteProviders = state.providerConfigs.filter(
      (provider) => provider.provider_id !== PROVIDER_ID_LOCAL_SIDECAR
    );
    return [
      {
        label: "已启用连接",
        value: `${enabledProviders.length} / ${state.providerConfigs.length || 0}`,
      },
      {
        label: "本地默认",
        value: localRuntime?.model_id ?? "待解析",
      },
      {
        label: "远端可编辑",
        value: `${editableRemoteProviders.length} 个`,
      },
      {
        label: "当前编辑",
        value: selectedProviderConfig()?.display_name ?? "先选择连接",
      },
    ];
  }

  if (section === "model-tests") {
    const globalSelection = selectedGlobalModelSelection();
    const librarySelection = selectedLibraryModelSelection();
    return [
      {
        label: "全局草稿",
        value: globalSelection.model_id || "未解析",
      },
      {
        label: "当前库草稿",
        value: library ? librarySelection.model_id || "沿用默认" : "等待库",
      },
      {
        label: "原生输入",
        value: selectedGlobalTestModalities().map((modality) => modelTestModalityDisplayName(modality)).join("、") || "未解析",
      },
    ];
  }

  if (section === "diagnostics") {
    const runtimeOverview = runtimeHealthOverview();
    return [
      {
        label: "App / Qdrant",
        value: runtimeOverview
          ? `${runtimeOverview.processIssues.length ? "有受限项" : "正常"}`
          : "待刷新",
      },
      {
        label: "已启用连接",
        value: runtimeOverview ? `${runtimeOverview.enabledProviders.length} 个` : "待刷新",
      },
      {
        label: "受限连接",
        value: runtimeOverview ? `${runtimeOverview.providerIssues.length} 个` : "待刷新",
      },
      {
        label: "退役执行空间",
        value: library ? `${retiredVectorSpaceDiagnostics().length} 个` : "等待库",
      },
    ];
  }

  const contentType = selectedGlobalContentTypeKey();
  const binding = selectedGlobalContentTypeBinding();
  const totalTypes = availableContentTypeKeys(state.globalContentTypes);
  const enabledTypes = totalTypes.filter((key) => state.globalContentTypes.content_types[key]?.enabled).length;
  return [
    {
      label: "已启用",
      value: `${enabledTypes} / ${totalTypes.length || 0}`,
    },
    {
      label: "当前类型",
      value: contentType ? contentTypeDisplayName(contentType) : "未选择",
    },
    {
      label: "当前绑定",
      value: binding.model || "未配置",
    },
    {
      label: "向量类型",
      value: binding.vector_type || "未设置",
    },
  ];
}

export function selectedProviderConfig(): ProviderConfigSnapshot | null {
  return (
    state.providerConfigs.find((provider) => provider.provider_id === state.editingProviderId) ??
    null
  );
}

export function providerConfigLabel(providerId?: string | null) {
  if (!providerId) {
    return "inherit";
  }
  const provider = state.providerConfigs.find((item) => item.provider_id === providerId);
  if (!provider) {
    return `${providerId} (缺失)`;
  }
  return `${provider.display_name} (${provider.provider_kind})`;
}

export function contentTypeOrderValue(contentType: string) {
  const index = CONTENT_TYPE_ORDER.indexOf(contentType as (typeof CONTENT_TYPE_ORDER)[number]);
  return index >= 0 ? index : CONTENT_TYPE_ORDER.length;
}

export function sortContentTypes(values: Iterable<string>) {
  return [...values].sort((left, right) => {
    return contentTypeOrderValue(left) - contentTypeOrderValue(right) || left.localeCompare(right);
  });
}

export function sortedContentTypeKeys(payload: ContentTypesPayload) {
  return sortContentTypes(Object.keys(payload.content_types));
}

export function availableContentTypeKeys(
  ...payloads: Array<{ content_types?: Record<string, unknown> } | null | undefined>
) {
  const keys = new Set<string>(CONTENT_TYPE_ORDER);
  for (const payload of payloads) {
    for (const key of Object.keys(payload?.content_types ?? {})) {
      keys.add(key);
    }
  }
  return sortContentTypes(keys);
}

export function catalogEntriesForProvider(providerId: string | null | undefined): ModelCatalogEntry[] {
  if (!providerId) {
    return [];
  }
  return state.modelCatalog.filter((entry) => entry.provider_id === providerId);
}

export function selectedCatalogEntryForProvider(
  providerId: string | null | undefined,
  modelId?: string | null
): ModelCatalogEntry | null {
  const entries = catalogEntriesForProvider(providerId);
  if (!entries.length) {
    return null;
  }
  if (modelId) {
    return entries.find((entry) => entry.model_id === modelId) ?? null;
  }
  return entries[0] ?? null;
}

export function selectedCatalogEntryForSelection(
  selection: Pick<ModelSelectionPayload, "provider_id" | "model_id">
) {
  return selectedCatalogEntryForProvider(selection.provider_id, selection.model_id);
}

export function splitModelReference(modelReference: string): ModelSelectionPayload {
  const slashIndex = modelReference.indexOf("/");
  if (slashIndex <= 0) {
    return {
      provider_id: "",
      model_id: modelReference,
    };
  }
  return {
    provider_id: modelReference.slice(0, slashIndex),
    model_id: modelReference.slice(slashIndex + 1),
  };
}

export function composeModelReference(selection: Pick<ModelSelectionPayload, "provider_id" | "model_id">) {
  if (!selection.provider_id || !selection.model_id) {
    return "";
  }
  return `${selection.provider_id}/${selection.model_id}`;
}

export function defaultContentTypeBinding(): ContentTypeBindingPayload {
  return {
    enabled: false,
    model: "",
    vector_type: "",
  };
}

export function selectedGlobalContentTypeKey() {
  const selected = state.selectedGlobalContentType;
  const available = availableContentTypeKeys(state.globalContentTypes);
  if (selected && available.includes(selected)) {
    return selected;
  }
  return available[0] ?? "";
}

export function selectedLibraryContentTypeKey() {
  const selected = state.selectedLibraryContentType;
  const available = availableContentTypeKeys(
    state.globalContentTypes,
    state.libraryContentTypes,
    state.resolvedContentModels ? { content_types: state.resolvedContentModels.content_types } : null
  );
  if (selected && available.includes(selected)) {
    return selected;
  }
  return available[0] ?? "";
}

export function selectedGlobalContentTypeBinding(): ContentTypeBindingPayload {
  return (
    state.globalContentTypes.content_types[selectedGlobalContentTypeKey()] ??
    defaultContentTypeBinding()
  );
}

export function selectedLibraryContentTypeBinding(): ContentTypeBindingPayload {
  const contentType = selectedLibraryContentTypeKey();
  return (
    state.libraryContentTypes.content_types[contentType] ??
    state.globalContentTypes.content_types[contentType] ??
    defaultContentTypeBinding()
  );
}

export function libraryContentTypeHasOverride(contentType: string) {
  return Object.prototype.hasOwnProperty.call(state.libraryContentTypes.content_types, contentType);
}

export function selectedLibraryContentTypeHasOverride() {
  const contentType = selectedLibraryContentTypeKey();
  return contentType ? libraryContentTypeHasOverride(contentType) : false;
}

export function selectionFromBinding(binding: ContentTypeBindingPayload): ModelSelectionPayload {
  const selection = splitModelReference(binding.model);
  return {
    provider_id: selection.provider_id || PROVIDER_ID_LOCAL_SIDECAR,
    model_id: selection.model_id || "",
  };
}

export function selectedGlobalModelSelection(): ModelSelectionPayload {
  return selectionFromBinding(selectedGlobalContentTypeBinding());
}

export function selectedLibraryModelSelection(): ModelSelectionPayload {
  return selectionFromBinding(selectedLibraryContentTypeBinding());
}

export function vectorTypeOptionsForSelection(selection: ModelSelectionPayload, currentValue: string) {
  const options = [
    ...(selectedCatalogEntryForSelection(selection)?.embedding_capabilities.vector_types ?? []),
  ];
  if (currentValue && !options.includes(currentValue)) {
    options.push(currentValue);
  }
  return options;
}

export function normalizeContentTypeBindingForProvider(
  providerId: string,
  currentBinding: ContentTypeBindingPayload
): ContentTypeBindingPayload {
  const currentSelection = splitModelReference(currentBinding.model);
  const catalogEntry = selectedCatalogEntryForProvider(providerId, currentSelection.model_id || null);
  const modelId = currentSelection.model_id || catalogEntry?.model_id || "";
  const vectorTypes = catalogEntry?.embedding_capabilities.vector_types ?? [];
  const vectorType = vectorTypes.includes(currentBinding.vector_type)
    ? currentBinding.vector_type
    : vectorTypes[0] ?? currentBinding.vector_type;

  return {
    ...currentBinding,
    model: composeModelReference({ provider_id: providerId, model_id: modelId }),
    vector_type: vectorType,
  };
}

export function supportedTestModalitiesForSelection(
  providerId: string | null | undefined,
  modelId?: string | null
): ModelTestModality[] {
  const entry = selectedCatalogEntryForProvider(providerId, modelId);
  return MODEL_TEST_MODALITIES.filter((modality) =>
    entry?.embedding_capabilities?.input_types?.includes(modality)
  );
}

export function activeProviderDraftForSelection(providerId: string): {
  enabled?: boolean;
  baseUrl?: string | null;
} {
  const allowEditableBaseUrl = providerId !== PROVIDER_ID_LOCAL_SIDECAR;
  if (state.editingProviderId === providerId) {
    return {
      enabled: state.providerEnabledDraft,
      baseUrl: allowEditableBaseUrl ? state.providerBaseUrlDraft.trim() || null : null,
    };
  }
  const provider = state.providerConfigs.find((item) => item.provider_id === providerId);
  return {
    enabled: provider?.enabled,
    baseUrl: allowEditableBaseUrl ? provider?.base_url ?? null : null,
  };
}

export function selectedGlobalTestModalities(): ModelTestModality[] {
  const selection = selectionFromBinding(selectedGlobalContentTypeBinding());
  return supportedTestModalitiesForSelection(selection.provider_id, selection.model_id);
}

export function selectedLibraryTestModalities(): ModelTestModality[] {
  const selection = selectionFromBinding(selectedLibraryContentTypeBinding());
  return supportedTestModalitiesForSelection(selection.provider_id, selection.model_id);
}

export function modelTestFileAccept(modality: ModelTestModality | "") {
  switch (modality) {
    case "image":
      return "image/*";
    default:
      return "";
  }
}

export function modelTestFileLabel(modality: ModelTestModality | "") {
  switch (modality) {
    case "image":
      return "测试图片";
    default:
      return "测试文件";
  }
}

export function settingsModelTestSupportMessage(
  selection: ModelSelectionPayload,
  supportedModalities: ModelTestModality[]
) {
  const entry = selectedCatalogEntryForSelection(selection);
  if (!entry) {
    return "当前模型目录中没有这条 provider + model 组合。";
  }
  if (!supportedModalities.length) {
    return entry.message;
  }
  return `${entry.message} · 原生输入：${supportedModalities.map((modality) => modelTestModalityDisplayName(modality)).join("、")}`;
}

export function canExecuteSettingsModelTest(selection: ModelSelectionPayload) {
  const entry = selectedCatalogEntryForSelection(selection);
  return entry?.status === "available";
}

export function currentDraftProviderSummary(providerId: string) {
  const provider = state.providerConfigs.find((item) => item.provider_id === providerId);
  const draft = activeProviderDraftForSelection(providerId);
  const parts = [provider?.display_name ?? providerId, providerId];
  if (draft.baseUrl) {
    parts.push(draft.baseUrl);
  }
  if (draft.enabled === false) {
    parts.push("已停用");
  }
  return parts.join(" · ");
}
