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
  SearchOutcomeState,
  SourceInventoryItem,
  SourceRootRulesPayload,
  SourceRootSnapshot,
  SourceRootsListData,
  SourcesListData,
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

interface DemoFixture {
  path: string;
  query: string;
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

const demoFixture: DemoFixture = {
  path: "tests/fixtures/tatdqa-page-images/images/tatdqa-page-0001.png",
  query: "What is the percentage change in the net cash provided from operating activities?",
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
  libraryDisplayNameDraft: "",
  libraryIdDraft: "",
  selectedLibraryId: "",
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
  searchOutcome: null,
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
let lastRenderedDetailSignature: string | null = null;

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

function libraryDisplayName(
  library: Pick<LibrarySnapshot, "display_name" | "id"> | null | undefined
): string {
  if (!library) {
    return "";
  }
  return library.display_name?.trim() || library.id;
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

function resetSearchFilters() {
  state.searchFilters = {
    visualUnitKind: "",
    sourceType: "",
    pathPrefix: "",
    timeRangeStartMsDraft: "",
    timeRangeEndMsDraft: "",
  };
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
    return `${providerId} (missing)`;
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
    parts.push(`runtime ${selection.model_revision}`);
  }
  return parts.join(" · ");
}

function formatResolvedContentModel(selection: ResolvedContentModelSelectionPayload | undefined) {
  return formatResolvedModel(selection);
}

function formatBindingSource(bindingSource: BindingSource | undefined) {
  switch (bindingSource) {
    case "global_content_type":
      return "global content type";
    case "library_content_type":
      return "library content type";
    case "settings_model_test":
      return "settings model test";
    default:
      return bindingSource ? bindingSource.replaceAll("_", " ") : "unknown";
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
  return values?.length ? values.join(", ") : "none";
}

function formatEmbeddingCapabilities(
  capabilities: EmbeddingCapabilities | undefined,
  options: { includePrefix?: boolean } = {}
) {
  if (!capabilities) {
    return options.includePrefix ? "Embedding capabilities · unavailable" : "unavailable";
  }

  const parts = [
    `inputs ${formatEmbeddingCapabilityValues(capabilities.input_types)}`,
    `vectors ${formatEmbeddingCapabilityValues(capabilities.vector_types)}`,
    `mixed inputs ${capabilities.supports_mixed_inputs ? "yes" : "no"}`,
  ];
  if (options.includePrefix) {
    parts.unshift("Embedding capabilities");
  }
  return parts.join(" · ");
}

function formatExecutionInputTypes(inputTypes: string[] | undefined, options: { includePrefix?: boolean } = {}) {
  const value = inputTypes?.length ? inputTypes.join(", ") : "none";
  return options.includePrefix ? `Execution inputs · ${value}` : value;
}

function resetInventoryState() {
  state.librarySources = [];
  state.inventorySummary = emptyInventorySummary();
}

function searchHasMoreResults() {
  return Boolean(state.searchOutcome?.next_cursor && state.lastSearchRequest);
}

function searchFiltersSummary() {
  const tokens = [];
  if (state.searchFilters.visualUnitKind) {
    tokens.push(`kind=${state.searchFilters.visualUnitKind}`);
  }
  if (state.searchFilters.sourceType) {
    tokens.push(`source_type=${state.searchFilters.sourceType}`);
  }
  if (state.searchFilters.pathPrefix.trim()) {
    tokens.push(`path_prefix=${state.searchFilters.pathPrefix.trim()}`);
  }
  if (
    state.searchFilters.timeRangeStartMsDraft.trim() ||
    state.searchFilters.timeRangeEndMsDraft.trim()
  ) {
    tokens.push(
      `time_range=${state.searchFilters.timeRangeStartMsDraft.trim() || "?"}→${state.searchFilters.timeRangeEndMsDraft.trim() || "?"}`
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

function sortedContentTypeKeys(payload: ContentTypesPayload) {
  return Object.keys(payload.content_types).sort((left, right) => {
    return contentTypeOrderValue(left) - contentTypeOrderValue(right) || left.localeCompare(right);
  });
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
  if (selected && state.globalContentTypes.content_types[selected]) {
    return selected;
  }
  return sortedContentTypeKeys(state.globalContentTypes)[0] ?? "";
}

function selectedLibraryContentTypeKey() {
  const selected = state.selectedLibraryContentType;
  if (selected && state.libraryContentTypes.content_types[selected]) {
    return selected;
  }
  return sortedContentTypeKeys(state.libraryContentTypes)[0] ?? "";
}

function selectedGlobalContentTypeBinding(): ContentTypeBindingPayload {
  return (
    state.globalContentTypes.content_types[selectedGlobalContentTypeKey()] ??
    defaultContentTypeBinding()
  );
}

function selectedLibraryContentTypeBinding(): ContentTypeBindingPayload {
  return (
    state.libraryContentTypes.content_types[selectedLibraryContentTypeKey()] ??
    defaultContentTypeBinding()
  );
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
  return `${entry.message} · 原生输入：${supportedModalities.join(", ")}`;
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
    parts.push("disabled");
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
          <h5>Vectors</h5>
          <pre data-testid="${testIdPrefix}-vectors">${escapeHtml(JSON.stringify(result.vectors, null, 2))}</pre>
        </div>
        ${
          result.pooled_vector?.length
            ? `
              <div class="detail-block">
                <h5>Pooled vector</h5>
                <pre data-testid="${testIdPrefix}-pooled-vector">${escapeHtml(JSON.stringify(result.pooled_vector, null, 2))}</pre>
              </div>
            `
            : ""
        }
      </div>
      <div class="detail-block">
        <h5>Input summary</h5>
        <pre>${escapeHtml(JSON.stringify(result.input_summary, null, 2))}</pre>
      </div>
      ${
        result.comparison
          ? `
            <div class="detail-block">
              <h5>Comparison</h5>
              <div class="job-meta">
                <span class="pill muted">${escapeHtml(result.comparison.operation_kind)}</span>
                <span class="pill muted" data-testid="${testIdPrefix}-comparison-shape">${escapeHtml(
                  formatModelTestShape(result.comparison.vector_shape)
                )}</span>
                <span class="pill ready" data-testid="${testIdPrefix}-similarity">${escapeHtml(
                  result.comparison.similarity_to_primary.toFixed(6)
                )}</span>
              </div>
              <p class="helper">input_modality: ${escapeHtml(result.comparison.input_modality)}</p>
              <div class="detail-grid model-test-grid">
                <div class="detail-block">
                  <h5>Comparison vectors</h5>
                  <pre data-testid="${testIdPrefix}-comparison-vectors">${escapeHtml(
                    JSON.stringify(result.comparison.vectors, null, 2)
                  )}</pre>
                </div>
                ${
                  result.comparison.pooled_vector?.length
                    ? `
                      <div class="detail-block">
                        <h5>Comparison pooled vector</h5>
                        <pre data-testid="${testIdPrefix}-comparison-pooled-vector">${escapeHtml(
                          JSON.stringify(result.comparison.pooled_vector, null, 2)
                        )}</pre>
                      </div>
                    `
                    : ""
                }
              </div>
              <div class="detail-block">
                <h5>Comparison input summary</h5>
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

  return `
    <section class="model-test-panel" data-testid="${testIdPrefix}-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">Test</p>
          <h3>${scope === "global" ? "Test current global model" : "Test current library model"}</h3>
        </div>
      </div>
      <p class="helper" data-testid="${testIdPrefix}-draft-summary">
        ${escapeHtml(currentDraftProviderSummary(selection.provider_id))} · ${escapeHtml(selection.model_id)}
      </p>
      <p class="helper" data-testid="${testIdPrefix}-support-message">
        ${escapeHtml(settingsModelTestSupportMessage(selection, supportedModalities))}
      </p>
      <form id="${testIdPrefix}-form" class="stack-form" data-testid="${testIdPrefix}-form">
        <div class="filter-grid settings-filter-grid">
          <label>
            <span>primary.input_modality</span>
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
                          ${escapeHtml(modality)}
                        </option>
                      `
                    )
                    .join("")
                : '<option value="" selected>not_supported</option>'}
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
            <span>comparison.input_modality</span>
            <select
              id="${testIdPrefix}-comparison-modality"
              data-testid="${testIdPrefix}-comparison-modality"
              ${supportedModalities.length ? "" : "disabled"}
            >
              <option value="" ${comparisonModalityDraft ? "" : "selected"}>none</option>
              ${supportedModalities
                .map(
                  (modality) => `
                    <option value="${escapeHtml(modality)}" ${
                      modality === comparisonModalityDraft ? "selected" : ""
                    }>
                      ${escapeHtml(modality)}
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
            ${pending ? "Testing..." : "测试当前模型"}
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
  return source.source_root_label || source.source_root_id || "manual import";
}

function currentWorkspaceMeta() {
  if (state.activeWorkspace === "inventory") {
    return {
      title: "Inventory workspace",
      summary:
        "来源清单、状态过滤与 coverage 核对集中到独立 Inventory 工作区；Search 回到查询、结果与详情的连续主任务流。",
    };
  }

  if (state.activeWorkspace === "settings") {
    return {
      title: "Settings workspace",
      summary:
        "在同一处查看内建 provider、全局 content type 绑定、库级 content type 绑定和当前 resolved content model。",
    };
  }

  return {
    title: "Search workspace",
    summary:
      "查询入口、结果浏览与详情保持在同一搜索工作区连续完成；来源清单与状态过滤已移到独立 Inventory 工作区。",
  };
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

function selectedVisualUnitId() {
  return state.selectedVisualUnit?.visual_unit?.visual_unit_id ?? null;
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
        <h4>Working</h4>
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
          <h4>Accepted</h4>
          <ul class="data-list">
            ${state.importReceipt.accepted
              .map(
                (item) => `
                  <li>
                    <div class="list-head">
                      <strong>${escapeHtml(item.kind)}</strong>
                      <span class="helper">${(item.visual_units ?? []).length} 个 visual units</span>
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
                                    查看 ${escapeHtml(visualUnit.kind)} · ${escapeHtml(visualUnit.visual_unit_id)}
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
          <h4>Rejected</h4>
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

  parts.push(includeGlobs.length ? `include ${includeGlobs.length}` : "include all");
  parts.push(excludeGlobs.length ? `exclude ${excludeGlobs.length}` : "exclude none");
  parts.push(includeExtensions.length ? includeExtensions.join(", ") : "all source types");
  return parts.join(" · ");
}

function formatScanTime(lastScanAtMs) {
  if (!lastScanAtMs) {
    return "尚未 refresh / rescan";
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
        库级 refresh
      </button>
      <button
        type="button"
        id="library-rescan-button"
        data-testid="library-rescan-button"
        class="secondary-button"
        ${library && state.sourceRoots.length ? "" : "disabled"}
      >
        库级 rescan
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
                    <div><dt>Observed</dt><dd>${sourceRoot.coverage_summary?.observed_file_count ?? 0}</dd></div>
                    <div><dt>Matched</dt><dd>${sourceRoot.coverage_summary?.matched_file_count ?? 0}</dd></div>
                    <div><dt>Active</dt><dd>${sourceRoot.coverage_summary?.active_source_count ?? 0}</dd></div>
                    <div><dt>Inactive</dt><dd>${sourceRoot.coverage_summary?.inactive_source_count ?? 0}</dd></div>
                  </dl>
                  <p class="helper">${escapeHtml(renderSourceRootRulesSummary(sourceRoot.rules))}</p>
                  <p class="helper">Last scan: ${escapeHtml(formatScanTime(sourceRoot.coverage_summary?.last_scan_at_ms))}</p>
                  ${
                    sourceRoot.last_action
                      ? `<p class="helper">Last action: ${escapeHtml(sourceRoot.last_action.action)} · ${escapeHtml(sourceRoot.last_action.status)} · ${escapeHtml(sourceRoot.last_action.summary)}</p>`
                      : ""
                  }
                  <div class="inline-actions">
                    <button type="button" class="secondary-button" data-source-root-edit-id="${escapeHtml(sourceRoot.source_root_id)}">编辑</button>
                    <button type="button" data-source-root-refresh-id="${escapeHtml(sourceRoot.source_root_id)}" ${sourceRoot.enabled ? "" : "disabled"}>refresh</button>
                    <button type="button" class="secondary-button" data-source-root-rescan-id="${escapeHtml(sourceRoot.source_root_id)}" ${sourceRoot.enabled ? "" : "disabled"}>rescan</button>
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
    : '<p class="empty" data-testid="source-root-empty">当前库还没有来源根。先创建一个本地目录来源根，再触发 refresh / rescan。</p>';

  return `
    <section class="panel panel-tight">
      <div class="panel-head">
        <div>
          <p class="eyebrow">Sources</p>
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
          <span>Include globs</span>
          <textarea
            id="source-root-include-globs"
            data-testid="source-root-include-globs-input"
            rows="3"
            placeholder="images/**&#10;reports/*.pdf"
            ${library ? "" : "disabled"}
          >${escapeHtml(state.sourceRootIncludeGlobsDraft)}</textarea>
        </label>
        <label>
          <span>Exclude globs</span>
          <textarea
            id="source-root-exclude-globs"
            data-testid="source-root-exclude-globs-input"
            rows="3"
            placeholder="**/*.tmp&#10;archive/**"
            ${library ? "" : "disabled"}
          >${escapeHtml(state.sourceRootExcludeGlobsDraft)}</textarea>
        </label>
        <label>
          <span>Include extensions</span>
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
  return `
    <div class="workspace-switch" data-testid="workspace-switch">
      <button
        type="button"
        class="${state.activeWorkspace === "search" ? "" : "secondary-button"}"
        data-testid="workspace-tab-search"
        data-workspace="search"
      >
        Search
      </button>
      <button
        type="button"
        class="${state.activeWorkspace === "inventory" ? "" : "secondary-button"}"
        data-testid="workspace-tab-inventory"
        data-workspace="inventory"
      >
        Inventory
      </button>
      <button
        type="button"
        class="${state.activeWorkspace === "settings" ? "" : "secondary-button"}"
        data-testid="workspace-tab-settings"
        data-workspace="settings"
      >
        Settings
      </button>
    </div>
  `;
}

function renderInventoryBridge(library) {
  if (!library) {
    return "";
  }

  const summaryText =
    state.activeWorkspace === "inventory" && state.inventorySummary.total
      ? `当前库共有 ${state.inventorySummary.total} 条来源记录，active ${state.inventorySummary.active}，invalidated ${state.inventorySummary.invalidated}，out_of_scope ${state.inventorySummary.out_of_scope}。`
      : "来源清单、状态过滤与来源级观察已移到独立 Inventory 工作区。";

  return `
    <div class="workspace-bridge" data-testid="inventory-bridge">
      <p class="eyebrow">Inventory</p>
      <p class="helper" data-testid="inventory-bridge-summary">${escapeHtml(summaryText)}</p>
      <div class="inline-actions">
        ${
          state.activeWorkspace === "inventory"
            ? '<span class="pill ready" data-testid="inventory-bridge-state">Inventory active</span>'
            : `<button
                type="button"
                class="secondary-button"
                data-testid="inventory-bridge-button"
                data-workspace="inventory"
              >
                查看 Inventory
              </button>`
        }
      </div>
    </div>
  `;
}

function renderInventorySummaryBar() {
  const summaryItems = [
    { label: "Total", value: state.inventorySummary.total, testId: "inventory-summary-total" },
    { label: "Active", value: state.inventorySummary.active, testId: "inventory-summary-active" },
    {
      label: "Invalidated",
      value: state.inventorySummary.invalidated,
      testId: "inventory-summary-invalidated",
    },
    {
      label: "Out of scope",
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

function renderProviderOptions(currentValue = "", includeEmpty = false) {
  const emptyOption = includeEmpty
    ? `<option value="" ${!currentValue ? "selected" : ""}>none</option>`
    : "";
  const hasCurrentValue =
    !!currentValue && state.providerConfigs.some((provider) => provider.provider_id === currentValue);
  const missingOption =
    currentValue && !hasCurrentValue
      ? `<option value="${escapeHtml(currentValue)}" selected>${escapeHtml(currentValue)} (configured)</option>`
      : "";
  return `${emptyOption}${missingOption}${state.providerConfigs
    .map(
      (provider) => `
        <option value="${escapeHtml(provider.provider_id)}" ${provider.provider_id === currentValue ? "selected" : ""}>
          ${escapeHtml(provider.display_name)} (${escapeHtml(provider.provider_kind)}${provider.enabled ? "" : " · disabled"})
        </option>
      `
    )
    .join("")}`;
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
            `${selection.content_type}: ${formatResolvedContentModel(selection)} · ${selection.status}`
        )
        .join(" | ")
    : "当前库的 resolved content model 尚未加载。";

  return `
    <div class="workspace-bridge" data-testid="provider-bridge">
      <p class="eyebrow">Providers</p>
      <p class="helper" data-testid="provider-bridge-summary">${escapeHtml(summary)}</p>
      <div class="inline-actions">
        ${
          state.activeWorkspace === "settings"
            ? '<span class="pill ready" data-testid="provider-bridge-state">Settings active</span>'
            : `<button
                type="button"
                class="secondary-button"
                data-testid="provider-bridge-button"
                data-workspace="settings"
              >
                查看 Settings
              </button>`
        }
      </div>
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
    : `<p class="empty">当前还没有可见 provider。</p>`;

  return `
    <section class="panel settings-panel" data-testid="provider-configs-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">Providers</p>
          <h2>Built-in providers</h2>
        </div>
      </div>
      <form id="provider-config-form" class="stack-form" data-testid="provider-config-form">
        <label>
          <span>Provider</span>
          <select id="provider-config-id" data-testid="provider-config-id">
            <option value="" ${!state.editingProviderId ? "selected" : ""}>选择一个 provider</option>
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
            <span>Enabled</span>
          </label>
          <label>
            <span>Base URL</span>
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
          editingProvider?.readonly_reason
            ? `<p class="helper" data-testid="provider-readonly-reason">${escapeHtml(editingProvider.readonly_reason)}</p>`
            : ""
        }
        <div class="inline-actions">
          <button type="submit" data-testid="provider-config-submit-button" ${!editingProvider ? "disabled" : ""}>
            保存 provider 配置
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
      ${listMarkup}
    </section>
  `;
}

function renderModelIdOptions(providerId: string, currentValue: string, includeEmpty = false) {
  const entries = catalogEntriesForProvider(providerId);
  const hasCurrentValue = !!currentValue && entries.some((entry) => entry.model_id === currentValue);
  const emptyOption = includeEmpty
    ? `<option value="" ${!currentValue ? "selected" : ""}>none</option>`
    : "";
  const missingOption =
    currentValue && !hasCurrentValue
      ? `<option value="${escapeHtml(currentValue)}" selected>${escapeHtml(currentValue)} (configured)</option>`
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

function renderGlobalContentTypesPanel() {
  const contentType = selectedGlobalContentTypeKey();
  const binding = selectedGlobalContentTypeBinding();
  const selection = selectionFromBinding(binding);
  const catalogEntry = selectedCatalogEntryForProvider(selection.provider_id, selection.model_id);
  const supportedModalities = selectedGlobalTestModalities();

  return `
    <section class="panel settings-panel" data-testid="global-content-types-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">Defaults</p>
          <h2>Global content type bindings</h2>
        </div>
      </div>
      <form id="global-content-types-form" class="stack-form" data-testid="global-content-types-form">
        <div class="filter-grid settings-filter-grid">
          <label>
            <span>content_type</span>
            <select id="global-content-type" data-testid="global-content-type">
              ${sortedContentTypeKeys(state.globalContentTypes)
                .map(
                  (value) => `
                    <option value="${escapeHtml(value)}" ${value === contentType ? "selected" : ""}>
                      ${escapeHtml(value)}
                    </option>
                  `
                )
                .join("")}
            </select>
          </label>
          <label class="checkbox-field">
            <input
              id="global-content-type-enabled"
              data-testid="global-content-type-enabled"
              type="checkbox"
              ${binding.enabled ? "checked" : ""}
            />
            <span>Enabled</span>
          </label>
        </div>
        <div class="filter-grid settings-filter-grid">
          <label>
            <span>provider</span>
            <select id="global-content-type-provider-id" data-testid="global-content-type-provider-id">
              ${renderProviderOptions(selection.provider_id)}
            </select>
          </label>
          <label>
            <span>model_id</span>
            <select
              id="global-content-type-model-id"
              data-testid="global-content-type-model-id"
              ${selection.provider_id === PROVIDER_ID_LOCAL_SIDECAR ? "disabled" : ""}
            >
              ${renderModelIdOptions(selection.provider_id, selection.model_id)}
            </select>
          </label>
          <label>
            <span>vector_type</span>
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
            `${contentType} -> ${binding.model || "unconfigured"} · ${binding.vector_type || "no-vector-type"} · ${binding.enabled ? "enabled" : "disabled"}`
          )}
        </p>
        <div class="inline-actions">
          <button type="submit" data-testid="global-content-types-submit-button">保存全局 content type 绑定</button>
        </div>
      </form>
      ${renderSettingsModelTestPanel({
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
      })}
    </section>
  `;
}

function renderLibraryContentTypesPanel(library: LibrarySnapshot | null) {
  if (!library) {
    return `
      <section class="panel settings-panel" data-testid="library-content-types-panel">
        <div class="panel-head">
          <div>
            <p class="eyebrow">Overrides</p>
            <h2>Library content type bindings</h2>
          </div>
        </div>
        <p class="empty">先选择一个库，再编辑库级 content type binding。</p>
      </section>
    `;
  }

  const contentType = selectedLibraryContentTypeKey();
  const binding = selectedLibraryContentTypeBinding();
  const selection = selectionFromBinding(binding);
  const catalogEntry = selectedCatalogEntryForProvider(selection.provider_id, selection.model_id);
  const supportedModalities = selectedLibraryTestModalities();

  return `
    <section class="panel settings-panel" data-testid="library-content-types-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">Overrides</p>
          <h2>Library content type bindings</h2>
        </div>
      </div>
      <form id="library-content-types-form" class="stack-form" data-testid="library-content-types-form">
        <div class="filter-grid settings-filter-grid">
          <label>
            <span>content_type</span>
            <select id="library-content-type" data-testid="library-content-type">
              ${sortedContentTypeKeys(state.libraryContentTypes)
                .map(
                  (value) => `
                    <option value="${escapeHtml(value)}" ${value === contentType ? "selected" : ""}>
                      ${escapeHtml(value)}
                    </option>
                  `
                )
                .join("")}
            </select>
          </label>
          <label class="checkbox-field">
            <input
              id="library-content-type-enabled"
              data-testid="library-content-type-enabled"
              type="checkbox"
              ${binding.enabled ? "checked" : ""}
            />
            <span>Enabled</span>
          </label>
        </div>
        <div class="filter-grid settings-filter-grid">
          <label>
            <span>provider</span>
            <select id="library-content-type-provider-id" data-testid="library-content-type-provider-id">
              ${renderProviderOptions(selection.provider_id)}
            </select>
          </label>
          <label>
            <span>model_id</span>
            <select
              id="library-content-type-model-id"
              data-testid="library-content-type-model-id"
              ${selection.provider_id === PROVIDER_ID_LOCAL_SIDECAR ? "disabled" : ""}
            >
              ${renderModelIdOptions(selection.provider_id, selection.model_id)}
            </select>
          </label>
          <label>
            <span>vector_type</span>
            <select
              id="library-content-type-vector-type"
              data-testid="library-content-type-vector-type"
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
            `${contentType} -> ${binding.model || "unconfigured"} · ${binding.vector_type || "no-vector-type"} · ${binding.enabled ? "enabled" : "disabled"}`
          )}
        </p>
        <div class="inline-actions">
          <button type="submit" data-testid="library-content-types-submit-button">保存库级 content type 绑定</button>
          <button
            type="button"
            id="library-content-types-reset-button"
            data-testid="library-content-types-reset-button"
            class="secondary-button"
          >
            重置为全局默认
          </button>
        </div>
      </form>
      ${renderSettingsModelTestPanel({
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
      })}
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
            <strong>${escapeHtml(contentType)}</strong>
            <span class="helper">${escapeHtml(formatResolvedContentModel(selection))} · ${escapeHtml(formatBindingSource(selection.binding_source))}</span>
            <span class="helper">${escapeHtml(formatResolvedContentModelContext(selection))}</span>
            <span class="helper">${escapeHtml(`vector_type ${selection.vector_type}`)}</span>
            ${
              selection.vector_space_id
                ? `<span class="helper">${escapeHtml(`vector_space ${selection.vector_space_id}`)}</span>`
                : ""
            }
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
          <p class="eyebrow">Resolved</p>
          <h2>Resolved content models for ${escapeHtml(libraryDisplayName(library))}</h2>
        </div>
      </div>
      <ul class="provider-resolution-list">${rows || '<li class="empty">暂无 resolved content model。</li>'}</ul>
    </section>
  `;
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
        vectorSpace.model_version ? `version ${vectorSpace.model_version}` : null,
        vectorSpace.vector_type ? `vector_type ${vectorSpace.vector_type}` : null,
        vectorSpace.content_types.length
          ? `content_types ${vectorSpace.content_types.join(", ")}`
          : null,
        typeof vectorSpace.retired_at_ms === "number"
          ? `retired_at ${new Date(vectorSpace.retired_at_ms).toLocaleString()}`
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
          <p class="eyebrow">Diagnostics</p>
          <h2>Vector spaces for ${escapeHtml(libraryDisplayName(library))}</h2>
        </div>
      </div>
      <ul class="provider-resolution-list">${rows || '<li class="empty">暂无 vector space diagnostics。</li>'}</ul>
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
                <span class="helper">${escapeHtml(`checked ${snapshot.last_checked_at}`)}</span>
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
            provider.model_version ? `version ${provider.model_version}` : null,
            provider.model_revision ? `revision ${provider.model_revision}` : null,
            provider.last_probed_at ? `probed ${provider.last_probed_at}` : null,
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
                `runtime adapters ${provider.runtime_adapters.join(", ")}`
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
          <p class="eyebrow">Runtime</p>
          <h2>Runtime health</h2>
        </div>
      </div>
      <ul class="provider-resolution-list">
        ${processRows || '<li class="empty">暂无 runtime health 快照。</li>'}
      </ul>
      <div class="inline-actions">
        <a href="${endpoints.appHealth}" target="_blank" rel="noreferrer">App health</a>
        <a href="${endpoints.sidecarHealth}" target="_blank" rel="noreferrer">Sidecar health</a>
        <a href="${endpoints.qdrantCollections}" target="_blank" rel="noreferrer">Qdrant</a>
      </div>
      <ul class="provider-resolution-list">
        ${providerRows || '<li class="empty">暂无 provider runtime diagnostics。</li>'}
      </ul>
    </section>
  `;
}

function renderSettingsPanel(library: LibrarySnapshot | null) {
  return `
    <div class="settings-stack" data-testid="settings-workspace">
      ${renderRuntimeHealthPanel()}
      ${renderProviderConfigsPanel()}
      ${renderGlobalContentTypesPanel()}
      ${renderLibraryContentTypesPanel(library)}
      ${renderResolvedContentModelsPanel(library)}
      ${renderVectorSpaceDiagnosticsPanel(library)}
    </div>
  `;
}

function renderLibrarySourcesPanel(library) {
  const list = state.librarySources.length
    ? `
        <ul class="inventory-source-list" data-testid="library-source-list">
          ${state.librarySources
            .map(
              (source) => `
                <li class="inventory-source-row" data-testid="library-source-card" data-source-id="${escapeHtml(source.source_id)}">
                  <div class="inventory-source-main">
                    <strong class="inventory-source-path">${escapeHtml(source.source_path)}</strong>
                    <p class="helper">${escapeHtml(sourceRootInventoryLabel(source))} · ${escapeHtml(source.source_type)} · ${escapeHtml(source.kind)}</p>
                  </div>
                  <div class="inventory-source-meta">
                    <span class="pill ${sourceStatusPillClass(source.status)}">${escapeHtml(source.status)}</span>
                    <span class="pill muted">visual units ${escapeHtml(source.visual_unit_count)}</span>
                    ${
                      source.status_reason
                        ? `<span class="helper inventory-source-reason">${escapeHtml(source.status_reason)}</span>`
                        : ""
                    }
                  </div>
                </li>
              `
            )
            .join("")}
        </ul>
      `
    : '<p class="empty" data-testid="library-source-empty">当前筛选条件下没有来源内容。</p>';

  return `
    <section class="panel inventory-panel" data-testid="inventory-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">Inventory</p>
          <h2>来源观察工作区</h2>
        </div>
      </div>
      ${renderInventorySummaryBar()}
      <div class="inventory-filter-dock">
        <div class="filter-grid inventory-filter-grid">
          <label>
            <span>来源根</span>
            <select id="source-filter-root" data-testid="source-filter-root" ${library ? "" : "disabled"}>
              <option value="">全部来源根</option>
              <option value="manual" ${state.inventoryFilters.sourceRootId === "manual" ? "selected" : ""}>manual import</option>
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
              <option value="image" ${state.inventoryFilters.sourceType === "image" ? "selected" : ""}>image</option>
              <option value="pdf" ${state.inventoryFilters.sourceType === "pdf" ? "selected" : ""}>pdf</option>
              <option value="video" ${state.inventoryFilters.sourceType === "video" ? "selected" : ""}>video</option>
            </select>
          </label>
          <label>
            <span>来源状态</span>
            <select id="source-filter-status" data-testid="source-filter-status" ${library ? "" : "disabled"}>
              <option value="">全部状态</option>
              <option value="active" ${state.inventoryFilters.sourceStatus === "active" ? "selected" : ""}>active</option>
              <option value="invalidated" ${state.inventoryFilters.sourceStatus === "invalidated" ? "selected" : ""}>invalidated</option>
              <option value="out_of_scope" ${state.inventoryFilters.sourceStatus === "out_of_scope" ? "selected" : ""}>out_of_scope</option>
            </select>
          </label>
        </div>
        <p class="helper" data-testid="inventory-filter-summary">
          当前显示 ${state.librarySources.length} / ${state.inventorySummary.total} 条来源记录。
        </p>
      </div>
      ${list}
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
  const title = `${visualUnit.kind} · ${sourceName(visualUnit.source_path)}`;

  if (visualUnit.kind === "image") {
    return `
      <img
        class="preview-image"
        data-testid="visual-preview"
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
        data-testid="visual-preview"
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
      data-testid="visual-preview"
      src="${escapeHtml(preview.url)}"
      title="${escapeHtml(title)}"
      loading="lazy"
    ></iframe>
  `;
}

function renderVisualUnitDetail() {
  if (!state.selectedVisualUnit) {
    return '<p class="empty">从导入回执或搜索结果里选择一个 visual unit，右侧会显示预览、定位信息和上下文。</p>';
  }

  const visualUnit = state.selectedVisualUnit.visual_unit;
  const preview = state.selectedVisualUnit.preview;
  const page = pageLabel(visualUnit.locator);
  const segment = videoLabel(visualUnit.locator);
  return `
    <div class="detail-card" data-testid="visual-unit-detail">
      <div class="detail-preview">
        ${renderVisualPreview()}
      </div>
      <div class="detail-head">
        <div class="job-meta">
          <span class="pill ready">${escapeHtml(visualUnit.kind)}</span>
          ${page ? `<span class="pill muted">${escapeHtml(page)}</span>` : ""}
          ${segment ? `<span class="pill muted">${escapeHtml(segment)}</span>` : ""}
        </div>
        <h4>${escapeHtml(sourceName(visualUnit.source_path))}</h4>
        <p class="helper">${escapeHtml(visualUnit.visual_unit_id)}</p>
      </div>
      <dl class="stats">
        <div><dt>Source type</dt><dd>${escapeHtml(visualUnit.source_type)}</dd></div>
        <div><dt>Source path</dt><dd class="detail-path">${escapeHtml(visualUnit.source_path)}</dd></div>
      </dl>
      <div class="detail-grid">
        <div class="detail-block">
          <h5>Locator</h5>
          <pre>${escapeHtml(JSON.stringify(visualUnit.locator, null, 2))}</pre>
        </div>
        <div class="detail-block">
          <h5>Preview</h5>
          <div class="inline-actions">
            <a data-testid="preview-link" href="${escapeHtml(preview.url)}" target="_blank" rel="noreferrer">打开预览</a>
            ${
              visualUnit.kind === "image" || visualUnit.kind === "document_page"
                ? `<button type="button" class="secondary-button" data-testid="detail-use-as-query-image-button" data-use-query-visual-unit-id="${escapeHtml(visualUnit.visual_unit_id)}">作为查询图片</button>`
                : ""
            }
            ${
              visualUnit.kind === "document_page"
                ? `<button type="button" class="secondary-button" data-testid="detail-use-as-query-document-button" data-use-query-document-visual-unit-id="${escapeHtml(visualUnit.visual_unit_id)}">作为查询文档</button>`
                : ""
            }
            ${
              visualUnit.kind === "video_segment"
                ? `<button type="button" class="secondary-button" data-testid="detail-use-as-query-video-button" data-use-query-video-visual-unit-id="${escapeHtml(visualUnit.visual_unit_id)}">作为查询视频</button>`
                : ""
            }
          </div>
        </div>
      </div>
      <div class="detail-block">
        <h5>Neighbor context</h5>
        <pre>${escapeHtml(JSON.stringify(state.selectedVisualUnit.neighbor_context, null, 2))}</pre>
      </div>
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
              <small>${job.progress.completed}/${job.progress.total} ${escapeHtml(job.progress.unit)}</small>
            </li>
          `
        )
        .join("")}
    </ul>
  `;
}

function renderSearchOutcome() {
  if (!state.searchOutcome) {
    return '<p class="empty">结果列表会显示在这里。导入成功后，这里会以统一列表混排 `video_segment`、`image` 和 `document_page`，并可直接打开右侧详情。</p>';
  }

  if (state.searchOutcome.error) {
    const details = state.searchOutcome.error.details?.content_types ?? [];
    return `
      <div class="notice error" data-testid="search-error-notice">
        <h4 data-testid="search-error-code">${escapeHtml(state.searchOutcome.error.code)}</h4>
        <p data-testid="search-error-message">${escapeHtml(state.searchOutcome.error.message)}</p>
        ${
          details.length
            ? `<ul class="data-list" data-testid="search-error-details">
                ${details
                  .map(
                    (item) => `
                      <li>
                        <strong>${escapeHtml(item.content_type ?? "unknown")}</strong>
                        <span>${escapeHtml(item.job?.job_id ?? "no-job")} · ${escapeHtml(item.job?.phase ?? item.status)}</span>
                      </li>
                    `
                  )
                  .join("")}
              </ul>`
            : ""
        }
      </div>
    `;
  }

  const results = state.searchOutcome.results ?? [];
  const unsupportedContentTypes = state.searchOutcome.unsupported_content_types ?? [];
  if (!results.length) {
    return `
      <div class="notice success">
        <h4>No results</h4>
        <p>当前真实检索链路没有返回匹配结果。可以换一个查询词，或确认目标库已经导入相关内容。</p>
        ${
          unsupportedContentTypes.length
            ? `<ul class="data-list" data-testid="search-unsupported-content-types">
                ${unsupportedContentTypes
                  .map(
                    (item) => `
                      <li>
                        <strong>${escapeHtml(item.content_type)}</strong>
                        <span>${escapeHtml(item.model)} · ${escapeHtml(item.reason)}</span>
                      </li>
                    `
                  )
                  .join("")}
              </ul>`
            : ""
        }
      </div>
    `;
  }

  return `
    ${
      unsupportedContentTypes.length
        ? `<div class="notice warning" data-testid="search-unsupported-content-types">
            <h4>部分内容类型已跳过</h4>
            <ul class="data-list">
              ${unsupportedContentTypes
                .map(
                  (item) => `
                    <li>
                      <strong>${escapeHtml(item.content_type)}</strong>
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
      <div>
        <p class="eyebrow">Results</p>
        <h3>命中 ${results.length} 条结果</h3>
      </div>
      <p class="helper" data-testid="search-results-summary">
        ${escapeHtml(searchHasMoreResults() ? "当前页后仍有更多结果，可继续追加加载。" : "点击结果卡片后，右侧会更新预览和详情。")}
      </p>
    </div>
    <ul class="result-list" data-testid="result-list">
      ${results
        .map(
          (item) => {
            const scoreLabel = formatScore(item.score);
            const page = pageLabel(item.locator);
            const segment = videoLabel(item.locator);
            return `
            <li
              class="result-card ${item.visual_unit_id === selectedVisualUnitId() ? "active" : ""}"
              data-testid="result-card"
              data-kind="${escapeHtml(item.kind)}"
              data-visual-unit-id="${escapeHtml(item.visual_unit_id)}"
            >
              <button
                type="button"
                class="result-select"
                data-visual-unit-id="${escapeHtml(item.visual_unit_id)}"
              >
                <div class="result-topline">
                  <span class="pill ${item.kind === "image" ? "ready" : "pending"}">${escapeHtml(item.kind)}</span>
                  ${page ? `<span class="pill muted">${escapeHtml(page)}</span>` : ""}
                  ${segment ? `<span class="pill muted">${escapeHtml(segment)}</span>` : ""}
                  ${scoreLabel ? `<span class="pill score-pill" data-testid="result-score">score ${escapeHtml(scoreLabel)}</span>` : ""}
                </div>
                <strong>${escapeHtml(sourceName(item.source_path))}</strong>
                <span class="helper">${escapeHtml(item.source_path)}</span>
                <span class="helper">${escapeHtml(item.source_type)} · ${escapeHtml(JSON.stringify(item.locator))}</span>
              </button>
              <div class="inline-actions">
                <button type="button" class="secondary-button" data-visual-unit-id="${escapeHtml(item.visual_unit_id)}">查看详情</button>
                ${
                  item.kind === "image" || item.kind === "document_page"
                    ? `<button type="button" class="secondary-button" data-testid="use-as-query-image-button" data-use-query-visual-unit-id="${escapeHtml(item.visual_unit_id)}">作为查询图片</button>`
                    : ""
                }
                ${
                  item.kind === "document_page"
                    ? `<button type="button" class="secondary-button" data-testid="use-as-query-document-button" data-use-query-document-visual-unit-id="${escapeHtml(item.visual_unit_id)}">作为查询文档</button>`
                    : ""
                }
                ${
                  item.kind === "video_segment"
                    ? `<button type="button" class="secondary-button" data-testid="use-as-query-video-button" data-use-query-video-visual-unit-id="${escapeHtml(item.visual_unit_id)}">作为查询视频</button>`
                    : ""
                }
                <a href="${escapeHtml(item.preview.url)}" target="_blank" rel="noreferrer">Preview</a>
              </div>
            </li>
          `;
          }
        )
        .join("")}
    </ul>
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
              Load more
            </button>
          </div>
        `
        : ""
    }
  `;
}

function renderSearchControls(library) {
  const queryPreview = queryImagePreviewUrl();
  const queryVideoPreview = queryVideoPreviewUrl();
  const queryDocumentPreview = queryDocumentPreviewUrl();
  const queryVideoDuration = state.queryVideoDurationMs;
  const queryVideoStartMs = currentQueryVideoStartMs();
  const queryVideoEndMs = currentQueryVideoEndMs();
  return `
    <div class="search-mode-switch" data-testid="search-mode-switch">
      <button
        type="button"
        class="${state.searchMode === "text" ? "" : "secondary-button"}"
        data-testid="search-mode-text"
        data-search-mode="text"
      >
        Text
      </button>
      <button
        type="button"
        class="${state.searchMode === "image" ? "" : "secondary-button"}"
        data-testid="search-mode-image"
        data-search-mode="image"
      >
        Image
      </button>
      <button
        type="button"
        class="${state.searchMode === "video" ? "" : "secondary-button"}"
        data-testid="search-mode-video"
        data-search-mode="video"
      >
        Video
      </button>
      <button
        type="button"
        class="${state.searchMode === "document" ? "" : "secondary-button"}"
        data-testid="search-mode-document"
        data-search-mode="document"
      >
        Document
      </button>
    </div>
    <form id="search-form" class="stack-form search-form" data-testid="search-form">
      <div class="search-filter-dock" data-testid="search-filter-dock">
        <div class="job-meta">
          <strong>搜索过滤器</strong>
          <span class="helper" data-testid="search-filter-summary">${escapeHtml(searchFiltersSummary())}</span>
        </div>
        <div class="filter-grid search-filter-grid">
          <label>
            <span>视觉对象类型</span>
            <select id="search-filter-kind" data-testid="search-filter-kind" ${library ? "" : "disabled"}>
              <option value="">全部</option>
              <option value="image" ${state.searchFilters.visualUnitKind === "image" ? "selected" : ""}>image</option>
              <option value="document_page" ${state.searchFilters.visualUnitKind === "document_page" ? "selected" : ""}>document_page</option>
              <option value="video_segment" ${state.searchFilters.visualUnitKind === "video_segment" ? "selected" : ""}>video_segment</option>
            </select>
          </label>
          <label>
            <span>来源类型</span>
            <select id="search-filter-source-type" data-testid="search-filter-source-type" ${library ? "" : "disabled"}>
              <option value="">全部</option>
              <option value="image" ${state.searchFilters.sourceType === "image" ? "selected" : ""}>image</option>
              <option value="pdf" ${state.searchFilters.sourceType === "pdf" ? "selected" : ""}>pdf</option>
              <option value="video" ${state.searchFilters.sourceType === "video" ? "selected" : ""}>video</option>
            </select>
          </label>
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
            <span>时间范围开始 ms</span>
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
            <span>时间范围结束 ms</span>
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
        </div>
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
      ${
        state.searchMode === "text"
          ? `
            <label>
              <span>查询文本</span>
              <input
                id="search-text"
                data-testid="search-text-input"
                type="text"
                value="${escapeHtml(state.searchTextDraft)}"
                placeholder="尝试输入财报页面中的问题或关键词"
                ${library ? "" : "disabled"}
              />
            </label>
          `
          : state.searchMode === "image"
            ? `
            <div class="query-image-panel" data-testid="query-image-panel">
              <label>
                <span>查询图片</span>
                <input
                  id="query-image-input"
                  data-testid="query-image-input"
                  type="file"
                  accept="image/*"
                  ${library ? "" : "disabled"}
                />
              </label>
              <div class="query-image-card" data-testid="query-image-card">
                <div class="job-meta">
                  <span class="pill ${state.queryImageAsset || state.queryImageLibraryObject ? "ready" : "muted"}">${escapeHtml(queryImageStatusLabel())}</span>
                  ${
                    queryImageDisplayName()
                      ? `<span class="helper">${escapeHtml(queryImageDisplayName())}</span>`
                      : ""
                  }
                </div>
                ${
                  queryPreview
                    ? isDocumentPageQueryImage()
                      ? `<iframe class="query-image-preview-frame" data-testid="query-image-preview" src="${escapeHtml(queryPreview)}" title="Query image preview" loading="lazy"></iframe>`
                      : `<img class="query-image-preview" data-testid="query-image-preview" src="${escapeHtml(queryPreview)}" alt="Query image preview" />`
                    : `<p class="empty" data-testid="query-image-empty">选择一张本地图片后，这里会显示查询图片预览。</p>`
                }
                <div class="inline-actions">
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
              <label>
                <span>查询视频</span>
                <input
                  id="query-video-input"
                  data-testid="query-video-input"
                  type="file"
                  accept="video/mp4,video/quicktime,video/x-m4v,video/*"
                  ${library ? "" : "disabled"}
                />
              </label>
              <label>
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
              <div class="query-video-card" data-testid="query-video-card">
                <div class="job-meta">
                  <span class="pill ${state.queryVideoAsset || state.queryVideoSource || state.queryVideoLibraryObject ? "ready" : "muted"}">${escapeHtml(queryVideoStatusLabel())}</span>
                  ${
                    queryVideoDisplayName()
                      ? `<span class="helper">${escapeHtml(queryVideoDisplayName())}</span>`
                      : ""
                  }
                </div>
                ${
                  queryVideoPreview
                    ? `<video
                        class="query-video-preview"
                        data-testid="query-video-preview"
                        src="${escapeHtml(queryVideoPreview)}"
                        controls
                        preload="metadata"
                      ></video>`
                    : `<p class="empty" data-testid="query-video-empty">选择一个本地视频或库内视频源后，这里会显示查询视频预览。</p>`
                }
                <div class="query-range-card" data-testid="query-video-range-card">
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
                <div class="inline-actions">
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
              <label>
                <span>查询文档</span>
                <input
                  id="query-document-input"
                  data-testid="query-document-input"
                  type="file"
                  accept="application/pdf,.pdf"
                  ${library ? "" : "disabled"}
                />
              </label>
              <div class="query-document-card" data-testid="query-document-card">
                <div class="job-meta">
                  <span class="pill ${state.queryDocumentAsset || state.queryDocumentLibraryObject ? "ready" : "muted"}">${escapeHtml(queryDocumentStatusLabel())}</span>
                  ${
                    queryDocumentDisplayName()
                      ? `<span class="helper">${escapeHtml(queryDocumentDisplayName())}</span>`
                      : ""
                  }
                </div>
                ${
                  queryDocumentPreview
                    ? `<iframe class="query-document-preview-frame" data-testid="query-document-preview" src="${escapeHtml(queryDocumentPreview)}" title="Query document preview" loading="lazy"></iframe>`
                    : `<p class="empty" data-testid="query-document-empty">选择一个本地 PDF 或从结果复用 document_page 后，这里会显示查询文档预览。</p>`
                }
                <div class="query-range-card" data-testid="query-document-range-card">
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
                <div class="inline-actions">
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
      <button type="submit" data-testid="search-submit-button" ${library ? "" : "disabled"}>
        ${
          state.searchMode === "text"
            ? "搜索"
            : state.searchMode === "image"
              ? "以图片搜索"
              : state.searchMode === "video"
                ? "以视频搜索"
                : "以文档搜索"
        }
      </button>
    </form>
  `;
}

function patchWorkspaceMarkupPreservingDetail(nextMarkup) {
  if (!(root instanceof HTMLElement)) {
    return false;
  }

  const currentShell = root.querySelector("main.shell");
  const currentHero = currentShell?.querySelector(".hero");
  const currentDesk = currentShell?.querySelector(".workspace-desk");
  const currentLeft = currentDesk?.querySelector(".workspace-left");
  const currentCenter = currentDesk?.querySelector(".workspace-center");
  if (
    !(currentShell instanceof HTMLElement) ||
    !(currentHero instanceof HTMLElement) ||
    !(currentLeft instanceof HTMLElement) ||
    !(currentCenter instanceof HTMLElement)
  ) {
    return false;
  }

  const template = document.createElement("template");
  template.innerHTML = nextMarkup.trim();
  const nextShell = template.content.firstElementChild;
  const nextHero = nextShell?.querySelector(".hero");
  const nextStatusStack = nextShell?.querySelector(".status-stack");
  const nextDesk = nextShell?.querySelector(".workspace-desk");
  const nextLeft = nextDesk?.querySelector(".workspace-left");
  const nextCenter = nextDesk?.querySelector(".workspace-center");
  if (
    !(nextShell instanceof HTMLElement) ||
    !(nextHero instanceof HTMLElement) ||
    !(nextLeft instanceof HTMLElement) ||
    !(nextCenter instanceof HTMLElement)
  ) {
    return false;
  }

  currentHero.replaceWith(nextHero);

  const currentStatusStack = currentShell.querySelector(".status-stack");
  const insertedHero = currentShell.querySelector(".hero");
  if (nextStatusStack instanceof HTMLElement) {
    if (currentStatusStack instanceof HTMLElement) {
      currentStatusStack.replaceWith(nextStatusStack);
    } else if (insertedHero instanceof HTMLElement) {
      insertedHero.after(nextStatusStack);
    } else {
      return false;
    }
  } else if (currentStatusStack instanceof HTMLElement) {
    currentStatusStack.remove();
  }

  currentLeft.replaceWith(nextLeft);
  currentCenter.replaceWith(nextCenter);
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
  const workspaceMeta = currentWorkspaceMeta();
  const isSearchWorkspace = state.activeWorkspace === "search";
  const focusedEditableState = captureFocusedEditableState();
  const detailSignature = selectedVisualUnitDetailSignature();
  const previousDetailPanel = root?.querySelector('[data-testid="detail-panel"]') ?? null;
  const shouldPreserveDetailPanel =
    isSearchWorkspace &&
    previousDetailPanel instanceof HTMLElement &&
    detailSignature !== null &&
    detailSignature === lastRenderedDetailSignature;

  const nextMarkup = `
    <main class="shell" data-testid="workspace-shell">
      <section class="hero">
        <p class="eyebrow">FauniSearch</p>
        <h1>${escapeHtml(workspaceMeta.title)}</h1>
        <p class="summary">
          ${escapeHtml(workspaceMeta.summary)}
        </p>
        ${renderWorkspaceSwitcher()}
        <div class="service-strip">
          <a href="${endpoints.uiRoot}" target="_blank" rel="noreferrer">UI</a>
          <a href="${endpoints.appHealth}" target="_blank" rel="noreferrer">App health</a>
          <a href="${endpoints.sidecarHealth}" target="_blank" rel="noreferrer">Sidecar health</a>
          <a href="${endpoints.qdrantCollections}" target="_blank" rel="noreferrer">Qdrant</a>
        </div>
      </section>

      ${renderStatusNotices()}

      <section class="workspace-desk ${isSearchWorkspace ? "workspace-desk-search" : "workspace-desk-inventory"}">
        <aside class="workspace-column workspace-left">
          <section class="panel panel-tight">
            <div class="panel-head">
              <div>
                <p class="eyebrow">Library</p>
                <h2>库上下文</h2>
              </div>
            </div>
            <form id="create-library-form" class="stack-form" data-testid="create-library-form">
              <label>
                <span>显示名称</span>
                <input
                  id="library-name"
                  data-testid="library-name-input"
                  name="libraryDisplayName"
                  type="text"
                  value="${escapeHtml(state.libraryDisplayNameDraft)}"
                  placeholder="例如：Invoice Demo"
                  required
                />
              </label>
              <label>
                <span>自定义 library_id（可选）</span>
                <input
                  id="library-id"
                  data-testid="library-id-input"
                  name="libraryId"
                  type="text"
                  value="${escapeHtml(state.libraryIdDraft)}"
                  placeholder="例如：invoice-demo"
                />
              </label>
              <button type="submit" data-testid="create-library-button">创建库</button>
            </form>
            <label class="stack-form">
              <span>当前库</span>
              <select id="library-select" data-testid="library-select" ${state.libraries.length ? "" : "disabled"}>
                ${
                  state.libraries.length
                    ? state.libraries
                        .map(
                          (item) => `
                            <option value="${escapeHtml(item.id)}" ${item.id === state.selectedLibraryId ? "selected" : ""}>
                              ${escapeHtml(libraryDisplayName(item))} (${escapeHtml(item.id)})
                            </option>
                          `
                        )
                        .join("")
                    : `<option value="">还没有库</option>`
                }
              </select>
            </label>
            <div class="context-card" data-testid="current-library-card">
              ${
                library
                  ? `
                    <p class="eyebrow">Current</p>
                    <h3 data-testid="current-library-name">${escapeHtml(libraryDisplayName(library))}</h3>
                    <p class="helper" data-testid="current-library-id">${escapeHtml(library.id)}</p>
                    <dl class="stats">
                      <div><dt>Accepted items</dt><dd>${library.counts.accepted_items}</dd></div>
                      <div><dt>Pending jobs</dt><dd>${library.counts.pending_jobs}</dd></div>
                      <div><dt>Latest job</dt><dd>${escapeHtml(library.latest_job_id ?? "none")}</dd></div>
                    </dl>
                    ${renderInventoryBridge(library)}
                    ${renderProviderBridge(library)}
                  `
                  : `<p class="empty">先创建一个库，再进入导入和搜索步骤。</p>`
              }
            </div>
          </section>

          ${renderSourceRootsPanel(library)}

          <section class="panel panel-tight">
            <div class="panel-head">
              <div>
                <p class="eyebrow">Import</p>
                <h2>路径导入</h2>
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
            <div class="quick-card" data-testid="demo-card">
              <p class="eyebrow">Quick demo</p>
              <h3>真实索引和检索</h3>
              <p class="helper">使用仓库内置的 TATDQA 图片 fixture，可直接触发真实 document/image embedding、Qdrant 写入和文本搜索。浏览器文件选择不会暴露服务器可读的绝对路径，所以当前仍以路径输入为主。</p>
              <code>${escapeHtml(demoFixture.path)}</code>
              <div class="inline-actions">
                <button id="fill-demo-button" data-testid="fill-demo-button" type="button" class="secondary-button" ${library ? "" : "disabled"}>填入 demo 路径和查询</button>
                <button id="run-demo-button" data-testid="run-demo-button" type="button" ${library ? "" : "disabled"}>导入并搜索 demo</button>
              </div>
            </div>
            ${renderImportReceipt()}
          </section>

          <section class="panel panel-tight">
            <div class="panel-head">
              <div>
                <p class="eyebrow">Tasks</p>
                <h2>任务面板</h2>
              </div>
            </div>
            ${renderJobs()}
          </section>
        </aside>

        <section class="workspace-column workspace-center">
          ${
            isSearchWorkspace
              ? `
                <section class="panel search-panel" data-testid="search-panel">
                  <div class="panel-head">
                    <div>
                      <p class="eyebrow">Search</p>
                      <h2>统一搜索入口</h2>
                    </div>
                  </div>
                  ${renderSearchControls(library)}
                  ${renderSearchOutcome()}
                </section>
              `
              : state.activeWorkspace === "inventory"
                ? renderLibrarySourcesPanel(library)
                : renderSettingsPanel(library)
          }
        </section>

        <aside class="workspace-column workspace-right ${isSearchWorkspace ? "" : "workspace-right-hidden"}">
          ${
            isSearchWorkspace
              ? `
                <section class="panel detail-panel" data-testid="detail-panel">
                  <div class="panel-head">
                    <div>
                      <p class="eyebrow">Detail</p>
                      <h2>详情侧栏</h2>
                    </div>
                  </div>
                  ${renderVisualUnitDetail()}
                </section>
              `
              : ""
          }
        </aside>
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
  document.querySelector("#create-library-form")?.addEventListener("submit", onCreateLibrary);
  document.querySelector("#library-name")?.addEventListener("input", onLibraryNameInput);
  document.querySelector("#library-id")?.addEventListener("input", onLibraryIdInput);
  document.querySelector("#library-select")?.addEventListener("change", onSelectLibrary);
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
  document.querySelector("#import-form")?.addEventListener("submit", onImportPaths);
  document.querySelector("#import-paths")?.addEventListener("input", onImportPathsInput);
  document.querySelector("#search-form")?.addEventListener("submit", onSearchSubmit);
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
  document.querySelector("#fill-demo-button")?.addEventListener("click", onFillDemo);
  document.querySelector("#run-demo-button")?.addEventListener("click", onRunDemo);
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

  const queryVideoPreview = document.querySelector("#query-video-preview");
  if (queryVideoPreview instanceof HTMLVideoElement) {
    queryVideoPreview.addEventListener("loadedmetadata", onQueryVideoPreviewLoadedMetadata);
    if (queryVideoPreview.readyState >= 1) {
      syncQueryVideoDurationFromVideoElement(queryVideoPreview);
    }
  }

  const detailVideoPreview = document.querySelector('[data-testid="visual-preview"][data-preview-kind="video"]');
  if (detailVideoPreview instanceof HTMLVideoElement) {
    attachBoundedVideoPlayback(detailVideoPreview);
  }

  lastRenderedDetailSignature = detailSignature;
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
  if (state.activeWorkspace === "inventory") {
    await refreshLibrarySources();
  } else if (state.activeWorkspace === "settings") {
    await refreshRuntimeHealth();
    await refreshModelCatalog();
    await refreshGlobalContentTypes();
  } else {
    state.runtimeHealth = null;
  }
  await refreshJobs();
  await refreshVideoSources();
  renderWorkspace();
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
    state.searchOutcome = null;
    state.lastSearchRequest = null;
    state.statusMessage = null;
    state.libraryDisplayNameDraft = "";
    state.libraryIdDraft = "";
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

async function onSelectLibrary(event) {
  state.selectedLibraryId = event.target.value;
  resetSourceRootEditor();
  resetInventoryFilters();
  resetSearchFilters();
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
  state.searchOutcome = null;
  state.lastSearchRequest = null;
  state.globalError = null;
  state.statusMessage = null;
  await refreshWorkspace({ keepSelection: true });
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
    state.libraryContentTypes = emptyContentTypes();
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

function setDemoDrafts() {
  state.importPathsDraft = demoFixture.path;
  state.searchTextDraft = demoFixture.query;
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
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

async function triggerSourceAction(path, statusMessage) {
  state.globalError = null;
  state.statusMessage = statusMessage;
  renderWorkspace();

  const receipt = await apiRequest(path, { method: "POST" });
  await refreshWorkspace({ keepSelection: true });

  const job = receipt.job;
  if (job && !isTerminalJobStatus(job.status)) {
    await waitForJobTerminal(job.job_id);
  }

  state.statusMessage = null;
  await refreshWorkspace({ keepSelection: true });
}

async function onRefreshLibrarySources() {
  if (!state.selectedLibraryId) {
    return;
  }

  try {
    await triggerSourceAction(
      `/libraries/${state.selectedLibraryId}/refresh`,
      "正在执行库级 refresh..."
    );
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

async function onRescanLibrarySources() {
  if (!state.selectedLibraryId) {
    return;
  }

  try {
    await triggerSourceAction(
      `/libraries/${state.selectedLibraryId}/rescan`,
      "正在执行库级 rescan..."
    );
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
    await triggerSourceAction(
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
    await triggerSourceAction(
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
  if (firstVisualUnit) {
    await loadVisualUnit(firstVisualUnit.visual_unit_id);
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
    state.statusMessage = null;
    renderWorkspace();
  } catch (error) {
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
    library_id: state.selectedLibraryId,
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
  state.searchOutcome = {
    ...data,
    results: mergedResults,
  };
  state.lastSearchRequest = request;
  renderWorkspace();
  if (!options.append && data.results?.[0]?.visual_unit_id) {
    await loadVisualUnit(data.results[0].visual_unit_id);
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

  state.statusMessage = "正在执行真实文本搜索...";
  renderWorkspace();
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
    state.statusMessage = "正在上传查询图片...";
    renderWorkspace();
    await uploadQueryImage(state.queryImageFile);
  }

  state.statusMessage = "正在执行真实图片搜索...";
  renderWorkspace();
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
    state.statusMessage = "正在上传查询视频...";
    renderWorkspace();
    await uploadQueryVideo(state.queryVideoFile);
  }

  state.statusMessage = "正在执行真实视频搜索...";
  renderWorkspace();
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
    state.statusMessage = "正在上传查询文档...";
    renderWorkspace();
    await uploadQueryDocument(state.queryDocumentFile);
  }

  state.statusMessage = "正在执行真实文档搜索...";
  renderWorkspace();
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

function onFillDemo() {
  setDemoDrafts();
  state.activeWorkspace = "search";
  state.searchMode = "text";
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

async function onRunDemo() {
  if (!state.selectedLibraryId) {
    return;
  }

  try {
    setDemoDrafts();
    state.activeWorkspace = "search";
    state.globalError = null;
    state.statusMessage = "正在导入 demo fixture，并写入 Qdrant...";
    renderWorkspace();
    await importPaths([demoFixture.path]);
    state.statusMessage = "索引完成，正在执行 demo 查询...";
    renderWorkspace();
    await searchText(demoFixture.query);
    state.statusMessage = null;
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    state.statusMessage = null;
    renderWorkspace();
  }
}

async function onSelectWorkspace(event) {
  const nextWorkspace = event.currentTarget.dataset.workspace as WorkspaceKind | undefined;
  if (!nextWorkspace || nextWorkspace === state.activeWorkspace) {
    return;
  }

  state.activeWorkspace = nextWorkspace;
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

function onSelectSearchMode(event) {
  state.searchMode = event.currentTarget.dataset.searchMode;
  state.globalError = null;
  state.statusMessage = null;
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

function resolveLibraryObjectQueryImage(visualUnitId): LibraryObjectQueryImage | null {
  const resultItem =
    state.searchOutcome?.results?.find((item) => item.visual_unit_id === visualUnitId) ?? null;
  if (resultItem?.kind === "image") {
    return {
      visual_unit_id: resultItem.visual_unit_id,
      kind: resultItem.kind,
      source_path: resultItem.source_path,
      preview: resultItem.preview,
    };
  }
  if (resultItem?.kind === "document_page") {
    return {
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
      visual_unit_id: detailVisualUnit.visual_unit_id,
      kind: detailVisualUnit.kind,
      source_path: detailVisualUnit.source_path,
      preview: state.selectedVisualUnit.preview,
    };
  }

  return null;
}

function resolveLibraryObjectQueryVideo(visualUnitId): LibraryObjectQueryVideo | null {
  const resultItem =
    state.searchOutcome?.results?.find((item) => item.visual_unit_id === visualUnitId) ?? null;
  if (resultItem?.kind === "video_segment") {
    return {
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
      visual_unit_id: detailVisualUnit.visual_unit_id,
      kind: detailVisualUnit.kind,
      source_path: detailVisualUnit.source_path,
      locator: detailVisualUnit.locator,
      preview: state.selectedVisualUnit.preview,
    };
  }

  return null;
}

function resolveLibraryObjectQueryDocument(
  visualUnitId
): LibraryObjectQueryDocument | null {
  const resultItem =
    state.searchOutcome?.results?.find((item) => item.visual_unit_id === visualUnitId) ?? null;
  if (resultItem?.kind === "document_page") {
    const page = Number(resultItem.locator?.page ?? 0);
    return {
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

  return null;
}

function onUseAsQueryImage(event) {
  const visualUnitId = event.currentTarget.dataset.useQueryVisualUnitId;
  const libraryObject = resolveLibraryObjectQueryImage(visualUnitId);
  if (!libraryObject) {
    state.globalError = {
      code: "not_supported",
      message: "当前只能把库内 image 或 document_page 对象作为查询图片。",
    };
    renderWorkspace();
    return;
  }

  clearQueryImageState();
  state.queryImageLibraryObject = libraryObject;
  state.searchMode = "image";
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onUseAsQueryVideo(event) {
  const visualUnitId = event.currentTarget.dataset.useQueryVideoVisualUnitId;
  const libraryObject = resolveLibraryObjectQueryVideo(visualUnitId);
  if (!libraryObject) {
    state.globalError = {
      code: "not_supported",
      message: "当前只能把库内 video_segment 对象作为查询视频片段。",
    };
    renderWorkspace();
    return;
  }

  setLibraryQueryVideoVisualUnit(libraryObject);
  state.searchMode = "video";
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onUseAsQueryDocument(event) {
  const visualUnitId = event.currentTarget.dataset.useQueryDocumentVisualUnitId;
  const libraryObject = resolveLibraryObjectQueryDocument(visualUnitId);
  if (!libraryObject) {
    state.globalError = {
      code: "not_supported",
      message: "当前只能把库内 document_page 对象作为查询文档。",
    };
    renderWorkspace();
    return;
  }

  setLibraryQueryDocumentVisualUnit(libraryObject);
  state.searchMode = "document";
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

async function loadVisualUnit(visualUnitId: string): Promise<void> {
  if (!state.selectedLibraryId) {
    return;
  }

  try {
    state.globalError = null;
    state.selectedVisualUnit = await apiRequest<VisualUnitDetailData>(
      `/libraries/${state.selectedLibraryId}/visual-units/${encodeURIComponent(visualUnitId)}`
    );
    renderWorkspace();
  } catch (error) {
    state.globalError = toApiError(error);
    renderWorkspace();
  }
}

async function onSelectVisualUnit(event) {
  const visualUnitId = event.currentTarget.dataset.visualUnitId;
  await loadVisualUnit(visualUnitId);
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
