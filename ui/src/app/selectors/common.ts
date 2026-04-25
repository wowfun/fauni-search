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
import { state } from "../state/store";

export function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

export function libraryDisplayName(
  library: Pick<LibrarySnapshot, "display_name" | "id"> | null | undefined
): string {
  if (!library) {
    return "";
  }
  return library.display_name?.trim() || library.id;
}

export function libraryIsArchived(
  library: Pick<LibrarySnapshot, "lifecycle_state"> | null | undefined
): boolean {
  return library?.lifecycle_state === "archived";
}

export function libraryLifecycleLabel(
  library: Pick<LibrarySnapshot, "lifecycle_state"> | null | undefined
): string {
  return libraryIsArchived(library) ? "已归档" : "活跃";
}

export function libraryLifecyclePillClass(
  library: Pick<LibrarySnapshot, "lifecycle_state"> | null | undefined
): string {
  return libraryIsArchived(library) ? "muted" : "ready";
}

export function contentTypeDisplayName(contentType: string) {
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

export function sourceTypeDisplayName(sourceType: string) {
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

export function visualUnitKindDisplayName(kind: string) {
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

export function sourceStatusDisplayName(status: string) {
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

export function modelTestModalityDisplayName(modality: ModelTestModality | string | "") {
  switch (modality) {
    case "text":
      return "文本";
    case "image":
      return "图片";
    default:
      return modality || "未选择";
  }
}

export function workspaceDisplayName(workspace: WorkspaceKind) {
  switch (workspace) {
    case "inventory":
      return "库管理";
    case "settings":
      return "设置";
    default:
      return "搜索";
  }
}

export function searchModeDisplayName(mode: SearchMode) {
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

export function providerSelectionPillClass(status: string) {
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

export function providerProbePillClass(status?: string | null) {
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

export function formatResolvedModel(selection: ResolvedModelSelectionPayload | undefined) {
  if (!selection) {
    return "未解析";
  }
  const parts = [selection.provider_id, `${selection.model_id}@${selection.model_version}`];
  if (selection.model_revision && selection.model_revision !== selection.model_version) {
    parts.push(`修订 ${selection.model_revision}`);
  }
  return parts.join(" · ");
}

export function formatResolvedContentModel(selection: ResolvedContentModelSelectionPayload | undefined) {
  return formatResolvedModel(selection);
}

export function formatBindingSource(bindingSource: BindingSource | undefined) {
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

export function formatResolvedModelContext(selection: ResolvedModelSelectionPayload | undefined) {
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

export function formatResolvedContentModelContext(
  selection: ResolvedContentModelSelectionPayload | undefined
) {
  return formatResolvedModelContext(selection);
}

export function formatEmbeddingCapabilityValues(values: string[] | undefined) {
  return values?.length ? values.join(", ") : "无";
}

export function formatEmbeddingCapabilities(
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

export function formatExecutionInputTypes(inputTypes: string[] | undefined, options: { includePrefix?: boolean } = {}) {
  const value = inputTypes?.length ? inputTypes.join(", ") : "无";
  return options.includePrefix ? `执行输入 · ${value}` : value;
}

export function sourceName(path) {
  return String(path).split(/[/\\]/).pop() ?? path;
}

export function pageLabel(locator) {
  return locator?.page_label ?? (locator?.page ? `P${locator.page}` : null);
}

export function videoLabel(locator) {
  if (typeof locator?.start_ms !== "number" || typeof locator?.end_ms !== "number") {
    return null;
  }
  return `${formatDurationMs(locator.start_ms)} → ${formatDurationMs(locator.end_ms)}`;
}

export function formatScore(score) {
  if (typeof score !== "number" || Number.isNaN(score)) {
    return null;
  }
  return score.toFixed(4);
}

export function formatDurationMs(durationMs) {
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
