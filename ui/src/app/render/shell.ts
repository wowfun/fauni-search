import {
  currentStatusCapsule,
  renderUiIcon,
  state,
  type LibrarySnapshot,
} from "../core";
import { renderStatusButton } from "./shared/primitives";

export function renderContextRail(library: LibrarySnapshot | null) {
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
        ${renderStatusButton(status.label, status.pillClass as any, {
          className: "utility-trigger-pill",
          testId: "status-capsule-button",
          prefixHtml: '<span class="status-dot"></span>',
          attrs: {
            "data-open-settings-section": "diagnostics",
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
