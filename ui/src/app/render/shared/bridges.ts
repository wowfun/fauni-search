import type { LibrarySnapshot } from "../../../types";
import {
  contentTypeDisplayName,
  escapeHtml,
  formatResolvedContentModel,
} from "../../selectors/common";
import { libraryDisplayName } from "../../selectors/common";
import { state } from "../../state/store";
import { renderStatusTag, renderUiButton } from "./primitives";

export function renderInventoryBridge(library: LibrarySnapshot | null) {
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
            ? renderStatusTag("库管理已打开", "ready", { testId: "inventory-bridge-state" })
            : renderUiButton("前往库管理", {
                tone: "secondary",
                testId: "inventory-bridge-button",
                attrs: { "data-workspace": "inventory" },
              })
        }
      </div>
    </div>
  `;
}

export function renderProviderBridge(library: LibrarySnapshot | null) {
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
            ? renderStatusTag("设置已打开", "ready", { testId: "provider-bridge-state" })
            : renderUiButton("前往设置", {
                tone: "secondary",
                testId: "provider-bridge-button",
                attrs: { "data-workspace": "settings" },
              })
        }
      </div>
    </div>
  `;
}
