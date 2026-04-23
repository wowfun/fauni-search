import "./style.css";
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
  LibraryContentTypesData,
  LibrariesListData,
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
  SearchRequestSnapshot,
  SearchResultItem,
  SearchMode,
  SearchScopeKind,
  SearchOutcomeState,
  SourceActionData,
  SourceInventoryItem,
  SourceRootRulesPayload,
  SourceRootSnapshot,
  SourceRootsListData,
  SettingsSection,
  SourcesListData,
  UtilityDrawerSection,
  VectorSpaceDiagnosticsData,
  VideoSourceItem,
  VideoSourcesData,
  VisualUnitDetailData,
  WorkspaceKind,
} from "./types";

interface ApiSuccessEnvelope<T> {
  data: T;
}

interface ApiErrorEnvelope {
  error: ApiErrorPayload;
}

type ApiEnvelope<T> = ApiSuccessEnvelope<T> | ApiErrorEnvelope;

interface EndpointConfig {
  appHealth: string;
  sidecarHealth: string;
  qdrantCollections: string;
  uiRoot: string;
}

interface FocusedEditableState {
  id: string;
  value: string | null;
  selectionStart: number | null;
  selectionEnd: number | null;
}

function requireEnv(name: string): string {
  const value = import.meta.env[name];
  if (!value) {
    throw new Error(`Missing required environment variable ${name}`);
  }
  return String(value);
}

const endpoints: EndpointConfig = {
  appHealth: `http://${requireEnv("APP_HOST")}:${requireEnv("APP_PORT")}/health`,
  sidecarHealth: `http://${requireEnv("SIDECAR_HOST")}:${requireEnv("SIDECAR_PORT")}/health`,
  qdrantCollections: `${requireEnv("QDRANT_URL").replace(/\/$/, "")}/collections`,
  uiRoot: `http://${requireEnv("UI_HOST")}:${requireEnv("UI_PORT")}/`,
};

const JOB_POLL_INTERVAL_MS = 1000;
const JOB_POLL_TIMEOUT_MS = 5 * 60 * 1000;
const WORKSPACE_POLL_INTERVAL_MS = 3000;
const SEARCH_PAGE_SIZE = 5;
const PROVIDER_ID_LOCAL_SIDECAR = "local_sidecar";
const MODEL_TEST_MODALITIES: readonly ModelTestModality[] = ["text", "image"];
const CONTENT_TYPE_ORDER = ["image", "document", "video", "text"] as const;

function emptyContentTypes(): ContentTypesPayload {
  return {
    content_types: {},
  };
}

const state: AppState = {
  libraries: [],
  jobs: [],
  videoSources: [],
  sourceRoots: [],
  providerConfigs: [],
  modelCatalog: [],
  globalContentTypes: emptyContentTypes(),
  libraryContentTypes: emptyContentTypes(),
  resolvedContentModels: null,
  vectorSpaceDiagnostics: null,
  runtimeHealth: null,
  activeWorkspace: "search",
  selectedSettingsSection: "content-types",
  inventoryFilters: {
    sourceRootId: "",
    sourceType: "",
    sourceStatus: "",
  },
  searchFilters: {
    visualUnitKind: "",
    sourceType: "",
    pathPrefix: "",
    timeRangeStartMsDraft: "",
    timeRangeEndMsDraft: "",
  },
  inventorySummary: {
    total: 0,
    active: 0,
    invalidated: 0,
    out_of_scope: 0,
  },
  librarySources: [],
  selectedInventorySourceId: "",
  libraryDisplayNameDraft: "",
  libraryManagementDisplayNameDraft: "",
  libraryManagementDraftLibraryId: "",
  libraryIdDraft: "",
  selectedLibraryId: "",
  searchScope: "library",
  createLibraryPopoverOpen: false,
  manageLibraryPopoverOpen: false,
  utilityDrawerOpen: false,
  utilityDrawerSection: "status",
  searchFiltersPanelOpen: false,
  searchPreparationDisclosureOpen: false,
  searchJobsDisclosureOpen: false,
  searchDetailSheetOpen: false,
  inventoryDetailSheetOpen: false,
  editingSourceRootId: "",
  sourceRootPathDraft: "",
  sourceRootEnabledDraft: true,
  sourceRootIncludeGlobsDraft: "",
  sourceRootExcludeGlobsDraft: "",
  sourceRootIncludeExtensionsDraft: "",
  importPathsDraft: "",
  searchMode: "text",
  searchTextDraft: "",
  queryImageFile: null,
  queryImageObjectUrl: null,
  queryImageAsset: null,
  queryImageLibraryObject: null,
  queryVideoFile: null,
  queryVideoObjectUrl: null,
  queryVideoAsset: null,
  queryVideoSource: null,
  queryVideoLibraryObject: null,
  queryVideoDurationMs: null,
  queryVideoRange: null,
  queryDocumentFile: null,
  queryDocumentObjectUrl: null,
  queryDocumentAsset: null,
  queryDocumentLibraryObject: null,
  queryDocumentPageCount: null,
  queryDocumentStartPageDraft: "",
  queryDocumentEndPageDraft: "",
  importReceipt: null,
  selectedVisualUnit: null,
  selectedVisualUnitLibraryId: "",
  searchOutcome: null,
  searchInFlight: false,
  searchResultLibraryFocusId: "",
  lastSearchRequest: null,
  editingProviderId: "",
  providerEnabledDraft: true,
  providerBaseUrlDraft: "",
  selectedGlobalContentType: "",
  selectedLibraryContentType: "",
  globalModelTestModalityDraft: "",
  globalModelTestTextDraft: "",
  globalModelTestFile: null,
  globalModelTestComparisonModalityDraft: "",
  globalModelTestComparisonTextDraft: "",
  globalModelTestComparisonFile: null,
  globalModelTestResult: null,
  globalModelTestError: null,
  globalModelTestPending: false,
  libraryModelTestModalityDraft: "",
  libraryModelTestTextDraft: "",
  libraryModelTestFile: null,
  libraryModelTestComparisonModalityDraft: "",
  libraryModelTestComparisonTextDraft: "",
  libraryModelTestComparisonFile: null,
  libraryModelTestResult: null,
  libraryModelTestError: null,
  libraryModelTestPending: false,
  globalError: null,
  statusMessage: null,
};

const EDITABLE_TARGET_SELECTOR = 'input, textarea, [contenteditable="true"], [contenteditable=""], select';
let lastRenderedDetailPanelKey: string | null = null;

const root = document.querySelector<HTMLElement>("#app");

if (!root) {
  throw new Error("Missing #app root element.");
}

function toApiError(error: unknown): ApiErrorPayload {
  if (typeof error === "string") {
    return {
      code: "request_failed",
      message: error,
    };
  }

  if (error && typeof error === "object") {
    const candidate = error as Partial<ApiErrorPayload>;
    return {
      code: typeof candidate.code === "string" ? candidate.code : "request_failed",
      message:
        typeof candidate.message === "string" ? candidate.message : "Unexpected request failure.",
      details:
        candidate.details && typeof candidate.details === "object"
          ? candidate.details
          : undefined,
      retryable: typeof candidate.retryable === "boolean" ? candidate.retryable : undefined,
    };
  }

  return {
    code: "request_failed",
    message: "Unexpected request failure.",
  };
}

function selectedVisualUnitDetailSignature(): string | null {
  if (!state.selectedVisualUnit) {
    return null;
  }

  const visualUnit = state.selectedVisualUnit.visual_unit;
  return JSON.stringify({
    library_id: state.selectedVisualUnitLibraryId || state.selectedLibraryId || null,
    visual_unit_id: visualUnit.visual_unit_id,
    source_id: visualUnit.source_id,
    source_path: visualUnit.source_path,
    source_type: visualUnit.source_type,
    kind: visualUnit.kind,
    locator: visualUnit.locator,
    preview_url: state.selectedVisualUnit.preview?.url ?? null,
    neighbor_context: state.selectedVisualUnit.neighbor_context ?? null,
  });
}

function currentDetailPanelRenderKey(): string | null {
  const detailSignature = selectedVisualUnitDetailSignature();
  if (!detailSignature) {
    return null;
  }

  return JSON.stringify({
    detailSignature,
    searchDetailSheetOpen: state.searchDetailSheetOpen,
  });
}

function searchDetailSheetIsOpen() {
  return Boolean(state.selectedVisualUnit && state.searchDetailSheetOpen);
}

function inventoryDetailSheetIsOpen() {
  return Boolean(selectedInventorySource() && state.inventoryDetailSheetOpen);
}

function captureFocusedEditableState(): FocusedEditableState | null {
  const activeElement = document.activeElement;
  if (
    !(activeElement instanceof HTMLElement) ||
    !root.contains(activeElement) ||
    !activeElement.matches(EDITABLE_TARGET_SELECTOR) ||
    !activeElement.id
  ) {
    return null;
  }

  const snapshot = {
    id: activeElement.id,
    value: null,
    selectionStart: null,
    selectionEnd: null,
  };

  if (
    activeElement instanceof HTMLInputElement ||
    activeElement instanceof HTMLTextAreaElement ||
    activeElement instanceof HTMLSelectElement
  ) {
    snapshot.value = activeElement.value;
  }

  if (
    (activeElement instanceof HTMLInputElement && activeElement.type !== "number") ||
    activeElement instanceof HTMLTextAreaElement
  ) {
    snapshot.selectionStart = activeElement.selectionStart;
    snapshot.selectionEnd = activeElement.selectionEnd;
  }

  return snapshot;
}

function hasFocusedEditableControl() {
  return captureFocusedEditableState() !== null;
}

