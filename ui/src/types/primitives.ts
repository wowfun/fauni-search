export type WorkspaceKind = "search" | "inventory" | "settings";
export type SearchMode = "text" | "image" | "video" | "document";
export type SearchScopeKind = "library" | "all_libraries";
export type SettingsSection =
  | "content-types"
  | "library-overrides"
  | "providers"
  | "diagnostics";
export type VisualUnitKind = "image" | "document_page" | "video_segment" | string;
export type Locator = Record<string, string | number | boolean | null | undefined>;
export type ModelTestModality = "text" | "image";

export interface PreviewReference {
  url: string;
}

export interface InventoryFilters {
  sourceRootId: string;
  sourceType: string;
  sourceStatus: string;
}

export interface SearchFilters {
  visualUnitKind: string;
  sourceType: string;
  pathPrefix: string;
  timeRangeStartMsDraft: string;
  timeRangeEndMsDraft: string;
}

export interface InventorySummary {
  total: number;
  active: number;
  invalidated: number;
  out_of_scope: number;
}

export interface VideoRangeState {
  start_ms: number;
  end_ms: number;
}
