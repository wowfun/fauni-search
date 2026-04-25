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
import { formatDurationMs, sourceName } from "./common";
import { selectedVisualUnitOriginLibraryId } from "./library";

export function selectedVisualUnitId() {
  const visualUnitId = state.selectedVisualUnit?.visual_unit?.visual_unit_id ?? null;
  const libraryId = selectedVisualUnitOriginLibraryId();
  if (!visualUnitId || !libraryId) {
    return visualUnitId;
  }
  return `${libraryId}:${visualUnitId}`;
}

export function queryImagePreviewUrl() {
  return (
    state.queryImageObjectUrl ??
    state.queryImageAsset?.preview?.url ??
    state.queryImageLibraryObject?.preview?.url ??
    null
  );
}

export function queryImageStatusLabel() {
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

export function queryImageDisplayName() {
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

export function activeQueryImagePreview(): PreviewReference | null {
  return state.queryImageAsset?.preview ?? state.queryImageLibraryObject?.preview ?? null;
}

export function isDocumentPageQueryImage() {
  return state.queryImageLibraryObject?.kind === "document_page";
}

export function queryVideoPreviewUrl() {
  return (
    state.queryVideoObjectUrl ??
    state.queryVideoAsset?.preview?.url ??
    state.queryVideoSource?.preview?.url ??
    state.queryVideoLibraryObject?.preview?.url ??
    null
  );
}

export function queryVideoStatusLabel() {
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

export function queryVideoDisplayName() {
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

export function activeQueryVideoPreview(): PreviewReference | null {
  return (
    state.queryVideoAsset?.preview ??
    state.queryVideoSource?.preview ??
    state.queryVideoLibraryObject?.preview ??
    null
  );
}

export function currentQueryVideoStartMs() {
  if (typeof state.queryVideoLibraryObject?.locator?.start_ms === "number") {
    return state.queryVideoLibraryObject.locator.start_ms;
  }
  return state.queryVideoRange?.start_ms ?? 0;
}

export function currentQueryVideoEndMs() {
  if (typeof state.queryVideoLibraryObject?.locator?.end_ms === "number") {
    return state.queryVideoLibraryObject.locator.end_ms;
  }
  return state.queryVideoRange?.end_ms ?? state.queryVideoDurationMs ?? 0;
}

export function queryVideoRangeSummary() {
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

export function queryVideoLocatorPayload() {
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

export function queryVideoRangeStep() {
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

export function queryDocumentPreviewUrl() {
  return (
    state.queryDocumentObjectUrl ??
    state.queryDocumentAsset?.preview?.url ??
    state.queryDocumentLibraryObject?.preview?.url ??
    null
  );
}

export function queryDocumentStatusLabel() {
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

export function queryDocumentDisplayName() {
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

export function activeQueryDocumentPreview(): PreviewReference | null {
  return state.queryDocumentAsset?.preview ?? state.queryDocumentLibraryObject?.preview ?? null;
}

export function currentQueryDocumentStartPage() {
  if (state.queryDocumentLibraryObject?.locator?.start_page != null) {
    return state.queryDocumentLibraryObject.locator.start_page;
  }
  return state.queryDocumentStartPageDraft;
}

export function currentQueryDocumentEndPage() {
  if (state.queryDocumentLibraryObject?.locator?.end_page != null) {
    return state.queryDocumentLibraryObject.locator.end_page;
  }
  return state.queryDocumentEndPageDraft;
}

export function queryDocumentRangeSummary() {
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

export function queryDocumentLocatorPayload() {
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