function restoreFocusedEditableState(snapshot: FocusedEditableState | null): void {
  if (!snapshot?.id) {
    return;
  }

  const nextElement = document.getElementById(snapshot.id);
  if (
    !(nextElement instanceof HTMLElement) ||
    !nextElement.matches(EDITABLE_TARGET_SELECTOR) ||
    nextElement.hasAttribute("disabled")
  ) {
    return;
  }

  nextElement.focus({ preventScroll: true });

  if (
    snapshot.value !== null &&
    ((nextElement instanceof HTMLInputElement && nextElement.type !== "file") ||
      nextElement instanceof HTMLTextAreaElement ||
      nextElement instanceof HTMLSelectElement)
  ) {
    nextElement.value = snapshot.value;
  }

  if (
    snapshot.selectionStart !== null &&
    snapshot.selectionEnd !== null &&
    ((nextElement instanceof HTMLInputElement && nextElement.type !== "number") ||
      nextElement instanceof HTMLTextAreaElement)
  ) {
    nextElement.setSelectionRange(snapshot.selectionStart, snapshot.selectionEnd);
  }
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function selectedLibrary(): LibrarySnapshot | null {
  return state.libraries.find((library) => library.id === state.selectedLibraryId) ?? null;
}

function libraryById(libraryId: string | null | undefined): LibrarySnapshot | null {
  if (!libraryId) {
    return null;
  }
  return state.libraries.find((library) => library.id === libraryId) ?? null;
}

function selectedVisualUnitOriginLibraryId(): string {
  return state.selectedVisualUnitLibraryId || state.selectedLibraryId || "";
}

function allLibrariesTextScopeActive() {
  return state.searchScope === "all_libraries" && state.searchMode === "text";
}

function searchScopeLabel(): string {
  if (state.searchScope === "all_libraries") {
    return `所有库 · ${state.libraries.length} 个库`;
  }
  const library = selectedLibrary();
  return library ? `当前库 · ${libraryDisplayName(library)}` : "当前库";
}

function searchScopeRequestPayload() {
  if (state.searchScope === "all_libraries") {
    return { kind: "all_libraries" };
  }
  return {
    kind: "library",
    library_id: state.selectedLibraryId,
  };
}

function libraryDisplayName(
  library: Pick<LibrarySnapshot, "display_name" | "id"> | null | undefined
): string {
  if (!library) {
    return "";
  }
  return library.display_name?.trim() || library.id;
}

function libraryIsArchived(
  library: Pick<LibrarySnapshot, "lifecycle_state"> | null | undefined
): boolean {
  return library?.lifecycle_state === "archived";
}

function libraryLifecycleLabel(
  library: Pick<LibrarySnapshot, "lifecycle_state"> | null | undefined
): string {
  return libraryIsArchived(library) ? "已归档" : "活跃";
}

function libraryLifecyclePillClass(
  library: Pick<LibrarySnapshot, "lifecycle_state"> | null | undefined
): string {
  return libraryIsArchived(library) ? "muted" : "ready";
}

function contentTypeDisplayName(contentType: string) {
  switch (contentType) {
    case "text":
      return "文本";
    case "image":
      return "图片";
    case "video":
      return "视频";
    case "document":
      return "文档";
    default:
      return contentType;
  }
}

function sourceTypeDisplayName(sourceType: string) {
  switch (sourceType) {
    case "image":
      return "图片";
    case "pdf":
      return "PDF";
    case "video":
      return "视频";
    default:
      return sourceType;
  }
}

function visualUnitKindDisplayName(kind: string) {
  switch (kind) {
    case "image":
      return "图片";
    case "document_page":
      return "文档页";
    case "video_segment":
      return "视频片段";
    default:
      return kind.replaceAll("_", " ");
  }
}

function sourceStatusDisplayName(status: string) {
  switch (status) {
    case "active":
      return "正常";
    case "invalidated":
      return "已失效";
    case "out_of_scope":
      return "超出范围";
    default:
      return status.replaceAll("_", " ");
  }
}

function modelTestModalityDisplayName(modality: ModelTestModality | string | "") {
  switch (modality) {
    case "text":
      return "文本";
    case "image":
      return "图片";
    default:
      return modality || "未选择";
  }
}

function workspaceDisplayName(workspace: WorkspaceKind) {
  switch (workspace) {
    case "inventory":
      return "库管理";
    case "settings":
      return "设置";
    default:
      return "搜索";
  }
}

function searchModeDisplayName(mode: SearchMode) {
  switch (mode) {
    case "image":
      return "图片";
    case "video":
      return "视频";
    case "document":
      return "文档";
    default:
      return "文本";
  }
}

function renderUiIcon(
  kind:
    | "search"
    | "library"
    | "tools"
    | "settings"
    | "content-types"
    | "override"
    | "providers"
    | "experiment"
    | "diagnostics"
    | "filter"
    | "image"
    | "video"
    | "document"
) {
  const path =
    kind === "search"
      ? '<circle cx="11" cy="11" r="6.5"></circle><path d="m16 16 5 5"></path>'
      : kind === "library"
        ? '<path d="M4 5.5h16"></path><path d="M6 5.5v13.5a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1V5.5"></path><path d="M9 5.5V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v1.5"></path>'
        : kind === "tools"
          ? '<path d="M12 3v4"></path><path d="M12 17v4"></path><path d="M3 12h4"></path><path d="M17 12h4"></path><path d="m5.6 5.6 2.8 2.8"></path><path d="m15.6 15.6 2.8 2.8"></path><path d="m18.4 5.6-2.8 2.8"></path><path d="m8.4 15.6-2.8 2.8"></path>'
          : kind === "settings"
            ? '<circle cx="12" cy="12" r="3.2"></circle><path d="M19.4 15a1 1 0 0 0 .2 1.1l.1.1a1 1 0 0 1 0 1.4l-1.1 1.1a1 1 0 0 1-1.4 0l-.1-.1a1 1 0 0 0-1.1-.2 1 1 0 0 0-.6.9v.3a1 1 0 0 1-1 1h-1.6a1 1 0 0 1-1-1v-.2a1 1 0 0 0-.7-1 1 1 0 0 0-1.1.2l-.1.1a1 1 0 0 1-1.4 0l-1.1-1.1a1 1 0 0 1 0-1.4l.1-.1a1 1 0 0 0 .2-1.1 1 1 0 0 0-.9-.6H4a1 1 0 0 1-1-1v-1.6a1 1 0 0 1 1-1h.2a1 1 0 0 0 1-.7 1 1 0 0 0-.2-1.1l-.1-.1a1 1 0 0 1 0-1.4L6 5.3a1 1 0 0 1 1.4 0l.1.1a1 1 0 0 0 1.1.2H9a1 1 0 0 0 .6-.9V4.4a1 1 0 0 1 1-1h1.6a1 1 0 0 1 1 1v.2a1 1 0 0 0 .7 1 1 1 0 0 0 1.1-.2l.1-.1a1 1 0 0 1 1.4 0L19 6.4a1 1 0 0 1 0 1.4l-.1.1a1 1 0 0 0-.2 1.1V9c0 .4.2.8.6.9h.3a1 1 0 0 1 1 1v1.6a1 1 0 0 1-1 1h-.2a1 1 0 0 0-1 .7z"></path>'
            : kind === "content-types"
              ? '<rect x="4" y="4" width="6" height="6" rx="1.2"></rect><rect x="14" y="4" width="6" height="6" rx="1.2"></rect><rect x="4" y="14" width="6" height="6" rx="1.2"></rect><rect x="14" y="14" width="6" height="6" rx="1.2"></rect>'
              : kind === "override"
                ? '<path d="M12 4 4 8l8 4 8-4-8-4Z"></path><path d="m4 12 8 4 8-4"></path><path d="m4 16 8 4 8-4"></path>'
                : kind === "providers"
                  ? '<path d="M9 7h6"></path><path d="M7.5 10.5h9"></path><path d="M6.5 14h11"></path><path d="M8 18h8"></path><path d="M5 7h.01"></path><path d="M19 10.5h.01"></path><path d="M5 14h.01"></path><path d="M19 18h.01"></path>'
                  : kind === "experiment"
                    ? '<path d="M10 3v5l-4.5 7.5A3 3 0 0 0 8 20h8a3 3 0 0 0 2.5-4.5L14 8V3"></path><path d="M8.5 3h7"></path><path d="M8 14h8"></path>'
                    : kind === "diagnostics"
                      ? '<path d="M4 13h3l2-5 4 9 2-4h5"></path><path d="M4 5.5h16"></path><path d="M4 18.5h16"></path>'
            : kind === "filter"
              ? '<path d="M4 6h16"></path><path d="M7 12h10"></path><path d="M10 18h4"></path>'
              : kind === "image"
                ? '<rect x="4" y="5" width="16" height="14" rx="2"></rect><path d="m7.5 15.5 3.2-3.6 2.8 2.8 2.5-2.7L19 15.5"></path><circle cx="9" cy="9" r="1.2"></circle>'
                : kind === "video"
                  ? '<rect x="3.5" y="6" width="17" height="12" rx="2"></rect><path d="m10 9 5 3-5 3z"></path>'
                  : '<path d="M8 3.5h6l4 4V20a1 1 0 0 1-1 1H8a1 1 0 0 1-1-1V4.5a1 1 0 0 1 1-1z"></path><path d="M14 3.5V8h4"></path>';

  return `<svg class="ui-icon" viewBox="0 0 24 24" aria-hidden="true">${path}</svg>`;
}

function settingsSectionLabel(section: SettingsSection) {
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

function settingsSectionIcon(section: SettingsSection) {
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

function settingsSectionDescription(section: SettingsSection, library: LibrarySnapshot | null) {
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

function settingsSectionNavSummary(section: SettingsSection, library: LibrarySnapshot | null) {
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

function settingsSectionPill(section: SettingsSection, library: LibrarySnapshot | null) {
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

function settingsMetricsForSection(section: SettingsSection, library: LibrarySnapshot | null) {
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

function renderSettingsStage(section: SettingsSection, library: LibrarySnapshot | null, body: string) {
  const pill = settingsSectionPill(section, library);
  const metrics = settingsMetricsForSection(section, library);

  return `
    <section
      class="settings-stage"
      data-testid="settings-stage"
      data-settings-stage="${escapeHtml(section)}"
    >
      <div class="settings-stage-hero">
        <div class="settings-stage-copy">
          <p class="eyebrow">设置章节</p>
          <h2 data-testid="settings-stage-title">${escapeHtml(settingsSectionLabel(section))}</h2>
          <p class="helper" data-testid="settings-stage-summary">${escapeHtml(
            settingsSectionDescription(section, library)
          )}</p>
        </div>
        <span class="pill ${pill.pillClass}" data-testid="settings-stage-pill">${escapeHtml(pill.label)}</span>
      </div>
      <div class="settings-stage-metrics" data-testid="settings-stage-metrics">
        ${metrics
          .map(
            (item) => `
              <article class="settings-stage-metric">
                <span class="settings-stage-metric-label">${escapeHtml(item.label)}</span>
                <strong class="settings-stage-metric-value">${escapeHtml(item.value)}</strong>
              </article>
            `
          )
          .join("")}
      </div>
      <div class="settings-stage-body">
        ${body}
      </div>
    </section>
  `;
}

function selectedInventorySource(): SourceInventoryItem | null {
  if (!state.librarySources.length) {
    return null;
  }
  return (
    state.librarySources.find((source) => source.source_id === state.selectedInventorySourceId) ??
    state.librarySources[0] ??
    null
  );
}

function selectedInventoryRepresentativeVisualUnit(source: SourceInventoryItem | null) {
  return source?.representative_visual_unit ?? null;
}

function selectedInventoryRepresentativePreview(source: SourceInventoryItem | null) {
  return source?.representative_preview ?? null;
}

function ensureSelectedInventorySource() {
  if (!state.librarySources.length) {
    state.selectedInventorySourceId = "";
    state.inventoryDetailSheetOpen = false;
    return;
  }
  if (!state.librarySources.some((source) => source.source_id === state.selectedInventorySourceId)) {
    state.selectedInventorySourceId = state.librarySources[0]?.source_id ?? "";
  }
}

function canSearchLibrary(library: LibrarySnapshot | null) {
  return Boolean(library && libraryOperationalReadiness(library).searchableUnits > 0);
}

function librarySearchStageNextAction(library: LibrarySnapshot | null) {
  if (!library) {
    return "library";
  }

  const readiness = libraryOperationalReadiness(library);
  if (readiness.searchableUnits > 0) {
    if (readiness.status === "配置需关注") {
      return "settings";
    }
    if (readiness.status === "观察未稳定" || readiness.status === "需要关注") {
      return "source-prep";
    }
    return "search";
  }

  if (readiness.status === "等待配置") {
    return "settings";
  }
  if (readiness.status === "正在准备中") {
    return "jobs";
  }
  return "source-prep";
}

function currentSearchScopeStageState(library: LibrarySnapshot | null) {
  if (!library) {
    return {
      status: "准备中",
      pillClass: "pending",
      summary: "先创建或选择一个库，搜索舞台就会接入真实内容。",
      nextAction: "library",
      searchEnabled: false,
      needsPreparation: true,
      searchableLibraries: 0,
      totalLibraries: 0,
      searchableUnits: 0,
      pendingLibraries: 0,
    };
  }

  if (!allLibrariesTextScopeActive()) {
    const readiness = libraryOperationalReadiness(library);
    return {
      status: readiness.status,
      pillClass: readiness.pillClass,
      summary: readiness.summary,
      nextAction: librarySearchStageNextAction(library),
      searchEnabled: readiness.searchableUnits > 0,
      needsPreparation: readiness.searchableUnits <= 0,
      searchableLibraries: readiness.searchableUnits > 0 ? 1 : 0,
      totalLibraries: 1,
      searchableUnits: readiness.searchableUnits,
      pendingLibraries: readiness.pendingJobs > 0 ? 1 : 0,
    };
  }

  const readiness = libraryOperationalReadiness(library);
  const totalLibraries = state.libraries.length;
  const searchableLibraries = state.libraries.filter((item) => item.counts.accepted_items > 0).length;
  const searchableUnits = state.libraries.reduce(
    (sum, item) => sum + Math.max(item.counts.accepted_items, 0),
    0
  );
  const pendingLibraries = state.libraries.filter((item) => item.counts.pending_jobs > 0).length;

  if (searchableLibraries > 0) {
    const trailingSummary =
      searchableLibraries < totalLibraries
        ? `其余 ${totalLibraries - searchableLibraries} 个库仍在准备中或为空。`
        : "当前范围里的库都已经进入可搜索状态。";
    return {
      status: "可搜索",
      pillClass: "ready",
      summary: `当前范围覆盖 ${totalLibraries} 个库；其中 ${searchableLibraries} 个库已可搜索，共 ${searchableUnits} 个对象可以直接参与文本搜索。${trailingSummary}`,
      nextAction: "search",
      searchEnabled: true,
      needsPreparation: false,
      searchableLibraries,
      totalLibraries,
      searchableUnits,
      pendingLibraries,
    };
  }

  const nextAction = librarySearchStageNextAction(library);
  const selectedLibrarySummary =
    totalLibraries > 1
      ? `当前选中库 ${libraryDisplayName(library)}：${readiness.summary}`
      : readiness.summary;

  if (pendingLibraries > 0) {
    return {
      status: "准备中",
      pillClass: "pending",
      summary: `所有库范围里还没有可搜索库；${pendingLibraries} 个库仍在导入或建索引。${selectedLibrarySummary ? ` ${selectedLibrarySummary}` : ""}`,
      nextAction,
      searchEnabled: false,
      needsPreparation: true,
      searchableLibraries,
      totalLibraries,
      searchableUnits,
      pendingLibraries,
    };
  }

  const pillClass =
    readiness.pillClass === "error"
      ? "error"
      : readiness.pillClass === "pending"
        ? "pending"
        : "muted";
  const status = pillClass === "error" ? "部分受限" : pillClass === "pending" ? "准备中" : "等待内容";

  return {
    status,
    pillClass,
    summary: `所有库范围里还没有可搜索库；先让至少一个库进入可搜索状态。${selectedLibrarySummary ? ` ${selectedLibrarySummary}` : ""}`,
    nextAction,
    searchEnabled: false,
    needsPreparation: true,
    searchableLibraries,
    totalLibraries,
    searchableUnits,
    pendingLibraries,
  };
}

function canSearchCurrentScope(library: LibrarySnapshot | null) {
  return currentSearchScopeStageState(library).searchEnabled;
}

function libraryNeedsPreparation(library: LibrarySnapshot | null) {
  return currentSearchScopeStageState(library).needsPreparation;
}

function searchStageNextAction(library: LibrarySnapshot | null) {
  return currentSearchScopeStageState(library).nextAction;
}

function currentSearchStageState(library: LibrarySnapshot | null) {
  const readiness = currentSearchScopeStageState(library);
  return {
    status: readiness.status,
    pillClass: readiness.pillClass,
    summary: readiness.summary,
  };
}

function runtimeHealthOverview() {
  if (!state.runtimeHealth) {
    return null;
  }

  const processSnapshots = [state.runtimeHealth.app, state.runtimeHealth.qdrant];
  const processIssues = processSnapshots.filter((snapshot) => snapshot.status !== "available");
  const enabledProviders = state.runtimeHealth.providers.filter((provider) => provider.enabled);
  const providerIssues = enabledProviders.filter((provider) => provider.status !== "available");

  return {
    processSnapshots,
    processIssues,
    enabledProviders,
    providerIssues,
    summary:
      processIssues.length || providerIssues.length
        ? `运行时有 ${processIssues.length + providerIssues.length} 个受限项，建议打开诊断查看详细状态。`
        : enabledProviders.length
          ? `运行时健康，${enabledProviders.length} 个已启用连接当前可用。`
          : "运行时健康，当前没有启用中的连接异常。",
  };
}

function shellRuntimeStatusLabel(status: string) {
  if (status === "available") {
    return "正常";
  }
  if (status === "not_enabled") {
    return "未启用";
  }
  if (status === "runtime_unavailable" || status === "not_supported") {
    return "受限";
  }
  return "待确认";
}

function currentStatusCapsule(library: LibrarySnapshot | null) {
  const runtimeOverview = runtimeHealthOverview();

  if (state.globalError) {
    return {
      label: "部分受限",
      pillClass: "error",
      summary: state.globalError.message,
    };
  }
  if (runtimeOverview && (runtimeOverview.processIssues.length || runtimeOverview.providerIssues.length)) {
    return {
      label: "部分受限",
      pillClass: "error",
      summary: runtimeOverview.summary,
    };
  }
  if (!library) {
    return {
      label: "准备中",
      pillClass: "pending",
      summary: "还没有选定库，先创建或选择一个库。",
    };
  }

  const readiness = libraryOperationalReadiness(library);

  if (library.counts.pending_jobs > 0 && readiness.searchableUnits <= 0) {
    return {
      label: "准备中",
      pillClass: "pending",
      summary: readiness.summary,
    };
  }
  if (readiness.searchableUnits <= 0) {
    return {
      label: readiness.pillClass === "pending" ? "准备中" : "部分受限",
      pillClass: readiness.pillClass,
      summary: readiness.summary,
    };
  }
  if (library.counts.pending_jobs > 0) {
    return {
      label: "准备中",
      pillClass: "pending",
      summary: `当前库还有 ${library.counts.pending_jobs} 个后台任务未完成。`,
    };
  }
  if (readiness.status === "观察未稳定" || readiness.status === "需要关注" || readiness.status === "配置需关注") {
    return {
      label: "部分受限",
      pillClass: readiness.pillClass,
      summary: readiness.summary,
    };
  }
  return {
    label: "Ready",
    pillClass: "ready",
    summary: "当前库可直接执行搜索和结果复用。",
  };
}

function renderSearchStatusNextStep(library: LibrarySnapshot | null, context: "utility" | "outcome" = "utility") {
  if (!library) {
    return "";
  }

  const allLibrariesScope = allLibrariesTextScopeActive();
  const readiness = libraryOperationalReadiness(library);
  const scopeState = currentSearchScopeStageState(library);
  const nextAction = scopeState.nextAction;
  const actions = [];
  let title = allLibrariesScope ? "可以直接跨库搜索" : "可以直接搜索";
  let summary = allLibrariesScope
    ? scopeState.summary
    : "当前库已经进入可搜索状态，下一步更适合直接发起查询或调整查询方式。";

  if (nextAction === "settings") {
    title = allLibrariesScope ? "先完成一个库的搜索配置" : "检查当前库覆盖";
    summary = scopeState.summary;
    actions.push(`
      <button
        type="button"
        class="${context === "utility" ? "secondary-button" : ""}"
        data-testid="${escapeHtml(context === "utility" ? "utility-drawer-status-open-library-overrides" : "search-error-open-library-overrides")}"
        data-open-settings-section="library-overrides"
      >
        前往当前库覆盖
      </button>
    `);
  } else if (nextAction === "jobs") {
    title = allLibrariesScope ? "等待至少一个库准备完成" : "等待当前任务完成";
    summary = scopeState.summary;
    actions.push(`
      <button
        type="button"
        class="${context === "utility" ? "secondary-button" : ""}"
        data-testid="${escapeHtml(context === "utility" ? "utility-drawer-status-open-jobs" : "search-outcome-open-jobs")}"
        data-utilities-action="focus-search-jobs"
      >
        查看任务
      </button>
    `);
  } else if (nextAction === "source-prep") {
    title = allLibrariesScope
      ? "让至少一个库进入可搜索状态"
      : readiness.status === "尚未接入来源根"
        ? "接入第一个来源根"
        : readiness.status === "来源根已停用"
          ? "恢复一个来源根"
          : readiness.status === "需要关注"
            ? "先检查来源根健康"
            : readiness.status === "观察未稳定"
              ? "恢复来源观察"
              : "准备第一批内容";
    summary = scopeState.summary;
    actions.push(`
      <button
        type="button"
        class="${context === "utility" ? "secondary-button" : ""}"
        data-testid="${escapeHtml(context === "utility" ? "utility-drawer-status-open-source-prep" : "search-outcome-open-source-prep")}"
        data-utilities-action="focus-source-prep"
      >
        打开来源准备
      </button>
    `);
    actions.push(`
      <button
        type="button"
        class="secondary-button"
        data-testid="${escapeHtml(context === "utility" ? "utility-drawer-status-open-inventory" : "search-outcome-open-inventory")}"
        data-workspace="inventory"
      >
        前往库管理
      </button>
    `);
  }

  return `
    <div class="utility-drawer-summary-card search-status-next-step" data-testid="${escapeHtml(
      context === "utility" ? "utility-drawer-status-next-step" : "search-outcome-next-step"
    )}">
      <strong>${escapeHtml(title)}</strong>
      <p class="helper">${escapeHtml(summary)}</p>
      ${actions.length ? `<div class="inline-actions">${actions.join("")}</div>` : ""}
    </div>
  `;
}

function emptyInventorySummary(): InventorySummary {
  return {
    total: 0,
    active: 0,
    invalidated: 0,
    out_of_scope: 0,
  };
}

function resetInventoryFilters() {
  state.inventoryFilters = {
    sourceRootId: "",
    sourceType: "",
    sourceStatus: "",
  };
}

function hydrateLibraryManagementDraft(library: LibrarySnapshot | null) {
  if (!library) {
    state.libraryManagementDraftLibraryId = "";
    state.libraryManagementDisplayNameDraft = "";
    state.manageLibraryPopoverOpen = false;
    return;
  }

  state.libraryManagementDraftLibraryId = library.id;
  state.libraryManagementDisplayNameDraft = library.display_name;
}

function upsertLibrarySnapshot(library: LibrarySnapshot) {
  const index = state.libraries.findIndex((item) => item.id === library.id);
  if (index >= 0) {
    state.libraries.splice(index, 1, library);
    return;
  }
  state.libraries.unshift(library);
}

function resetSearchFilters() {
  state.searchFilters = {
    visualUnitKind: "",
    sourceType: "",
    pathPrefix: "",
    timeRangeStartMsDraft: "",
    timeRangeEndMsDraft: "",
  };
}

function resetSearchResultLibraryFocus() {
  state.searchResultLibraryFocusId = "";
}

function resetProviderEditor() {
  state.editingProviderId = "";
  state.providerEnabledDraft = true;
  state.providerBaseUrlDraft = "";
}

function selectedProviderConfig(): ProviderConfigSnapshot | null {
  return (
    state.providerConfigs.find((provider) => provider.provider_id === state.editingProviderId) ??
    null
  );
}

function providerConfigLabel(providerId?: string | null) {
  if (!providerId) {
    return "inherit";
  }
  const provider = state.providerConfigs.find((item) => item.provider_id === providerId);
  if (!provider) {
    return `${providerId} (缺失)`;
  }
  return `${provider.display_name} (${provider.provider_kind})`;
}

function providerSelectionPillClass(status: string) {
  if (status === "available") {
    return "ready";
  }
  if (status === "not_supported" || status === "runtime_unavailable") {
    return "error";
  }
  if (status === "not_enabled") {
    return "pending";
  }
  return "muted";
}

function providerProbePillClass(status?: string | null) {
  if (status === "available") {
    return "ready";
  }
  if (status === "runtime_unavailable" || status === "not_supported") {
    return "error";
  }
  if (status === "not_enabled") {
    return "pending";
  }
  return "muted";
}

function formatResolvedModel(selection: ResolvedModelSelectionPayload | undefined) {
  if (!selection) {
    return "未解析";
  }
  const parts = [selection.provider_id, `${selection.model_id}@${selection.model_version}`];
  if (selection.model_revision && selection.model_revision !== selection.model_version) {
    parts.push(`修订 ${selection.model_revision}`);
  }
  return parts.join(" · ");
}

function formatResolvedContentModel(selection: ResolvedContentModelSelectionPayload | undefined) {
  return formatResolvedModel(selection);
}

function formatBindingSource(bindingSource: BindingSource | undefined) {
  switch (bindingSource) {
    case "global_content_type":
      return "全局内容类型";
    case "library_content_type":
      return "当前库覆盖";
    case "settings_model_test":
      return "设置模型测试";
    default:
      return bindingSource ? bindingSource.replaceAll("_", " ") : "未知来源";
  }
}

function formatResolvedModelContext(selection: ResolvedModelSelectionPayload | undefined) {
  if (!selection) {
    return "未解析";
  }
  const parts = [formatBindingSource(selection.binding_source), selection.status];
  const provider = state.providerConfigs.find((item) => item.provider_id === selection.provider_id);
  if (provider?.base_url) {
    parts.push(provider.base_url);
  }
  return parts.join(" · ");
}

function formatResolvedContentModelContext(
  selection: ResolvedContentModelSelectionPayload | undefined
) {
  return formatResolvedModelContext(selection);
}

function formatEmbeddingCapabilityValues(values: string[] | undefined) {
  return values?.length ? values.join(", ") : "无";
}

function formatEmbeddingCapabilities(
  capabilities: EmbeddingCapabilities | undefined,
  options: { includePrefix?: boolean } = {}
) {
  if (!capabilities) {
    return options.includePrefix ? "嵌入能力 · 不可用" : "不可用";
  }

  const parts = [
    `输入 ${formatEmbeddingCapabilityValues(capabilities.input_types)}`,
    `向量 ${formatEmbeddingCapabilityValues(capabilities.vector_types)}`,
    `混合输入 ${capabilities.supports_mixed_inputs ? "是" : "否"}`,
  ];
  if (options.includePrefix) {
    parts.unshift("嵌入能力");
  }
  return parts.join(" · ");
}

function formatExecutionInputTypes(inputTypes: string[] | undefined, options: { includePrefix?: boolean } = {}) {
  const value = inputTypes?.length ? inputTypes.join(", ") : "无";
  return options.includePrefix ? `执行输入 · ${value}` : value;
}

function resetInventoryState() {
  state.librarySources = [];
  state.inventorySummary = emptyInventorySummary();
  state.selectedInventorySourceId = "";
  state.inventoryDetailSheetOpen = false;
}

function searchHasMoreResults() {
  return Boolean(state.searchOutcome?.next_cursor && state.lastSearchRequest);
}

function searchFiltersSummary() {
  const tokens = [];
  if (state.searchFilters.visualUnitKind) {
    tokens.push(`对象类型=${visualUnitKindDisplayName(state.searchFilters.visualUnitKind)}`);
  }
  if (state.searchFilters.sourceType) {
    tokens.push(`来源类型=${sourceTypeDisplayName(state.searchFilters.sourceType)}`);
  }
  if (state.searchFilters.pathPrefix.trim()) {
    tokens.push(`路径前缀=${state.searchFilters.pathPrefix.trim()}`);
  }
  if (
    state.searchFilters.timeRangeStartMsDraft.trim() ||
    state.searchFilters.timeRangeEndMsDraft.trim()
  ) {
    tokens.push(
      `时间范围=${state.searchFilters.timeRangeStartMsDraft.trim() || "?"}→${state.searchFilters.timeRangeEndMsDraft.trim() || "?"}`
    );
  }
  return tokens.length ? tokens.join(" · ") : "未启用额外过滤器";
}

function parseNonNegativeIntegerDraft(value: string, field: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }

  if (!/^\d+$/.test(trimmed)) {
    throw {
      code: "validation_failed",
      message: `${field} 必须是非负整数。`,
      details: {
        field,
      },
    } satisfies ApiErrorPayload;
  }

  return Number(trimmed);
}

function searchFiltersPayload() {
  const filters: Record<string, unknown> = {};
  if (state.searchFilters.visualUnitKind) {
    filters["visual_unit.kind"] = state.searchFilters.visualUnitKind;
  }
  if (state.searchFilters.sourceType) {
    filters.source_type = state.searchFilters.sourceType;
  }
  if (state.searchFilters.pathPrefix.trim()) {
    filters.path_prefix = state.searchFilters.pathPrefix.trim();
  }

  const timeRangeStartMs = parseNonNegativeIntegerDraft(
    state.searchFilters.timeRangeStartMsDraft,
    "filters.time_range.start_ms"
  );
  const timeRangeEndMs = parseNonNegativeIntegerDraft(
    state.searchFilters.timeRangeEndMsDraft,
    "filters.time_range.end_ms"
  );

  if (timeRangeStartMs !== null || timeRangeEndMs !== null) {
    if (timeRangeStartMs === null || timeRangeEndMs === null || timeRangeStartMs >= timeRangeEndMs) {
      throw {
        code: "validation_failed",
        message: "时间范围过滤器必须同时填写开始和结束毫秒，且开始值必须小于结束值。",
        details: {
          field: "filters.time_range",
        },
      } satisfies ApiErrorPayload;
    }
    filters.time_range = {
      start_ms: timeRangeStartMs,
      end_ms: timeRangeEndMs,
    };
  }

  return Object.keys(filters).length ? filters : undefined;
}

function summarizeInventorySources(sources: SourceInventoryItem[]): InventorySummary {
  const summary = emptyInventorySummary();
  summary.total = sources.length;
  for (const source of sources) {
    if (source.status === "active") {
      summary.active += 1;
    } else if (source.status === "invalidated") {
      summary.invalidated += 1;
    } else if (source.status === "out_of_scope") {
      summary.out_of_scope += 1;
    }
  }
  return summary;
}

function resetSourceRootEditor() {
  state.editingSourceRootId = "";
  state.sourceRootPathDraft = "";
  state.sourceRootEnabledDraft = true;
  state.sourceRootIncludeGlobsDraft = "";
  state.sourceRootExcludeGlobsDraft = "";
  state.sourceRootIncludeExtensionsDraft = "";
}

function keepSearchPreparationDisclosureOpen() {
  state.searchPreparationDisclosureOpen = true;
}

function populateSourceRootEditor(sourceRoot) {
  state.editingSourceRootId = sourceRoot.source_root_id;
  state.sourceRootPathDraft = sourceRoot.root_path ?? "";
  state.sourceRootEnabledDraft = Boolean(sourceRoot.enabled);
  state.sourceRootIncludeGlobsDraft = (sourceRoot.rules?.include_globs ?? []).join("\n");
  state.sourceRootExcludeGlobsDraft = (sourceRoot.rules?.exclude_globs ?? []).join("\n");
  state.sourceRootIncludeExtensionsDraft = (sourceRoot.rules?.include_extensions ?? []).join(", ");
}

function multilineDraftToList(value) {
  return String(value ?? "")
    .split("\n")
    .map((item) => item.trim())
    .filter(Boolean);
}

function commaDraftToList(value) {
  return String(value ?? "")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function sourceRootPayloadFromDraft() {
  return {
    root_path: state.sourceRootPathDraft.trim(),
    enabled: state.sourceRootEnabledDraft,
    rules: {
      include_globs: multilineDraftToList(state.sourceRootIncludeGlobsDraft),
      exclude_globs: multilineDraftToList(state.sourceRootExcludeGlobsDraft),
      include_extensions: commaDraftToList(state.sourceRootIncludeExtensionsDraft),
    },
  };
}

function hydrateProviderEditor(provider: ProviderConfigSnapshot | null) {
  if (!provider) {
    resetProviderEditor();
    return;
  }

  state.editingProviderId = provider.provider_id;
  state.providerEnabledDraft = provider.enabled;
  state.providerBaseUrlDraft = provider.base_url ?? "";
}

function contentTypeOrderValue(contentType: string) {
  const index = CONTENT_TYPE_ORDER.indexOf(contentType as (typeof CONTENT_TYPE_ORDER)[number]);
  return index >= 0 ? index : CONTENT_TYPE_ORDER.length;
}

function sortContentTypes(values: Iterable<string>) {
  return [...values].sort((left, right) => {
    return contentTypeOrderValue(left) - contentTypeOrderValue(right) || left.localeCompare(right);
  });
}

function sortedContentTypeKeys(payload: ContentTypesPayload) {
  return sortContentTypes(Object.keys(payload.content_types));
}

function availableContentTypeKeys(
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

function catalogEntriesForProvider(providerId: string | null | undefined): ModelCatalogEntry[] {
  if (!providerId) {
    return [];
  }
  return state.modelCatalog.filter((entry) => entry.provider_id === providerId);
}

function selectedCatalogEntryForProvider(
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

function selectedCatalogEntryForSelection(
  selection: Pick<ModelSelectionPayload, "provider_id" | "model_id">
) {
  return selectedCatalogEntryForProvider(selection.provider_id, selection.model_id);
}

function splitModelReference(modelReference: string): ModelSelectionPayload {
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

function composeModelReference(selection: Pick<ModelSelectionPayload, "provider_id" | "model_id">) {
  if (!selection.provider_id || !selection.model_id) {
    return "";
  }
  return `${selection.provider_id}/${selection.model_id}`;
}

function defaultContentTypeBinding(): ContentTypeBindingPayload {
  return {
    enabled: false,
    model: "",
    vector_type: "",
  };
}

function selectedGlobalContentTypeKey() {
  const selected = state.selectedGlobalContentType;
  const available = availableContentTypeKeys(state.globalContentTypes);
  if (selected && available.includes(selected)) {
    return selected;
  }
  return available[0] ?? "";
}

function selectedLibraryContentTypeKey() {
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

function selectedGlobalContentTypeBinding(): ContentTypeBindingPayload {
  return (
    state.globalContentTypes.content_types[selectedGlobalContentTypeKey()] ??
    defaultContentTypeBinding()
  );
}

function selectedLibraryContentTypeBinding(): ContentTypeBindingPayload {
  const contentType = selectedLibraryContentTypeKey();
  return (
    state.libraryContentTypes.content_types[contentType] ??
    state.globalContentTypes.content_types[contentType] ??
    defaultContentTypeBinding()
  );
}

function libraryContentTypeHasOverride(contentType: string) {
  return Object.prototype.hasOwnProperty.call(state.libraryContentTypes.content_types, contentType);
}

function selectedLibraryContentTypeHasOverride() {
  const contentType = selectedLibraryContentTypeKey();
  return contentType ? libraryContentTypeHasOverride(contentType) : false;
}

function selectionFromBinding(binding: ContentTypeBindingPayload): ModelSelectionPayload {
  const selection = splitModelReference(binding.model);
  return {
    provider_id: selection.provider_id || PROVIDER_ID_LOCAL_SIDECAR,
    model_id: selection.model_id || "",
  };
}

function selectedGlobalModelSelection(): ModelSelectionPayload {
  return selectionFromBinding(selectedGlobalContentTypeBinding());
}

function selectedLibraryModelSelection(): ModelSelectionPayload {
  return selectionFromBinding(selectedLibraryContentTypeBinding());
}

function vectorTypeOptionsForSelection(selection: ModelSelectionPayload, currentValue: string) {
  const options = [
    ...(selectedCatalogEntryForSelection(selection)?.embedding_capabilities.vector_types ?? []),
  ];
  if (currentValue && !options.includes(currentValue)) {
    options.push(currentValue);
  }
  return options;
}

function normalizeContentTypeBindingForProvider(
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

function supportedTestModalitiesForSelection(
  providerId: string | null | undefined,
  modelId?: string | null
): ModelTestModality[] {
  const entry = selectedCatalogEntryForProvider(providerId, modelId);
  return MODEL_TEST_MODALITIES.filter((modality) =>
    entry?.embedding_capabilities?.input_types?.includes(modality)
  );
}

function activeProviderDraftForSelection(providerId: string): {
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

function selectedGlobalTestModalities(): ModelTestModality[] {
  const selection = selectionFromBinding(selectedGlobalContentTypeBinding());
  return supportedTestModalitiesForSelection(selection.provider_id, selection.model_id);
}

function selectedLibraryTestModalities(): ModelTestModality[] {
  const selection = selectionFromBinding(selectedLibraryContentTypeBinding());
  return supportedTestModalitiesForSelection(selection.provider_id, selection.model_id);
}

function ensureValidModelTestDrafts() {
  const globalContentType = selectedGlobalContentTypeKey();
  if (state.selectedGlobalContentType !== globalContentType) {
    state.selectedGlobalContentType = globalContentType;
  }
  const libraryContentType = selectedLibraryContentTypeKey();
  if (state.selectedLibraryContentType !== libraryContentType) {
    state.selectedLibraryContentType = libraryContentType;
  }

  const globalModalities = selectedGlobalTestModalities();
  if (!globalModalities.includes(state.globalModelTestModalityDraft as ModelTestModality)) {
    state.globalModelTestModalityDraft =
      (globalModalities.includes("text") ? "text" : globalModalities[0]) ?? "";
    state.globalModelTestFile = null;
    state.globalModelTestResult = null;
    state.globalModelTestError = null;
  }
  if (
    state.globalModelTestComparisonModalityDraft &&
    !globalModalities.includes(state.globalModelTestComparisonModalityDraft as ModelTestModality)
  ) {
    state.globalModelTestComparisonModalityDraft = "";
    state.globalModelTestComparisonFile = null;
    state.globalModelTestResult = null;
    state.globalModelTestError = null;
  }

  const libraryModalities = selectedLibraryTestModalities();
  if (!libraryModalities.includes(state.libraryModelTestModalityDraft as ModelTestModality)) {
    state.libraryModelTestModalityDraft =
      (libraryModalities.includes("text") ? "text" : libraryModalities[0]) ?? "";
    state.libraryModelTestFile = null;
    state.libraryModelTestResult = null;
    state.libraryModelTestError = null;
  }
  if (
    state.libraryModelTestComparisonModalityDraft &&
    !libraryModalities.includes(state.libraryModelTestComparisonModalityDraft as ModelTestModality)
  ) {
    state.libraryModelTestComparisonModalityDraft = "";
    state.libraryModelTestComparisonFile = null;
    state.libraryModelTestResult = null;
    state.libraryModelTestError = null;
  }
}

function resetGlobalModelTestState() {
  state.globalModelTestFile = null;
  state.globalModelTestComparisonFile = null;
  state.globalModelTestResult = null;
  state.globalModelTestError = null;
  state.globalModelTestPending = false;
  ensureValidModelTestDrafts();
}

function resetLibraryModelTestState() {
  state.libraryModelTestFile = null;
  state.libraryModelTestComparisonFile = null;
  state.libraryModelTestResult = null;
  state.libraryModelTestError = null;
  state.libraryModelTestPending = false;
  ensureValidModelTestDrafts();
}

function formatModelTestShape(shape: number[] | undefined) {
  if (!shape?.length) {
    return "[]";
  }
  return `[${shape.join(", ")}]`;
}

function modelTestFileAccept(modality: ModelTestModality | "") {
  switch (modality) {
    case "image":
      return "image/*";
    default:
      return "";
  }
}

function modelTestFileLabel(modality: ModelTestModality | "") {
  switch (modality) {
    case "image":
      return "测试图片";
    default:
      return "测试文件";
  }
}

function settingsModelTestSupportMessage(
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

function canExecuteSettingsModelTest(selection: ModelSelectionPayload) {
  const entry = selectedCatalogEntryForSelection(selection);
  return entry?.status === "available";
}

function currentDraftProviderSummary(providerId: string) {
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

function renderModelTestResult(testIdPrefix: string, result: ModelTestData | null) {
  if (!result) {
    return "";
  }

  return `
    <div class="model-test-result" data-testid="${testIdPrefix}-result">
      <div class="job-meta">
        <span class="pill ready" data-testid="${testIdPrefix}-resolved-model">${escapeHtml(formatResolvedModel(result.resolved_model))}</span>
        <span class="pill muted">${escapeHtml(result.operation_kind)}</span>
        <span class="pill muted" data-testid="${testIdPrefix}-shape">${escapeHtml(formatModelTestShape(result.vector_shape))}</span>
      </div>
      <p class="helper">${escapeHtml(formatResolvedModelContext(result.resolved_model))}</p>
      <p class="helper">${escapeHtml(formatEmbeddingCapabilities(result.resolved_model.embedding_capabilities, { includePrefix: true }))}</p>
      <p class="helper">${escapeHtml(result.resolved_model.message)}</p>
      <div class="detail-grid model-test-grid">
        <div class="detail-block">
          <h5>向量</h5>
          <pre data-testid="${testIdPrefix}-vectors">${escapeHtml(JSON.stringify(result.vectors, null, 2))}</pre>
        </div>
        ${
          result.pooled_vector?.length
            ? `
              <div class="detail-block">
                <h5>池化向量</h5>
                <pre data-testid="${testIdPrefix}-pooled-vector">${escapeHtml(JSON.stringify(result.pooled_vector, null, 2))}</pre>
              </div>
            `
            : ""
        }
      </div>
      <div class="detail-block">
        <h5>输入摘要</h5>
        <pre>${escapeHtml(JSON.stringify(result.input_summary, null, 2))}</pre>
      </div>
      ${
        result.comparison
          ? `
            <div class="detail-block">
              <h5>对照结果</h5>
              <div class="job-meta">
                <span class="pill muted">${escapeHtml(result.comparison.operation_kind)}</span>
                <span class="pill muted" data-testid="${testIdPrefix}-comparison-shape">${escapeHtml(
                  formatModelTestShape(result.comparison.vector_shape)
                )}</span>
                <span class="pill ready" data-testid="${testIdPrefix}-similarity">${escapeHtml(
                  result.comparison.similarity_to_primary.toFixed(6)
                )}</span>
              </div>
              <p class="helper">输入模态：${escapeHtml(modelTestModalityDisplayName(result.comparison.input_modality))}</p>
              <div class="detail-grid model-test-grid">
                <div class="detail-block">
                  <h5>对照向量</h5>
                  <pre data-testid="${testIdPrefix}-comparison-vectors">${escapeHtml(
                    JSON.stringify(result.comparison.vectors, null, 2)
                  )}</pre>
                </div>
                ${
                  result.comparison.pooled_vector?.length
                    ? `
                      <div class="detail-block">
                        <h5>对照池化向量</h5>
                        <pre data-testid="${testIdPrefix}-comparison-pooled-vector">${escapeHtml(
                          JSON.stringify(result.comparison.pooled_vector, null, 2)
                        )}</pre>
                      </div>
                    `
                    : ""
                }
              </div>
              <div class="detail-block">
                <h5>对照输入摘要</h5>
                <pre>${escapeHtml(JSON.stringify(result.comparison.input_summary, null, 2))}</pre>
              </div>
            </div>
          `
          : ""
      }
    </div>
  `;
}

function renderSettingsModelTestPanel(options: {
  scope: "global" | "library";
  selection: ModelSelectionPayload;
  supportedModalities: ModelTestModality[];
  modalityDraft: ModelTestModality | "";
  textDraft: string;
  file: File | null;
  comparisonModalityDraft: ModelTestModality | "";
  comparisonTextDraft: string;
  comparisonFile: File | null;
  result: ModelTestData | null;
  error: ApiErrorPayload | null;
  pending: boolean;
}) {
  const {
    scope,
    selection,
    supportedModalities,
    modalityDraft,
    textDraft,
    file,
    comparisonModalityDraft,
    comparisonTextDraft,
    comparisonFile,
    result,
    error,
    pending,
  } = options;
  const testIdPrefix = `${scope}-model-test`;
  const inputModality = modalityDraft || supportedModalities[0] || "";
  const fileRequired = inputModality === "image";
  const comparisonFileRequired = comparisonModalityDraft === "image";
  const disabled =
    !supportedModalities.length || !canExecuteSettingsModelTest(selection) || pending;
  const catalogEntry = selectedCatalogEntryForSelection(selection);

  return `
    <section class="model-test-panel" data-testid="${testIdPrefix}-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">测试</p>
          <h3>${scope === "global" ? "测试当前全局模型" : "测试当前库模型"}</h3>
        </div>
      </div>
      <p class="helper" data-testid="${testIdPrefix}-draft-summary">
        ${escapeHtml(currentDraftProviderSummary(selection.provider_id))} · ${escapeHtml(selection.model_id)}
      </p>
      <p class="helper" data-testid="${testIdPrefix}-support-message">
        ${escapeHtml(settingsModelTestSupportMessage(selection, supportedModalities))}
      </p>
      ${
        catalogEntry
          ? `<p class="helper" data-testid="${scope}-model-capabilities">${escapeHtml(
              formatEmbeddingCapabilities(catalogEntry.embedding_capabilities, { includePrefix: true })
            )}</p>`
          : ""
      }
      <form id="${testIdPrefix}-form" class="stack-form" data-testid="${testIdPrefix}-form">
        <div class="filter-grid settings-filter-grid">
          <label>
            <span>主输入模态</span>
            <select
              id="${testIdPrefix}-modality"
              data-testid="${testIdPrefix}-modality"
              ${supportedModalities.length ? "" : "disabled"}
            >
              ${supportedModalities.length
                ? supportedModalities
                    .map(
                      (modality) => `
                        <option value="${escapeHtml(modality)}" ${modality === inputModality ? "selected" : ""}>
                          ${escapeHtml(modelTestModalityDisplayName(modality))}
                        </option>
                      `
                    )
                    .join("")
                : '<option value="" selected>当前不可用</option>'}
            </select>
          </label>
          ${
            fileRequired
              ? `
                <label>
                  <span>${escapeHtml(modelTestFileLabel(inputModality))}</span>
                  <input
                    id="${testIdPrefix}-file"
                    data-testid="${testIdPrefix}-file"
                    type="file"
                    accept="${escapeHtml(modelTestFileAccept(inputModality))}"
                    ${supportedModalities.length ? "" : "disabled"}
                  />
                </label>
              `
              : `
                <label class="model-test-textarea">
                  <span>测试文本</span>
                  <textarea
                    id="${testIdPrefix}-text"
                    data-testid="${testIdPrefix}-text"
                    rows="4"
                    placeholder="输入一段测试文本"
                    ${supportedModalities.length ? "" : "disabled"}
                  >${escapeHtml(textDraft)}</textarea>
                </label>
              `
          }
        </div>
        ${
          fileRequired && file
            ? `<p class="helper" data-testid="${testIdPrefix}-file-name">${escapeHtml(file.name)} · ${escapeHtml(file.type || "application/octet-stream")}</p>`
            : ""
        }
        <div class="filter-grid settings-filter-grid">
          <label>
            <span>对照输入模态</span>
            <select
              id="${testIdPrefix}-comparison-modality"
              data-testid="${testIdPrefix}-comparison-modality"
              ${supportedModalities.length ? "" : "disabled"}
            >
              <option value="" ${comparisonModalityDraft ? "" : "selected"}>不启用</option>
              ${supportedModalities
                .map(
                  (modality) => `
                    <option value="${escapeHtml(modality)}" ${
                      modality === comparisonModalityDraft ? "selected" : ""
                    }>
                      ${escapeHtml(modelTestModalityDisplayName(modality))}
                    </option>
                  `
                )
                .join("")}
            </select>
          </label>
          ${
            comparisonModalityDraft
              ? comparisonFileRequired
                ? `
                  <label>
                    <span>${escapeHtml(modelTestFileLabel(comparisonModalityDraft))}</span>
                    <input
                      id="${testIdPrefix}-comparison-file"
                      data-testid="${testIdPrefix}-comparison-file"
                      type="file"
                      accept="${escapeHtml(modelTestFileAccept(comparisonModalityDraft))}"
                      ${supportedModalities.length ? "" : "disabled"}
                    />
                  </label>
                `
                : `
                  <label class="model-test-textarea">
                    <span>对照测试文本</span>
                    <textarea
                      id="${testIdPrefix}-comparison-text"
                      data-testid="${testIdPrefix}-comparison-text"
                      rows="4"
                      placeholder="输入第二个用于比较的文本"
                      ${supportedModalities.length ? "" : "disabled"}
                    >${escapeHtml(comparisonTextDraft)}</textarea>
                  </label>
                `
              : ""
          }
        </div>
        ${
          comparisonFileRequired && comparisonFile
            ? `<p class="helper" data-testid="${testIdPrefix}-comparison-file-name">${escapeHtml(comparisonFile.name)} · ${escapeHtml(comparisonFile.type || "application/octet-stream")}</p>`
            : ""
        }
        ${
          error
            ? `<div class="notice error" data-testid="${testIdPrefix}-error"><h4>${escapeHtml(error.code)}</h4><p>${escapeHtml(error.message)}</p></div>`
            : ""
        }
        <div class="inline-actions">
          <button
            type="submit"
            data-testid="${testIdPrefix}-submit-button"
            ${disabled ? "disabled" : ""}
          >
            ${pending ? "测试中..." : "测试当前模型"}
          </button>
        </div>
      </form>
      ${renderModelTestResult(testIdPrefix, result)}
    </section>
  `;
}

function sourceRootDisplayName(sourceRootId) {
  if (!sourceRootId) {
    return "全部来源根";
  }
  const sourceRoot = state.sourceRoots.find((item) => item.source_root_id === sourceRootId);
  return sourceRoot?.root_path ?? sourceRootId;
}

function sourceRootInventoryLabel(source: SourceInventoryItem) {
  return source.source_root_label || source.source_root_id || "手动导入";
}

function currentWorkspaceMeta() {
  if (state.activeWorkspace === "inventory") {
    return {
      title: "库管理",
      summary: "在独立工作区里管理当前库、核对来源状态并浏览详情。",
    };
  }

  if (state.activeWorkspace === "settings") {
    return {
      title: "设置",
      summary: "在同一处调整内容类型、当前库覆盖、连接、模型测试和诊断信息。",
    };
  }

  return {
    title: "搜索",
    summary: "把建库、导入、搜索、阅读结果和对象复用收束到同一主舞台里完成。",
  };
}

function utilityDrawerSectionLabel(section: UtilityDrawerSection) {
  switch (section) {
    case "jobs":
      return "当前库任务";
    case "source-prep":
      return "来源准备";
    case "maintenance":
      return "运行时 / 维护";
    default:
      return "系统状态";
  }
}

function utilityDrawerToolSection(): UtilityDrawerSection {
  return state.utilityDrawerSection === "status" ? "maintenance" : state.utilityDrawerSection;
}

function renderLibraryOptions(items: LibrarySnapshot[]) {
  return items
    .map(
      (item) => `
        <option value="${escapeHtml(item.id)}" ${item.id === state.selectedLibraryId ? "selected" : ""}>
          ${escapeHtml(
            `${libraryDisplayName(item)} (${item.id})${libraryIsArchived(item) ? " · 已归档" : ""}`
          )}
        </option>
      `
    )
    .join("");
}

function renderManageLibraryPopover(library: LibrarySnapshot | null) {
  if (!library) {
    return "";
  }

  return `
    <details
      class="manage-library-popover"
      data-testid="manage-library-popover"
      ${state.manageLibraryPopoverOpen ? "open" : ""}
    >
      <summary class="secondary-button" data-testid="open-manage-library-button">管理当前库</summary>
      <div class="compact-form stack-form" data-testid="manage-library-card">
        <div class="compact-callout">
          <strong>${escapeHtml(libraryDisplayName(library))}</strong>
          <p class="helper">
            当前稳定标识是 ${escapeHtml(library.id)}。
            ${
              libraryIsArchived(library)
                ? "这个库当前已归档，仍然会保留内容与来源，可以随时恢复。"
                : "这里可以修改显示名称、归档当前库，或直接删除。"
            }
          </p>
        </div>
        <form
          id="rename-library-form"
          class="stack-form"
          data-testid="rename-library-form"
          data-library-rename-form="true"
        >
          <label>
            <span>显示名称</span>
            <input
              id="manage-library-name"
              data-testid="manage-library-name-input"
              data-library-management-display-name-input="true"
              name="manageLibraryDisplayName"
              type="text"
              value="${escapeHtml(state.libraryManagementDisplayNameDraft)}"
              placeholder="例如：季度报告库"
              required
            />
          </label>
          <button type="submit" data-testid="rename-library-button">保存名称</button>
        </form>
        <button
          type="button"
          id="toggle-library-archive-button"
          data-testid="toggle-library-archive-button"
          data-library-archive-action="true"
          class="secondary-button"
        >
          ${libraryIsArchived(library) ? "恢复当前库" : "归档当前库"}
        </button>
        <button
          type="button"
          id="delete-library-button"
          data-testid="delete-library-button"
          data-library-delete-action="true"
          class="secondary-button destructive-button"
        >
          删除当前库
        </button>
      </div>
    </details>
  `;
}

function renderCreateLibraryPopover() {
  return `
    <details
      class="create-library-popover"
      data-testid="create-library-popover"
      ${state.createLibraryPopoverOpen || !state.libraries.length ? "open" : ""}
    >
      <summary class="secondary-button" data-testid="open-create-library-button">新建库</summary>
      <form id="create-library-form" class="stack-form compact-form" data-testid="create-library-form">
        <label>
          <span>显示名称</span>
          <input
            id="library-name"
            data-testid="library-name-input"
            name="libraryDisplayName"
            type="text"
            value="${escapeHtml(state.libraryDisplayNameDraft)}"
            placeholder="例如：季度报告库"
            required
          />
        </label>
        <label>
          <span>自定义库编号（library_id，可选）</span>
          <input
            id="library-id"
            data-testid="library-id-input"
            name="libraryId"
            type="text"
            value="${escapeHtml(state.libraryIdDraft)}"
            placeholder="例如：quarterly-reports"
          />
        </label>
        <button type="submit" data-testid="create-library-button">创建库</button>
      </form>
    </details>
  `;
}

function renderLibraryContextCluster(
  library: LibrarySnapshot | null,
  placement: "search" | "workspace" = "search"
) {
  const activeLibraries = state.libraries.filter((item) => !libraryIsArchived(item));
  const archivedLibraries = state.libraries.filter((item) => libraryIsArchived(item));

  if (placement === "search") {
    return renderSearchScopeBar(library, activeLibraries, archivedLibraries);
  }

  const label = "当前库";
  const selectorLabel = "切换库";
  const identity = library
    ? library.id
    : state.libraries.length
      ? "先选择一个库"
      : "先创建第一个库";
  const summary = library
    ? `${library.id} · 可搜索 ${library.counts.accepted_items} · 待处理 ${library.counts.pending_jobs}`
    : state.libraries.length
      ? "先选择一个库，再继续当前工作。"
      : "先创建第一个库，再开始导入或搜索。";

  return `
    <section
      class="library-context-cluster library-context-cluster-${escapeHtml(placement)}"
      data-testid="library-context-cluster"
    >
      <div class="library-context-head">
        <div class="library-context-copy">
          <span class="scope-label">${escapeHtml(label)}</span>
          <div class="library-context-current" data-testid="current-library-card">
            <strong data-testid="current-library-name">${escapeHtml(library ? libraryDisplayName(library) : state.libraries.length ? "未选择库" : "还没有库")}</strong>
            <span class="helper" data-testid="current-library-id">${escapeHtml(identity)}</span>
            <span class="helper library-context-summary">${escapeHtml(summary)}</span>
            ${
              library
                ? `
                  <span
                    class="pill ${libraryLifecyclePillClass(library)}"
                    data-testid="current-library-lifecycle"
                  >
                    ${escapeHtml(libraryLifecycleLabel(library))}
                  </span>
                `
                : ""
            }
          </div>
        </div>
        <div class="library-context-controls">
          <label class="context-rail-field context-rail-selector">
            <span>${escapeHtml(selectorLabel)}</span>
            <select id="library-select" data-testid="library-select" ${state.libraries.length ? "" : "disabled"}>
              ${
                state.libraries.length
                  ? [
                      activeLibraries.length
                        ? `
                          <optgroup label="活跃库">
                            ${renderLibraryOptions(activeLibraries)}
                          </optgroup>
                        `
                        : "",
                      archivedLibraries.length
                        ? `
                          <optgroup label="已归档">
                            ${renderLibraryOptions(archivedLibraries)}
                          </optgroup>
                        `
                        : "",
                    ]
                      .filter(Boolean)
                      .join("")
                  : `<option value="">还没有库</option>`
              }
            </select>
          </label>
          <div class="library-context-actions">
            ${renderManageLibraryPopover(library)}
            ${renderCreateLibraryPopover()}
          </div>
        </div>
      </div>
    </section>
  `;
}

function renderSearchScopeBar(
  library: LibrarySnapshot | null,
  activeLibraries: LibrarySnapshot[],
  archivedLibraries: LibrarySnapshot[]
) {
  const allLibrariesActive = state.searchScope === "all_libraries";
  const searchButtonDisabled = !library || !canSearchCurrentScope(library);
  const hasAdvancedFilters =
    Boolean(state.searchFilters.pathPrefix.trim()) ||
    Boolean(state.searchFilters.timeRangeStartMsDraft.trim()) ||
    Boolean(state.searchFilters.timeRangeEndMsDraft.trim());
  const hasFilterSelections =
    Boolean(state.searchFilters.visualUnitKind) ||
    Boolean(state.searchFilters.sourceType) ||
    hasAdvancedFilters;

  return `
    <section
      class="library-context-cluster library-context-cluster-search search-scope-bar"
      data-testid="search-scope-bar"
    >
      <div class="search-scope-controls">
        <div class="search-scope-row">
          <div class="search-scope-toggle-group">
            <button
              type="button"
              class="search-scope-toggle ${state.searchScope === "library" ? "active" : ""}"
              data-testid="search-scope-library"
              data-search-scope="library"
            >
              当前库
            </button>
            <button
              type="button"
              class="search-scope-toggle ${allLibrariesActive ? "active" : ""}"
              data-testid="search-scope-all-libraries"
              data-search-scope="all_libraries"
              ${state.libraries.length ? "" : "disabled"}
            >
              所有库
            </button>
          </div>
          <div class="search-scope-actions">
            <label class="context-rail-selector search-scope-selector" aria-label="当前库">
              <select id="library-select" data-testid="library-select" ${state.libraries.length ? "" : "disabled"}>
                ${
                  state.libraries.length
                    ? [
                        activeLibraries.length
                          ? `
                            <optgroup label="活跃库">
                              ${renderLibraryOptions(activeLibraries)}
                            </optgroup>
                          `
                          : "",
                        archivedLibraries.length
                          ? `
                            <optgroup label="已归档">
                              ${renderLibraryOptions(archivedLibraries)}
                            </optgroup>
                          `
                          : "",
                      ]
                        .filter(Boolean)
                        .join("")
                    : `<option value="">还没有库</option>`
                }
              </select>
            </label>
            <button
              type="submit"
              form="search-form"
              class="search-submit-inline"
              data-testid="search-submit-button"
              ${searchButtonDisabled ? "disabled" : ""}
            >
              Search
            </button>
          </div>
        </div>
        ${
          hasFilterSelections
            ? `<p class="helper search-filter-summary" data-testid="search-filter-summary">${escapeHtml(searchFiltersSummary())}</p>`
            : ""
        }
      </div>
    </section>
  `;
}

function renderContextRail(library: LibrarySnapshot | null) {
  const status = currentStatusCapsule(library);
  return `
    <div class="context-rail-shell context-rail-shell-product" data-testid="context-rail">
      <div class="context-rail-brand">
        <p class="eyebrow">FauniSearch</p>
        <div class="context-rail-brand-copy">
          <p class="context-rail-tagline">Unified · Native · Powerful</p>
        </div>
      </div>
      <div class="context-rail-status">
        <button
          type="button"
          class="pill ${status.pillClass} utility-trigger-pill"
          data-testid="utility-drawer-open-status"
          data-utility-drawer-open="status"
          aria-expanded="${state.utilityDrawerOpen && state.utilityDrawerSection === "status" ? "true" : "false"}"
        >
          <span class="status-dot"></span>
          ${escapeHtml(status.label)}
        </button>
      </div>
    </div>
  `;
}

function renderUtilityDrawerStatusSection(library: LibrarySnapshot | null) {
  const status = currentStatusCapsule(library);
  const stageState = currentSearchStageState(library);
  const runtimeOverview = runtimeHealthOverview();
  const providerStatusClass =
    runtimeOverview && runtimeOverview.providerIssues.length
      ? "error"
      : runtimeOverview?.enabledProviders.length
        ? "ready"
        : "muted";

  return `
    <section class="utility-drawer-section" data-testid="utility-drawer-section-status">
      <div class="utility-drawer-section-head">
        <div>
          <p class="eyebrow">系统状态</p>
          <h3>状态摘要</h3>
        </div>
        <span class="pill ${status.pillClass}" data-testid="utility-drawer-status-pill">${escapeHtml(status.label)}</span>
      </div>
      <p class="helper" data-testid="utility-drawer-status-summary">${escapeHtml(status.summary)}</p>
      <div class="status-capsule-stage" data-testid="utility-drawer-stage-state">
        <span class="pill ${stageState.pillClass}">${escapeHtml(stageState.status)}</span>
        <p class="helper">${escapeHtml(stageState.summary)}</p>
      </div>
      ${renderSearchStatusNextStep(library)}
      <dl class="stats compact-stats utility-drawer-stats">
        <div><dt>当前库</dt><dd>${escapeHtml(library ? libraryDisplayName(library) : "未选择")}</dd></div>
        <div><dt>可搜索内容</dt><dd>${escapeHtml(library?.counts.accepted_items ?? 0)}</dd></div>
        <div><dt>后台任务</dt><dd>${escapeHtml(library?.counts.pending_jobs ?? 0)}</dd></div>
      </dl>
      <ul class="status-capsule-runtime-list">
        ${
          runtimeOverview
            ? `
              <li class="status-capsule-runtime-item" data-testid="utility-drawer-runtime-app">
                <div>
                  <strong>${escapeHtml(state.runtimeHealth!.app.display_name)}</strong>
                  <p class="helper">${escapeHtml(state.runtimeHealth!.app.message)}</p>
                </div>
                <span class="pill ${providerSelectionPillClass(state.runtimeHealth!.app.status)}">${escapeHtml(shellRuntimeStatusLabel(state.runtimeHealth!.app.status))}</span>
              </li>
              <li class="status-capsule-runtime-item" data-testid="utility-drawer-runtime-qdrant">
                <div>
                  <strong>${escapeHtml(state.runtimeHealth!.qdrant.display_name)}</strong>
                  <p class="helper">${escapeHtml(state.runtimeHealth!.qdrant.message)}</p>
                </div>
                <span class="pill ${providerSelectionPillClass(state.runtimeHealth!.qdrant.status)}">${escapeHtml(shellRuntimeStatusLabel(state.runtimeHealth!.qdrant.status))}</span>
              </li>
              <li class="status-capsule-runtime-item" data-testid="utility-drawer-runtime-providers">
                <div>
                  <strong>连接</strong>
                  <p class="helper">${escapeHtml(runtimeOverview.summary)}</p>
                </div>
                <span class="pill ${providerStatusClass}">${escapeHtml(
                  runtimeOverview.providerIssues.length
                    ? `${runtimeOverview.providerIssues.length} 受限`
                    : runtimeOverview.enabledProviders.length
                      ? `${runtimeOverview.enabledProviders.length} 可用`
                      : "未启用"
                )}</span>
              </li>
            `
            : `
              <li class="status-capsule-runtime-item" data-testid="utility-drawer-runtime-loading">
                <div>
                  <strong>运行时快照</strong>
                  <p class="helper">当前还没有拿到运行时健康快照，进入诊断页后会显示更完整的状态。</p>
                </div>
                <span class="pill muted">待刷新</span>
              </li>
            `
        }
      </ul>
      <div class="utility-drawer-actions">
        <button
          type="button"
          class="secondary-button"
          data-testid="utility-drawer-open-diagnostics"
          data-workspace="settings"
          data-settings-section="diagnostics"
        >
          前往诊断
        </button>
        <button
          type="button"
          class="secondary-button"
          data-testid="utility-drawer-open-jobs"
          data-utility-drawer-section="jobs"
        >
          查看任务
        </button>
        <button
          type="button"
          class="secondary-button"
          data-testid="utility-drawer-open-source-prep"
          data-utility-drawer-section="source-prep"
        >
          来源准备
        </button>
      </div>
    </section>
  `;
}

function renderSourcePrepSummaryCards(library: LibrarySnapshot | null) {
  if (!library) {
    return `
      <div class="utility-drawer-summary-card">
        <strong>还没有当前库</strong>
        <p class="helper">先在顶部创建或选择一个库，来源准备和导入动作才会接入真实库上下文。</p>
      </div>
    `;
  }

  const activeSourceRoots = state.sourceRoots.filter((item) => item.enabled).length;
  const pendingJobs = library.counts.pending_jobs;
  const lastActionSummary = state.sourceRoots
    .map((item) => item.last_action?.summary)
    .find(Boolean);

  return `
    <div class="utility-drawer-summary-grid">
      <div class="utility-drawer-summary-card">
        <strong>${escapeHtml(libraryDisplayName(library))}</strong>
        <p class="helper">${escapeHtml(library.id)} · 可搜索 ${escapeHtml(library.counts.accepted_items)} · 待处理 ${escapeHtml(pendingJobs)}</p>
      </div>
      <div class="utility-drawer-summary-card">
        <strong>来源根</strong>
        <p class="helper">${escapeHtml(state.sourceRoots.length)} 个来源根，启用中 ${escapeHtml(activeSourceRoots)} 个。</p>
      </div>
      <div class="utility-drawer-summary-card">
        <strong>导入状态</strong>
        <p class="helper">${escapeHtml(lastActionSummary ?? "还没有来源准备动作记录，可从搜索舞台或来源浏览开始。")}</p>
      </div>
    </div>
  `;
}

function renderUtilityDrawerSourcePrepSection(library: LibrarySnapshot | null) {
  return `
    <section class="utility-drawer-section" data-testid="utility-drawer-section-source-prep">
      <div class="utility-drawer-section-head">
        <div>
          <p class="eyebrow">来源准备</p>
          <h3>导入与来源准备</h3>
        </div>
      </div>
      <p class="helper">桌面端把来源准备收拢到统一辅助面里，但真正的导入表单和来源根编辑仍在搜索舞台或来源浏览里完成。</p>
      ${renderSourcePrepSummaryCards(library)}
      <div class="utility-drawer-actions utility-drawer-actions-stacked">
        <button
          type="button"
          class="secondary-button"
          data-testid="utility-drawer-focus-search-prep"
          data-utilities-action="focus-source-prep"
          ${library ? "" : "disabled"}
        >
          在搜索舞台打开来源准备
        </button>
        <button
          type="button"
          class="secondary-button"
          data-testid="utility-drawer-open-inventory"
          data-workspace="inventory"
          ${library ? "" : "disabled"}
        >
          前往来源浏览
        </button>
        <button
          type="button"
          class="secondary-button"
          data-testid="utility-drawer-open-diagnostics-from-source-prep"
          data-workspace="settings"
          data-settings-section="diagnostics"
          ${library ? "" : "disabled"}
        >
          前往诊断
        </button>
      </div>
    </section>
  `;
}

function renderUtilityDrawerJobsSection(library: LibrarySnapshot | null) {
  const shouldInlineJobs = !shouldRenderSearchNextStepDock(library);

  return `
    <section class="utility-drawer-section" data-testid="utility-drawer-section-jobs">
      <div class="utility-drawer-section-head">
        <div>
          <p class="eyebrow">当前库任务</p>
          <h3>任务面板</h3>
        </div>
        ${
          library
            ? `<span class="pill ${library.counts.pending_jobs > 0 ? "pending" : "muted"}">${escapeHtml(
                library.counts.pending_jobs > 0 ? `${library.counts.pending_jobs} 进行中` : "无进行中任务"
              )}</span>`
            : ""
        }
      </div>
      ${
        shouldInlineJobs
          ? renderJobs()
          : `
            <div class="utility-drawer-summary-card">
              <strong>任务仍在准备流里可见</strong>
              <p class="helper">当前搜索舞台已经显示下一步引导和任务回执。为了避免重复占位，这里保留工作区跳转入口。</p>
            </div>
          `
      }
      <div class="utility-drawer-actions">
        <button
          type="button"
          class="secondary-button"
          data-testid="utility-drawer-focus-search-jobs"
          data-utilities-action="focus-search-jobs"
          ${library ? "" : "disabled"}
        >
          在搜索舞台打开任务
        </button>
        <button
          type="button"
          class="secondary-button"
          data-testid="utility-drawer-open-diagnostics-from-jobs"
          data-workspace="settings"
          data-settings-section="diagnostics"
          ${library ? "" : "disabled"}
        >
          前往诊断
        </button>
      </div>
    </section>
  `;
}

function renderUtilityDrawerMaintenanceSection(library: LibrarySnapshot | null) {
  const retiredVectorSpaces = retiredVectorSpaceDiagnostics();

  return `
    <section class="utility-drawer-section" data-testid="utility-drawer-section-maintenance">
      <div class="utility-drawer-section-head">
        <div>
          <p class="eyebrow">运行时 / 维护</p>
          <h3>工具动作</h3>
        </div>
      </div>
      <div class="utility-drawer-summary-grid">
        <div class="utility-drawer-summary-card">
          <strong>当前库维护</strong>
          <p class="helper">${escapeHtml(
            library
              ? `${libraryDisplayName(library)} 当前可执行刷新、重扫、重建和退役执行空间清理。`
              : "先选择一个库，再执行库级维护动作。"
          )}</p>
        </div>
        <div class="utility-drawer-summary-card">
          <strong>退役执行空间</strong>
          <p class="helper">${escapeHtml(
            retiredVectorSpaces.length
              ? `${retiredVectorSpaces.length} 个退役执行空间可立即清理。`
              : "当前没有可清理的退役执行空间。"
          )}</p>
        </div>
      </div>
      <div class="utility-drawer-actions utility-drawer-actions-stacked">
        <button
          type="button"
          class="secondary-button"
          data-testid="utility-drawer-open-runtime-health"
          data-workspace="settings"
          data-settings-section="diagnostics"
        >
          查看运行时健康
        </button>
        <button type="button" class="secondary-button" data-testid="utility-drawer-refresh-library" data-utilities-action="refresh-library" ${library ? "" : "disabled"}>刷新当前库</button>
        <button type="button" class="secondary-button" data-testid="utility-drawer-rescan-library" data-utilities-action="rescan-library" ${library ? "" : "disabled"}>重扫当前库</button>
        <button type="button" class="secondary-button" data-testid="utility-drawer-rebuild-library" data-utilities-action="rebuild-library" ${library ? "" : "disabled"}>重建当前库</button>
        <button type="button" class="secondary-button" data-testid="utility-drawer-cleanup-retired-vector-spaces" data-utilities-action="cleanup-retired-vector-spaces" ${library && retiredVectorSpaces.length ? "" : "disabled"}>清理退役执行空间</button>
      </div>
    </section>
  `;
}

function renderUtilityDrawer(library: LibrarySnapshot | null) {
  if (!state.utilityDrawerOpen) {
    return "";
  }

  const section =
    state.utilityDrawerSection === "jobs"
      ? renderUtilityDrawerJobsSection(library)
      : state.utilityDrawerSection === "source-prep"
        ? renderUtilityDrawerSourcePrepSection(library)
        : state.utilityDrawerSection === "maintenance"
          ? renderUtilityDrawerMaintenanceSection(library)
          : renderUtilityDrawerStatusSection(library);

  return `
    <aside
      class="panel utility-drawer"
      data-testid="utility-drawer"
      data-drawer-section="${escapeHtml(state.utilityDrawerSection)}"
    >
      <div class="utility-drawer-head">
        <div>
          <p class="eyebrow">辅助面</p>
          <h2>${escapeHtml(utilityDrawerSectionLabel(state.utilityDrawerSection))}</h2>
        </div>
        <button
          type="button"
          class="secondary-button utility-drawer-close"
          data-testid="utility-drawer-close"
          data-utility-drawer-close="true"
        >
          收起
        </button>
      </div>
      <nav class="utility-drawer-nav" data-testid="utility-drawer-nav" aria-label="辅助面分段">
        ${(["status", "jobs", "source-prep", "maintenance"] as const)
          .map(
            (sectionId) => `
              <button
                type="button"
                class="secondary-button utility-drawer-tab ${state.utilityDrawerSection === sectionId ? "active" : ""}"
                data-testid="utility-drawer-tab-${escapeHtml(sectionId)}"
                data-utility-drawer-section="${escapeHtml(sectionId)}"
              >
                ${escapeHtml(utilityDrawerSectionLabel(sectionId))}
              </button>
            `
          )
          .join("")}
      </nav>
      <div class="utility-drawer-body">
        ${section}
      </div>
    </aside>
  `;
}

function renderSearchStateStrip(library: LibrarySnapshot | null) {
  const stageState = currentSearchStageState(library);
  if (stageState.status === "可搜索") {
    return "";
  }
  return `
    <div class="search-state-strip" data-testid="search-state-strip">
      <span class="pill ${stageState.pillClass}">${escapeHtml(stageState.status)}</span>
      <p class="helper">${escapeHtml(stageState.summary)}</p>
    </div>
  `;
}

function renderSearchLoadingNotice() {
  if (!state.searchInFlight) {
    return "";
  }

  return `
    <div class="search-results-loading" data-testid="search-loading-notice">
      <p class="helper">搜索中...</p>
    </div>
  `;
}

function shouldRenderSearchNextStepDock(library: LibrarySnapshot | null) {
  if (!library) {
    return true;
  }

  if (libraryNeedsPreparation(library)) {
    return true;
  }

  return (
    state.searchPreparationDisclosureOpen ||
    state.searchJobsDisclosureOpen ||
    Boolean(state.editingSourceRootId)
  );
}

function renderSearchNextStepDock(library: LibrarySnapshot | null) {
  if (!shouldRenderSearchNextStepDock(library)) {
    return "";
  }

  if (!library) {
    return `
      <aside class="next-step-dock" data-testid="search-next-step-dock">
        <p class="eyebrow">下一步</p>
        <h3>先去库管理</h3>
        <p class="helper">新建库和当前库管理已经移到库管理工作区；先完成库准备，再回到 Search 发起查询。</p>
        <div class="dock-note">
          <strong>主路径</strong>
          <p class="helper">库管理 → 导入内容 → 等待可搜索 → 发起搜索。</p>
        </div>
        <div class="dock-actions">
          <button
            type="button"
            data-testid="search-next-step-open-inventory"
            data-workspace="inventory"
          >
            前往库管理
          </button>
        </div>
      </aside>
    `;
  }

  const allLibrariesScope = allLibrariesTextScopeActive();
  const readiness = libraryOperationalReadiness(library);
  const scopeState = currentSearchScopeStageState(library);
  const nextAction = scopeState.nextAction;
  const supportDisclosures = renderSearchSupportDisclosures(library, nextAction);
  const dockFacts = allLibrariesScope
    ? [
        `覆盖 ${scopeState.totalLibraries} 个库`,
        `可搜索库 ${scopeState.searchableLibraries}`,
        `对象 ${scopeState.searchableUnits}`,
        `准备中 ${scopeState.pendingLibraries}`,
      ]
    : [
        `启用来源根 ${readiness.enabledRoots}`,
        `可搜索 ${readiness.searchableUnits}`,
        `待处理 ${readiness.pendingJobs}`,
      ];
  const readinessNote = `
    <div class="dock-note">
      <strong>${escapeHtml(allLibrariesScope ? "当前范围" : libraryDisplayName(library))}</strong>
      <p class="helper">${escapeHtml(dockFacts.join(" · "))}</p>
      ${
        !allLibrariesScope && readiness.lastActionSummary
          ? `<p class="helper">${escapeHtml(readiness.lastActionSummary)}</p>`
          : ""
      }
    </div>
  `;

  if (nextAction === "settings") {
    return `
      <aside class="next-step-dock" data-testid="search-next-step-dock">
        <p class="eyebrow">下一步</p>
        <h3>${escapeHtml(allLibrariesScope ? "先完成一个库的搜索配置" : "检查当前库覆盖")}</h3>
        <p class="helper">${escapeHtml(scopeState.summary)}</p>
        <div class="dock-note">
          <strong>配置状态</strong>
          <p class="helper">${escapeHtml(
            allLibrariesScope
              ? "所有库范围里还没有可搜索库；先让至少一个库的内容类型与 resolved model 完成就绪。"
              : `${readiness.blockedContentTypes} 个启用内容类型仍未就绪；先确认当前库覆盖、连接与 resolved model。`
          )}</p>
        </div>
        <div class="dock-actions">
          <button
            type="button"
            data-testid="search-next-step-open-library-overrides"
            data-open-settings-section="library-overrides"
          >
            前往当前库覆盖
          </button>
        </div>
        ${renderProviderBridge(library)}
        ${supportDisclosures}
      </aside>
    `;
  }

  if (nextAction === "jobs") {
    return `
      <aside class="next-step-dock" data-testid="search-next-step-dock">
        <p class="eyebrow">下一步</p>
        <h3>${escapeHtml(allLibrariesScope ? "等待至少一个库准备完成" : "等待当前任务完成")}</h3>
        <p class="helper">${escapeHtml(scopeState.summary)}</p>
        ${readinessNote}
        <div class="dock-actions">
          <button
            type="button"
            data-testid="search-next-step-open-jobs"
            data-utilities-action="focus-search-jobs"
          >
            查看任务
          </button>
          <button
            type="button"
            class="secondary-button"
            data-testid="search-next-step-open-source-prep"
            data-utilities-action="focus-source-prep"
          >
            打开来源准备
          </button>
        </div>
        ${supportDisclosures}
      </aside>
    `;
  }

  if (libraryNeedsPreparation(library)) {
    const title =
      allLibrariesScope
        ? "让至少一个库进入可搜索状态"
        : readiness.status === "尚未接入来源根"
        ? "接入第一个来源根"
        : readiness.status === "来源根已停用"
          ? "恢复一个来源根"
          : readiness.status === "需要关注"
            ? "先检查来源根健康"
            : readiness.status === "观察未稳定"
              ? "恢复来源观察"
              : "准备第一批内容";
    return `
      <aside class="next-step-dock" data-testid="search-next-step-dock">
        <p class="eyebrow">下一步</p>
        <h3>${escapeHtml(title)}</h3>
        <p class="helper">${escapeHtml(scopeState.summary)}</p>
        ${readinessNote}
        <div class="dock-actions">
          <button
            type="button"
            data-testid="search-next-step-open-source-prep"
            data-utilities-action="focus-source-prep"
          >
            打开来源准备
          </button>
          <button
            type="button"
            class="secondary-button"
            data-testid="search-next-step-open-inventory"
            data-workspace="inventory"
          >
            前往库管理
          </button>
        </div>
        ${renderInventoryBridge(library)}
        ${renderProviderBridge(library)}
        ${supportDisclosures}
      </aside>
    `;
  }

  return `
    <aside class="next-step-dock" data-testid="search-next-step-dock">
      <p class="eyebrow">${escapeHtml(allLibrariesScope ? "当前范围" : "当前库")}</p>
      <h3>${escapeHtml(allLibrariesScope ? "所有库" : libraryDisplayName(library))}</h3>
      <p class="helper">${escapeHtml(
        allLibrariesScope
          ? scopeState.summary
          : `${library.id} · 可搜索 ${library.counts.accepted_items} · 待处理 ${library.counts.pending_jobs}`
      )}</p>
      <div class="dock-note">
        <strong>建议</strong>
        <p class="helper">${escapeHtml(
          allLibrariesScope
            ? "先发起一轮跨库文本查询，再从结果卡或详情面继续下钻到命中库。"
            : "先搜一轮，再从结果卡或详情面直接复用对象作为下一次查询输入。"
        )}</p>
      </div>
      <div class="dock-actions">
        <button
          type="button"
          class="secondary-button"
          data-testid="search-next-step-open-source-prep"
          data-utilities-action="focus-source-prep"
        >
          打开来源准备
        </button>
      </div>
      ${renderInventoryBridge(library)}
      ${renderProviderBridge(library)}
      ${supportDisclosures}
    </aside>
  `;
}

function renderSearchSupportDisclosures(library: LibrarySnapshot | null, nextAction = searchStageNextAction(library)) {
  if (!library) {
    return "";
  }

  const preparationOpen =
    state.searchPreparationDisclosureOpen ||
    Boolean(state.editingSourceRootId) ||
    nextAction === "source-prep";
  const showJobs = Boolean(state.importReceipt) || library.counts.pending_jobs > 0 || nextAction === "jobs";
  const jobsOpen = state.searchJobsDisclosureOpen || library.counts.pending_jobs > 0 || nextAction === "jobs";

  return `
    <div class="next-step-support">
      <details
        id="search-preparation-disclosure"
        class="support-disclosure support-disclosure-subtle"
        ${preparationOpen ? "open" : ""}
      >
        <summary>导入与来源准备</summary>
        <div class="support-disclosure-body">
          ${renderImportPanel(library)}
          ${renderSourceRootsPanel(library)}
        </div>
      </details>
      ${
        showJobs
          ? `
            <details
              id="search-jobs-disclosure"
              class="support-disclosure support-disclosure-subtle"
              ${jobsOpen ? "open" : ""}
            >
              <summary>任务与回执</summary>
              <div class="support-disclosure-body">
                <section class="panel panel-tight utility-panel">
                  <div class="panel-head">
                    <div>
                      <p class="eyebrow">任务</p>
                      <h2>任务面板</h2>
                    </div>
                  </div>
                  ${renderJobs()}
                </section>
              </div>
            </details>
          `
          : ""
      }
    </div>
  `;
}

function renderImportPanel(library: LibrarySnapshot | null) {
  return `
    <section class="panel panel-tight utility-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">导入</p>
          <h2>导入内容</h2>
        </div>
      </div>
      <form id="import-form" class="stack-form" data-testid="import-form">
        <label>
          <span>本地路径</span>
          <textarea
            id="import-paths"
            data-testid="import-paths-input"
            rows="6"
            placeholder="/path/to/file.pdf&#10;/path/to/image.png"
            ${library ? "" : "disabled"}
          >${escapeHtml(state.importPathsDraft)}</textarea>
        </label>
        <button type="submit" data-testid="import-submit-button" ${library ? "" : "disabled"}>提交导入</button>
      </form>
      <p class="helper">当前仍以服务器可读的本地路径作为正式导入入口；逐行填写文件或目录路径后即可提交导入。</p>
      ${renderImportReceipt()}
    </section>
  `;
}

function sleep(ms) {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function isTerminalJobStatus(status) {
  return ["completed", "failed", "canceled"].includes(status);
}

function jobPillClass(status) {
  if (status === "completed") {
    return "ready";
  }
  if (status === "failed" || status === "canceled") {
    return "error";
  }
  if (status === "queued" || status === "running") {
    return "pending";
  }
  return "muted";
}

function canCancelJob(job: JobSnapshot) {
  return job.cancelable && !isTerminalJobStatus(job.status);
}

function canRetryJob(job: JobSnapshot) {
  return job.retryable && (job.status === "failed" || job.status === "canceled");
}

function canResumeJob(job: JobSnapshot) {
  return job.retryable && (job.status === "failed" || job.status === "canceled");
}

function formatJobAttemptLabel(job: JobSnapshot) {
  const parts = [`第 ${job.current_attempt.attempt} 次尝试`];
  if (job.retried_from_job_id) {
    parts.push(`重试自 ${job.retried_from_job_id}`);
  }
  return parts.join(" · ");
}

function selectedVisualUnitId() {
  const visualUnitId = state.selectedVisualUnit?.visual_unit?.visual_unit_id ?? null;
  const libraryId = selectedVisualUnitOriginLibraryId();
  if (!visualUnitId || !libraryId) {
    return visualUnitId;
  }
  return `${libraryId}:${visualUnitId}`;
}

function sourceName(path) {
  return String(path).split(/[/\\]/).pop() ?? path;
}

function pageLabel(locator) {
  return locator?.page_label ?? (locator?.page ? `P${locator.page}` : null);
}

function videoLabel(locator) {
  if (typeof locator?.start_ms !== "number" || typeof locator?.end_ms !== "number") {
    return null;
  }
  return `${formatDurationMs(locator.start_ms)} → ${formatDurationMs(locator.end_ms)}`;
}

function formatScore(score) {
  if (typeof score !== "number" || Number.isNaN(score)) {
    return null;
  }
  return score.toFixed(4);
}

function clearQueryImageState() {
  if (state.queryImageObjectUrl) {
    URL.revokeObjectURL(state.queryImageObjectUrl);
  }
  state.queryImageFile = null;
  state.queryImageObjectUrl = null;
  state.queryImageAsset = null;
  state.queryImageLibraryObject = null;
}

function normalizeQueryImageFile(file, fallbackName = "pasted-image.png") {
  if (file.name && file.name.trim()) {
    return file;
  }

  return new File([file], fallbackName, {
    type: file.type || "image/png",
    lastModified: Date.now(),
  });
}

function setPendingQueryImageFile(file) {
  clearQueryImageState();
  state.queryImageFile = normalizeQueryImageFile(file);
  state.queryImageObjectUrl = URL.createObjectURL(state.queryImageFile);
}

function clearQueryVideoState() {
  if (state.queryVideoObjectUrl) {
    URL.revokeObjectURL(state.queryVideoObjectUrl);
  }
  state.queryVideoFile = null;
  state.queryVideoObjectUrl = null;
  state.queryVideoAsset = null;
  state.queryVideoSource = null;
  state.queryVideoLibraryObject = null;
  state.queryVideoDurationMs = null;
  state.queryVideoRange = null;
}

function normalizeQueryVideoFile(file, fallbackName = "query-video.mp4") {
  if (file.name && file.name.trim()) {
    return file;
  }

  return new File([file], fallbackName, {
    type: file.type || "video/mp4",
    lastModified: Date.now(),
  });
}

function setQueryVideoDuration(durationMs) {
  if (typeof durationMs !== "number" || Number.isNaN(durationMs) || durationMs <= 0) {
    return;
  }

  const normalizedDurationMs = Math.max(Math.round(durationMs), 1);
  state.queryVideoDurationMs = normalizedDurationMs;
  if (!state.queryVideoRange) {
    return;
  }

  const startMs = Math.max(
    0,
    Math.min(state.queryVideoRange.start_ms ?? 0, normalizedDurationMs - 1)
  );
  const endMs = Math.max(
    startMs + 1,
    Math.min(state.queryVideoRange.end_ms ?? normalizedDurationMs, normalizedDurationMs)
  );

  if (startMs === 0 && endMs === normalizedDurationMs) {
    state.queryVideoRange = null;
    return;
  }

  state.queryVideoRange = {
    start_ms: startMs,
    end_ms: endMs,
  };
}

function setPendingQueryVideoFile(file) {
  clearQueryVideoState();
  state.queryVideoFile = normalizeQueryVideoFile(file);
  state.queryVideoObjectUrl = URL.createObjectURL(state.queryVideoFile);
}

function clearQueryDocumentState() {
  if (state.queryDocumentObjectUrl) {
    URL.revokeObjectURL(state.queryDocumentObjectUrl);
  }
  state.queryDocumentFile = null;
  state.queryDocumentObjectUrl = null;
  state.queryDocumentAsset = null;
  state.queryDocumentLibraryObject = null;
  state.queryDocumentPageCount = null;
  state.queryDocumentStartPageDraft = "";
  state.queryDocumentEndPageDraft = "";
}

function normalizeQueryDocumentFile(file, fallbackName = "query-document.pdf") {
  if (file.name && file.name.trim()) {
    return file;
  }

  return new File([file], fallbackName, {
    type: file.type || "application/pdf",
    lastModified: Date.now(),
  });
}

function setQueryDocumentPageCount(pageCount) {
  if (typeof pageCount === "number" && Number.isFinite(pageCount) && pageCount > 0) {
    state.queryDocumentPageCount = Math.max(1, Math.round(pageCount));
    return;
  }
  state.queryDocumentPageCount = null;
}

function setPendingQueryDocumentFile(file) {
  clearQueryDocumentState();
  state.queryDocumentFile = normalizeQueryDocumentFile(file);
  state.queryDocumentObjectUrl = URL.createObjectURL(state.queryDocumentFile);
}

function setLibraryQueryDocumentVisualUnit(visualUnit: LibraryObjectQueryDocument) {
  clearQueryDocumentState();
  state.queryDocumentLibraryObject = visualUnit;
}

function setLibraryQueryVideoSource(source: VideoSourceItem) {
  clearQueryVideoState();
  state.queryVideoSource = source;
  setQueryVideoDuration(source?.duration_ms ?? null);
}

function setLibraryQueryVideoVisualUnit(visualUnit: LibraryObjectQueryVideo) {
  clearQueryVideoState();
  state.queryVideoLibraryObject = visualUnit;
  setQueryVideoDuration(
    visualUnit?.locator?.duration_ms ??
      (typeof visualUnit?.locator?.end_ms === "number" ? visualUnit.locator.end_ms : null)
  );
}

function probeVideoDurationFromUrl(url) {
  return new Promise((resolve, reject) => {
    const video = document.createElement("video");
    video.preload = "metadata";
    video.src = url;
    video.onloadedmetadata = () => {
      if (!Number.isFinite(video.duration) || video.duration <= 0) {
        reject(new Error("video_duration_unavailable"));
        return;
      }
      resolve(Math.round(video.duration * 1000));
    };
    video.onerror = () => reject(new Error("video_metadata_load_failed"));
  });
}

function firstClipboardImageFile(clipboardData: DataTransfer | null | undefined) {
  if (!clipboardData) {
    return null;
  }

  const fileList = Array.from(clipboardData.files ?? []);
  const directFile = fileList.find((file) => file?.type?.startsWith("image/"));
  if (directFile) {
    return directFile;
  }

  for (const item of Array.from(clipboardData.items ?? [])) {
    if (item.kind === "file" && item.type?.startsWith("image/")) {
      const file = item.getAsFile();
      if (file) {
        return file;
      }
    }
  }

  return null;
}

function formatDurationMs(durationMs) {
  if (typeof durationMs !== "number" || Number.isNaN(durationMs) || durationMs < 0) {
    return null;
  }

  const totalMs = Math.round(durationMs);
  const hours = Math.floor(totalMs / 3_600_000);
  const minutes = Math.floor((totalMs % 3_600_000) / 60_000);
  const seconds = Math.floor((totalMs % 60_000) / 1000);
  const milliseconds = totalMs % 1000;
  const mm = String(minutes).padStart(hours ? 2 : 1, "0");
  const ss = String(seconds).padStart(2, "0");
  const mmm = String(milliseconds).padStart(3, "0");

  if (hours) {
    return `${hours}:${mm}:${ss}.${mmm}`;
  }
  return `${minutes}:${ss}.${mmm}`;
}

function queryImagePreviewUrl() {
  return (
    state.queryImageObjectUrl ??
    state.queryImageAsset?.preview?.url ??
    state.queryImageLibraryObject?.preview?.url ??
    null
  );
}

function queryImageStatusLabel() {
  if (state.queryImageLibraryObject) {
    return `库内对象 · ${state.queryImageLibraryObject.visual_unit_id}`;
  }
  if (state.queryImageAsset) {
    return `已上传 · ${state.queryImageAsset.temp_asset_id}`;
  }
  if (state.queryImageFile) {
    return "待上传";
  }
  return "未选择";
}

function queryImageDisplayName() {
  if (state.queryImageFile) {
    return state.queryImageFile.name;
  }
  if (state.queryImageAsset?.original_filename) {
    return state.queryImageAsset.original_filename;
  }
  if (state.queryImageLibraryObject?.source_path) {
    return sourceName(state.queryImageLibraryObject.source_path);
  }
  return null;
}

function activeQueryImagePreview(): PreviewReference | null {
  return state.queryImageAsset?.preview ?? state.queryImageLibraryObject?.preview ?? null;
}

function isDocumentPageQueryImage() {
  return state.queryImageLibraryObject?.kind === "document_page";
}

function queryVideoPreviewUrl() {
  return (
    state.queryVideoObjectUrl ??
    state.queryVideoAsset?.preview?.url ??
    state.queryVideoSource?.preview?.url ??
    state.queryVideoLibraryObject?.preview?.url ??
    null
  );
}

function queryVideoStatusLabel() {
  if (state.queryVideoLibraryObject) {
    return `库内片段 · ${state.queryVideoLibraryObject.visual_unit_id}`;
  }
  if (state.queryVideoSource) {
    return `库内视频 · ${state.queryVideoSource.source_id}`;
  }
  if (state.queryVideoAsset) {
    return `已上传 · ${state.queryVideoAsset.temp_asset_id}`;
  }
  if (state.queryVideoFile) {
    return "待上传";
  }
  return "未选择";
}

function queryVideoDisplayName() {
  if (state.queryVideoFile) {
    return state.queryVideoFile.name;
  }
  if (state.queryVideoAsset?.original_filename) {
    return state.queryVideoAsset.original_filename;
  }
  if (state.queryVideoLibraryObject?.source_path) {
    return sourceName(state.queryVideoLibraryObject.source_path);
  }
  if (state.queryVideoSource?.source_path) {
    return sourceName(state.queryVideoSource.source_path);
  }
  return null;
}

function activeQueryVideoPreview(): PreviewReference | null {
  return (
    state.queryVideoAsset?.preview ??
    state.queryVideoSource?.preview ??
    state.queryVideoLibraryObject?.preview ??
    null
  );
}

function currentQueryVideoStartMs() {
  if (typeof state.queryVideoLibraryObject?.locator?.start_ms === "number") {
    return state.queryVideoLibraryObject.locator.start_ms;
  }
  return state.queryVideoRange?.start_ms ?? 0;
}

function currentQueryVideoEndMs() {
  if (typeof state.queryVideoLibraryObject?.locator?.end_ms === "number") {
    return state.queryVideoLibraryObject.locator.end_ms;
  }
  return state.queryVideoRange?.end_ms ?? state.queryVideoDurationMs ?? 0;
}

function queryVideoRangeSummary() {
  if (!state.queryVideoDurationMs) {
    return "加载视频后可选择时间范围。";
  }

  if (state.queryVideoLibraryObject) {
    return `库内片段 · ${formatDurationMs(currentQueryVideoStartMs())} → ${formatDurationMs(
      currentQueryVideoEndMs()
    )}`;
  }

  if (!state.queryVideoRange) {
    return `整段视频 · 0 → ${formatDurationMs(state.queryVideoDurationMs)}`;
  }

  return `${formatDurationMs(currentQueryVideoStartMs())} → ${formatDurationMs(
    currentQueryVideoEndMs()
  )}`;
}

function queryVideoLocatorPayload() {
  if (state.queryVideoLibraryObject?.locator) {
    return state.queryVideoLibraryObject.locator;
  }
  if (!state.queryVideoDurationMs || !state.queryVideoRange) {
    return null;
  }

  const startMs = Math.max(0, currentQueryVideoStartMs());
  const endMs = Math.min(currentQueryVideoEndMs(), state.queryVideoDurationMs);
  if (startMs <= 0 && endMs >= state.queryVideoDurationMs) {
    return null;
  }

  return {
    start_ms: startMs,
    end_ms: endMs,
  };
}

function queryVideoRangeStep() {
  if (!state.queryVideoDurationMs) {
    return 250;
  }
  if (state.queryVideoDurationMs <= 10_000) {
    return 100;
  }
  if (state.queryVideoDurationMs <= 60_000) {
    return 250;
  }
  return 1000;
}

function queryDocumentPreviewUrl() {
  return (
    state.queryDocumentObjectUrl ??
    state.queryDocumentAsset?.preview?.url ??
    state.queryDocumentLibraryObject?.preview?.url ??
    null
  );
}

function queryDocumentStatusLabel() {
  if (state.queryDocumentLibraryObject) {
    return `库内页面 · ${state.queryDocumentLibraryObject.visual_unit_id}`;
  }
  if (state.queryDocumentAsset) {
    return `已上传 · ${state.queryDocumentAsset.temp_asset_id}`;
  }
  if (state.queryDocumentFile) {
    return "待上传";
  }
  return "未选择";
}

function queryDocumentDisplayName() {
  if (state.queryDocumentFile) {
    return state.queryDocumentFile.name;
  }
  if (state.queryDocumentAsset?.original_filename) {
    return state.queryDocumentAsset.original_filename;
  }
  if (state.queryDocumentLibraryObject?.source_path) {
    return sourceName(state.queryDocumentLibraryObject.source_path);
  }
  return null;
}

function activeQueryDocumentPreview(): PreviewReference | null {
  return state.queryDocumentAsset?.preview ?? state.queryDocumentLibraryObject?.preview ?? null;
}

function currentQueryDocumentStartPage() {
  if (state.queryDocumentLibraryObject?.locator?.start_page != null) {
    return state.queryDocumentLibraryObject.locator.start_page;
  }
  return state.queryDocumentStartPageDraft;
}

function currentQueryDocumentEndPage() {
  if (state.queryDocumentLibraryObject?.locator?.end_page != null) {
    return state.queryDocumentLibraryObject.locator.end_page;
  }
  return state.queryDocumentEndPageDraft;
}

function queryDocumentRangeSummary() {
  if (state.queryDocumentLibraryObject?.locator?.start_page != null) {
    const page = state.queryDocumentLibraryObject.locator.start_page;
    return `库内页面 · P${page}`;
  }

  if (!state.queryDocumentStartPageDraft && !state.queryDocumentEndPageDraft) {
    return state.queryDocumentPageCount
      ? `整份文档 · 共 ${state.queryDocumentPageCount} 页`
      : "整份文档";
  }

  if (state.queryDocumentStartPageDraft && !state.queryDocumentEndPageDraft) {
    return `单页 · P${state.queryDocumentStartPageDraft}`;
  }

  return `页范围 · P${state.queryDocumentStartPageDraft} → P${state.queryDocumentEndPageDraft}`;
}

function syncQueryDocumentRangeUi() {
  const summary = document.querySelector("#query-document-range-summary");
  if (summary) {
    summary.textContent = queryDocumentRangeSummary();
  }

  const clearButton = document.querySelector("#clear-query-document-range-button");
  if (clearButton instanceof HTMLButtonElement) {
    clearButton.disabled =
      Boolean(state.queryDocumentLibraryObject) ||
      (!state.queryDocumentStartPageDraft && !state.queryDocumentEndPageDraft);
  }
}

function queryDocumentLocatorPayload() {
  if (state.queryDocumentLibraryObject?.locator) {
    return state.queryDocumentLibraryObject.locator;
  }

  const startDraft = String(state.queryDocumentStartPageDraft ?? "").trim();
  const endDraft = String(state.queryDocumentEndPageDraft ?? "").trim();
  if (!startDraft && !endDraft) {
    return null;
  }
  if (!startDraft) {
    throw {
      code: "validation_failed",
      message: "指定文档页范围时必须先填写起始页。",
    };
  }

  const startPage = Math.trunc(Number(startDraft));
  const endPage = endDraft ? Math.trunc(Number(endDraft)) : startPage;
  if (!Number.isFinite(startPage) || startPage < 1) {
    throw {
      code: "validation_failed",
      message: "起始页必须是大于等于 1 的整数。",
    };
  }
  if (!Number.isFinite(endPage) || endPage < startPage) {
    throw {
      code: "validation_failed",
      message: "结束页必须是大于等于起始页的整数。",
    };
  }

  return {
    start_page: startPage,
    end_page: endPage,
  };
}

function renderStatusNotices() {
  const blocks = [];

  if (state.globalError) {
    blocks.push(`
      <div class="notice error">
        <h4>${escapeHtml(state.globalError.code ?? "error")}</h4>
        <p>${escapeHtml(state.globalError.message ?? state.globalError)}</p>
      </div>
    `);
  }

  if (state.statusMessage) {
    blocks.push(`
      <div class="notice success">
        <h4>进行中</h4>
        <p>${escapeHtml(state.statusMessage)}</p>
      </div>
    `);
  }

  if (!blocks.length) {
    return "";
  }

  return `<section class="status-stack">${blocks.join("")}</section>`;
}

function renderImportReceipt() {
  if (!state.importReceipt) {
    return '<p class="empty" data-testid="import-receipt-empty">还没有导入回执。提交路径后会在这里显示接受和拒绝结果。</p>';
  }

  const accepted = state.importReceipt.accepted.length
    ? `
        <div class="receipt-group" data-testid="import-accepted-group">
          <h4>已接受</h4>
          <ul class="data-list">
            ${state.importReceipt.accepted
              .map(
                (item) => `
                  <li>
                    <div class="list-head">
                      <strong>${escapeHtml(visualUnitKindDisplayName(item.kind))}</strong>
                      <span class="helper">${(item.visual_units ?? []).length} 个可搜索对象</span>
                    </div>
                    <span>${escapeHtml(item.normalized_path ?? item.original_path)}</span>
                    ${
                      item.visual_units?.length
                        ? `<div class="inline-actions">
                            ${item.visual_units
                              .map(
                                (visualUnit) => `
                                  <button
                                    type="button"
                                    class="secondary-button"
                                    data-visual-unit-id="${escapeHtml(visualUnit.visual_unit_id)}"
                                  >
                                    查看 ${escapeHtml(visualUnitKindDisplayName(visualUnit.kind))} · ${escapeHtml(visualUnit.visual_unit_id)}
                                  </button>
                                `
                              )
                              .join("")}
                          </div>`
                        : ""
                    }
                  </li>
                `
              )
              .join("")}
          </ul>
        </div>
      `
    : "";

  const rejected = state.importReceipt.rejected.length
    ? `
        <div class="receipt-group" data-testid="import-rejected-group">
          <h4>已拒绝</h4>
          <ul class="data-list">
            ${state.importReceipt.rejected
              .map(
                (item) => `
                  <li data-testid="import-rejected-item" data-reason-code="${escapeHtml(item.reason_code)}">
                    <strong data-testid="import-rejected-reason">${escapeHtml(item.reason_code)}</strong>
                    <span>${escapeHtml(item.original_path)} · ${escapeHtml(item.message)}</span>
                  </li>
                `
              )
              .join("")}
          </ul>
        </div>
      `
    : "";

  const jobSummary = state.importReceipt.job
    ? `<p class="helper" data-testid="import-job-summary">任务 ${escapeHtml(state.importReceipt.job.job_id)} 当前处于 ${escapeHtml(state.importReceipt.job.phase)}。</p>`
    : `<p class="helper" data-testid="import-no-job">这次提交没有创建后台任务。</p>`;

  return `<div data-testid="import-receipt">${accepted}${rejected}${jobSummary}</div>`;
}

function sourceRootStatusPillClass(status) {
  if (status === "ready") {
    return "ready";
  }
  if (status === "degraded") {
    return "error";
  }
  if (status === "disabled") {
    return "muted";
  }
  return "pending";
}

function sourceStatusPillClass(status) {
  if (status === "active") {
    return "ready";
  }
  if (status === "invalidated") {
    return "error";
  }
  if (status === "out_of_scope") {
    return "pending";
  }
  return "muted";
}

function renderSourceRootRulesSummary(rules) {
  const parts = [];
  const includeGlobs = rules?.include_globs ?? [];
  const excludeGlobs = rules?.exclude_globs ?? [];
  const includeExtensions = rules?.include_extensions ?? [];

  parts.push(includeGlobs.length ? `包含规则 ${includeGlobs.length}` : "包含全部");
  parts.push(excludeGlobs.length ? `排除规则 ${excludeGlobs.length}` : "不排除");
  parts.push(includeExtensions.length ? includeExtensions.join(", ") : "全部来源类型");
  return parts.join(" · ");
}

function formatScanTime(lastScanAtMs) {
  if (!lastScanAtMs) {
    return "尚未刷新或重扫";
  }
  return new Date(Number(lastScanAtMs)).toLocaleString();
}

function renderSourceRootsPanel(library) {
  const submitLabel = state.editingSourceRootId ? "保存来源根" : "创建来源根";
  const panelActions = `
    <div class="inline-actions">
      <button
        type="button"
        id="library-refresh-button"
        data-testid="library-refresh-button"
        ${library && state.sourceRoots.length ? "" : "disabled"}
      >
        库级刷新
      </button>
      <button
        type="button"
        id="library-rescan-button"
        data-testid="library-rescan-button"
        class="secondary-button"
        ${library && state.sourceRoots.length ? "" : "disabled"}
      >
        库级重扫
      </button>
    </div>
  `;

  const list = state.sourceRoots.length
    ? `
        <ul class="data-list source-root-list" data-testid="source-root-list">
          ${state.sourceRoots
            .map(
              (sourceRoot) => `
                <li class="source-root-card" data-testid="source-root-card" data-source-root-id="${escapeHtml(sourceRoot.source_root_id)}">
                  <div class="list-head">
                    <strong>${escapeHtml(sourceRoot.root_path)}</strong>
                    <span class="helper">${escapeHtml(sourceRoot.source_root_id)}</span>
                  </div>
                  <div class="pill-row compact-row">
                    <span class="pill ${sourceRootStatusPillClass(sourceRoot.status)}">${escapeHtml(sourceRoot.status)}</span>
                    <span class="pill muted">${escapeHtml(sourceRoot.watch_state)}</span>
                  </div>
                  <dl class="stats compact-stats">
                    <div><dt>已观察</dt><dd>${sourceRoot.coverage_summary?.observed_file_count ?? 0}</dd></div>
                    <div><dt>匹配</dt><dd>${sourceRoot.coverage_summary?.matched_file_count ?? 0}</dd></div>
                    <div><dt>正常</dt><dd>${sourceRoot.coverage_summary?.active_source_count ?? 0}</dd></div>
                    <div><dt>未启用</dt><dd>${sourceRoot.coverage_summary?.inactive_source_count ?? 0}</dd></div>
                  </dl>
                  <p class="helper">${escapeHtml(renderSourceRootRulesSummary(sourceRoot.rules))}</p>
                  <p class="helper">最近扫描：${escapeHtml(formatScanTime(sourceRoot.coverage_summary?.last_scan_at_ms))}</p>
                  ${
                    sourceRoot.last_action
                      ? `<p class="helper">最近动作：${escapeHtml(sourceRoot.last_action.action)} · ${escapeHtml(sourceRoot.last_action.status)} · ${escapeHtml(sourceRoot.last_action.summary)}</p>`
                      : ""
                  }
                  <div class="inline-actions">
                    <button type="button" class="secondary-button" data-source-root-edit-id="${escapeHtml(sourceRoot.source_root_id)}">编辑</button>
                    <button type="button" data-source-root-refresh-id="${escapeHtml(sourceRoot.source_root_id)}" ${sourceRoot.enabled ? "" : "disabled"}>刷新</button>
                    <button type="button" class="secondary-button" data-source-root-rescan-id="${escapeHtml(sourceRoot.source_root_id)}" ${sourceRoot.enabled ? "" : "disabled"}>重扫</button>
                    <button type="button" class="secondary-button" data-source-root-toggle-id="${escapeHtml(sourceRoot.source_root_id)}">
                      ${sourceRoot.enabled ? "停用" : "启用"}
                    </button>
                    <button type="button" class="secondary-button danger-button" data-source-root-delete-id="${escapeHtml(sourceRoot.source_root_id)}">删除</button>
                  </div>
                </li>
              `
            )
            .join("")}
        </ul>
      `
    : '<p class="empty" data-testid="source-root-empty">当前库还没有来源根。先创建一个本地目录来源根，再触发刷新或重扫。</p>';

  return `
    <section class="panel panel-tight">
      <div class="panel-head">
        <div>
          <p class="eyebrow">来源</p>
          <h2>来源根管理</h2>
        </div>
        ${panelActions}
      </div>
      <form id="source-root-form" class="stack-form" data-testid="source-root-form">
        <label>
          <span>目录根路径</span>
          <input
            id="source-root-path"
            data-testid="source-root-path-input"
            type="text"
            placeholder="/path/to/library-root"
            value="${escapeHtml(state.sourceRootPathDraft)}"
            ${library ? "" : "disabled"}
          />
        </label>
        <label class="checkbox-line">
          <input
            id="source-root-enabled"
            data-testid="source-root-enabled-input"
            type="checkbox"
            ${state.sourceRootEnabledDraft ? "checked" : ""}
            ${library ? "" : "disabled"}
          />
          <span>启用该来源根并接入 watcher</span>
        </label>
        <label>
          <span>包含规则（globs）</span>
          <textarea
            id="source-root-include-globs"
            data-testid="source-root-include-globs-input"
            rows="3"
            placeholder="images/**&#10;reports/*.pdf"
            ${library ? "" : "disabled"}
          >${escapeHtml(state.sourceRootIncludeGlobsDraft)}</textarea>
        </label>
        <label>
          <span>排除规则（globs）</span>
          <textarea
            id="source-root-exclude-globs"
            data-testid="source-root-exclude-globs-input"
            rows="3"
            placeholder="**/*.tmp&#10;archive/**"
            ${library ? "" : "disabled"}
          >${escapeHtml(state.sourceRootExcludeGlobsDraft)}</textarea>
        </label>
        <label>
          <span>包含扩展名</span>
          <input
            id="source-root-include-extensions"
            data-testid="source-root-include-extensions-input"
            type="text"
            placeholder="png, jpg, pdf"
            value="${escapeHtml(state.sourceRootIncludeExtensionsDraft)}"
            ${library ? "" : "disabled"}
          />
        </label>
        <div class="inline-actions">
          <button type="submit" data-testid="source-root-submit-button" ${library ? "" : "disabled"}>
            ${escapeHtml(submitLabel)}
          </button>
          <button type="button" id="source-root-reset-button" class="secondary-button" data-testid="source-root-reset-button" ${library ? "" : "disabled"}>
            清空
          </button>
        </div>
      </form>
      ${list}
    </section>
  `;
}

function renderWorkspaceSwitcher() {
  const toolsActive = state.utilityDrawerOpen && state.utilityDrawerSection !== "status";
  return `
    <nav class="workspace-switch" data-testid="workspace-switch" aria-label="主工作区切换">
      <button
        type="button"
        class="workspace-switch-button ${state.activeWorkspace === "search" ? "active" : ""}"
        data-testid="workspace-tab-search"
        data-workspace="search"
      >
        ${renderUiIcon("search")}
        <span>搜索</span>
      </button>
      <button
        type="button"
        class="workspace-switch-button ${state.activeWorkspace === "inventory" ? "active" : ""}"
        data-testid="workspace-tab-inventory"
        data-workspace="inventory"
      >
        ${renderUiIcon("library")}
        <span>库管理</span>
      </button>
      <button
        type="button"
        class="workspace-switch-button ${toolsActive ? "active" : ""}"
        data-testid="workspace-tab-tools"
        data-utility-drawer-open="${escapeHtml(utilityDrawerToolSection())}"
        aria-expanded="${toolsActive ? "true" : "false"}"
      >
        ${renderUiIcon("tools")}
        <span>工具</span>
      </button>
      <button
        type="button"
        class="workspace-switch-button ${state.activeWorkspace === "settings" ? "active" : ""}"
        data-testid="workspace-tab-settings"
        data-workspace="settings"
      >
        ${renderUiIcon("settings")}
        <span>设置</span>
      </button>
    </nav>
  `;
}

function renderInventoryBridge(library) {
  if (!library) {
    return "";
  }

  const summaryText =
    state.activeWorkspace === "inventory" && state.inventorySummary.total
      ? `当前库共有 ${state.inventorySummary.total} 条来源记录，正常 ${state.inventorySummary.active}，已失效 ${state.inventorySummary.invalidated}，超出范围 ${state.inventorySummary.out_of_scope}。`
      : "来源清单、状态过滤与来源级观察已移到独立来源浏览工作区。";

  return `
    <div class="workspace-bridge" data-testid="inventory-bridge">
      <p class="eyebrow">库管理</p>
      <p class="helper" data-testid="inventory-bridge-summary">${escapeHtml(summaryText)}</p>
      <div class="inline-actions">
        ${
          state.activeWorkspace === "inventory"
            ? '<span class="pill ready" data-testid="inventory-bridge-state">库管理已打开</span>'
            : `<button
                type="button"
                class="secondary-button"
                data-testid="inventory-bridge-button"
                data-workspace="inventory"
              >
                前往库管理
              </button>`
        }
      </div>
    </div>
  `;
}

function renderInventorySummaryBar() {
  const summaryItems = [
    { label: "来源记录", value: state.inventorySummary.total, testId: "inventory-summary-total" },
    { label: "正常", value: state.inventorySummary.active, testId: "inventory-summary-active" },
    {
      label: "已失效",
      value: state.inventorySummary.invalidated,
      testId: "inventory-summary-invalidated",
    },
    {
      label: "超出范围",
      value: state.inventorySummary.out_of_scope,
      testId: "inventory-summary-out-of-scope",
    },
  ];

  return `
    <div class="inventory-summary-bar" data-testid="inventory-summary">
      ${summaryItems
        .map(
          (item) => `
            <article class="inventory-summary-card" data-testid="${item.testId}">
              <span class="inventory-summary-label">${escapeHtml(item.label)}</span>
              <strong class="inventory-summary-value">${escapeHtml(item.value)}</strong>
            </article>
          `
        )
        .join("")}
    </div>
  `;
}

function inventoryFilterSummaryItems() {
  const items = [];
  if (state.inventoryFilters.sourceRootId) {
    if (state.inventoryFilters.sourceRootId === "manual") {
      items.push("手动导入");
    } else {
      const sourceRoot = state.sourceRoots.find(
        (item) => item.source_root_id === state.inventoryFilters.sourceRootId
      );
      items.push(sourceRoot?.root_path ?? state.inventoryFilters.sourceRootId);
    }
  }
  if (state.inventoryFilters.sourceType) {
    items.push(sourceTypeDisplayName(state.inventoryFilters.sourceType));
  }
  if (state.inventoryFilters.sourceStatus) {
    items.push(sourceStatusDisplayName(state.inventoryFilters.sourceStatus));
  }
  return items;
}

function renderInventoryActionRow(library: LibrarySnapshot | null) {
  return `
    <div class="inline-actions inventory-action-row">
      <button
        type="button"
        class="secondary-button"
        data-testid="inventory-action-focus-source-prep"
        data-utilities-action="focus-source-prep"
        ${library ? "" : "disabled"}
      >
        前往来源准备
      </button>
      <button
        type="button"
        class="secondary-button"
        data-testid="inventory-action-refresh-library"
        data-utilities-action="refresh-library"
        ${library ? "" : "disabled"}
      >
        刷新当前库
      </button>
      <button
        type="button"
        class="secondary-button"
        data-testid="inventory-action-rescan-library"
        data-utilities-action="rescan-library"
        ${library ? "" : "disabled"}
      >
        重扫当前库
      </button>
    </div>
  `;
}

function sourceRootStatusDisplayName(status) {
  if (status === "ready") {
    return "就绪";
  }
  if (status === "degraded") {
    return "需关注";
  }
  if (status === "disabled") {
    return "已停用";
  }
  return status;
}

function sourceRootWatchStateDisplayName(watchState) {
  if (watchState === "watching") {
    return "监视中";
  }
  if (watchState === "disabled") {
    return "未监视";
  }
  if (watchState === "starting") {
    return "启动中";
  }
  if (watchState === "stopped") {
    return "已停止";
  }
  return watchState;
}

function sourceRootWatchStatePillClass(watchState) {
  if (watchState === "watching") {
    return "ready";
  }
  if (watchState === "disabled") {
    return "muted";
  }
  return "pending";
}

function inventorySourceRootPriority(sourceRoot: SourceRootSnapshot) {
  if (sourceRoot.status === "degraded") {
    return 3;
  }
  if (sourceRoot.enabled && sourceRoot.watch_state !== "watching") {
    return 2;
  }
  if (sourceRoot.enabled) {
    return 1;
  }
  return 0;
}

function contentTypeResolvedStatusLabel(status: string) {
  if (status === "available") {
    return "已就绪";
  }
  if (status === "not_enabled") {
    return "连接未启用";
  }
  if (status === "runtime_unavailable") {
    return "运行时受限";
  }
  if (status === "not_supported") {
    return "当前不支持";
  }
  return status;
}

function contentTypeReadinessEntries() {
  const contentTypes = availableContentTypeKeys(
    state.globalContentTypes,
    state.libraryContentTypes,
    state.resolvedContentModels ? { content_types: state.resolvedContentModels.content_types } : null
  );

  return contentTypes.map((contentType) => {
    const binding =
      state.libraryContentTypes.content_types[contentType] ??
      state.globalContentTypes.content_types[contentType] ??
      defaultContentTypeBinding();
    const resolved = state.resolvedContentModels?.content_types?.[contentType];
    const selection = selectionFromBinding(binding);
    const hasOverride = libraryContentTypeHasOverride(contentType);

    if (!binding.enabled) {
      return {
        contentType,
        statusLabel: "已停用",
        pillClass: "muted",
        summary: "当前不参与这个库的后续搜索与入库。",
        context: hasOverride ? "当前库覆盖" : "沿用全局默认",
      };
    }

    if (!binding.model) {
      return {
        contentType,
        statusLabel: "未配置模型",
        pillClass: "error",
        summary: "已经启用，但当前没有绑定模型。",
        context: hasOverride ? "当前库覆盖" : "沿用全局默认",
      };
    }

    if (!resolved) {
      return {
        contentType,
        statusLabel: "等待解析",
        pillClass: "pending",
        summary: `${composeModelReference(selection) || binding.model} 尚未出现在当前 resolved model 摘要里。`,
        context: hasOverride ? "当前库覆盖" : "沿用全局默认",
      };
    }

    if (resolved.status !== "available") {
      return {
        contentType,
        statusLabel: contentTypeResolvedStatusLabel(resolved.status),
        pillClass: providerSelectionPillClass(resolved.status),
        summary: resolved.message,
        context: formatResolvedContentModelContext(resolved),
      };
    }

    return {
      contentType,
      statusLabel: "已就绪",
      pillClass: "ready",
      summary: formatResolvedContentModel(resolved),
      context: `${formatBindingSource(resolved.binding_source)} · 向量类型 ${resolved.vector_type}`,
    };
  });
}

function libraryOperationalReadiness(library: LibrarySnapshot) {
  const enabledRoots = state.sourceRoots.filter((item) => item.enabled);
  const degradedRoots = state.sourceRoots.filter((item) => item.status === "degraded");
  const nonWatchingRoots = enabledRoots.filter((item) => item.watch_state !== "watching");
  const watchIssues = nonWatchingRoots.length;
  const searchableUnits = library.counts.accepted_items;
  const pendingJobs = library.counts.pending_jobs;
  const contentTypeEntries = contentTypeReadinessEntries();
  const blockedContentTypes = contentTypeEntries.filter(
    (entry) => entry.pillClass === "error" || entry.pillClass === "pending"
  );
  const lastActionSummary =
    state.sourceRoots.map((item) => item.last_action?.summary).find(Boolean) ?? "";

  if (!state.sourceRoots.length) {
    return {
      status: "尚未接入来源根",
      pillClass: "muted",
      summary: "这个库还没有来源根。先接入一个本地目录来源根，再执行 refresh 或 rescan。",
      enabledRoots: 0,
      degradedRoots: 0,
      watchIssues: 0,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  if (!enabledRoots.length) {
    return {
      status: "来源根已停用",
      pillClass: "muted",
      summary: "当前所有来源根都已停用；恢复至少一个来源根后，这个库才会继续承接 watcher、refresh 与 rescan。",
      enabledRoots: 0,
      degradedRoots: degradedRoots.length,
      watchIssues,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  if (pendingJobs > 0 && searchableUnits <= 0) {
    return {
      status: "正在准备中",
      pillClass: "pending",
      summary: "当前已有来源根接入，后台任务正在导入或建索引；任务完成后，这个库就会进入可搜索状态。",
      enabledRoots: enabledRoots.length,
      degradedRoots: degradedRoots.length,
      watchIssues,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  if (degradedRoots.length > 0) {
    const details = [];
    details.push(`${degradedRoots.length} 个来源根处于需关注状态`);
    if (watchIssues > 0) {
      details.push(`${watchIssues} 个启用来源根当前不在监视中`);
    }
    details.push(
      searchableUnits > 0
        ? `当前仍有 ${searchableUnits} 个可搜索对象可继续使用`
        : "建议先检查来源根，再执行一次 refresh 或 rescan"
    );
    return {
      status: "需要关注",
      pillClass: "error",
      summary: `${details.join("，")}。`,
      enabledRoots: enabledRoots.length,
      degradedRoots: degradedRoots.length,
      watchIssues,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  if (watchIssues > 0) {
    return {
      status: "观察未稳定",
      pillClass: "pending",
      summary:
        searchableUnits > 0
          ? `${watchIssues} 个启用来源根当前不在监视中，但这个库仍有 ${searchableUnits} 个可搜索对象。`
          : `${watchIssues} 个启用来源根当前不在监视中；建议先恢复监视或手动执行 refresh / rescan。`,
      enabledRoots: enabledRoots.length,
      degradedRoots: degradedRoots.length,
      watchIssues,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  if (blockedContentTypes.length > 0 && searchableUnits <= 0) {
    return {
      status: "等待配置",
      pillClass: "pending",
      summary: `${blockedContentTypes.length} 个启用内容类型当前还未就绪；先检查当前库覆盖与 resolved model，再继续导入或搜索。`,
      enabledRoots: enabledRoots.length,
      degradedRoots: degradedRoots.length,
      watchIssues,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  if (blockedContentTypes.length > 0) {
    return {
      status: "配置需关注",
      pillClass: "pending",
      summary: `${blockedContentTypes.length} 个启用内容类型当前未完全就绪；已有对象仍可搜索，但后续入库会受当前库覆盖与 resolved model 影响。`,
      enabledRoots: enabledRoots.length,
      degradedRoots: degradedRoots.length,
      watchIssues,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  if (searchableUnits <= 0) {
    return {
      status: "等待内容",
      pillClass: "muted",
      summary: "来源根已经接入，但这个库还没有可搜索对象。先导入首批内容，或对现有来源执行 refresh / rescan。",
      enabledRoots: enabledRoots.length,
      degradedRoots: degradedRoots.length,
      watchIssues,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  return {
    status: "可搜索",
    pillClass: "ready",
    summary: `当前库已接入 ${enabledRoots.length} 个启用来源根，${searchableUnits} 个对象可以直接参与搜索。`,
    enabledRoots: enabledRoots.length,
    degradedRoots: degradedRoots.length,
    watchIssues,
    searchableUnits,
    pendingJobs,
    blockedContentTypes: blockedContentTypes.length,
    lastActionSummary,
  };
}

function renderInventoryLibraryManagementBand(library: LibrarySnapshot | null) {
  if (!library) {
    return "";
  }

  const readiness = libraryOperationalReadiness(library);
  const contentTypeEntries = contentTypeReadinessEntries();
  const configIssues = contentTypeEntries.filter(
    (entry) => entry.pillClass === "error" || entry.pillClass === "pending"
  ).length;
  const sourceRootSnapshots = [...state.sourceRoots].sort((left, right) => {
    const priorityDiff = inventorySourceRootPriority(right) - inventorySourceRootPriority(left);
    if (priorityDiff !== 0) {
      return priorityDiff;
    }
    return left.root_path.localeCompare(right.root_path);
  });
  const metrics = [
    { label: "启用来源根", value: readiness.enabledRoots },
    { label: "需关注", value: readiness.degradedRoots + readiness.watchIssues + configIssues },
    { label: "可搜索对象", value: readiness.searchableUnits },
    { label: "待处理任务", value: readiness.pendingJobs },
  ];

  return `
    <section class="inventory-library-band" data-testid="inventory-library-management">
      <article class="panel inventory-library-overview">
        <div class="inventory-library-head">
          <div>
            <p class="eyebrow">当前库管理</p>
            <h3 data-testid="inventory-library-name">${escapeHtml(libraryDisplayName(library))}</h3>
            <p class="helper inventory-library-id">${escapeHtml(library.id)}</p>
          </div>
          <span
            class="pill ${libraryLifecyclePillClass(library)}"
            data-testid="inventory-library-lifecycle"
          >
            ${escapeHtml(libraryLifecycleLabel(library))}
          </span>
        </div>
        <div class="inventory-library-readiness" data-testid="inventory-library-readiness">
          <div class="inventory-library-readiness-head">
            <span class="pill ${escapeHtml(readiness.pillClass)}" data-testid="inventory-library-readiness-status">
              ${escapeHtml(readiness.status)}
            </span>
            <span class="helper inventory-library-note">当前库级动作直接作用于这个库本身。</span>
          </div>
          <p class="helper" data-testid="inventory-library-readiness-summary">${escapeHtml(readiness.summary)}</p>
          ${
            readiness.lastActionSummary
              ? `<p class="helper inventory-library-readiness-meta">${escapeHtml(readiness.lastActionSummary)}</p>`
              : ""
          }
        </div>
        <div class="inventory-library-metrics" data-testid="inventory-library-metrics">
          ${metrics
            .map(
              (item) => `
                <article class="inventory-library-metric">
                  <span class="inventory-library-metric-label">${escapeHtml(item.label)}</span>
                  <strong class="inventory-library-metric-value">${escapeHtml(item.value)}</strong>
                </article>
              `
            )
            .join("")}
        </div>
        <section class="inventory-library-root-strip" data-testid="inventory-library-root-strip">
          <div class="inventory-library-root-strip-head">
            <strong>来源根快照</strong>
            <span class="helper">
              ${
                state.sourceRoots.length
                  ? `${escapeHtml(state.sourceRoots.length)} 个来源根`
                  : "还没有来源根"
              }
            </span>
          </div>
          ${
            sourceRootSnapshots.length
              ? `
                <ul class="inventory-library-root-list" data-testid="inventory-library-root-list">
                  ${sourceRootSnapshots
                    .map(
                      (sourceRoot) => `
                        <li
                          class="inventory-library-root-card"
                          data-testid="inventory-library-root-card"
                          data-source-root-id="${escapeHtml(sourceRoot.source_root_id)}"
                        >
                          <div class="inventory-library-root-head">
                            <strong>${escapeHtml(sourceRoot.root_path)}</strong>
                            <span class="helper">
                              匹配 ${escapeHtml(sourceRoot.coverage_summary?.matched_file_count ?? 0)} · 正常 ${escapeHtml(
                                sourceRoot.coverage_summary?.active_source_count ?? 0
                              )}
                            </span>
                          </div>
                          <div class="pill-row compact-row inventory-library-root-pills">
                            <span class="pill ${sourceRootStatusPillClass(sourceRoot.status)}">
                              ${escapeHtml(sourceRootStatusDisplayName(sourceRoot.status))}
                            </span>
                            <span class="pill ${sourceRootWatchStatePillClass(sourceRoot.watch_state)}">
                              ${escapeHtml(sourceRootWatchStateDisplayName(sourceRoot.watch_state))}
                            </span>
                          </div>
                          <p class="helper inventory-library-root-meta">
                            ${
                              sourceRoot.last_action?.summary
                                ? escapeHtml(sourceRoot.last_action.summary)
                                : escapeHtml(`最近扫描：${formatScanTime(sourceRoot.coverage_summary?.last_scan_at_ms)}`)
                            }
                          </p>
                        </li>
                      `
                    )
                    .join("")}
                </ul>
              `
              : '<p class="helper inventory-library-root-empty">先从来源准备接入第一个本地目录来源根。</p>'
          }
        </section>
        <section class="inventory-library-config-strip" data-testid="inventory-library-config-strip">
          <div class="inventory-library-root-strip-head">
            <div>
              <strong>当前库覆盖与模型绑定</strong>
              <p class="helper inventory-library-root-meta">
                当前展示当前库在各内容类型上的启用状态、resolved model 与需要关注的绑定问题。
              </p>
            </div>
            <button
              type="button"
              class="secondary-button"
              data-testid="inventory-open-library-overrides-button"
              data-open-settings-section="library-overrides"
            >
              查看当前库覆盖
            </button>
          </div>
          <ul class="inventory-library-config-list" data-testid="inventory-library-config-list">
            ${contentTypeEntries
              .map(
                (entry) => `
                  <li
                    class="inventory-library-config-card"
                    data-testid="inventory-library-config-card"
                    data-content-type="${escapeHtml(entry.contentType)}"
                  >
                    <div class="inventory-library-config-head">
                      <strong>${escapeHtml(contentTypeDisplayName(entry.contentType))}</strong>
                      <span class="pill ${entry.pillClass}">${escapeHtml(entry.statusLabel)}</span>
                    </div>
                    <p class="helper inventory-library-config-summary">${escapeHtml(entry.summary)}</p>
                    <p class="helper inventory-library-config-context">${escapeHtml(entry.context)}</p>
                  </li>
                `
              )
              .join("")}
          </ul>
        </section>
      </article>
      <article class="panel inventory-library-actions">
        <div class="panel-head">
          <div>
            <p class="eyebrow">管理动作</p>
            <h3>修改当前库身份与生命周期</h3>
          </div>
        </div>
        <form
          class="stack-form compact-form"
          data-testid="inventory-manage-library-form"
          data-library-rename-form="true"
        >
          <label>
            <span>显示名称</span>
            <input
              data-testid="inventory-manage-library-name-input"
              data-library-management-display-name-input="true"
              name="manageLibraryDisplayName"
              type="text"
              value="${escapeHtml(state.libraryManagementDisplayNameDraft)}"
              placeholder="例如：季度报告库"
              required
            />
          </label>
          <button type="submit" data-testid="inventory-rename-library-button">保存名称</button>
        </form>
        <div class="inline-actions inventory-library-action-row">
          <button
            type="button"
            class="secondary-button"
            data-testid="inventory-toggle-library-archive-button"
            data-library-archive-action="true"
          >
            ${libraryIsArchived(library) ? "恢复当前库" : "归档当前库"}
          </button>
          <button
            type="button"
            class="secondary-button destructive-button"
            data-testid="inventory-delete-library-button"
            data-library-delete-action="true"
          >
            删除当前库
          </button>
        </div>
      </article>
    </section>
  `;
}

function inventoryRepresentativeKind(source: SourceInventoryItem) {
  return source.representative_visual_unit?.kind ?? source.kind;
}

function inventoryRepresentativeSourceType(source: SourceInventoryItem) {
  return source.representative_visual_unit?.source_type ?? source.source_type;
}

function inventoryRepresentativeKindIcon(source: SourceInventoryItem) {
  const kind = inventoryRepresentativeKind(source);
  if (kind === "video_segment") {
    return "video";
  }
  if (kind === "document_page") {
    return "document";
  }
  return "image";
}

function renderInventorySourceThumbnail(source: SourceInventoryItem) {
  const kind = inventoryRepresentativeKind(source);
  const preview = source.representative_preview;
  if (kind === "image" && preview?.url) {
    return `
      <img
        class="inventory-source-thumbnail"
        src="${escapeHtml(preview.url)}"
        alt="${escapeHtml(sourceName(source.source_path))}"
        loading="lazy"
      />
    `;
  }

  return `
    <div class="inventory-source-thumbnail inventory-source-thumbnail-placeholder">
      ${renderUiIcon(inventoryRepresentativeKindIcon(source))}
      <span>${escapeHtml(visualUnitKindDisplayName(kind))}</span>
    </div>
  `;
}

function renderProviderOptions(currentValue = "", includeEmpty = false) {
  const emptyOption = includeEmpty
    ? `<option value="" ${!currentValue ? "selected" : ""}>未选择</option>`
    : "";
  const hasCurrentValue =
    !!currentValue && state.providerConfigs.some((provider) => provider.provider_id === currentValue);
  const missingOption =
    currentValue && !hasCurrentValue
      ? `<option value="${escapeHtml(currentValue)}" selected>${escapeHtml(currentValue)} (已配置)</option>`
      : "";
  return `${emptyOption}${missingOption}${state.providerConfigs
    .map(
      (provider) => `
        <option value="${escapeHtml(provider.provider_id)}" ${provider.provider_id === currentValue ? "selected" : ""}>
          ${escapeHtml(provider.display_name)} (${escapeHtml(provider.provider_kind)}${provider.enabled ? "" : " · 已停用"})
        </option>
      `
    )
    .join("")}`;
}

function renderContentTypeTabs(scope: "global" | "library", selected: string, contentTypes: string[]) {
  return `
    <div class="content-type-tabs" data-testid="${escapeHtml(scope)}-content-type-tabs">
      ${contentTypes
        .map(
          (contentType) => `
            <button
              type="button"
              class="content-type-tab ${contentType === selected ? "active" : ""}"
              data-testid="${escapeHtml(scope)}-content-type-tab-${escapeHtml(contentType)}"
              data-content-type-scope="${escapeHtml(scope)}"
              data-content-type="${escapeHtml(contentType)}"
            >
              <span class="eyebrow">${escapeHtml(contentType)}</span>
              <strong>${escapeHtml(contentTypeDisplayName(contentType))}</strong>
            </button>
          `
        )
        .join("")}
    </div>
  `;
}

function renderProviderBridge(library: LibrarySnapshot | null) {
  if (!library) {
    return "";
  }

  const selections = Object.values(state.resolvedContentModels?.content_types ?? {});
  const summary = selections.length
    ? selections
        .map(
          (selection) =>
            `${contentTypeDisplayName(selection.content_type)}：${formatResolvedContentModel(selection)} · ${selection.status}`
        )
        .join(" | ")
    : "当前库的当前生效模型尚未加载。";

  return `
    <div class="workspace-bridge" data-testid="provider-bridge">
      <p class="eyebrow">设置</p>
      <p class="helper" data-testid="provider-bridge-summary">${escapeHtml(summary)}</p>
      <div class="inline-actions">
        ${
          state.activeWorkspace === "settings"
            ? '<span class="pill ready" data-testid="provider-bridge-state">设置已打开</span>'
            : `<button
                type="button"
                class="secondary-button"
                data-testid="provider-bridge-button"
                data-workspace="settings"
              >
                前往设置
              </button>`
        }
      </div>
    </div>
  `;
}

function providerRuntimeSnapshot(providerId: string) {
  return state.runtimeHealth?.providers.find((provider) => provider.provider_id === providerId) ?? null;
}

function renderProviderRuntimeSummary(providerId: string, options: { editor?: boolean } = {}) {
  const runtimeProvider = providerRuntimeSnapshot(providerId);
  const testId = options.editor
    ? "provider-editor-runtime-summary"
    : `provider-runtime-summary-${providerId}`;

  if (!runtimeProvider) {
    return `
      <div class="provider-runtime-summary" data-testid="${escapeHtml(testId)}">
        <p class="helper">当前还没有拿到这个连接的运行时模型快照。</p>
      </div>
    `;
  }

  const facts = [
    runtimeProvider.model_id ? `当前模型 ${runtimeProvider.model_id}` : "当前模型 未解析",
    runtimeProvider.model_version ? `模型版本 ${runtimeProvider.model_version}` : "模型版本 未解析",
    runtimeProvider.model_revision ? `模型修订 ${runtimeProvider.model_revision}` : null,
  ]
    .filter(Boolean)
    .map((value) => `<span class="helper">${escapeHtml(String(value))}</span>`)
    .join("");

  return `
    <div class="provider-runtime-summary" data-testid="${escapeHtml(testId)}">
      ${facts}
    </div>
  `;
}

function renderProviderConfigsPanel() {
  const editingProvider = selectedProviderConfig();
  const listMarkup = state.providerConfigs.length
    ? `
      <ul class="provider-profile-list" data-testid="provider-config-list">
        ${state.providerConfigs
          .map(
            (provider) => `
              <li class="provider-profile-row" data-testid="provider-config-row">
                <div class="provider-profile-main">
                  <strong>${escapeHtml(provider.display_name)}</strong>
                  <span class="helper">${escapeHtml(provider.provider_id)} · ${escapeHtml(provider.provider_kind)}</span>
                  ${
                    provider.base_url
                      ? `<span class="helper">${escapeHtml(provider.base_url)}</span>`
                      : ""
                  }
                  ${renderProviderRuntimeSummary(provider.provider_id)}
                </div>
                <div class="provider-profile-meta">
                  <span class="pill ${providerProbePillClass(provider.probe?.status)}">${escapeHtml(provider.probe?.status ?? "unknown")}</span>
                  <button type="button" class="secondary-button" data-provider-edit-id="${escapeHtml(provider.provider_id)}">编辑</button>
                </div>
              </li>
            `
          )
          .join("")}
      </ul>
    `
    : `<p class="empty">当前还没有可用连接。</p>`;

  return `
    <section class="panel settings-panel" data-testid="provider-configs-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">连接</p>
          <h2>连接</h2>
        </div>
      </div>
      <div class="provider-configs-layout">
        <div class="provider-config-list-surface">
          <p class="helper">左侧保持连接状态与连接摘要；右侧只编辑当前选中的连接，不把这章变成新的运维控制台。</p>
          ${listMarkup}
        </div>
        <form id="provider-config-form" class="stack-form provider-config-editor" data-testid="provider-config-form">
          <label>
            <span>连接</span>
            <select id="provider-config-id" data-testid="provider-config-id">
              <option value="" ${!state.editingProviderId ? "selected" : ""}>选择一个连接</option>
              ${state.providerConfigs
                .map(
                  (provider) => `
                    <option value="${escapeHtml(provider.provider_id)}" ${provider.provider_id === state.editingProviderId ? "selected" : ""}>
                      ${escapeHtml(provider.display_name)}
                    </option>
                  `
                )
                .join("")}
            </select>
          </label>
          <div class="filter-grid settings-filter-grid">
            <label class="checkbox-field">
              <input
                id="provider-enabled"
                data-testid="provider-enabled"
                type="checkbox"
                ${state.providerEnabledDraft ? "checked" : ""}
                ${!editingProvider ? "disabled" : ""}
              />
              <span>启用</span>
            </label>
            <label>
              <span>连接地址</span>
              <input
                id="provider-base-url"
                data-testid="provider-base-url"
                type="url"
                value="${escapeHtml(state.providerBaseUrlDraft)}"
                placeholder="https://dashscope.aliyuncs.com"
                ${!editingProvider || editingProvider.provider_id === PROVIDER_ID_LOCAL_SIDECAR ? "disabled" : ""}
              />
            </label>
          </div>
          ${
            editingProvider
              ? `
                  <p class="helper">${escapeHtml(editingProvider.provider_id)} · ${escapeHtml(editingProvider.provider_kind)}</p>
                  ${renderProviderRuntimeSummary(editingProvider.provider_id, { editor: true })}
                `
              : `<p class="helper">先从左侧选择一个连接，再修改启用状态或连接地址。</p>`
          }
          ${
            editingProvider?.readonly_reason
              ? `<p class="helper" data-testid="provider-readonly-reason">${escapeHtml(editingProvider.readonly_reason)}</p>`
              : ""
          }
          <div class="inline-actions">
            <button type="submit" data-testid="provider-config-submit-button" ${!editingProvider ? "disabled" : ""}>
              保存连接配置
            </button>
            <button
              type="button"
              id="provider-config-reset-button"
              data-testid="provider-config-reset-button"
              class="secondary-button"
            >
              重置
            </button>
          </div>
        </form>
      </div>
    </section>
  `;
}

function renderModelIdOptions(providerId: string, currentValue: string, includeEmpty = false) {
  const entries = catalogEntriesForProvider(providerId);
  const hasCurrentValue = !!currentValue && entries.some((entry) => entry.model_id === currentValue);
  const emptyOption = includeEmpty
    ? `<option value="" ${!currentValue ? "selected" : ""}>未选择</option>`
    : "";
  const missingOption =
    currentValue && !hasCurrentValue
      ? `<option value="${escapeHtml(currentValue)}" selected>${escapeHtml(currentValue)} (已配置)</option>`
      : "";
  return `${emptyOption}${missingOption}${entries
    .map(
      (entry) => `
        <option value="${escapeHtml(entry.model_id)}" ${entry.model_id === currentValue ? "selected" : ""}>
          ${escapeHtml(`${entry.model_id}@${entry.model_version}`)}
        </option>
      `
    )
    .join("")}`;
}

function renderVectorTypeOptions(selection: ModelSelectionPayload, currentValue: string) {
  return vectorTypeOptionsForSelection(selection, currentValue)
    .map(
      (value) => `
        <option value="${escapeHtml(value)}" ${value === currentValue ? "selected" : ""}>
          ${escapeHtml(value)}
        </option>
      `
    )
    .join("");
}

function renderGlobalContentTypesPanel(includeTestPanel = true) {
  const contentType = selectedGlobalContentTypeKey();
  const binding = selectedGlobalContentTypeBinding();
  const selection = selectionFromBinding(binding);
  const catalogEntry = selectedCatalogEntryForProvider(selection.provider_id, selection.model_id);
  const supportedModalities = selectedGlobalTestModalities();
  const contentTypes = availableContentTypeKeys(state.globalContentTypes);

  return `
    <section class="panel settings-panel" data-testid="global-content-types-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">内容类型</p>
          <h2>全局内容类型</h2>
        </div>
      </div>
      ${renderContentTypeTabs("global", contentType, contentTypes)}
      <form id="global-content-types-form" class="stack-form" data-testid="global-content-types-form">
        <input id="global-content-type" data-testid="global-content-type" type="hidden" value="${escapeHtml(contentType)}" />
        <div class="filter-grid settings-filter-grid">
          <label class="checkbox-field">
            <input
              id="global-content-type-enabled"
              data-testid="global-content-type-enabled"
              type="checkbox"
              ${binding.enabled ? "checked" : ""}
            />
            <span>启用</span>
          </label>
        </div>
        <div class="filter-grid settings-filter-grid">
          <label>
            <span>连接</span>
            <select id="global-content-type-provider-id" data-testid="global-content-type-provider-id">
              ${renderProviderOptions(selection.provider_id)}
            </select>
          </label>
          <label>
            <span>模型</span>
            <select
              id="global-content-type-model-id"
              data-testid="global-content-type-model-id"
              ${selection.provider_id === PROVIDER_ID_LOCAL_SIDECAR ? "disabled" : ""}
            >
              ${renderModelIdOptions(selection.provider_id, selection.model_id)}
            </select>
          </label>
          <label>
            <span>向量类型</span>
            <select
              id="global-content-type-vector-type"
              data-testid="global-content-type-vector-type"
            >
              ${renderVectorTypeOptions(selection, binding.vector_type)}
            </select>
          </label>
        </div>
        ${
          catalogEntry
            ? `
              <p class="helper" data-testid="model-catalog-summary">${escapeHtml(catalogEntry.message)}</p>
              <p class="helper" data-testid="global-model-capabilities">${escapeHtml(
                formatEmbeddingCapabilities(catalogEntry.embedding_capabilities, { includePrefix: true })
              )}</p>
            `
            : ""
        }
        <p class="helper" data-testid="global-content-type-summary">
          ${escapeHtml(
            `${contentTypeDisplayName(contentType)} → ${binding.model || "未配置"} · ${binding.vector_type || "未设置向量类型"} · ${binding.enabled ? "已启用" : "已停用"}`
          )}
        </p>
        <div class="inline-actions">
          <button type="submit" data-testid="global-content-types-submit-button">保存全局内容类型绑定</button>
        </div>
      </form>
      ${
        includeTestPanel
          ? renderSettingsModelTestPanel({
              scope: "global",
              selection,
              supportedModalities,
              modalityDraft: state.globalModelTestModalityDraft,
              textDraft: state.globalModelTestTextDraft,
              file: state.globalModelTestFile,
              comparisonModalityDraft: state.globalModelTestComparisonModalityDraft,
              comparisonTextDraft: state.globalModelTestComparisonTextDraft,
              comparisonFile: state.globalModelTestComparisonFile,
              result: state.globalModelTestResult,
              error: state.globalModelTestError,
              pending: state.globalModelTestPending,
            })
          : ""
      }
    </section>
  `;
}

function renderLibraryContentTypesPanel(library: LibrarySnapshot | null, includeTestPanel = true) {
  if (!library) {
    return `
      <section class="panel settings-panel" data-testid="library-content-types-panel">
        <div class="panel-head">
          <div>
            <p class="eyebrow">当前库覆盖</p>
            <h2>当前库覆盖</h2>
          </div>
        </div>
        <p class="empty">先选择一个库，再编辑库级内容类型绑定。</p>
      </section>
    `;
  }

  const contentType = selectedLibraryContentTypeKey();
  const binding = selectedLibraryContentTypeBinding();
  const selection = selectionFromBinding(binding);
  const catalogEntry = selectedCatalogEntryForProvider(selection.provider_id, selection.model_id);
  const supportedModalities = selectedLibraryTestModalities();
  const hasOverride = selectedLibraryContentTypeHasOverride();
  const contentTypes = availableContentTypeKeys(
    state.globalContentTypes,
    state.libraryContentTypes,
    state.resolvedContentModels ? { content_types: state.resolvedContentModels.content_types } : null
  );

  return `
    <section class="panel settings-panel" data-testid="library-content-types-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">当前库覆盖</p>
          <h2>当前库覆盖</h2>
        </div>
      </div>
      ${renderContentTypeTabs("library", contentType, contentTypes)}
      <form id="library-content-types-form" class="stack-form" data-testid="library-content-types-form">
        <input id="library-content-type" data-testid="library-content-type" type="hidden" value="${escapeHtml(contentType)}" />
        <div class="override-mode-switch" data-testid="library-override-mode-switch">
          <button
            type="button"
            class="${!hasOverride ? "active" : "secondary-button"}"
            data-testid="library-override-mode-inherit"
            data-library-override-mode="inherit"
          >
            继承默认
          </button>
          <button
            type="button"
            class="${hasOverride ? "active" : "secondary-button"}"
            data-testid="library-override-mode-override"
            data-library-override-mode="override"
          >
            覆盖当前库
          </button>
        </div>
        <div class="override-mode-summary ${hasOverride ? "override-mode-summary-override" : ""}">
          <p class="helper">
            ${
              hasOverride
                ? escapeHtml(`当前 ${contentTypeDisplayName(contentType)} 已切到库级覆盖，保存后只影响 ${libraryDisplayName(library)}。`)
                : escapeHtml(`当前 ${contentTypeDisplayName(contentType)} 正沿用全局内容类型。点击“覆盖当前库”后才会进入可编辑状态。`)
            }
          </p>
        </div>
        <div class="filter-grid settings-filter-grid">
          <label class="checkbox-field">
            <input
              id="library-content-type-enabled"
              data-testid="library-content-type-enabled"
              type="checkbox"
              ${binding.enabled ? "checked" : ""}
              ${hasOverride ? "" : "disabled"}
            />
            <span>启用</span>
          </label>
        </div>
        <div class="filter-grid settings-filter-grid">
          <label>
            <span>连接</span>
            <select id="library-content-type-provider-id" data-testid="library-content-type-provider-id" ${hasOverride ? "" : "disabled"}>
              ${renderProviderOptions(selection.provider_id)}
            </select>
          </label>
          <label>
            <span>模型</span>
            <select
              id="library-content-type-model-id"
              data-testid="library-content-type-model-id"
              ${hasOverride && selection.provider_id !== PROVIDER_ID_LOCAL_SIDECAR ? "" : "disabled"}
            >
              ${renderModelIdOptions(selection.provider_id, selection.model_id)}
            </select>
          </label>
          <label>
            <span>向量类型</span>
            <select
              id="library-content-type-vector-type"
              data-testid="library-content-type-vector-type"
              ${hasOverride ? "" : "disabled"}
            >
              ${renderVectorTypeOptions(selection, binding.vector_type)}
            </select>
          </label>
        </div>
        ${
          catalogEntry
            ? `<p class="helper" data-testid="library-model-capabilities">${escapeHtml(
                formatEmbeddingCapabilities(catalogEntry.embedding_capabilities, { includePrefix: true })
              )}</p>`
            : ""
        }
        <p class="helper" data-testid="library-content-type-summary">
          ${escapeHtml(
            `${contentTypeDisplayName(contentType)} → ${binding.model || "未配置"} · ${binding.vector_type || "未设置向量类型"} · ${binding.enabled ? "已启用" : "已停用"}`
          )}
        </p>
        <div class="inline-actions">
          <button type="submit" data-testid="library-content-types-submit-button" ${hasOverride ? "" : "disabled"}>保存库级内容类型绑定</button>
          <button
            type="button"
            id="library-content-types-reset-button"
            data-testid="library-content-types-reset-button"
            class="secondary-button"
            ${hasOverride ? "" : "disabled"}
          >
            恢复默认
          </button>
        </div>
      </form>
      ${
        includeTestPanel
          ? renderSettingsModelTestPanel({
              scope: "library",
              selection,
              supportedModalities,
              modalityDraft: state.libraryModelTestModalityDraft,
              textDraft: state.libraryModelTestTextDraft,
              file: state.libraryModelTestFile,
              comparisonModalityDraft: state.libraryModelTestComparisonModalityDraft,
              comparisonTextDraft: state.libraryModelTestComparisonTextDraft,
              comparisonFile: state.libraryModelTestComparisonFile,
              result: state.libraryModelTestResult,
              error: state.libraryModelTestError,
              pending: state.libraryModelTestPending,
            })
          : ""
      }
    </section>
  `;
}

function renderResolvedContentModelsPanel(library: LibrarySnapshot | null) {
  if (!library) {
    return "";
  }

  const rows = Object.entries(state.resolvedContentModels?.content_types ?? {})
    .map(
      ([contentType, selection]) => `
        <li class="provider-resolution-row">
          <div>
            <strong>${escapeHtml(contentTypeDisplayName(contentType))}</strong>
            <span class="helper">${escapeHtml(formatResolvedContentModel(selection))} · ${escapeHtml(formatBindingSource(selection.binding_source))}</span>
            <span class="helper">${escapeHtml(formatResolvedContentModelContext(selection))}</span>
            <span class="helper">${escapeHtml(`向量类型 ${selection.vector_type}`)}</span>
            <span class="helper">${escapeHtml(
              formatEmbeddingCapabilities(selection.embedding_capabilities, { includePrefix: true })
            )}</span>
            <span class="helper">${escapeHtml(selection.message)}</span>
          </div>
          <span class="pill ${providerSelectionPillClass(selection.status)}">${escapeHtml(selection.status)}</span>
        </li>
      `
    )
    .join("");

  return `
    <section class="panel settings-panel" data-testid="resolved-content-models-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">当前生效结果</p>
          <h2>${escapeHtml(libraryDisplayName(library))} 的当前生效模型</h2>
        </div>
      </div>
      <ul class="provider-resolution-list">${rows || '<li class="empty">暂无当前生效模型。</li>'}</ul>
    </section>
  `;
}

function retiredVectorSpaceDiagnostics() {
  return (state.vectorSpaceDiagnostics?.vector_spaces ?? []).filter(
    (vectorSpace) => vectorSpace.lifecycle_state !== "active"
  );
}

function renderVectorSpaceDiagnosticsPanel(library: LibrarySnapshot | null) {
  if (!library) {
    return "";
  }

  const rows = (state.vectorSpaceDiagnostics?.vector_spaces ?? [])
    .map((vectorSpace) => {
      const details = [
        vectorSpace.provider_id && vectorSpace.model_id
          ? `${vectorSpace.provider_id}/${vectorSpace.model_id}`
          : null,
        vectorSpace.model_version ? `版本 ${vectorSpace.model_version}` : null,
        vectorSpace.vector_type ? `向量类型 ${vectorSpace.vector_type}` : null,
        vectorSpace.content_types.length
          ? `内容类型 ${vectorSpace.content_types.map((contentType) => contentTypeDisplayName(contentType)).join("、")}`
          : null,
        typeof vectorSpace.retired_at_ms === "number"
          ? `停用时间 ${new Date(vectorSpace.retired_at_ms).toLocaleString()}`
          : null,
      ]
        .filter(Boolean)
        .map((value) => `<span class="helper">${escapeHtml(String(value))}</span>`)
        .join("");

      return `
        <li class="provider-resolution-row">
          <div>
            <strong>${escapeHtml(vectorSpace.vector_space_id)}</strong>
            ${details}
          </div>
          <span class="pill ${providerSelectionPillClass(
            vectorSpace.lifecycle_state === "active" ? "available" : "degraded"
          )}">${escapeHtml(vectorSpace.lifecycle_state)}</span>
        </li>
      `;
    })
    .join("");

  return `
    <section class="panel settings-panel" data-testid="vector-space-diagnostics-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">诊断</p>
          <h2>${escapeHtml(libraryDisplayName(library))} 的执行空间</h2>
        </div>
      </div>
      <ul class="provider-resolution-list">${rows || '<li class="empty">暂无执行空间诊断。</li>'}</ul>
    </section>
  `;
}

function renderMaintenanceActionsPanel(library: LibrarySnapshot | null) {
  const retiredVectorSpaces = retiredVectorSpaceDiagnostics();

  if (!library) {
    return `
      <section class="panel settings-panel" data-testid="maintenance-actions-panel">
        <div class="panel-head">
          <div>
            <p class="eyebrow">维护</p>
            <h2>维护动作</h2>
          </div>
        </div>
        <p class="empty">先选择一个库，再执行重建或清理动作。</p>
      </section>
    `;
  }

  const retiredSummary = retiredVectorSpaces.length
    ? `当前库还有 ${retiredVectorSpaces.length} 个退役执行空间可立即清理。`
    : "当前没有退役执行空间待清理。";
  const retiredList = retiredVectorSpaces.length
    ? `
        <div class="pill-row compact-row" data-testid="maintenance-retired-vector-spaces">
          ${retiredVectorSpaces
            .map(
              (vectorSpace) => `
                <span class="pill muted">${escapeHtml(vectorSpace.vector_space_id)}</span>
              `
            )
            .join("")}
        </div>
      `
    : "";

  return `
    <section class="panel settings-panel" data-testid="maintenance-actions-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">维护</p>
          <h2>${escapeHtml(libraryDisplayName(library))} 的维护动作</h2>
        </div>
      </div>
      <p class="helper">显式维护动作仍走后台任务路径，用来处理重建与退役执行空间清理，而不是把这些动作散落在搜索主舞台。</p>
      <p class="helper" data-testid="maintenance-retired-summary">${escapeHtml(retiredSummary)}</p>
      ${retiredList}
      <div class="inline-actions">
        <button
          type="button"
          id="diagnostics-rebuild-library-button"
          data-testid="diagnostics-rebuild-library"
        >
          重建当前库
        </button>
        <button
          type="button"
          id="diagnostics-cleanup-retired-vector-spaces-button"
          data-testid="diagnostics-cleanup-retired-vector-spaces"
          class="secondary-button"
          ${retiredVectorSpaces.length ? "" : "disabled"}
        >
          清理退役执行空间
        </button>
      </div>
    </section>
  `;
}

function renderRuntimeHealthPanel() {
  const runtimeHealth = state.runtimeHealth;
  const processRows = runtimeHealth
    ? [runtimeHealth.app, runtimeHealth.qdrant]
        .map((snapshot) => {
          const details = Object.entries(snapshot.details ?? {})
            .map(
              ([key, value]) =>
                `<span class="helper">${escapeHtml(`${key} ${String(value)}`)}</span>`
            )
            .join("");
          return `
            <li class="provider-resolution-row">
              <div>
                <strong>${escapeHtml(snapshot.display_name)}</strong>
                <span class="helper">${escapeHtml(snapshot.message)}</span>
                <span class="helper">${escapeHtml(`最近检查 ${snapshot.last_checked_at}`)}</span>
                ${details}
              </div>
              <span class="pill ${providerSelectionPillClass(snapshot.status)}">${escapeHtml(snapshot.status)}</span>
            </li>
          `;
        })
        .join("")
    : "";
  const providerRows = runtimeHealth
    ? runtimeHealth.providers
        .map((provider) => {
          const details = [
            provider.model_id ? `${provider.provider_id}/${provider.model_id}` : provider.provider_id,
            provider.model_version ? `版本 ${provider.model_version}` : null,
            provider.model_revision ? `修订 ${provider.model_revision}` : null,
            provider.last_probed_at ? `最近探测 ${provider.last_probed_at}` : null,
          ]
            .filter(Boolean)
            .map((value) => `<span class="helper">${escapeHtml(String(value))}</span>`)
            .join("");
          const capabilities = provider.embedding_capabilities
            ? `<span class="helper">${escapeHtml(
                formatEmbeddingCapabilities(provider.embedding_capabilities, { includePrefix: true })
              )}</span>`
            : "";
          const executionInputs = provider.execution_input_types.length
            ? `<span class="helper" data-testid="runtime-provider-execution-input-types">${escapeHtml(
                formatExecutionInputTypes(provider.execution_input_types, { includePrefix: true })
              )}</span>`
            : "";
          const adapters = provider.runtime_adapters.length
            ? `<span class="helper">${escapeHtml(
                `运行时适配器 ${provider.runtime_adapters.join(", ")}`
              )}</span>`
            : "";

          return `
            <li class="provider-resolution-row">
              <div>
                <strong>${escapeHtml(provider.display_name)}</strong>
                <span class="helper">${escapeHtml(provider.message)}</span>
                ${details}
                ${capabilities}
                ${executionInputs}
                ${adapters}
              </div>
              <span class="pill ${providerSelectionPillClass(provider.status)}">${escapeHtml(provider.status)}</span>
            </li>
          `;
        })
        .join("")
    : "";

  return `
    <section class="panel settings-panel" data-testid="runtime-health-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">运行时</p>
          <h2>运行时健康</h2>
        </div>
      </div>
      <ul class="provider-resolution-list">
        ${processRows || '<li class="empty">暂无运行时健康快照。</li>'}
      </ul>
      <div class="inline-actions">
        <a href="${endpoints.appHealth}" target="_blank" rel="noreferrer">App 健康</a>
        <a href="${endpoints.sidecarHealth}" target="_blank" rel="noreferrer">Sidecar 健康</a>
        <a href="${endpoints.qdrantCollections}" target="_blank" rel="noreferrer">Qdrant</a>
      </div>
      <ul class="provider-resolution-list">
        ${providerRows || '<li class="empty">暂无连接运行时诊断。</li>'}
      </ul>
    </section>
  `;
}

function renderModelTestsSection(library: LibrarySnapshot | null) {
  const globalSelection = selectedGlobalModelSelection();
  const librarySelection = selectedLibraryModelSelection();

  return `
    <div class="settings-stack">
      <section class="panel settings-panel settings-explainer-panel">
        <div class="panel-head">
          <div>
            <p class="eyebrow">模型测试</p>
            <h2>测试当前草稿</h2>
          </div>
        </div>
        <p class="helper">模型测试固定面向当前未保存草稿，用来验证输入模态、向量形状和相似度，不直接替代正式保存流程。</p>
      </section>
      ${renderSettingsModelTestPanel({
        scope: "global",
        selection: globalSelection,
        supportedModalities: selectedGlobalTestModalities(),
        modalityDraft: state.globalModelTestModalityDraft,
        textDraft: state.globalModelTestTextDraft,
        file: state.globalModelTestFile,
        comparisonModalityDraft: state.globalModelTestComparisonModalityDraft,
        comparisonTextDraft: state.globalModelTestComparisonTextDraft,
        comparisonFile: state.globalModelTestComparisonFile,
        result: state.globalModelTestResult,
        error: state.globalModelTestError,
        pending: state.globalModelTestPending,
      })}
      ${
        library
          ? renderSettingsModelTestPanel({
              scope: "library",
              selection: librarySelection,
              supportedModalities: selectedLibraryTestModalities(),
              modalityDraft: state.libraryModelTestModalityDraft,
              textDraft: state.libraryModelTestTextDraft,
              file: state.libraryModelTestFile,
              comparisonModalityDraft: state.libraryModelTestComparisonModalityDraft,
              comparisonTextDraft: state.libraryModelTestComparisonTextDraft,
              comparisonFile: state.libraryModelTestComparisonFile,
              result: state.libraryModelTestResult,
              error: state.libraryModelTestError,
              pending: state.libraryModelTestPending,
            })
          : ""
      }
    </div>
  `;
}

function renderSettingsNavRail() {
  const sections: SettingsSection[] = [
    "content-types",
    "library-overrides",
    "providers",
    "model-tests",
    "diagnostics",
  ];

  return `
    <nav class="settings-nav-rail" data-testid="settings-nav-rail" aria-label="设置章节">
      <div class="settings-nav-rail-head">
        <p class="eyebrow">章节导航</p>
        <p class="helper">先定默认，再核对当前库差异；连接、测试与诊断后置。</p>
      </div>
      ${sections
        .map(
          (section) => {
            const pill = settingsSectionPill(section, selectedLibrary());
            return `
            <button
              type="button"
              class="settings-nav-button ${state.selectedSettingsSection === section ? "active" : ""}"
              data-testid="settings-nav-${escapeHtml(section)}"
              data-settings-section="${escapeHtml(section)}"
            >
              <span class="settings-nav-icon">${renderUiIcon(settingsSectionIcon(section))}</span>
              <span class="settings-nav-copy">
                <strong>${escapeHtml(settingsSectionLabel(section))}</strong>
                <span class="helper">${escapeHtml(settingsSectionNavSummary(section, selectedLibrary()))}</span>
              </span>
              <span class="pill ${pill.pillClass} settings-nav-pill">${escapeHtml(pill.label)}</span>
            </button>
          `;
          }
        )
        .join("")}
    </nav>
  `;
}

function renderSettingsPanel(library: LibrarySnapshot | null) {
  let activeSurface = "";
  if (state.selectedSettingsSection === "providers") {
    activeSurface = renderProviderConfigsPanel();
  } else if (state.selectedSettingsSection === "library-overrides") {
    activeSurface = `
      <div class="settings-dual-surface" data-testid="library-overrides-surface">
        ${renderLibraryContentTypesPanel(library, false)}
        ${renderResolvedContentModelsPanel(library)}
      </div>
    `;
  } else if (state.selectedSettingsSection === "model-tests") {
    activeSurface = renderModelTestsSection(library);
  } else if (state.selectedSettingsSection === "diagnostics") {
    activeSurface = `
      <div class="settings-stack">
        ${renderRuntimeHealthPanel()}
        ${renderMaintenanceActionsPanel(library)}
        ${renderVectorSpaceDiagnosticsPanel(library)}
      </div>
    `;
  } else {
    activeSurface = renderGlobalContentTypesPanel(false);
  }

  return `
    <section class="settings-workspace" data-testid="settings-workspace">
      ${renderLibraryContextCluster(library, "workspace")}
      <div class="settings-workspace-head">
        <div>
          <p class="eyebrow">设置工作区</p>
          <h2>先配置默认，再核对当前库差异</h2>
        </div>
        <p class="helper">章节化设置先服务正式配置动作，再把模型测试与诊断后置到需要的时候展开。</p>
      </div>
      <div class="settings-layout">
        ${renderSettingsNavRail()}
        <div class="settings-active-surface">
          ${renderSettingsStage(state.selectedSettingsSection, library, activeSurface)}
        </div>
      </div>
    </section>
  `;
}

function renderInventoryDetailPanel(library: LibrarySnapshot | null) {
  const source = selectedInventorySource();
  const mobileSheetOpen = inventoryDetailSheetIsOpen();
  const mobileSheetClass = mobileSheetOpen ? "mobile-sheet-open" : "mobile-sheet-closed";
  const mobileSheetBackdrop = mobileSheetOpen
    ? `<button
        type="button"
        class="mobile-sheet-backdrop"
        data-testid="inventory-detail-sheet-backdrop"
        data-mobile-sheet-close="inventory"
        aria-label="关闭来源详情"
      ></button>`
    : "";

  if (!library) {
    return `
      ${mobileSheetBackdrop}
      <section
        class="panel inventory-detail-panel mobile-sheet-panel ${mobileSheetClass}"
        data-testid="inventory-detail-panel"
      >
        <div class="mobile-sheet-bar">
          <span class="mobile-sheet-handle" aria-hidden="true"></span>
          <button
            type="button"
            class="secondary-button mobile-sheet-close"
            data-testid="inventory-detail-sheet-close-button"
            data-mobile-sheet-close="inventory"
          >
            关闭
          </button>
        </div>
        <div class="panel-head">
          <div>
            <p class="eyebrow">详情</p>
            <h2>来源详情</h2>
          </div>
        </div>
        <p class="empty">先选择一个库，再浏览来源。</p>
      </section>
    `;
  }

  if (!source) {
    return `
      ${mobileSheetBackdrop}
      <section
        class="panel inventory-detail-panel mobile-sheet-panel ${mobileSheetClass}"
        data-testid="inventory-detail-panel"
      >
        <div class="mobile-sheet-bar">
          <span class="mobile-sheet-handle" aria-hidden="true"></span>
          <button
            type="button"
            class="secondary-button mobile-sheet-close"
            data-testid="inventory-detail-sheet-close-button"
            data-mobile-sheet-close="inventory"
          >
            关闭
          </button>
        </div>
        <div class="panel-head">
          <div>
            <p class="eyebrow">详情</p>
            <h2>来源详情</h2>
          </div>
        </div>
        <p class="empty">从左侧列表选中一个来源后，这里会显示它的状态、归属和可搜索单元摘要。</p>
      </section>
    `;
  }

  const representativeVisual = selectedInventoryRepresentativeVisualUnit(source);
  const representativePreview = selectedInventoryRepresentativePreview(source);
  const page = pageLabel(representativeVisual?.locator);
  const segment = videoLabel(representativeVisual?.locator);

  return `
    ${mobileSheetBackdrop}
    <section
      class="panel inventory-detail-panel mobile-sheet-panel ${mobileSheetClass}"
      data-testid="inventory-detail-panel"
    >
      <div class="mobile-sheet-bar">
        <span class="mobile-sheet-handle" aria-hidden="true"></span>
        <button
          type="button"
          class="secondary-button mobile-sheet-close"
          data-testid="inventory-detail-sheet-close-button"
          data-mobile-sheet-close="inventory"
        >
          关闭
        </button>
      </div>
      <div class="panel-head">
        <div>
          <p class="eyebrow">详情</p>
          <h2>${escapeHtml(sourceName(source.source_path))}</h2>
        </div>
      </div>
      <div class="detail-card inventory-detail-card">
        <div class="detail-preview inventory-detail-preview">
          ${
            representativeVisual && representativePreview
              ? renderPreviewSurface(
                  {
                    ...representativeVisual,
                    source_path: source.source_path,
                  },
                  representativePreview,
                  "inventory-detail-preview"
                )
              : `
                <div class="preview-placeholder" data-testid="inventory-detail-preview">
                  <p>当前来源还没有可用的代表性预览。完成一次 refresh / rescan 后，这里会优先显示图像、页预览或视频片段。</p>
                </div>
              `
          }
        </div>
        <div class="detail-head">
          <div class="detail-kicker">
            <span class="pill ${sourceStatusPillClass(source.status)}">${escapeHtml(sourceStatusDisplayName(source.status))}</span>
            <span class="pill muted">${escapeHtml(source.kind)}</span>
            <span class="pill muted">${escapeHtml(source.source_type)}</span>
            ${representativeVisual ? `<span class="pill ready">${escapeHtml(representativeVisual.kind)}</span>` : ""}
            ${page ? `<span class="pill muted">${escapeHtml(page)}</span>` : ""}
            ${segment ? `<span class="pill muted">${escapeHtml(segment)}</span>` : ""}
          </div>
          <h4>${escapeHtml(sourceName(source.source_path))}</h4>
          <p class="helper">${escapeHtml(source.source_path)}</p>
        </div>
        <div class="detail-action-row">
          ${
            representativePreview
              ? `<a data-testid="inventory-preview-link" href="${escapeHtml(representativePreview.url)}" target="_blank" rel="noreferrer">打开预览</a>`
              : ""
          }
          ${
            representativeVisual &&
            (representativeVisual.kind === "image" || representativeVisual.kind === "document_page")
              ? `<button type="button" class="secondary-button" data-testid="inventory-use-as-query-image-button" data-use-query-visual-unit-id="${escapeHtml(representativeVisual.visual_unit_id)}">作为查询图片</button>`
              : ""
          }
          ${
            representativeVisual && representativeVisual.kind === "document_page"
              ? `<button type="button" class="secondary-button" data-testid="inventory-use-as-query-document-button" data-use-query-document-visual-unit-id="${escapeHtml(representativeVisual.visual_unit_id)}">作为查询文档</button>`
              : ""
          }
          ${
            representativeVisual && representativeVisual.kind === "video_segment"
              ? `<button type="button" class="secondary-button" data-testid="inventory-use-as-query-video-button" data-use-query-video-visual-unit-id="${escapeHtml(representativeVisual.visual_unit_id)}">作为查询视频</button>`
              : ""
          }
        </div>
        <div class="inventory-detail-summary">
          <article class="inventory-detail-metric">
            <span class="inventory-detail-metric-label">当前状态</span>
            <strong class="inventory-detail-metric-value">${escapeHtml(sourceStatusDisplayName(source.status))}</strong>
          </article>
          <article class="inventory-detail-metric">
            <span class="inventory-detail-metric-label">来源类型</span>
            <strong class="inventory-detail-metric-value">${escapeHtml(sourceTypeDisplayName(source.source_type))}</strong>
          </article>
          <article class="inventory-detail-metric">
            <span class="inventory-detail-metric-label">可搜索对象</span>
            <strong class="inventory-detail-metric-value">${escapeHtml(source.visual_unit_count)}</strong>
          </article>
          <article class="inventory-detail-metric">
            <span class="inventory-detail-metric-label">代表对象</span>
            <strong class="inventory-detail-metric-value">${escapeHtml(
              representativeVisual ? visualUnitKindDisplayName(representativeVisual.kind) : "待生成"
            )}</strong>
          </article>
        </div>
        <dl class="stats">
          <div><dt>来源路径</dt><dd class="detail-path">${escapeHtml(source.source_path)}</dd></div>
          <div><dt>来源根</dt><dd>${escapeHtml(sourceRootInventoryLabel(source))}</dd></div>
          <div><dt>来源编号</dt><dd>${escapeHtml(source.source_id)}</dd></div>
          <div><dt>可搜索对象</dt><dd>${escapeHtml(source.visual_unit_count)}</dd></div>
        </dl>
        <div class="detail-grid">
          <div class="detail-block">
            <h5>当前库</h5>
            <p class="helper">${escapeHtml(libraryDisplayName(library))} · ${escapeHtml(library.id)}</p>
            ${
              source.status_reason
                ? `<p class="helper">${escapeHtml(source.status_reason)}</p>`
                : `<p class="helper">当前来源处于 ${escapeHtml(sourceStatusDisplayName(source.status))} 状态，可继续通过过滤器核对它在库中的归属。</p>`
            }
          </div>
          <div class="detail-block">
            <h5>代表性对象</h5>
            ${
              representativeVisual
                ? `
                  <p class="helper">${escapeHtml(representativeVisual.visual_unit_id)} · ${escapeHtml(representativeVisual.kind)}</p>
                  <p class="helper">${escapeHtml(page || segment || "当前以整张图像或默认对象作为代表性预览。")}</p>
                `
                : `<p class="helper">当前来源还没有可复用的代表性对象；来源列表仍保持只读观察语义。</p>`
            }
          </div>
        </div>
      </div>
    </section>
  `;
}

function renderInventoryWorkspace(library: LibrarySnapshot | null) {
  const filterSummaryItems = inventoryFilterSummaryItems();
  const list = state.librarySources.length
    ? `
        <ul class="inventory-source-list" data-testid="library-source-list">
          ${state.librarySources
            .map(
              (source) => `
                <li
                  class="inventory-source-row ${source.source_id === state.selectedInventorySourceId ? "active" : ""}"
                  data-testid="library-source-card"
                  data-source-id="${escapeHtml(source.source_id)}"
                >
                  <button
                    type="button"
                    class="inventory-source-select"
                    data-source-id="${escapeHtml(source.source_id)}"
                  >
                    <div class="inventory-source-visual">
                      ${renderInventorySourceThumbnail(source)}
                    </div>
                    <div class="inventory-source-main">
                      <strong class="inventory-source-name">${escapeHtml(sourceName(source.source_path))}</strong>
                      <p class="helper inventory-source-path">${escapeHtml(source.source_path)}</p>
                      <div class="detail-kicker inventory-source-pills">
                        <span class="pill muted">${escapeHtml(sourceRootInventoryLabel(source))}</span>
                        <span class="pill muted">${escapeHtml(sourceTypeDisplayName(source.source_type))}</span>
                        <span class="pill muted">${escapeHtml(
                          visualUnitKindDisplayName(inventoryRepresentativeKind(source))
                        )}</span>
                      </div>
                    </div>
                    <div class="inventory-source-meta">
                      <span class="pill ${sourceStatusPillClass(source.status)}">${escapeHtml(sourceStatusDisplayName(source.status))}</span>
                      <strong class="inventory-source-count">${escapeHtml(source.visual_unit_count)} 个对象</strong>
                      ${
                        source.status_reason
                          ? `<span class="helper inventory-source-reason">${escapeHtml(source.status_reason)}</span>`
                          : ""
                      }
                    </div>
                  </button>
                </li>
              `
            )
            .join("")}
        </ul>
      `
    : '<p class="empty" data-testid="library-source-empty">当前筛选条件下没有来源内容。</p>';

  return `
    <section class="inventory-workspace" data-testid="inventory-panel">
      ${renderLibraryContextCluster(library, "workspace")}
      ${renderInventoryLibraryManagementBand(library)}
      <div class="inventory-workspace-head">
        <div>
          <p class="eyebrow">库管理工作区</p>
          <h2>浏览来源，而不是退回后台表</h2>
        </div>
        <p class="helper">先判断当前库里有哪些来源、它们的状态是否正常，再在右侧阅读详情和代表性预览。</p>
      </div>
      ${renderInventorySummaryBar()}
      <div class="inventory-layout">
        <section class="panel inventory-panel inventory-panel-main">
          <div class="inventory-filter-dock">
            <div class="inventory-filter-head">
              <div>
                <p class="eyebrow">来源过滤</p>
                <h3>筛选当前库来源</h3>
              </div>
              ${renderInventoryActionRow(library)}
            </div>
            <div class="filter-grid inventory-filter-grid">
              <label>
                <span>来源根</span>
                <select id="source-filter-root" data-testid="source-filter-root" ${library ? "" : "disabled"}>
                  <option value="">全部来源根</option>
                  <option value="manual" ${state.inventoryFilters.sourceRootId === "manual" ? "selected" : ""}>手动导入</option>
                  ${state.sourceRoots
                    .map(
                      (sourceRoot) => `
                        <option value="${escapeHtml(sourceRoot.source_root_id)}" ${state.inventoryFilters.sourceRootId === sourceRoot.source_root_id ? "selected" : ""}>
                          ${escapeHtml(sourceRoot.root_path)}
                        </option>
                      `
                    )
                    .join("")}
                </select>
              </label>
              <label>
                <span>来源类型</span>
                <select id="source-filter-type" data-testid="source-filter-type" ${library ? "" : "disabled"}>
                  <option value="">全部类型</option>
                  <option value="image" ${state.inventoryFilters.sourceType === "image" ? "selected" : ""}>图片</option>
                  <option value="pdf" ${state.inventoryFilters.sourceType === "pdf" ? "selected" : ""}>PDF</option>
                  <option value="video" ${state.inventoryFilters.sourceType === "video" ? "selected" : ""}>视频</option>
                </select>
              </label>
              <label>
                <span>来源状态</span>
                <select id="source-filter-status" data-testid="source-filter-status" ${library ? "" : "disabled"}>
                  <option value="">全部状态</option>
                  <option value="active" ${state.inventoryFilters.sourceStatus === "active" ? "selected" : ""}>正常</option>
                  <option value="invalidated" ${state.inventoryFilters.sourceStatus === "invalidated" ? "selected" : ""}>已失效</option>
                  <option value="out_of_scope" ${state.inventoryFilters.sourceStatus === "out_of_scope" ? "selected" : ""}>超出范围</option>
                </select>
              </label>
            </div>
            <p class="helper" data-testid="inventory-filter-summary">
              当前显示 ${state.librarySources.length} / ${state.inventorySummary.total} 条来源记录。
            </p>
            <div class="pill-row inventory-filter-pills" data-testid="inventory-filter-pills">
              ${
                filterSummaryItems.length
                  ? filterSummaryItems
                      .map((item) => `<span class="pill muted">${escapeHtml(item)}</span>`)
                      .join("")
                  : '<span class="pill ready">当前显示全部来源</span>'
              }
            </div>
          </div>
          ${list}
        </section>
        ${renderInventoryDetailPanel(library)}
      </div>
    </section>
  `;
}

function renderLibrarySourcesPanel(library) {
  return renderInventoryWorkspace(library);
}

function renderPreviewSurface(visualUnit, preview, testId = "visual-preview") {
  const title = `${visualUnit.kind} · ${sourceName(visualUnit.source_path)}`;

  if (visualUnit.kind === "image") {
    return `
      <img
        class="preview-image"
        data-testid="${escapeHtml(testId)}"
        src="${escapeHtml(preview.url)}"
        alt="${escapeHtml(title)}"
        loading="lazy"
      />
    `;
  }

  if (visualUnit.kind === "video_segment") {
    const startMs = visualUnit.locator?.start_ms ?? 0;
    const endMs = visualUnit.locator?.end_ms ?? 0;
    return `
      <video
        class="preview-video"
        data-testid="${escapeHtml(testId)}"
        data-preview-kind="video"
        data-start-ms="${escapeHtml(startMs)}"
        data-end-ms="${escapeHtml(endMs)}"
        src="${escapeHtml(preview.url)}"
        controls
        preload="metadata"
      ></video>
    `;
  }

  return `
    <iframe
      class="preview-frame"
      data-testid="${escapeHtml(testId)}"
      src="${escapeHtml(preview.url)}"
      title="${escapeHtml(title)}"
      loading="lazy"
    ></iframe>
  `;
}

function renderSearchResultPreview(result: SearchResultItem) {
  const title = `${visualUnitKindDisplayName(result.kind)} · ${sourceName(result.source_path)}`;

  if (result.kind === "image") {
    return `
      <img
        class="result-preview-image"
        data-testid="result-preview"
        src="${escapeHtml(result.preview.url)}"
        alt="${escapeHtml(title)}"
        loading="lazy"
      />
    `;
  }

  if (result.kind === "video_segment") {
    const startMs = result.locator?.start_ms ?? 0;
    const endMs = result.locator?.end_ms ?? 0;
    return `
      <video
        class="result-preview-video"
        data-testid="result-preview"
        data-preview-kind="video"
        data-start-ms="${escapeHtml(startMs)}"
        data-end-ms="${escapeHtml(endMs)}"
        src="${escapeHtml(result.preview.url)}"
        muted
        playsinline
        preload="metadata"
      ></video>
    `;
  }

  return `
    <div
      class="result-preview-placeholder"
      data-testid="result-preview"
      role="img"
      aria-label="${escapeHtml(title)}"
    >
      <span class="result-preview-placeholder-sheet" aria-hidden="true"></span>
      <span class="result-preview-placeholder-label">${escapeHtml(sourceTypeDisplayName(result.source_type))}</span>
    </div>
  `;
}

function searchResultLibraryBreakdown() {
  const results = state.searchOutcome?.results ?? [];
  const breakdown = new Map<string, { libraryId: string; label: string; count: number }>();
  results.forEach((item) => {
    const libraryId = item.library_id?.trim();
    if (!libraryId) {
      return;
    }
    const existing = breakdown.get(libraryId);
    if (existing) {
      existing.count += 1;
      return;
    }
    const library = libraryById(libraryId);
    breakdown.set(libraryId, {
      libraryId,
      label: library ? libraryDisplayName(library) : libraryId,
      count: 1,
    });
  });
  return [...breakdown.values()];
}

function activeSearchResultLibraryFocus() {
  if (!allLibrariesTextScopeActive()) {
    return null;
  }
  const libraryId = state.searchResultLibraryFocusId.trim();
  if (!libraryId) {
    return null;
  }
  return searchResultLibraryBreakdown().find((item) => item.libraryId === libraryId) ?? null;
}

function visibleSearchResults() {
  const results = state.searchOutcome?.results ?? [];
  const activeFocus = activeSearchResultLibraryFocus();
  if (!activeFocus) {
    return results;
  }
  return results.filter((item) => item.library_id === activeFocus.libraryId);
}

function groupedSearchResults(results: SearchResultItem[]) {
  const groups = new Map<
    string,
    { libraryId: string; label: string; count: number; items: SearchResultItem[] }
  >();
  results.forEach((item) => {
    const libraryId = item.library_id?.trim() || "unknown";
    const existing = groups.get(libraryId);
    if (existing) {
      existing.items.push(item);
      existing.count += 1;
      return;
    }
    const library = libraryById(libraryId);
    groups.set(libraryId, {
      libraryId,
      label: library ? libraryDisplayName(library) : libraryId,
      count: 1,
      items: [item],
    });
  });
  return [...groups.values()];
}

function searchResultGroupSummary(libraryId: string, count: number) {
  if (libraryId === state.selectedLibraryId) {
    return `当前工作库 · ${count} 条结果`;
  }
  return `${count} 条结果 · 可留在 Search 里先聚焦这一组，或直接进入库管理。`;
}

function renderSearchResultCard(
  item: SearchResultItem,
  layout: "default" | "grouped" | "focused" = "default"
) {
  const scoreLabel = formatScore(item.score);
  const page = pageLabel(item.locator);
  const segment = videoLabel(item.locator);
  return `
    <li
      class="result-card result-card-${layout} ${`${item.library_id}:${item.visual_unit_id}` === selectedVisualUnitId() ? "active" : ""}"
      data-testid="result-card"
      data-kind="${escapeHtml(item.kind)}"
      data-visual-unit-id="${escapeHtml(item.visual_unit_id)}"
    >
      <button
        type="button"
        class="result-select"
        data-visual-unit-id="${escapeHtml(item.visual_unit_id)}"
        data-visual-unit-library-id="${escapeHtml(item.library_id)}"
      >
        <div class="result-visual">
          ${renderSearchResultPreview(item)}
        </div>
        <div class="result-body result-body-${layout}">
          <div class="result-topline">
            <span class="pill ${item.kind === "image" ? "ready" : "pending"}">${escapeHtml(
              visualUnitKindDisplayName(item.kind)
            )}</span>
            ${page ? `<span class="pill muted">${escapeHtml(page)}</span>` : ""}
            ${segment ? `<span class="pill muted">${escapeHtml(segment)}</span>` : ""}
          </div>
          <div class="result-title-row">
            <strong class="result-title">${escapeHtml(sourceName(item.source_path))}</strong>
            ${scoreLabel ? `<span class="pill score-pill" data-testid="result-score">相似度 ${escapeHtml(scoreLabel)}</span>` : ""}
          </div>
          <span class="helper result-path">${escapeHtml(item.source_path)}</span>
        </div>
      </button>
      <div class="inline-actions result-actions">
        <button type="button" class="secondary-button" data-visual-unit-id="${escapeHtml(item.visual_unit_id)}" data-visual-unit-library-id="${escapeHtml(item.library_id)}">查看详情</button>
        ${
          item.kind === "image" || item.kind === "document_page"
            ? `<button type="button" class="secondary-button" data-testid="use-as-query-image-button" data-use-query-visual-unit-id="${escapeHtml(item.visual_unit_id)}" data-use-query-library-id="${escapeHtml(item.library_id)}">作为查询图片</button>`
            : ""
        }
        ${
          item.kind === "document_page"
            ? `<button type="button" class="secondary-button" data-testid="use-as-query-document-button" data-use-query-document-visual-unit-id="${escapeHtml(item.visual_unit_id)}" data-use-query-library-id="${escapeHtml(item.library_id)}">作为查询文档</button>`
            : ""
        }
        ${
          item.kind === "video_segment"
            ? `<button type="button" class="secondary-button" data-testid="use-as-query-video-button" data-use-query-video-visual-unit-id="${escapeHtml(item.visual_unit_id)}" data-use-query-library-id="${escapeHtml(item.library_id)}">作为查询视频</button>`
            : ""
        }
        <a href="${escapeHtml(item.preview.url)}" target="_blank" rel="noreferrer">打开预览</a>
      </div>
    </li>
  `;
}

function renderSearchResultGroup(group: {
  libraryId: string;
  label: string;
  count: number;
  items: SearchResultItem[];
}) {
  return `
    <section
      class="result-library-group"
      data-testid="search-result-library-group"
      data-library-id="${escapeHtml(group.libraryId)}"
    >
      <div class="result-library-group-header">
        <div class="result-library-group-copy">
          <div class="result-library-group-meta">
            <span class="scope-label">${escapeHtml(group.libraryId === state.selectedLibraryId ? "当前库" : "命中库")}</span>
            <span class="helper" data-testid="search-result-library-group-count">${escapeHtml(`${group.count} 条结果`)}</span>
          </div>
          <strong data-testid="search-result-library-group-heading">${escapeHtml(group.label)}</strong>
          <p class="helper result-library-group-summary" data-testid="search-result-library-group-summary">${escapeHtml(
            searchResultGroupSummary(group.libraryId, group.count)
          )}</p>
        </div>
        <div class="inline-actions result-library-group-actions">
          <button
            type="button"
            class="secondary-button"
            data-testid="search-result-library-group-focus-${escapeHtml(group.libraryId)}"
            data-search-result-library-focus="${escapeHtml(group.libraryId)}"
          >
            仅看这个库
          </button>
          <button
            type="button"
            class="secondary-button"
            data-testid="search-result-library-group-open-inventory-${escapeHtml(group.libraryId)}"
            data-open-hit-library-id="${escapeHtml(group.libraryId)}"
          >
            在库管理查看
          </button>
        </div>
      </div>
      <ul class="result-list result-group-list">
        ${group.items.map((item) => renderSearchResultCard(item, "grouped")).join("")}
      </ul>
    </section>
  `;
}

function renderVisualPreview() {
  if (!state.selectedVisualUnit) {
    return `
      <div class="preview-placeholder" data-testid="visual-preview">
        <p>选择一个结果或导入项后，这里会显示图片或 PDF 页预览。</p>
      </div>
    `;
  }

  const visualUnit = state.selectedVisualUnit.visual_unit;
  const preview = state.selectedVisualUnit.preview;
  return renderPreviewSurface(visualUnit, preview, "visual-preview");
}

function renderVisualUnitDetail() {
  if (!state.selectedVisualUnit) {
    return '<p class="empty">从结果列表选择一个对象后，这里会显示预览与来源信息。</p>';
  }

  const visualUnit = state.selectedVisualUnit.visual_unit;
  const originLibraryId = selectedVisualUnitOriginLibraryId();
  const originLibrary = libraryById(originLibraryId);
  const page = pageLabel(visualUnit.locator);
  const segment = videoLabel(visualUnit.locator);
  const showCrossLibraryContext = allLibrariesTextScopeActive() && originLibraryId;
  return `
    <div class="detail-card" data-testid="visual-unit-detail">
      <div class="detail-preview">
        ${renderVisualPreview()}
      </div>
      <div class="detail-head">
        <div class="detail-kicker">
          ${
            state.searchScope === "all_libraries" && originLibraryId
              ? `<span class="pill muted">${escapeHtml(originLibrary ? libraryDisplayName(originLibrary) : originLibraryId)}</span>`
              : ""
          }
          <span class="pill ready">${escapeHtml(visualUnitKindDisplayName(visualUnit.kind))}</span>
          ${page ? `<span class="pill muted">${escapeHtml(page)}</span>` : ""}
          ${segment ? `<span class="pill muted">${escapeHtml(segment)}</span>` : ""}
        </div>
        <h4>${escapeHtml(sourceName(visualUnit.source_path))}</h4>
      </div>
      ${
        showCrossLibraryContext
          ? `
            <section class="detail-library-context" data-testid="detail-library-context">
              <div class="detail-library-context-copy">
                <span class="scope-label">命中库</span>
                <strong data-testid="detail-hit-library-name">${escapeHtml(
                  originLibrary ? libraryDisplayName(originLibrary) : originLibraryId
                )}</strong>
                <p class="helper" data-testid="detail-hit-library-summary">${escapeHtml(
                  originLibraryId === state.selectedLibraryId
                    ? "你当前已经在这个库的上下文里阅读结果。"
                    : `当前选中库仍是 ${libraryDisplayName(selectedLibrary()) || state.selectedLibraryId}；继续管理来源或复用结果时会自动切到命中库，如需先核对 readiness，也可以直接进入库管理。`
                )}</p>
              </div>
              <div class="inline-actions detail-library-context-actions">
                <button
                  type="button"
                  class="secondary-button"
                  data-testid="detail-open-hit-library-inventory"
                  data-open-hit-library-id="${escapeHtml(originLibraryId)}"
                >
                  在库管理查看
                </button>
              </div>
            </section>
          `
          : ""
      }
      <dl class="stats">
        ${
          originLibraryId
            ? `<div><dt>命中库</dt><dd>${escapeHtml(originLibrary ? `${libraryDisplayName(originLibrary)} (${originLibraryId})` : originLibraryId)}</dd></div>`
            : ""
        }
        <div><dt>对象编号</dt><dd>${escapeHtml(visualUnit.visual_unit_id)}</dd></div>
        <div><dt>来源类型</dt><dd>${escapeHtml(sourceTypeDisplayName(visualUnit.source_type))}</dd></div>
        <div><dt>来源路径</dt><dd class="detail-path">${escapeHtml(visualUnit.source_path)}</dd></div>
      </dl>
      <details class="detail-technical-disclosure" data-testid="detail-technical-disclosure">
        <summary>技术信息</summary>
        <div class="detail-technical-content" data-testid="detail-technical-content">
          <div class="detail-grid">
            <div class="detail-block">
              <h5>定位信息</h5>
              <pre>${escapeHtml(JSON.stringify(visualUnit.locator, null, 2))}</pre>
            </div>
            <div class="detail-block">
              <h5>阅读提示</h5>
              <p class="helper">当前详情面会在后台轮询期间保持预览挂载不变，方便继续阅读和复用结果。</p>
            </div>
          </div>
          <div class="detail-block">
            <h5>邻近上下文</h5>
            <pre>${escapeHtml(JSON.stringify(state.selectedVisualUnit.neighbor_context, null, 2))}</pre>
          </div>
        </div>
      </details>
    </div>
  `;
}

function renderJobs() {
  if (!state.selectedLibraryId) {
    return '<p class="empty">先创建或选择一个库，再查看任务。</p>';
  }

  if (!state.jobs.length) {
    return '<p class="empty">当前库还没有任务。</p>';
  }

  return `
    <ul class="job-list" data-testid="job-list">
      ${state.jobs
        .map(
          (job) => `
            <li class="job-card" data-testid="job-card" data-job-id="${escapeHtml(job.job_id)}" data-job-status="${escapeHtml(job.status)}">
              <div class="job-meta">
                <span class="pill ${jobPillClass(job.status)}">${escapeHtml(job.status)}</span>
                <span>${escapeHtml(job.job_id)}</span>
              </div>
              <h4>${escapeHtml(job.kind)} · ${escapeHtml(job.phase)}</h4>
              <p>${escapeHtml(job.current_attempt.summary)}</p>
              <p class="helper" data-testid="job-attempt-lineage">${escapeHtml(formatJobAttemptLabel(job))}</p>
              <div class="detail-action-row">
                <small>${job.progress.completed}/${job.progress.total} ${escapeHtml(job.progress.unit)}</small>
                ${
                  canCancelJob(job)
                    ? `
                      <button
                        type="button"
                        class="secondary-button"
                        data-testid="job-cancel-button"
                        data-job-cancel-id="${escapeHtml(job.job_id)}"
                      >
                        取消任务
                      </button>
                    `
                    : ""
                }
                ${
                  canResumeJob(job)
                    ? `
                      <button
                        type="button"
                        class="secondary-button"
                        data-testid="job-resume-button"
                        data-job-resume-id="${escapeHtml(job.job_id)}"
                      >
                        继续任务
                      </button>
                    `
                    : ""
                }
                ${
                  canRetryJob(job)
                    ? `
                      <button
                        type="button"
                        class="secondary-button"
                        data-testid="job-retry-button"
                        data-job-retry-id="${escapeHtml(job.job_id)}"
                      >
                        重试任务
                      </button>
                    `
                    : ""
                }
              </div>
            </li>
          `
        )
        .join("")}
    </ul>
  `;
}

function renderSearchOutcome() {
  const library = selectedLibrary();

  if (!state.searchOutcome) {
    return "";
  }

  if (state.searchOutcome.error) {
    const details = state.searchOutcome.error.details?.content_types ?? [];
    return `
      <div class="notice error" data-testid="search-error-notice">
        <p class="eyebrow">这次查询没有完成</p>
        <h4 data-testid="search-error-code">${escapeHtml(state.searchOutcome.error.code)}</h4>
        <p data-testid="search-error-message">${escapeHtml(state.searchOutcome.error.message)}</p>
        ${
          details.length
            ? `<p class="helper">部分内容类型当前没有完成准备或配置；这次失败不是“没有命中结果”，可以直接检查当前库覆盖。</p>`
            : ""
        }
        ${
          details.length
            ? `<ul class="data-list" data-testid="search-error-details">
                ${details
                  .map(
                    (item) => `
                      <li>
                        <strong>${escapeHtml(contentTypeDisplayName(item.content_type ?? "unknown"))}</strong>
                        <span>${escapeHtml(item.job?.job_id ?? "no-job")} · ${escapeHtml(item.job?.phase ?? item.status)}</span>
                      </li>
                    `
                  )
                  .join("")}
              </ul>`
            : ""
        }
        ${
          details.length
            ? `
              <div class="inline-actions">
                <button
                  type="button"
                  data-testid="search-error-open-library-overrides"
                  data-open-settings-section="library-overrides"
                >
                  前往当前库覆盖
                </button>
              </div>
            `
            : ""
        }
      </div>
    `;
  }

  const allResults = state.searchOutcome.results ?? [];
  const results = visibleSearchResults();
  const unsupportedContentTypes = state.searchOutcome.unsupported_content_types ?? [];
  const resultLibraryCount = new Set(allResults.map((item) => item.library_id).filter(Boolean)).size;
  const libraryBreakdown = searchResultLibraryBreakdown();
  const activeLibraryFocus = activeSearchResultLibraryFocus();
  const groupedResults = groupedSearchResults(results);
  const showLibraryGroupedResults =
    allLibrariesTextScopeActive() && !activeLibraryFocus && groupedResults.length > 1;
  const resultsSurfaceMode = showLibraryGroupedResults
    ? "grouped"
    : activeLibraryFocus
      ? "focused"
      : "default";
  if (!results.length) {
    return `
      <div class="notice neutral" data-testid="search-empty-notice">
        <p class="eyebrow">这次查询没有命中</p>
        <h4>${escapeHtml(
          allLibrariesTextScopeActive() ? "当前范围可搜索，但本次没有返回结果" : "当前库可搜索，但本次没有返回结果"
        )}</h4>
        <p>${escapeHtml(
          allLibrariesTextScopeActive()
            ? "可以换一个查询词、放宽过滤器，或确认当前范围里的相关内容已经导入到至少一个库。"
            : "可以换一个查询词、放宽过滤器，或确认当前范围里的相关内容已经导入并进入当前库。"
        )}</p>
        ${
          unsupportedContentTypes.length
            ? `<p class="helper">另外有部分内容类型在这次查询里被跳过；如果这不是预期，可以检查当前库覆盖。</p>
               <ul class="data-list" data-testid="search-unsupported-content-types">
                ${unsupportedContentTypes
                  .map(
                    (item) => `
                      <li>
                        <strong>${escapeHtml(contentTypeDisplayName(item.content_type))}</strong>
                        <span>${escapeHtml(item.model)} · ${escapeHtml(item.reason)}</span>
                      </li>
                    `
                  )
                  .join("")}
              </ul>
              ${renderSearchStatusNextStep(library, "outcome")}`
            : ""
        }
      </div>
    `;
  }

  return `
    <div
      class="search-results-surface search-results-surface-${resultsSurfaceMode}"
      data-testid="search-results-surface"
      data-search-results-surface="${resultsSurfaceMode}"
    >
    ${
      unsupportedContentTypes.length
        ? `<div class="notice warning" data-testid="search-unsupported-content-types">
            <h4>部分内容类型已跳过</h4>
            <ul class="data-list">
              ${unsupportedContentTypes
                .map(
                  (item) => `
                    <li>
                      <strong>${escapeHtml(contentTypeDisplayName(item.content_type))}</strong>
                      <span>${escapeHtml(item.model)} · ${escapeHtml(item.reason)}</span>
                    </li>
                  `
                )
                .join("")}
            </ul>
          </div>`
        : ""
    }
    <div class="results-summary">
      <h3 data-testid="search-results-summary">${
        activeLibraryFocus
          ? `当前查看 ${escapeHtml(activeLibraryFocus.label)} · ${results.length} 条结果`
          : `命中 ${results.length} 条结果${allLibrariesTextScopeActive() && resultLibraryCount ? ` · 来自 ${resultLibraryCount} 个库` : ""}`
      }</h3>
    </div>
    ${
      allLibrariesTextScopeActive() && libraryBreakdown.length
        ? `
          <section class="results-library-strip" data-testid="search-result-library-strip">
            <span class="scope-label">命中库分布</span>
            <div class="results-library-chips">
              <button
                type="button"
                class="secondary-button result-library-chip ${activeLibraryFocus ? "" : "active"}"
                data-testid="search-result-library-focus-all"
                data-search-result-library-focus=""
              >
                ${escapeHtml(`全部结果 · ${allResults.length}`)}
              </button>
              ${libraryBreakdown
                .map(
                  (item) => `
                    <button
                      type="button"
                      class="secondary-button result-library-chip ${activeLibraryFocus?.libraryId === item.libraryId ? "active" : ""}"
                      data-testid="search-result-library-focus-${escapeHtml(item.libraryId)}"
                      data-search-result-library-focus="${escapeHtml(item.libraryId)}"
                    >
                      ${escapeHtml(`${item.label} · ${item.count}`)}
                    </button>
                  `
                )
                .join("")}
            </div>
          </section>
        `
        : ""
    }
    ${
      showLibraryGroupedResults
        ? `
          <div class="result-library-groups" data-testid="search-result-library-groups">
            ${groupedResults.map((group) => renderSearchResultGroup(group)).join("")}
          </div>
        `
        : `
          <ul class="result-list" data-testid="result-list">
            ${results
              .map((item) =>
                renderSearchResultCard(item, activeLibraryFocus ? "focused" : "default")
              )
              .join("")}
          </ul>
        `
    }
    ${
      searchHasMoreResults()
        ? `
          <div class="results-footer">
            <button
              type="button"
              class="secondary-button"
              id="search-load-more-button"
              data-testid="search-load-more-button"
            >
              加载更多
            </button>
          </div>
        `
        : ""
    }
    </div>
  `;
}

function renderSearchControls(library, readingMode = false) {
  const queryPreview = queryImagePreviewUrl();
  const queryVideoPreview = queryVideoPreviewUrl();
  const queryDocumentPreview = queryDocumentPreviewUrl();
  const queryVideoDuration = state.queryVideoDurationMs;
  const queryVideoStartMs = currentQueryVideoStartMs();
  const queryVideoEndMs = currentQueryVideoEndMs();
  const hasAdvancedFilters =
    Boolean(state.searchFilters.pathPrefix.trim()) ||
    Boolean(state.searchFilters.timeRangeStartMsDraft.trim()) ||
    Boolean(state.searchFilters.timeRangeEndMsDraft.trim());
  const hasFilterSelections =
    Boolean(state.searchFilters.visualUnitKind) ||
    Boolean(state.searchFilters.sourceType) ||
    hasAdvancedFilters;
  const filterPanelOpen = state.searchFiltersPanelOpen || hasFilterSelections;
  const activeModeLabel = searchModeDisplayName(state.searchMode);
  const modeActionButtons = `
    <button
      type="button"
      class="secondary-button search-filter-button ${filterPanelOpen ? "active" : ""}"
      id="search-filter-toggle-button"
      data-testid="search-filter-toggle-button"
      aria-expanded="${filterPanelOpen ? "true" : "false"}"
    >
      ${renderUiIcon("filter")}
      <span>过滤</span>
    </button>
    ${
      state.searchMode !== "text"
        ? `
          <button
            type="button"
            class="secondary-button search-mode-pill"
            data-testid="search-mode-text"
            data-search-mode="text"
          >
            文本
          </button>
        `
        : ""
    }
    <button
      type="button"
      class="search-mode-icon-button ${state.searchMode === "image" ? "active" : ""}"
      data-testid="search-mode-image"
      data-search-mode="image"
      aria-label="图片查询"
      title="图片查询"
    >
      ${renderUiIcon("image")}
    </button>
    <button
      type="button"
      class="search-mode-icon-button ${state.searchMode === "video" ? "active" : ""}"
      data-testid="search-mode-video"
      data-search-mode="video"
      aria-label="视频查询"
      title="视频查询"
    >
      ${renderUiIcon("video")}
    </button>
    <button
      type="button"
      class="search-mode-icon-button ${state.searchMode === "document" ? "active" : ""}"
      data-testid="search-mode-document"
      data-search-mode="document"
      aria-label="文档查询"
      title="文档查询"
    >
      ${renderUiIcon("document")}
    </button>
  `;
  return `
    <form
      id="search-form"
      class="stack-form search-form ${readingMode ? "search-form-reading" : ""}"
      data-testid="search-form"
    >
      <div class="search-stage-card">
        <div class="search-composer-shell">
          <div class="search-composer-main ${state.searchMode === "text" ? "search-composer-main-text" : "search-composer-main-object"}">
            ${
              state.searchMode === "text"
                ? `
                  <label class="search-main-input query-text-card search-composer-input-shell">
                    <span class="search-input-row">
                      <span class="search-lens" aria-hidden="true"></span>
                      <input
                        id="search-text"
                        data-testid="search-text-input"
                        type="text"
                        value="${escapeHtml(state.searchTextDraft)}"
                        placeholder="Type, paste, or upload to search"
                        ${library ? "" : "disabled"}
                      />
                    </span>
                  </label>
                `
                : `
                  <div class="search-mode-copy">
                    <span class="pill ready">${escapeHtml(activeModeLabel)}查询</span>
                    <p class="helper">
                      ${
                        state.searchMode === "image"
                          ? "上传、粘贴或复用图片作为查询输入。"
                          : state.searchMode === "video"
                            ? "上传视频、选择库内视频源，或复用结果片段作为查询输入。"
                            : "上传 PDF 或复用结果页作为查询输入。"
                      }
                    </p>
                  </div>
                `
            }
          </div>
          <div class="search-composer-actions" data-testid="search-mode-switch">
            ${modeActionButtons}
          </div>
        </div>
        ${
          state.searchMode === "text"
            ? ""
            : state.searchMode === "image"
              ? `
            <div class="query-image-panel" data-testid="query-image-panel">
              <label class="query-source-field">
                <span>查询图片</span>
                <input
                  id="query-image-input"
                  data-testid="query-image-input"
                  type="file"
                  accept="image/*"
                  ${library ? "" : "disabled"}
                />
              </label>
              <div class="query-image-card query-surface-card" data-testid="query-image-card">
                <div class="job-meta query-surface-meta">
                  <div class="query-surface-copy">
                    <span class="pill ${state.queryImageAsset || state.queryImageLibraryObject ? "ready" : "muted"}">${escapeHtml(queryImageStatusLabel())}</span>
                    ${
                      queryImageDisplayName()
                        ? `<span class="helper query-surface-name">${escapeHtml(queryImageDisplayName())}</span>`
                        : `<span class="helper query-surface-placeholder">当前还没有查询图片。</span>`
                    }
                  </div>
                </div>
                <div class="query-preview-surface">
                  ${
                    queryPreview
                      ? isDocumentPageQueryImage()
                        ? `<iframe class="query-image-preview-frame" data-testid="query-image-preview" src="${escapeHtml(queryPreview)}" title="查询图片预览" loading="lazy"></iframe>`
                        : `<img class="query-image-preview" data-testid="query-image-preview" src="${escapeHtml(queryPreview)}" alt="查询图片预览" />`
                      : `<p class="empty query-preview-empty" data-testid="query-image-empty">选择一张本地图片后，这里会显示查询图片预览。</p>`
                  }
                </div>
                <div class="inline-actions query-surface-actions">
                  <button type="button" id="clear-query-image-button" data-testid="clear-query-image-button" class="secondary-button" ${state.queryImageFile || state.queryImageAsset || state.queryImageLibraryObject ? "" : "disabled"}>清除</button>
                  ${
                    activeQueryImagePreview()
                      ? `<a data-testid="query-image-preview-link" href="${escapeHtml(activeQueryImagePreview().url)}" target="_blank" rel="noreferrer">打开查询图片预览</a>`
                      : ""
                  }
                </div>
                <button
                  type="button"
                  class="paste-target"
                  id="query-image-paste-target"
                  data-testid="query-image-paste-target"
                  ${library ? "" : "disabled"}
                >
                  点击这里后按 Ctrl/Cmd+V 粘贴图片
                </button>
              </div>
            </div>
          `
              : state.searchMode === "video"
                ? `
            <div class="query-video-panel" data-testid="query-video-panel">
              <label class="query-source-field">
                <span>查询视频</span>
                <input
                  id="query-video-input"
                  data-testid="query-video-input"
                  type="file"
                  accept="video/mp4,video/quicktime,video/x-m4v,video/*"
                  ${library ? "" : "disabled"}
                />
              </label>
              <label class="query-source-field">
                <span>或复用库内视频源</span>
                <select
                  id="query-video-source-select"
                  data-testid="query-video-source-select"
                  ${library && state.videoSources.length ? "" : "disabled"}
                >
                  <option value="">不使用库内视频源</option>
                  ${state.videoSources
                    .map(
                      (source) => `
                        <option
                          value="${escapeHtml(source.source_id)}"
                          ${state.queryVideoSource?.source_id === source.source_id ? "selected" : ""}
                        >
                          ${escapeHtml(sourceName(source.source_path))} (${escapeHtml(source.source_id)})
                        </option>
                      `
                    )
                    .join("")}
                </select>
              </label>
              <div class="query-video-card query-surface-card" data-testid="query-video-card">
                <div class="job-meta query-surface-meta">
                  <div class="query-surface-copy">
                    <span class="pill ${state.queryVideoAsset || state.queryVideoSource || state.queryVideoLibraryObject ? "ready" : "muted"}">${escapeHtml(queryVideoStatusLabel())}</span>
                    ${
                      queryVideoDisplayName()
                        ? `<span class="helper query-surface-name">${escapeHtml(queryVideoDisplayName())}</span>`
                        : `<span class="helper query-surface-placeholder">当前还没有查询视频。</span>`
                    }
                  </div>
                </div>
                <div class="query-preview-surface">
                  ${
                    queryVideoPreview
                      ? `<video
                          class="query-video-preview"
                          data-testid="query-video-preview"
                          src="${escapeHtml(queryVideoPreview)}"
                          controls
                          preload="metadata"
                        ></video>`
                      : `<p class="empty query-preview-empty" data-testid="query-video-empty">选择一个本地视频或库内视频源后，这里会显示查询视频预览。</p>`
                  }
                </div>
                <div class="query-range-card query-surface-subcard" data-testid="query-video-range-card">
                  <div class="job-meta">
                    <strong>时间范围</strong>
                    <span class="helper">${escapeHtml(queryVideoRangeSummary())}</span>
                  </div>
                  ${
                    state.queryVideoLibraryObject
                      ? `<p class="helper">当前使用库内 video_segment；查询范围固定为该片段自身的时间范围。</p>`
                      : queryVideoDuration
                        ? `
                          <div class="range-grid">
                            <label>
                              <span>开始时间</span>
                              <input
                                id="query-video-range-start"
                                data-testid="query-video-range-start"
                                type="range"
                                min="0"
                                max="${escapeHtml(Math.max(queryVideoDuration - 1, 0))}"
                                step="${escapeHtml(queryVideoRangeStep())}"
                                value="${escapeHtml(queryVideoStartMs)}"
                              />
                            </label>
                            <label>
                              <span>结束时间</span>
                              <input
                                id="query-video-range-end"
                                data-testid="query-video-range-end"
                                type="range"
                                min="1"
                                max="${escapeHtml(queryVideoDuration)}"
                                step="${escapeHtml(queryVideoRangeStep())}"
                                value="${escapeHtml(Math.max(queryVideoEndMs, 1))}"
                              />
                            </label>
                          </div>
                        `
                        : `<p class="helper">视频元数据加载后即可通过时间轴拖选查询片段；不拖选时默认整段视频。</p>`
                  }
                  <div class="inline-actions">
                    <button
                      type="button"
                      id="clear-query-video-range-button"
                      data-testid="clear-query-video-range-button"
                      class="secondary-button"
                      ${queryVideoDuration && state.queryVideoRange && !state.queryVideoLibraryObject ? "" : "disabled"}
                    >
                      整段视频
                    </button>
                  </div>
                </div>
                <div class="inline-actions query-surface-actions">
                  <button
                    type="button"
                    id="clear-query-video-button"
                    data-testid="clear-query-video-button"
                    class="secondary-button"
                    ${state.queryVideoFile || state.queryVideoAsset || state.queryVideoSource || state.queryVideoLibraryObject ? "" : "disabled"}
                  >
                    清除
                  </button>
                  ${
                    activeQueryVideoPreview()
                      ? `<a data-testid="query-video-preview-link" href="${escapeHtml(activeQueryVideoPreview().url)}" target="_blank" rel="noreferrer">打开查询视频预览</a>`
                      : ""
                  }
                </div>
              </div>
            </div>
          `
                : `
            <div class="query-document-panel" data-testid="query-document-panel">
              <label class="query-source-field">
                <span>查询文档</span>
                <input
                  id="query-document-input"
                  data-testid="query-document-input"
                  type="file"
                  accept="application/pdf,.pdf"
                  ${library ? "" : "disabled"}
                />
              </label>
              <div class="query-document-card query-surface-card" data-testid="query-document-card">
                <div class="job-meta query-surface-meta">
                  <div class="query-surface-copy">
                    <span class="pill ${state.queryDocumentAsset || state.queryDocumentLibraryObject ? "ready" : "muted"}">${escapeHtml(queryDocumentStatusLabel())}</span>
                    ${
                      queryDocumentDisplayName()
                        ? `<span class="helper query-surface-name">${escapeHtml(queryDocumentDisplayName())}</span>`
                        : `<span class="helper query-surface-placeholder">当前还没有查询文档。</span>`
                    }
                  </div>
                </div>
                <div class="query-preview-surface">
                  ${
                    queryDocumentPreview
                      ? `<iframe class="query-document-preview-frame" data-testid="query-document-preview" src="${escapeHtml(queryDocumentPreview)}" title="查询文档预览" loading="lazy"></iframe>`
                      : `<p class="empty query-preview-empty" data-testid="query-document-empty">选择一个本地 PDF 或从结果复用 document_page 后，这里会显示查询文档预览。</p>`
                  }
                </div>
                <div class="query-range-card query-surface-subcard" data-testid="query-document-range-card">
                  <div class="job-meta">
                    <strong>页范围</strong>
                    <span class="helper" id="query-document-range-summary">${escapeHtml(queryDocumentRangeSummary())}</span>
                  </div>
                  ${
                    state.queryDocumentLibraryObject
                      ? `<p class="helper">当前使用库内 document_page；查询范围固定为该页面对应的单页范围。</p>`
                      : `
                          <div class="range-grid range-grid-pages">
                            <label>
                              <span>起始页</span>
                              <input
                                id="query-document-range-start"
                                data-testid="query-document-range-start"
                                type="number"
                                inputmode="numeric"
                                min="1"
                                step="1"
                                value="${escapeHtml(currentQueryDocumentStartPage())}"
                                placeholder="留空表示整份文档"
                              />
                            </label>
                            <label>
                              <span>结束页</span>
                              <input
                                id="query-document-range-end"
                                data-testid="query-document-range-end"
                                type="number"
                                inputmode="numeric"
                                min="1"
                                step="1"
                                value="${escapeHtml(currentQueryDocumentEndPage())}"
                                placeholder="只填起始页表示单页"
                              />
                            </label>
                          </div>
                        `
                  }
                  <div class="inline-actions">
                    <button
                      type="button"
                      id="clear-query-document-range-button"
                      data-testid="clear-query-document-range-button"
                      class="secondary-button"
                      ${!state.queryDocumentLibraryObject && (state.queryDocumentStartPageDraft || state.queryDocumentEndPageDraft) ? "" : "disabled"}
                    >
                      整份文档
                    </button>
                  </div>
                </div>
                <div class="inline-actions query-surface-actions">
                  <button
                    type="button"
                    id="clear-query-document-button"
                    data-testid="clear-query-document-button"
                    class="secondary-button"
                    ${state.queryDocumentFile || state.queryDocumentAsset || state.queryDocumentLibraryObject ? "" : "disabled"}
                  >
                    清除
                  </button>
                  ${
                    activeQueryDocumentPreview()
                      ? `<a data-testid="query-document-preview-link" href="${escapeHtml(activeQueryDocumentPreview().url)}" target="_blank" rel="noreferrer">打开查询文档预览</a>`
                      : ""
                  }
                </div>
              </div>
            </div>
          `
        }
      </div>
      ${
        filterPanelOpen
          ? `
            <section class="search-filter-panel" data-testid="search-filter-dock">
              <div class="search-common-filters">
                <label>
                  <span>视觉对象类型</span>
                  <select id="search-filter-kind" data-testid="search-filter-kind" ${library ? "" : "disabled"}>
                    <option value="">全部</option>
                    <option value="image" ${state.searchFilters.visualUnitKind === "image" ? "selected" : ""}>图片</option>
                    <option value="document_page" ${state.searchFilters.visualUnitKind === "document_page" ? "selected" : ""}>文档页</option>
                    <option value="video_segment" ${state.searchFilters.visualUnitKind === "video_segment" ? "selected" : ""}>视频片段</option>
                  </select>
                </label>
                <label>
                  <span>来源类型</span>
                  <select id="search-filter-source-type" data-testid="search-filter-source-type" ${library ? "" : "disabled"}>
                    <option value="">全部</option>
                    <option value="image" ${state.searchFilters.sourceType === "image" ? "selected" : ""}>图片</option>
                    <option value="pdf" ${state.searchFilters.sourceType === "pdf" ? "selected" : ""}>PDF</option>
                    <option value="video" ${state.searchFilters.sourceType === "video" ? "selected" : ""}>视频</option>
                  </select>
                </label>
              </div>
              <div class="search-advanced-grid">
                <label>
                  <span>路径前缀</span>
                  <input
                    id="search-filter-path-prefix"
                    data-testid="search-filter-path-prefix"
                    type="text"
                    value="${escapeHtml(state.searchFilters.pathPrefix)}"
                    placeholder="/abs/path/prefix"
                    ${library ? "" : "disabled"}
                  />
                </label>
                <label>
                  <span>起始时间（ms）</span>
                  <input
                    id="search-filter-time-range-start"
                    data-testid="search-filter-time-range-start"
                    type="number"
                    inputmode="numeric"
                    min="0"
                    step="1"
                    value="${escapeHtml(state.searchFilters.timeRangeStartMsDraft)}"
                    placeholder="仅作用于视频时间命中"
                    ${library ? "" : "disabled"}
                  />
                </label>
                <label>
                  <span>结束时间（ms）</span>
                  <input
                    id="search-filter-time-range-end"
                    data-testid="search-filter-time-range-end"
                    type="number"
                    inputmode="numeric"
                    min="0"
                    step="1"
                    value="${escapeHtml(state.searchFilters.timeRangeEndMsDraft)}"
                    placeholder="仅作用于视频时间命中"
                    ${library ? "" : "disabled"}
                  />
                </label>
                <div class="inline-actions">
                  <button
                    type="button"
                    id="clear-search-filters-button"
                    data-testid="clear-search-filters-button"
                    class="secondary-button"
                    ${library ? "" : "disabled"}
                  >
                    清除过滤器
                  </button>
                </div>
              </div>
            </section>
          `
          : ""
      }
      ${renderSearchStateStrip(library)}
    </form>
  `;
}

function patchWorkspaceMarkupPreservingDetail(nextMarkup) {
  if (!(root instanceof HTMLElement)) {
    return false;
  }

  const currentShell = root.querySelector("main.shell");
  const currentShellBar = currentShell?.querySelector(".shell-bar, .hero");
  const currentFrame = currentShell?.querySelector(".workspace-frame");
  const currentSidebar = currentFrame?.querySelector(".app-sidebar");
  const currentDesk = currentFrame?.querySelector(".workspace-desk");
  const currentLeft = currentDesk?.querySelector(".workspace-left");
  const currentRight = currentDesk?.querySelector(".workspace-right");
  if (
    !(currentShell instanceof HTMLElement) ||
    !(currentShellBar instanceof HTMLElement) ||
    !(currentFrame instanceof HTMLElement) ||
    !(currentSidebar instanceof HTMLElement) ||
    !(currentDesk instanceof HTMLElement) ||
    !(currentLeft instanceof HTMLElement)
  ) {
    return false;
  }

  const template = document.createElement("template");
  template.innerHTML = nextMarkup.trim();
  const nextShell = template.content.firstElementChild;
  const nextShellBar = nextShell?.querySelector(".shell-bar");
  const nextStatusStack = nextShell?.querySelector(".status-stack");
  const nextFrame = nextShell?.querySelector(".workspace-frame");
  const nextSidebar = nextFrame?.querySelector(".app-sidebar");
  const nextDesk = nextFrame?.querySelector(".workspace-desk");
  const nextLeft = nextDesk?.querySelector(".workspace-left");
  const nextRight = nextDesk?.querySelector(".workspace-right");
  if (
    !(nextShell instanceof HTMLElement) ||
    !(nextShellBar instanceof HTMLElement) ||
    !(nextFrame instanceof HTMLElement) ||
    !(nextSidebar instanceof HTMLElement) ||
    !(nextDesk instanceof HTMLElement) ||
    !(nextLeft instanceof HTMLElement)
  ) {
    return false;
  }

  const syncOptionalRegion = (parent, selector, nextNode) => {
    const currentNode = parent.querySelector(selector);
    if (currentNode instanceof HTMLElement && nextNode instanceof HTMLElement) {
      currentNode.replaceWith(nextNode);
      return;
    }
    if (currentNode instanceof HTMLElement) {
      currentNode.remove();
      return;
    }
    if (nextNode instanceof HTMLElement) {
      parent.append(nextNode);
    }
  };

  currentShellBar.replaceWith(nextShellBar);

  const currentStatusStack = currentShell.querySelector(".status-stack");
  const insertedShellBar = currentShell.querySelector(".shell-bar");
  if (nextStatusStack instanceof HTMLElement) {
    if (currentStatusStack instanceof HTMLElement) {
      currentStatusStack.replaceWith(nextStatusStack);
    } else if (insertedShellBar instanceof HTMLElement) {
      insertedShellBar.after(nextStatusStack);
    } else {
      return false;
    }
  } else if (currentStatusStack instanceof HTMLElement) {
    currentStatusStack.remove();
  }

  currentFrame.className = nextFrame.className;
  currentSidebar.replaceWith(nextSidebar);
  currentDesk.className = nextDesk.className;
  currentLeft.replaceWith(nextLeft);
  syncOptionalRegion(currentDesk, ".workspace-center", nextDesk.querySelector(".workspace-center"));
  if (currentRight instanceof HTMLElement && nextRight instanceof HTMLElement) {
    currentRight.className = nextRight.className;
  } else if (currentRight instanceof HTMLElement || nextRight instanceof HTMLElement) {
    return false;
  }
  syncOptionalRegion(
    currentFrame,
    '[data-testid="utility-drawer"]',
    nextFrame.querySelector('[data-testid="utility-drawer"]')
  );
  return true;
}

function bindClickListeners(selector, handler, skipWithin = null) {
  document.querySelectorAll(selector).forEach((button) => {
    if (skipWithin instanceof HTMLElement && skipWithin.contains(button)) {
      return;
    }
    button.addEventListener("click", handler);
  });
}

function renderWorkspace() {
  const library = selectedLibrary();
  const searchDetailSheetOpen = searchDetailSheetIsOpen();
  const isSearchWorkspace = state.activeWorkspace === "search";
  const searchMobileSheetViewport = window.matchMedia("(max-width: 720px)").matches;
  const searchNextStepDock = isSearchWorkspace ? renderSearchNextStepDock(library) : "";
  const searchHasResults = Boolean((state.searchOutcome?.results ?? []).length);
  const shouldShowSearchResultsColumn = isSearchWorkspace && (searchHasResults || state.searchInFlight);
  const shouldRenderSearchDetailPanel =
    isSearchWorkspace &&
    searchHasResults &&
    (!searchMobileSheetViewport || searchDetailSheetOpen);
  const searchLayoutClass = shouldShowSearchResultsColumn
    ? "workspace-desk-search workspace-desk-search-results"
    : "workspace-desk-search workspace-desk-search-stage-only";
  const searchStagePanelClass = searchHasResults
    ? "panel search-stage-panel search-stage-panel-reading"
    : "panel search-stage-panel";
  const focusedEditableState = captureFocusedEditableState();
  const detailPanelKey = currentDetailPanelRenderKey();
  const previousDetailPanel = root?.querySelector('[data-testid="detail-panel"]') ?? null;
  const shouldPreserveDetailPanel =
    isSearchWorkspace &&
    previousDetailPanel instanceof HTMLElement &&
    detailPanelKey !== null &&
    detailPanelKey === lastRenderedDetailPanelKey;

  const nextMarkup = `
    <main class="shell" data-testid="workspace-shell">
      <section class="shell-bar">
        ${renderContextRail(library)}
      </section>

      ${renderStatusNotices()}

      <section class="workspace-frame ${state.utilityDrawerOpen ? "workspace-frame-with-drawer" : "workspace-frame-main-only"}">
        <aside class="panel panel-tight app-sidebar" data-testid="app-sidebar">
          ${renderWorkspaceSwitcher()}
        </aside>
        <section class="workspace-desk ${
          isSearchWorkspace
            ? searchLayoutClass
            : state.activeWorkspace === "inventory"
              ? "workspace-desk-inventory"
              : "workspace-desk-settings"
        }">
          <aside class="workspace-column workspace-left">
            ${
              isSearchWorkspace
                ? `
                  <section class="${searchStagePanelClass}" data-testid="search-panel">
                    <div class="search-stage-layout ${
                      searchNextStepDock ? "search-stage-layout-with-dock" : "search-stage-layout-single"
                    }">
                      <div class="search-stage-main">
                        ${
                          searchHasResults
                            ? ""
                            : `
                              <div class="search-stage-head">
                                <h2>Search anything you want</h2>
                              </div>
                            `
                        }
                        ${renderSearchControls(library, searchHasResults)}
                        ${renderLibraryContextCluster(library, "search")}
                        ${
                          searchHasResults || !state.searchOutcome
                            ? ""
                            : `
                              <div class="search-stage-inline-outcome" data-testid="search-inline-outcome">
                                ${renderSearchOutcome()}
                              </div>
                            `
                        }
                      </div>
                      ${searchNextStepDock}
                    </div>
                  </section>
                `
                : state.activeWorkspace === "inventory"
                  ? renderLibrarySourcesPanel(library)
                  : renderSettingsPanel(library)
            }
          </aside>

          ${
            shouldShowSearchResultsColumn
              ? `
                <section class="workspace-column workspace-center" data-testid="search-results-column">
                  <section class="panel search-results-panel">
                    ${renderSearchLoadingNotice()}
                    ${renderSearchOutcome()}
                  </section>
                </section>
              `
              : ""
          }

          ${
            shouldRenderSearchDetailPanel
              ? `
                <aside class="workspace-column workspace-right">
                  ${
                    searchDetailSheetOpen
                      ? `<button
                          type="button"
                          class="mobile-sheet-backdrop"
                          data-testid="detail-sheet-backdrop"
                          data-mobile-sheet-close="search"
                          aria-label="关闭结果详情"
                        ></button>`
                      : ""
                  }
                  <section
                    class="panel detail-panel mobile-sheet-panel ${searchDetailSheetOpen ? "mobile-sheet-open" : "mobile-sheet-closed"}"
                    data-testid="detail-panel"
                  >
                    <div class="mobile-sheet-bar">
                      <span class="mobile-sheet-handle" aria-hidden="true"></span>
                      <button
                        type="button"
                        class="secondary-button mobile-sheet-close"
                        data-testid="detail-sheet-close-button"
                        data-mobile-sheet-close="search"
                      >
                        关闭
                      </button>
                    </div>
                    ${renderVisualUnitDetail()}
                  </section>
                </aside>
              `
              : ""
          }
        </section>
        ${renderUtilityDrawer(library)}
      </section>
    </main>
  `;

  const preservedDetailPanel =
    shouldPreserveDetailPanel && patchWorkspaceMarkupPreservingDetail(nextMarkup)
      ? previousDetailPanel
      : null;
  if (!preservedDetailPanel) {
    root.innerHTML = nextMarkup;
  }

  document.querySelectorAll("[data-workspace]").forEach((button) => {
    button.addEventListener("click", onSelectWorkspace);
  });
  document.querySelectorAll("[data-settings-section]").forEach((button) => {
    button.addEventListener("click", onSelectSettingsSection);
  });
  document.querySelectorAll("[data-open-settings-section]").forEach((button) => {
    button.addEventListener("click", onOpenSettingsSection);
  });
  document.querySelectorAll("[data-open-hit-library-id]").forEach((button) => {
    button.addEventListener("click", onOpenHitLibraryContext);
  });
  document.querySelectorAll("[data-content-type-scope]").forEach((button) => {
    button.addEventListener("click", onSelectContentTypeTab);
  });
  document.querySelectorAll("[data-library-override-mode]").forEach((button) => {
    button.addEventListener("click", onLibraryOverrideModeChange);
  });
  document.querySelector("#create-library-form")?.addEventListener("submit", onCreateLibrary);
  document.querySelectorAll("[data-library-rename-form]").forEach((form) => {
    form.addEventListener("submit", onRenameLibrary);
  });
  document.querySelector("#library-name")?.addEventListener("input", onLibraryNameInput);
  document.querySelector("#library-id")?.addEventListener("input", onLibraryIdInput);
  document.querySelectorAll("[data-library-management-display-name-input]").forEach((input) => {
    input.addEventListener("input", onManageLibraryNameInput);
  });
  document
    .querySelector('[data-testid="create-library-popover"]')
    ?.addEventListener("toggle", onCreateLibraryPopoverToggle);
  document
    .querySelector('[data-testid="manage-library-popover"]')
    ?.addEventListener("toggle", onManageLibraryPopoverToggle);
  document.querySelectorAll("[data-library-archive-action]").forEach((button) => {
    button.addEventListener("click", onToggleLibraryArchive);
  });
  document.querySelectorAll("[data-library-delete-action]").forEach((button) => {
    button.addEventListener("click", onDeleteLibrary);
  });
  document.querySelector("#library-select")?.addEventListener("change", onSelectLibrary);
  document.querySelectorAll("[data-job-cancel-id]").forEach((button) => {
    button.addEventListener("click", onCancelJob);
  });
  document.querySelectorAll("[data-job-retry-id]").forEach((button) => {
    button.addEventListener("click", onRetryJob);
  });
  document.querySelectorAll("[data-job-resume-id]").forEach((button) => {
    button.addEventListener("click", onResumeJob);
  });
  document
    .querySelector("#provider-config-form")
    ?.addEventListener("submit", onSubmitProviderConfig);
  document
    .querySelector("#provider-config-reset-button")
    ?.addEventListener("click", onResetProviderConfigForm);
  document
    .querySelector("#provider-config-id")
    ?.addEventListener("change", onProviderConfigSelect);
  document
    .querySelector("#provider-enabled")
    ?.addEventListener("change", onProviderEnabledChange);
  document
    .querySelector("#provider-base-url")
    ?.addEventListener("input", onProviderBaseUrlInput);
  document
    .querySelector("#global-content-types-form")
    ?.addEventListener("submit", onSubmitGlobalContentTypes);
  document
    .querySelector("#global-content-type")
    ?.addEventListener("change", onGlobalContentTypeChange);
  document
    .querySelector("#global-content-type-enabled")
    ?.addEventListener("change", onGlobalContentTypeEnabledChange);
  document
    .querySelector("#global-content-type-provider-id")
    ?.addEventListener("change", onGlobalContentTypeProviderChange);
  document
    .querySelector("#global-content-type-model-id")
    ?.addEventListener("change", onGlobalContentTypeModelIdInput);
  document
    .querySelector("#global-content-type-vector-type")
    ?.addEventListener("change", onGlobalContentTypeVectorTypeChange);
  document
    .querySelector("#global-model-test-form")
    ?.addEventListener("submit", onSubmitGlobalModelTest);
  document
    .querySelector("#global-model-test-modality")
    ?.addEventListener("change", onGlobalModelTestModalityChange);
  document
    .querySelector("#global-model-test-text")
    ?.addEventListener("input", onGlobalModelTestTextInput);
  document
    .querySelector("#global-model-test-file")
    ?.addEventListener("change", onGlobalModelTestFileInput);
  document
    .querySelector("#global-model-test-comparison-modality")
    ?.addEventListener("change", onGlobalModelTestComparisonModalityChange);
  document
    .querySelector("#global-model-test-comparison-text")
    ?.addEventListener("input", onGlobalModelTestComparisonTextInput);
  document
    .querySelector("#global-model-test-comparison-file")
    ?.addEventListener("change", onGlobalModelTestComparisonFileInput);
  document
    .querySelector("#library-content-types-form")
    ?.addEventListener("submit", onSubmitLibraryContentTypes);
  document
    .querySelector("#library-content-types-reset-button")
    ?.addEventListener("click", onResetLibraryContentTypes);
  document
    .querySelector("#library-content-type")
    ?.addEventListener("change", onLibraryContentTypeChange);
  document
    .querySelector("#library-content-type-enabled")
    ?.addEventListener("change", onLibraryContentTypeEnabledChange);
  document
    .querySelector("#library-content-type-provider-id")
    ?.addEventListener("change", onLibraryContentTypeProviderChange);
  document
    .querySelector("#library-content-type-model-id")
    ?.addEventListener("change", onLibraryContentTypeModelIdInput);
  document
    .querySelector("#library-content-type-vector-type")
    ?.addEventListener("change", onLibraryContentTypeVectorTypeChange);
  document
    .querySelector("#library-model-test-form")
    ?.addEventListener("submit", onSubmitLibraryModelTest);
  document
    .querySelector("#library-model-test-modality")
    ?.addEventListener("change", onLibraryModelTestModalityChange);
  document
    .querySelector("#library-model-test-text")
    ?.addEventListener("input", onLibraryModelTestTextInput);
  document
    .querySelector("#library-model-test-file")
    ?.addEventListener("change", onLibraryModelTestFileInput);
  document
    .querySelector("#library-model-test-comparison-modality")
    ?.addEventListener("change", onLibraryModelTestComparisonModalityChange);
  document
    .querySelector("#library-model-test-comparison-text")
    ?.addEventListener("input", onLibraryModelTestComparisonTextInput);
  document
    .querySelector("#library-model-test-comparison-file")
    ?.addEventListener("change", onLibraryModelTestComparisonFileInput);
  document.querySelector("#source-root-form")?.addEventListener("submit", onSubmitSourceRoot);
  document.querySelector("#source-root-reset-button")?.addEventListener("click", onResetSourceRootEditor);
  document
    .querySelector("#search-preparation-disclosure")
    ?.addEventListener("toggle", onSearchPreparationDisclosureToggle);
  document
    .querySelector("#search-jobs-disclosure")
    ?.addEventListener("toggle", onSearchJobsDisclosureToggle);
  document.querySelector("#source-root-path")?.addEventListener("input", onSourceRootPathInput);
  document
    .querySelector("#source-root-enabled")
    ?.addEventListener("change", onSourceRootEnabledInput);
  document
    .querySelector("#source-root-include-globs")
    ?.addEventListener("input", onSourceRootIncludeGlobsInput);
  document
    .querySelector("#source-root-exclude-globs")
    ?.addEventListener("input", onSourceRootExcludeGlobsInput);
  document
    .querySelector("#source-root-include-extensions")
    ?.addEventListener("input", onSourceRootIncludeExtensionsInput);
  document.querySelector("#library-refresh-button")?.addEventListener("click", onRefreshLibrarySources);
  document.querySelector("#library-rescan-button")?.addEventListener("click", onRescanLibrarySources);
  document.querySelector("#diagnostics-rebuild-library-button")?.addEventListener("click", onRebuildLibrarySources);
  document
    .querySelector("#diagnostics-cleanup-retired-vector-spaces-button")
    ?.addEventListener("click", onCleanupRetiredVectorSpaces);
  document.querySelector("#source-filter-root")?.addEventListener("change", onSourceFilterRootChange);
  document.querySelector("#source-filter-type")?.addEventListener("change", onSourceFilterTypeChange);
  document
    .querySelector("#source-filter-status")
    ?.addEventListener("change", onSourceFilterStatusChange);
  document.querySelectorAll("[data-source-root-edit-id]").forEach((button) => {
    button.addEventListener("click", onEditSourceRoot);
  });
  document.querySelectorAll("[data-source-root-refresh-id]").forEach((button) => {
    button.addEventListener("click", onRefreshSourceRoot);
  });
  document.querySelectorAll("[data-source-root-rescan-id]").forEach((button) => {
    button.addEventListener("click", onRescanSourceRoot);
  });
  document.querySelectorAll("[data-source-root-toggle-id]").forEach((button) => {
    button.addEventListener("click", onToggleSourceRoot);
  });
  document.querySelectorAll("[data-source-root-delete-id]").forEach((button) => {
    button.addEventListener("click", onDeleteSourceRoot);
  });
  document.querySelectorAll("[data-provider-edit-id]").forEach((button) => {
    button.addEventListener("click", onEditProviderConfig);
  });
  document.querySelectorAll("[data-utility-drawer-open]").forEach((button) => {
    button.addEventListener("click", onOpenUtilityDrawer);
  });
  document.querySelectorAll("[data-utility-drawer-section]").forEach((button) => {
    button.addEventListener("click", onSelectUtilityDrawerSection);
  });
  document.querySelectorAll("[data-utility-drawer-close]").forEach((button) => {
    button.addEventListener("click", onCloseUtilityDrawer);
  });
  document.querySelectorAll("[data-utilities-action]").forEach((button) => {
    button.addEventListener("click", onUtilitiesAction);
  });
  document.querySelectorAll(".inventory-source-select[data-source-id]").forEach((button) => {
    button.addEventListener("click", onSelectInventorySource);
  });
  document.querySelector("#import-form")?.addEventListener("submit", onImportPaths);
  document.querySelector("#import-paths")?.addEventListener("input", onImportPathsInput);
  document.querySelector("#search-form")?.addEventListener("submit", onSearchSubmit);
  document
    .querySelector("#search-filter-toggle-button")
    ?.addEventListener("click", onToggleSearchFiltersPanel);
  document.querySelectorAll("[data-search-scope]").forEach((button) => {
    button.addEventListener("click", onSelectSearchScope);
  });
  document.querySelectorAll("[data-search-result-library-focus]").forEach((button) => {
    button.addEventListener("click", onSelectSearchResultLibraryFocus);
  });
  document.querySelector("#search-text")?.addEventListener("input", onSearchTextInput);
  document.querySelector("#search-filter-kind")?.addEventListener("change", onSearchFilterKindChange);
  document
    .querySelector("#search-filter-source-type")
    ?.addEventListener("change", onSearchFilterSourceTypeChange);
  document
    .querySelector("#search-filter-path-prefix")
    ?.addEventListener("input", onSearchFilterPathPrefixInput);
  document
    .querySelector("#search-filter-time-range-start")
    ?.addEventListener("input", onSearchFilterTimeRangeStartInput);
  document
    .querySelector("#search-filter-time-range-end")
    ?.addEventListener("input", onSearchFilterTimeRangeEndInput);
  document
    .querySelector("#clear-search-filters-button")
    ?.addEventListener("click", onClearSearchFilters);
  document
    .querySelector("#search-load-more-button")
    ?.addEventListener("click", onLoadMoreSearchResults);
  document.querySelector("#query-image-input")?.addEventListener("change", onQueryImageInput);
  document.querySelector("#clear-query-image-button")?.addEventListener("click", onClearQueryImage);
  document.querySelector("#query-image-paste-target")?.addEventListener("paste", onQueryImagePaste);
  document.querySelector("#query-video-input")?.addEventListener("change", onQueryVideoInput);
  document.querySelector("#query-video-source-select")?.addEventListener("change", onQueryVideoSourceSelect);
  document.querySelector("#clear-query-video-button")?.addEventListener("click", onClearQueryVideo);
  document.querySelector("#clear-query-video-range-button")?.addEventListener("click", onClearQueryVideoRange);
  document.querySelector("#query-video-range-start")?.addEventListener("input", onQueryVideoRangeStartInput);
  document.querySelector("#query-video-range-end")?.addEventListener("input", onQueryVideoRangeEndInput);
  document.querySelector("#query-document-input")?.addEventListener("change", onQueryDocumentInput);
  document.querySelector("#clear-query-document-button")?.addEventListener("click", onClearQueryDocument);
  document
    .querySelector("#clear-query-document-range-button")
    ?.addEventListener("click", onClearQueryDocumentRange);
  document
    .querySelector("#query-document-range-start")
    ?.addEventListener("input", onQueryDocumentRangeStartInput);
  document
    .querySelector("#query-document-range-end")
    ?.addEventListener("input", onQueryDocumentRangeEndInput);
  document.querySelectorAll("[data-search-mode]").forEach((button) => {
    button.addEventListener("click", onSelectSearchMode);
  });
  document.querySelectorAll("[data-visual-unit-id]").forEach((button) => {
    button.addEventListener("click", onSelectVisualUnit);
  });
  bindClickListeners("[data-use-query-visual-unit-id]", onUseAsQueryImage, preservedDetailPanel);
  bindClickListeners("[data-use-query-video-visual-unit-id]", onUseAsQueryVideo, preservedDetailPanel);
  bindClickListeners(
    "[data-use-query-document-visual-unit-id]",
    onUseAsQueryDocument,
    preservedDetailPanel
  );
  document.querySelectorAll("[data-mobile-sheet-close]").forEach((button) => {
    button.addEventListener("click", onCloseMobileSheet);
  });

  const queryVideoPreview = document.querySelector("#query-video-preview");
  if (queryVideoPreview instanceof HTMLVideoElement) {
    queryVideoPreview.addEventListener("loadedmetadata", onQueryVideoPreviewLoadedMetadata);
    if (queryVideoPreview.readyState >= 1) {
      syncQueryVideoDurationFromVideoElement(queryVideoPreview);
    }
  }

  document.querySelectorAll('[data-preview-kind="video"]').forEach((previewElement) => {
    if (previewElement instanceof HTMLVideoElement) {
      attachBoundedVideoPlayback(previewElement);
    }
  });

  lastRenderedDetailPanelKey = detailPanelKey;
  restoreFocusedEditableState(focusedEditableState);
}

async function apiRequest<T = any>(path: string, options: RequestInit = {}): Promise<T> {
  const headers = new Headers(options.headers);
  const isFormDataBody = options.body instanceof FormData;
  if (!isFormDataBody && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  const response = await fetch(`/api${path}`, {
    ...options,
    headers,
  });

  let payload: ApiEnvelope<T> | null = null;
  try {
    payload = (await response.json()) as ApiEnvelope<T>;
  } catch {
    payload = null;
  }

  if (!response.ok || (payload && "error" in payload)) {
    throw toApiError((payload && "error" in payload ? payload.error : null) ?? {
      code: "request_failed",
      message: `Request failed with status ${response.status}`,
    });
  }

  if (!payload || !("data" in payload)) {
    throw toApiError({
      code: "request_failed",
      message: "Expected a successful JSON payload but did not receive one.",
    });
  }

  return payload.data;
}

function syncQueryVideoDurationFromVideoElement(videoElement) {
  if (!(videoElement instanceof HTMLVideoElement) || !Number.isFinite(videoElement.duration)) {
    return;
  }

  const durationMs = Math.max(Math.round(videoElement.duration * 1000), 1);
  if (durationMs === state.queryVideoDurationMs) {
    return;
  }

  setQueryVideoDuration(durationMs);
  renderWorkspace();
}

function attachBoundedVideoPlayback(videoElement) {
  if (!(videoElement instanceof HTMLVideoElement)) {
    return;
  }
  if (videoElement.dataset.boundedPlaybackAttached === "true") {
    return;
  }
  videoElement.dataset.boundedPlaybackAttached = "true";

  const startMs = Number(videoElement.dataset.startMs ?? "0");
  const endMs = Number(videoElement.dataset.endMs ?? "0");
  if (!Number.isFinite(startMs) || !Number.isFinite(endMs) || endMs <= startMs) {
    return;
  }

  const startSeconds = startMs / 1000;
  const endSeconds = endMs / 1000;
  const syncCurrentTime = () => {
    if (Number.isFinite(videoElement.duration) && videoElement.currentTime < startSeconds) {
      videoElement.currentTime = startSeconds;
    }
  };
  const clampPlayback = () => {
    if (videoElement.currentTime >= endSeconds) {
      videoElement.pause();
      videoElement.currentTime = startSeconds;
    }
  };

  videoElement.addEventListener("loadedmetadata", syncCurrentTime, { once: true });
  videoElement.addEventListener("timeupdate", clampPlayback);
  if (videoElement.readyState >= 1) {
    syncCurrentTime();
  }
}

async function refreshLibraries({ keepSelection = true } = {}) {
  const data = await apiRequest<LibrariesListData>("/libraries");
  state.libraries = data.libraries;

  if (!keepSelection || !state.libraries.some((item) => item.id === state.selectedLibraryId)) {
    state.selectedLibraryId = state.libraries[0]?.id ?? "";
  }

  const currentLibrary =
    state.libraries.find((item) => item.id === state.selectedLibraryId) ?? null;
  if (!currentLibrary || state.libraryManagementDraftLibraryId !== currentLibrary.id) {
    state.manageLibraryPopoverOpen = false;
  }
  if (!currentLibrary || state.libraryManagementDraftLibraryId !== currentLibrary.id) {
    hydrateLibraryManagementDraft(currentLibrary);
  }
}

async function refreshSourceRoots() {
  if (!state.selectedLibraryId) {
    state.sourceRoots = [];
    resetSourceRootEditor();
    return;
  }

  const data = await apiRequest<SourceRootsListData>(
    `/libraries/${encodeURIComponent(state.selectedLibraryId)}/source-roots`
  );
  state.sourceRoots = data.source_roots;

  if (
    state.editingSourceRootId &&
    !state.sourceRoots.some(
      (sourceRoot) => sourceRoot.source_root_id === state.editingSourceRootId
    )
  ) {
    resetSourceRootEditor();
  }
}

async function refreshLibrarySources() {
  if (!state.selectedLibraryId) {
    resetInventoryState();
    state.selectedInventorySourceId = "";
    return;
  }

  const unfilteredData = await apiRequest<SourcesListData>(
    `/libraries/${encodeURIComponent(state.selectedLibraryId)}/sources`
  );
  state.inventorySummary = summarizeInventorySources(unfilteredData.sources);

  const params = new URLSearchParams();
  if (state.inventoryFilters.sourceRootId && state.inventoryFilters.sourceRootId !== "manual") {
    params.set("source_root_id", state.inventoryFilters.sourceRootId);
  }
  if (state.inventoryFilters.sourceType) {
    params.set("source_type", state.inventoryFilters.sourceType);
  }
  if (state.inventoryFilters.sourceStatus) {
    params.set("status", state.inventoryFilters.sourceStatus);
  }

  const query = params.toString();
  const data =
    query.length > 0
      ? await apiRequest(
          `/libraries/${encodeURIComponent(state.selectedLibraryId)}/sources?${query}`
        )
      : unfilteredData;
  state.librarySources =
    state.inventoryFilters.sourceRootId === "manual"
      ? data.sources.filter((source) => !source.source_root_id)
      : data.sources;
  ensureSelectedInventorySource();
}

async function refreshJobs() {
  if (!state.selectedLibraryId) {
    state.jobs = [];
    return;
  }

  const data = await apiRequest<JobsListData>(
    `/jobs?library_id=${encodeURIComponent(state.selectedLibraryId)}`
  );
  state.jobs = data.jobs;
}

async function refreshVideoSources() {
  if (!state.selectedLibraryId) {
    state.videoSources = [];
    if (!state.queryVideoFile && !state.queryVideoAsset) {
      clearQueryVideoState();
    }
    return;
  }

  const data = await apiRequest<VideoSourcesData>(
    `/libraries/${encodeURIComponent(state.selectedLibraryId)}/video-sources`
  );
  state.videoSources = data.sources;

  if (state.queryVideoSource) {
    const refreshed = state.videoSources.find(
      (source) => source.source_id === state.queryVideoSource.source_id
    );
    if (refreshed) {
      state.queryVideoSource = refreshed;
      setQueryVideoDuration(refreshed.duration_ms ?? null);
    } else if (!state.queryVideoFile && !state.queryVideoAsset) {
      clearQueryVideoState();
    }
  }
}

async function refreshProviderConfigs() {
  const data = await apiRequest<ProvidersListData>("/settings/providers");
  state.providerConfigs = data.providers;
  if (
    state.editingProviderId &&
    !state.providerConfigs.some((provider) => provider.provider_id === state.editingProviderId)
  ) {
    resetProviderEditor();
  }
}

async function refreshModelCatalog() {
  const data = await apiRequest<ModelCatalogData>("/settings/model-catalog");
  state.modelCatalog = data.entries;
  ensureValidModelTestDrafts();
}

async function refreshGlobalContentTypes() {
  const data = await apiRequest<GlobalContentTypesData>("/settings/content-types");
  state.globalContentTypes = data.content_types;
  ensureValidModelTestDrafts();
}

async function refreshRuntimeHealth() {
  const data = await apiRequest<RuntimeHealthData>("/runtime-health");
  state.runtimeHealth = data;
}

async function refreshLibraryContentSettings() {
  if (!state.selectedLibraryId) {
    state.libraryContentTypes = emptyContentTypes();
    state.resolvedContentModels = null;
    state.vectorSpaceDiagnostics = null;
    resetLibraryModelTestState();
    return;
  }

  const [contentTypesData, resolvedData, diagnosticsData] = await Promise.all([
    apiRequest<LibraryContentTypesData>(
      `/libraries/${encodeURIComponent(state.selectedLibraryId)}/content-types`
    ),
    apiRequest<ResolvedContentModelsData>(
      `/libraries/${encodeURIComponent(state.selectedLibraryId)}/resolved-content-models`
    ),
    apiRequest<VectorSpaceDiagnosticsData>(
      `/libraries/${encodeURIComponent(state.selectedLibraryId)}/vector-space-diagnostics`
    ),
  ]);
  state.libraryContentTypes = contentTypesData.content_types;
  state.resolvedContentModels = resolvedData;
  state.vectorSpaceDiagnostics = diagnosticsData;
  ensureValidModelTestDrafts();
}

async function refreshProviderSettingsData() {
  await refreshProviderConfigs();
  await refreshLibraryContentSettings();
  if (state.activeWorkspace === "settings") {
    await refreshRuntimeHealth();
    await refreshModelCatalog();
    await refreshGlobalContentTypes();
  }
}

async function refreshJob(jobId) {
  return apiRequest<JobSnapshot>(`/jobs/${encodeURIComponent(jobId)}`);
}

async function refreshWorkspace(options) {
  await refreshLibraries(options);
  await refreshSourceRoots();
  await refreshProviderConfigs();
  await refreshLibraryContentSettings();
  await refreshRuntimeHealth();
  if (state.activeWorkspace === "inventory") {
    await refreshLibrarySources();
  } else if (state.activeWorkspace === "settings") {
    await refreshModelCatalog();
    await refreshGlobalContentTypes();
  }
  await refreshJobs();
  await refreshVideoSources();
  renderWorkspace();
}

async function switchCurrentLibrary(libraryId: string) {
  state.selectedLibraryId = libraryId;
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
  state.selectedVisualUnit = null;
  state.selectedVisualUnitLibraryId = "";
  state.searchOutcome = null;
  state.searchInFlight = false;
  state.lastSearchRequest = null;
  state.searchPreparationDisclosureOpen = false;
  state.searchJobsDisclosureOpen = false;
  state.searchDetailSheetOpen = false;
  state.createLibraryPopoverOpen = false;
  state.manageLibraryPopoverOpen = false;
  state.globalError = null;
  state.statusMessage = null;
  await refreshWorkspace({ keepSelection: true });
}

async function onCreateLibrary(event) {
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
    state.selectedVisualUnit = null;
    state.selectedVisualUnitLibraryId = "";
    state.searchOutcome = null;
    state.searchInFlight = false;
    state.lastSearchRequest = null;
    state.searchPreparationDisclosureOpen = false;
    state.searchJobsDisclosureOpen = false;
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

function onLibraryNameInput(event) {
  state.libraryDisplayNameDraft = event.target.value;
}

function onLibraryIdInput(event) {
  state.libraryIdDraft = event.target.value;
}

function onManageLibraryNameInput(event) {
  state.libraryManagementDisplayNameDraft = event.target.value;
}

function onCreateLibraryPopoverToggle(event) {
  const popover = event.currentTarget as HTMLDetailsElement | null;
  if (!(popover instanceof HTMLDetailsElement)) {
    return;
  }
  state.createLibraryPopoverOpen = popover.open;
  if (popover.open) {
    state.manageLibraryPopoverOpen = false;
  }
}

function onManageLibraryPopoverToggle(event) {
  const popover = event.currentTarget as HTMLDetailsElement | null;
  if (!(popover instanceof HTMLDetailsElement)) {
    return;
  }
  state.manageLibraryPopoverOpen = popover.open;
  if (popover.open) {
    state.createLibraryPopoverOpen = false;
  }
}

async function onRenameLibrary(event) {
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

async function onToggleLibraryArchive() {
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

async function onDeleteLibrary() {
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
    state.selectedVisualUnit = null;
    state.selectedVisualUnitLibraryId = "";
    state.searchOutcome = null;
    state.searchInFlight = false;
    state.lastSearchRequest = null;
    state.searchPreparationDisclosureOpen = false;
    state.searchJobsDisclosureOpen = false;
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

async function onSelectLibrary(event) {
  await switchCurrentLibrary(event.target.value);
}

function onProviderConfigSelect(event) {
  const providerId = event.target.value;
  const provider = state.providerConfigs.find((item) => item.provider_id === providerId) ?? null;
  hydrateProviderEditor(provider);
  renderWorkspace();
}

function onProviderEnabledChange(event) {
  state.providerEnabledDraft = event.target.checked;
}

function onProviderBaseUrlInput(event) {
  state.providerBaseUrlDraft = event.target.value;
}

async function onSubmitProviderConfig(event) {
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

function onResetProviderConfigForm() {
  hydrateProviderEditor(selectedProviderConfig());
  state.globalError = null;
  renderWorkspace();
}

async function onEditProviderConfig(event) {
  const providerId = event.currentTarget.dataset.providerEditId;
  if (!providerId) {
    return;
  }
  const provider = state.providerConfigs.find((item) => item.provider_id === providerId) ?? null;
  hydrateProviderEditor(provider);
  state.globalError = null;
  renderWorkspace();
}

function updateGlobalContentTypeBinding(
  updater: (binding: ContentTypeBindingPayload) => ContentTypeBindingPayload
) {
  const contentType = selectedGlobalContentTypeKey();
  if (!contentType) {
    return;
  }
  state.globalContentTypes.content_types[contentType] = updater(selectedGlobalContentTypeBinding());
}

function updateLibraryContentTypeBinding(
  updater: (binding: ContentTypeBindingPayload) => ContentTypeBindingPayload
) {
  const contentType = selectedLibraryContentTypeKey();
  if (!contentType) {
    return;
  }
  state.libraryContentTypes.content_types[contentType] = updater(selectedLibraryContentTypeBinding());
}

function onGlobalContentTypeChange(event) {
  state.selectedGlobalContentType = event.target.value;
  resetGlobalModelTestState();
  renderWorkspace();
}

function onSelectContentTypeTab(event: Event) {
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

function onGlobalContentTypeEnabledChange(event) {
  updateGlobalContentTypeBinding((binding) => ({
    ...binding,
    enabled: event.target.checked,
  }));
  renderWorkspace();
}

function onGlobalContentTypeProviderChange(event) {
  updateGlobalContentTypeBinding((binding) =>
    normalizeContentTypeBindingForProvider(event.target.value, binding)
  );
  resetGlobalModelTestState();
  renderWorkspace();
}

function onGlobalContentTypeModelIdInput(event) {
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

function onGlobalContentTypeVectorTypeChange(event) {
  updateGlobalContentTypeBinding((binding) => ({
    ...binding,
    vector_type: event.target.value,
  }));
  renderWorkspace();
}

async function onSubmitGlobalContentTypes(event) {
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

function onLibraryContentTypeChange(event) {
  state.selectedLibraryContentType = event.target.value;
  resetLibraryModelTestState();
  renderWorkspace();
}

function onLibraryOverrideModeChange(event: Event) {
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

function onLibraryContentTypeEnabledChange(event) {
  updateLibraryContentTypeBinding((binding) => ({
    ...binding,
    enabled: event.target.checked,
  }));
  renderWorkspace();
}

function onLibraryContentTypeProviderChange(event) {
  updateLibraryContentTypeBinding((binding) =>
    normalizeContentTypeBindingForProvider(event.target.value, binding)
  );
  resetLibraryModelTestState();
  renderWorkspace();
}

function onLibraryContentTypeModelIdInput(event) {
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

function onLibraryContentTypeVectorTypeChange(event) {
  updateLibraryContentTypeBinding((binding) => ({
    ...binding,
    vector_type: event.target.value,
  }));
  renderWorkspace();
}

async function onSubmitLibraryContentTypes(event) {
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

async function onResetLibraryContentTypes() {
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

function onGlobalModelTestModalityChange(event) {
  state.globalModelTestModalityDraft = event.target.value;
  state.globalModelTestFile = null;
  state.globalModelTestResult = null;
  state.globalModelTestError = null;
  renderWorkspace();
}

function onGlobalModelTestTextInput(event) {
  state.globalModelTestTextDraft = event.target.value;
  state.globalModelTestResult = null;
  state.globalModelTestError = null;
}

function onGlobalModelTestFileInput(event) {
  state.globalModelTestFile = event.target.files?.[0] ?? null;
  state.globalModelTestResult = null;
  state.globalModelTestError = null;
  renderWorkspace();
}

function onGlobalModelTestComparisonModalityChange(event) {
  state.globalModelTestComparisonModalityDraft = event.target.value;
  state.globalModelTestComparisonFile = null;
  state.globalModelTestResult = null;
  state.globalModelTestError = null;
  renderWorkspace();
}

function onGlobalModelTestComparisonTextInput(event) {
  state.globalModelTestComparisonTextDraft = event.target.value;
  state.globalModelTestResult = null;
  state.globalModelTestError = null;
}

function onGlobalModelTestComparisonFileInput(event) {
  state.globalModelTestComparisonFile = event.target.files?.[0] ?? null;
  state.globalModelTestResult = null;
  state.globalModelTestError = null;
  renderWorkspace();
}

function onLibraryModelTestModalityChange(event) {
  state.libraryModelTestModalityDraft = event.target.value;
  state.libraryModelTestFile = null;
  state.libraryModelTestResult = null;
  state.libraryModelTestError = null;
  renderWorkspace();
}

function onLibraryModelTestTextInput(event) {
  state.libraryModelTestTextDraft = event.target.value;
  state.libraryModelTestResult = null;
  state.libraryModelTestError = null;
}

function onLibraryModelTestFileInput(event) {
  state.libraryModelTestFile = event.target.files?.[0] ?? null;
  state.libraryModelTestResult = null;
  state.libraryModelTestError = null;
  renderWorkspace();
}

function onLibraryModelTestComparisonModalityChange(event) {
  state.libraryModelTestComparisonModalityDraft = event.target.value;
  state.libraryModelTestComparisonFile = null;
  state.libraryModelTestResult = null;
  state.libraryModelTestError = null;
  renderWorkspace();
}

function onLibraryModelTestComparisonTextInput(event) {
  state.libraryModelTestComparisonTextDraft = event.target.value;
  state.libraryModelTestResult = null;
  state.libraryModelTestError = null;
}

function onLibraryModelTestComparisonFileInput(event) {
  state.libraryModelTestComparisonFile = event.target.files?.[0] ?? null;
  state.libraryModelTestResult = null;
  state.libraryModelTestError = null;
  renderWorkspace();
}

async function submitSettingsModelTest(scope: "global" | "library") {
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

async function onSubmitGlobalModelTest(event) {
  event.preventDefault();
  await submitSettingsModelTest("global");
}

async function onSubmitLibraryModelTest(event) {
  event.preventDefault();
  await submitSettingsModelTest("library");
}

function onImportPathsInput(event) {
  state.importPathsDraft = event.target.value;
}

function onSearchTextInput(event) {
  state.searchTextDraft = event.target.value;
}

function onSearchFilterKindChange(event) {
  state.searchFilters.visualUnitKind = event.target.value;
}

function onSearchFilterSourceTypeChange(event) {
  state.searchFilters.sourceType = event.target.value;
}

function onSearchFilterPathPrefixInput(event) {
  state.searchFilters.pathPrefix = event.target.value;
}

function onSearchFilterTimeRangeStartInput(event) {
  state.searchFilters.timeRangeStartMsDraft = event.target.value;
}

function onSearchFilterTimeRangeEndInput(event) {
  state.searchFilters.timeRangeEndMsDraft = event.target.value;
}

function onClearSearchFilters() {
  resetSearchFilters();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onSourceRootPathInput(event) {
  state.sourceRootPathDraft = event.target.value;
}

function onSourceRootEnabledInput(event) {
  state.sourceRootEnabledDraft = event.target.checked;
}

function onSourceRootIncludeGlobsInput(event) {
  state.sourceRootIncludeGlobsDraft = event.target.value;
}

function onSourceRootExcludeGlobsInput(event) {
  state.sourceRootExcludeGlobsDraft = event.target.value;
}

function onSourceRootIncludeExtensionsInput(event) {
  state.sourceRootIncludeExtensionsDraft = event.target.value;
}

function onSourceFilterRootChange(event) {
  state.inventoryFilters.sourceRootId = event.target.value;
  refreshWorkspace({ keepSelection: true }).catch((error) => {
    state.globalError = toApiError(error);
    renderWorkspace();
  });
}

function onSourceFilterTypeChange(event) {
  state.inventoryFilters.sourceType = event.target.value;
  refreshWorkspace({ keepSelection: true }).catch((error) => {
    state.globalError = toApiError(error);
    renderWorkspace();
  });
}

function onSourceFilterStatusChange(event) {
  state.inventoryFilters.sourceStatus = event.target.value;
  refreshWorkspace({ keepSelection: true }).catch((error) => {
    state.globalError = toApiError(error);
    renderWorkspace();
  });
}

function parseImportPaths(value) {
  return value
    .split("\n")
    .map((item) => item.trim())
    .filter(Boolean);
}

async function onSubmitSourceRoot(event) {
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

function onResetSourceRootEditor() {
  resetSourceRootEditor();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onEditSourceRoot(event) {
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

function onSearchPreparationDisclosureToggle(event) {
  state.searchPreparationDisclosureOpen = event.currentTarget.open;
}

async function triggerJobBackedAction<T extends { job?: JobSnapshot | null }>(
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

async function onRefreshLibrarySources(event?) {
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

async function onRescanLibrarySources(event?) {
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

async function onRebuildLibrarySources() {
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

async function onCleanupRetiredVectorSpaces() {
  if (!state.selectedLibraryId) {
    return;
  }

  try {
    await triggerJobBackedAction<MaintenanceActionData>(
      `/libraries/${state.selectedLibraryId}/maintenance`,
      "正在清理退役执行空间...",
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

async function onCancelJob(event) {
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

async function onRetryJob(event) {
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

async function onResumeJob(event) {
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

async function onRefreshSourceRoot(event) {
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

async function onRescanSourceRoot(event) {
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

async function onToggleSourceRoot(event) {
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

async function onDeleteSourceRoot(event) {
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

async function onImportPaths(event) {
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

async function importPaths(paths: string[]): Promise<ImportPathsData> {
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

  const firstVisualUnit = state.importReceipt.accepted
    .flatMap((item) => item.visual_units ?? [])
    .at(0);
  if (firstVisualUnit && state.selectedLibraryId) {
    await loadVisualUnit(state.selectedLibraryId, firstVisualUnit.visual_unit_id);
  }
  return state.importReceipt;
}

async function waitForJobTerminal(jobId: string): Promise<JobSnapshot> {
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

async function onSearchSubmit(event) {
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

async function onLoadMoreSearchResults() {
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

function sharedSearchRequestFields() {
  const filters = searchFiltersPayload();
  return {
    search_scope: searchScopeRequestPayload(),
    top_k: SEARCH_PAGE_SIZE,
    debug: true,
    ...(filters ? { filters } : {}),
  };
}

async function executeSearchRequest(
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
  const mergedResults = options.append
    ? [...(state.searchOutcome?.results ?? []), ...(data.results ?? [])]
    : data.results;
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

async function runTextSearch() {
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

async function runImageSearch() {
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

async function runVideoSearch() {
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

async function runDocumentSearch() {
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

async function searchText(text: string): Promise<SearchOutcomeState> {
  return executeSearchRequest({
    endpoint: "/search/text",
    body: {
      ...sharedSearchRequestFields(),
      text,
    },
  });
}

async function uploadQueryImage(file: File): Promise<QueryAssetData> {
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

async function uploadQueryVideo(file: File): Promise<QueryAssetData> {
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

async function uploadQueryDocument(file: File): Promise<QueryAssetData> {
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

async function searchImage(imageInput: Record<string, unknown>): Promise<SearchOutcomeState> {
  return executeSearchRequest({
    endpoint: "/search/image",
    body: {
      ...sharedSearchRequestFields(),
      image_input: imageInput,
    },
  });
}

async function searchVideo(videoInput: Record<string, unknown>): Promise<SearchOutcomeState> {
  return executeSearchRequest({
    endpoint: "/search/video",
    body: {
      ...sharedSearchRequestFields(),
      video_input: videoInput,
    },
  });
}

async function searchDocument(documentInput: Record<string, unknown>): Promise<SearchOutcomeState> {
  return executeSearchRequest({
    endpoint: "/search/document",
    body: {
      ...sharedSearchRequestFields(),
      document_input: documentInput,
    },
  });
}

async function onSelectWorkspace(event) {
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

async function onOpenHitLibraryContext(event) {
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

async function onSelectSearchResultLibraryFocus(event) {
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

function onSelectSettingsSection(event) {
  const nextSection = event.currentTarget.dataset.settingsSection as SettingsSection | undefined;
  if (!nextSection || nextSection === state.selectedSettingsSection) {
    return;
  }
  state.selectedSettingsSection = nextSection;
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

async function onOpenSettingsSection(event) {
  const nextSection = event.currentTarget.dataset.openSettingsSection as SettingsSection | undefined;
  if (!nextSection) {
    return;
  }

  state.selectedSettingsSection = nextSection;
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

function onSelectInventorySource(event) {
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

function onSearchJobsDisclosureToggle(event) {
  const library = selectedLibrary();
  if (event.currentTarget.open && (library?.counts.pending_jobs ?? 0) > 0) {
    return;
  }
  state.searchJobsDisclosureOpen = event.currentTarget.open;
}

function onToggleSearchFiltersPanel() {
  state.searchFiltersPanelOpen = !state.searchFiltersPanelOpen;
  renderWorkspace();
}

function onSelectSearchScope(event) {
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

function onOpenUtilityDrawer(event) {
  const section = event.currentTarget.dataset.utilityDrawerOpen as UtilityDrawerSection | undefined;
  if (!section) {
    return;
  }

  state.utilityDrawerOpen = true;
  state.utilityDrawerSection = section;
  renderWorkspace();
}

function onSelectUtilityDrawerSection(event) {
  const section =
    event.currentTarget.dataset.utilityDrawerSection as UtilityDrawerSection | undefined;
  if (!section) {
    return;
  }

  state.utilityDrawerOpen = true;
  state.utilityDrawerSection = section;
  renderWorkspace();
}

function onCloseUtilityDrawer() {
  state.utilityDrawerOpen = false;
  renderWorkspace();
}

function onCloseMobileSheet(event) {
  const sheet = event.currentTarget.dataset.mobileSheetClose;
  if (sheet === "inventory") {
    state.inventoryDetailSheetOpen = false;
  } else {
    state.searchDetailSheetOpen = false;
  }
  renderWorkspace();
}

async function onUtilitiesAction(event) {
  const action = event.currentTarget.dataset.utilitiesAction;
  if (!action) {
    return;
  }

  state.globalError = null;
  state.statusMessage = null;

  if (action === "focus-source-prep") {
    state.activeWorkspace = "search";
    state.searchPreparationDisclosureOpen = true;
    state.utilityDrawerOpen = false;
    renderWorkspace();
    return;
  }

  if (action === "focus-search-jobs") {
    state.activeWorkspace = "search";
    state.searchJobsDisclosureOpen = true;
    state.utilityDrawerOpen = false;
    renderWorkspace();
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

function onSelectSearchMode(event) {
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

function onQueryImageInput(event) {
  const [file] = event.target.files ?? [];
  if (file) {
    setPendingQueryImageFile(file);
  }
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onQueryDocumentInput(event) {
  const [file] = event.target.files ?? [];
  if (file) {
    setPendingQueryDocumentFile(file);
  }
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

async function onQueryVideoInput(event) {
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

function onQueryImagePaste(event) {
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

function onClearQueryImage() {
  clearQueryImageState();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onClearQueryDocument() {
  clearQueryDocumentState();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onQueryVideoSourceSelect(event) {
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

function onClearQueryVideo() {
  clearQueryVideoState();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onClearQueryVideoRange() {
  state.queryVideoRange = null;
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onClearQueryDocumentRange() {
  state.queryDocumentStartPageDraft = "";
  state.queryDocumentEndPageDraft = "";
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onQueryVideoRangeStartInput(event) {
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

function onQueryVideoRangeEndInput(event) {
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

function onQueryVideoPreviewLoadedMetadata(event) {
  syncQueryVideoDurationFromVideoElement(event.currentTarget);
}

function onQueryDocumentRangeStartInput(event) {
  state.queryDocumentStartPageDraft = event.target.value.trim();
  state.globalError = null;
  state.statusMessage = null;
  syncQueryDocumentRangeUi();
}

function onQueryDocumentRangeEndInput(event) {
  state.queryDocumentEndPageDraft = event.target.value.trim();
  state.globalError = null;
  state.statusMessage = null;
  syncQueryDocumentRangeUi();
}

function resolveLibraryObjectQueryImage(
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

function resolveLibraryObjectQueryVideo(
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

function resolveLibraryObjectQueryDocument(
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

async function onUseAsQueryImage(event) {
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

async function onUseAsQueryVideo(event) {
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

async function onUseAsQueryDocument(event) {
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

async function loadVisualUnit(libraryId: string, visualUnitId: string): Promise<void> {
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

async function onSelectVisualUnit(event) {
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

let workspacePollInFlight = false;

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
