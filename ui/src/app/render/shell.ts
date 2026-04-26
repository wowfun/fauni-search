import {
  currentStatusCapsule,
  escapeHtml,
  renderUiIcon,
  state,
  type LibrarySnapshot,
} from "../core";
import { renderStatusButton } from "./shared/primitives";

function renderStatusCapsuleProgress(status: ReturnType<typeof currentStatusCapsule>) {
  if (!("progress" in status) || !status.progress) {
    return "";
  }

  const progress = status.progress;
  const indeterminate = progress.percent === null;
  const width = indeterminate ? 100 : progress.percent;
  return `
    <span
      class="status-capsule-progress ${indeterminate ? "status-capsule-progress-indeterminate" : ""}"
      data-testid="status-capsule-progress"
      data-progress-kind="${indeterminate ? "indeterminate" : "determinate"}"
      aria-hidden="true"
    >
      <span class="status-capsule-progress-fill" style="width: ${width}%"></span>
    </span>
    <span class="status-capsule-progress-label" data-testid="status-capsule-progress-label">
      ${escapeHtml(progress.label)}
    </span>
  `;
}

export function renderContextRail(library: LibrarySnapshot | null) {
  const status = currentStatusCapsule(library);
  const hasProgress = "progress" in status && Boolean(status.progress);
  return `
    <div class="context-rail-shell context-rail-shell-product" data-testid="context-rail">
      <div class="context-rail-brand">
        <p class="eyebrow">FauniSearch</p>
        <div class="context-rail-brand-copy">
          <p class="context-rail-tagline">Unified · Native · Powerful</p>
        </div>
      </div>
      <div class="context-rail-status">
        ${renderStatusButton(status.label, status.pillClass as any, {
          className: `utility-trigger-pill${hasProgress ? " status-capsule-with-progress" : ""}`,
          testId: "status-capsule-button",
          prefixHtml: '<span class="status-dot"></span>',
          childrenHtml: renderStatusCapsuleProgress(status),
          attrs: {
            "data-open-settings-section": "diagnostics",
            "aria-label": status.summary,
          },
        })}
      </div>
    </div>
  `;
}

export function renderWorkspaceSwitcher() {
  return `
    <div class="sidebar-sections">
      <nav class="workspace-switch" data-testid="workspace-switch" aria-label="主工作区切换">
        <button
          type="button"
          class="workspace-switch-button ui-selection-control"
          data-testid="workspace-tab-search"
          data-workspace="search"
          data-ui-selected="${state.activeWorkspace === "search" ? "true" : "false"}"
        >
          ${renderUiIcon("search")}
          <span>搜索</span>
        </button>
        <button
          type="button"
          class="workspace-switch-button ui-selection-control"
          data-testid="workspace-tab-inventory"
          data-workspace="inventory"
          data-ui-selected="${state.activeWorkspace === "inventory" ? "true" : "false"}"
        >
          ${renderUiIcon("library")}
          <span>库管理</span>
        </button>
        <button
          type="button"
          class="workspace-switch-button ui-selection-control"
          data-testid="workspace-tab-settings"
          data-workspace="settings"
          data-ui-selected="${state.activeWorkspace === "settings" ? "true" : "false"}"
        >
          ${renderUiIcon("settings")}
          <span>设置</span>
        </button>
      </nav>
    </div>
  `;
}
