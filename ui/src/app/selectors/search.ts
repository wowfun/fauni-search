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
import { allLibrariesTextScopeActive } from "./library";
import { libraryDisplayName, sourceTypeDisplayName, visualUnitKindDisplayName } from "./common";
import { libraryOperationalReadiness } from "./runtime";

export function canSearchLibrary(library: LibrarySnapshot | null) {
  return Boolean(library && libraryOperationalReadiness(library).searchableUnits > 0);
}

export function librarySearchStageNextAction(library: LibrarySnapshot | null) {
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

export function currentSearchScopeStageState(library: LibrarySnapshot | null) {
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

export function canSearchCurrentScope(library: LibrarySnapshot | null) {
  return currentSearchScopeStageState(library).searchEnabled;
}

export function libraryNeedsPreparation(library: LibrarySnapshot | null) {
  return currentSearchScopeStageState(library).needsPreparation;
}

export function searchStageNextAction(library: LibrarySnapshot | null) {
  return currentSearchScopeStageState(library).nextAction;
}

export function currentSearchStageState(library: LibrarySnapshot | null) {
  const readiness = currentSearchScopeStageState(library);
  return {
    status: readiness.status,
    pillClass: readiness.pillClass,
    summary: readiness.summary,
  };
}

export function searchHasMoreResults() {
  return Boolean(state.searchOutcome?.next_cursor && state.lastSearchRequest);
}

export function searchFiltersSummary() {
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

export function parseNonNegativeIntegerDraft(value: string, field: string) {
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

export function searchFiltersPayload() {
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

export function shouldRenderSearchNextStepDock(library: LibrarySnapshot | null) {
  if (!library) {
    return true;
  }

  return libraryNeedsPreparation(library) || currentSearchScopeStageState(library).nextAction === "jobs";
}
