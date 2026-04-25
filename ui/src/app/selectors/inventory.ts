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

export function selectedInventorySource(): SourceInventoryItem | null {
  if (!state.librarySources.length) {
    return null;
  }
  return (
    state.librarySources.find((source) => source.source_id === state.selectedInventorySourceId) ??
    state.librarySources[0] ??
    null
  );
}

export function selectedInventoryRepresentativeVisualUnit(source: SourceInventoryItem | null) {
  return source?.representative_visual_unit ?? null;
}

export function selectedInventoryRepresentativePreview(source: SourceInventoryItem | null) {
  return source?.representative_preview ?? null;
}

export function ensureSelectedInventorySource() {
  if (!state.librarySources.length) {
    state.selectedInventorySourceId = "";
    state.inventoryDetailSheetOpen = false;
    return;
  }
  if (!state.librarySources.some((source) => source.source_id === state.selectedInventorySourceId)) {
    state.selectedInventorySourceId = state.librarySources[0]?.source_id ?? "";
  }
}

export function emptyInventorySummary(): InventorySummary {
  return {
    total: 0,
    active: 0,
    invalidated: 0,
    out_of_scope: 0,
  };
}

export function summarizeInventorySources(sources: SourceInventoryItem[]): InventorySummary {
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

export function sourceRootDisplayName(sourceRootId) {
  if (!sourceRootId) {
    return "全部来源根";
  }
  const sourceRoot = state.sourceRoots.find((item) => item.source_root_id === sourceRootId);
  return sourceRoot?.root_path ?? sourceRootId;
}

export function sourceRootInventoryLabel(source: SourceInventoryItem) {
  return source.source_root_label || source.source_root_id || "手动导入";
}

export function sourceRootStatusPillClass(status) {
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

export function sourceStatusPillClass(status) {
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

export function sourceRootStatusDisplayName(status) {
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

export function sourceRootWatchStateDisplayName(watchState) {
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

export function sourceRootWatchStatePillClass(watchState) {
  if (watchState === "watching") {
    return "ready";
  }
  if (watchState === "disabled") {
    return "muted";
  }
  return "pending";
}

export function inventorySourceRootPriority(sourceRoot: SourceRootSnapshot) {
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
