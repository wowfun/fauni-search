import type { LibrarySnapshot } from "../../../types";
import { canSearchCurrentScope, searchFiltersSummary } from "../../selectors/search";
import {
  escapeHtml,
  libraryDisplayName,
  libraryIsArchived,
  libraryLifecycleLabel,
  libraryLifecyclePillClass,
} from "../../selectors/common";
import { libraryOperationalReadiness } from "../../selectors/runtime";
import { state } from "../../state/store";
import { renderStatusTag, renderUiButton } from "./primitives";

export function renderLibraryOptions(items: LibrarySnapshot[]) {
  return items
    .map(
      (item) => {
        const displayName = libraryDisplayName(item);
        const label = displayName === item.id ? displayName : `${displayName} (${item.id})`;
        return `
          <option value="${escapeHtml(item.id)}" ${item.id === state.selectedLibraryId ? "selected" : ""}>
            ${escapeHtml(`${label}${libraryIsArchived(item) ? " · 已归档" : ""}`)}
          </option>
        `;
      }
    )
    .join("");
}

export function renderLibrarySelectControl(
  label = "切换库",
  className = "context-rail-field context-rail-selector"
) {
  const activeLibraries = state.libraries.filter((item) => !libraryIsArchived(item));
  const archivedLibraries = state.libraries.filter((item) => libraryIsArchived(item));

  return `
    <label class="${escapeHtml(className)}">
      <span>${escapeHtml(label)}</span>
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
  `;
}

export function renderCreateLibraryPopover() {
  return `
    <details
      class="create-library-popover"
      data-testid="create-library-popover"
      ${state.createLibraryPopoverOpen || !state.libraries.length ? "open" : ""}
    >
      <summary class="ui-button ui-button-secondary" data-testid="open-create-library-button">新建库</summary>
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
        ${renderUiButton("创建库", { type: "submit", testId: "create-library-button" })}
      </form>
    </details>
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
            ${renderUiButton("当前库", {
              tone: "secondary",
              className: "search-scope-toggle",
              testId: "search-scope-library",
              attrs: { "data-search-scope": "library" },
              selected: state.searchScope === "library",
            })}
            ${renderUiButton("所有库", {
              tone: "secondary",
              className: "search-scope-toggle",
              testId: "search-scope-all-libraries",
              attrs: { "data-search-scope": "all_libraries" },
              disabled: !state.libraries.length,
              selected: allLibrariesActive,
            })}
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
            ${renderUiButton("Search", {
              type: "submit",
              form: "search-form",
              className: "search-submit-inline",
              testId: "search-submit-button",
              disabled: searchButtonDisabled,
            })}
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

type WorkspaceToolbarCapabilities = {
  showNameEditor?: boolean;
  showLibraryActions?: boolean;
  showMetrics?: boolean;
};

function renderWorkspaceLibraryDisplay(
  library: LibrarySnapshot | null,
  capabilities: WorkspaceToolbarCapabilities
) {
  const displayName = library
    ? libraryDisplayName(library)
    : state.libraries.length
      ? "未选择库"
      : "还没有库";
  const summary = library
    ? capabilities.showLibraryActions
      ? "当前来源浏览与库级管理都作用于此库。"
      : "当前配置、模型测试与诊断都作用于此库。"
    : state.libraries.length
      ? capabilities.showLibraryActions
        ? "先选择一个库再管理来源与状态。"
        : "先选择一个库再查看当前库配置。"
      : capabilities.showLibraryActions
        ? "先创建一个库再管理来源与状态。"
        : "去库管理创建第一个库。";

  return `
    <div class="workspace-library-display" data-testid="workspace-library-display">
      <strong data-testid="workspace-library-name">${escapeHtml(displayName)}</strong>
      <span class="helper workspace-library-summary" data-testid="workspace-library-summary">
        ${escapeHtml(summary)}
      </span>
    </div>
  `;
}

function renderWorkspaceLibraryStatusline(
  library: LibrarySnapshot | null,
  readiness: ReturnType<typeof libraryOperationalReadiness> | null
) {
  if (!library || !readiness) {
    return `
      <div class="workspace-library-statusline" data-testid="workspace-library-statusline">
        ${renderStatusTag("未选择", "pending")}
      </div>
    `;
  }

  return `
    <div class="workspace-library-statusline" data-testid="workspace-library-statusline">
      <span data-testid="workspace-library-lifecycle">
        ${renderStatusTag(libraryLifecycleLabel(library), libraryLifecyclePillClass(library) as any)}
      </span>
      <span data-testid="workspace-library-readiness">
        ${renderStatusTag(readiness.status, readiness.pillClass as any)}
      </span>
    </div>
  `;
}

function renderWorkspaceLibraryMetrics(
  library: LibrarySnapshot | null,
  readiness: ReturnType<typeof libraryOperationalReadiness> | null
) {
  if (!library || !readiness) {
    return "";
  }

  const metrics = [
    `来源根 ${readiness.enabledRoots}/${state.sourceRoots.length}`,
    `对象 ${readiness.searchableUnits}`,
    `任务 ${readiness.pendingJobs}`,
  ];

  return `
    <div class="workspace-library-metrics" data-testid="workspace-library-metrics">
      ${metrics.map((item) => `<span class="helper">${escapeHtml(item)}</span>`).join("")}
    </div>
  `;
}

function renderWorkspaceLibraryNameEditor(library: LibrarySnapshot | null) {
  if (!library) {
    return renderWorkspaceLibraryDisplay(library, {
      showLibraryActions: true,
      showMetrics: true,
    });
  }

  return `
    <form
      class="stack-form compact-form workspace-library-name-form"
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
      ${renderUiButton("保存", { type: "submit", testId: "inventory-rename-library-button" })}
    </form>
  `;
}

function renderWorkspaceLibraryActions(library: LibrarySnapshot | null) {
  if (!library) {
    return `
      <div class="ui-action-row workspace-library-actions workspace-library-actions-empty">
        ${renderCreateLibraryPopover()}
      </div>
    `;
  }

  return `
    <div class="ui-action-row workspace-library-actions">
      ${renderUiButton(libraryIsArchived(library) ? "恢复当前库" : "归档当前库", {
        tone: "secondary",
        testId: "inventory-toggle-library-archive-button",
        attrs: { "data-library-archive-action": "true" },
      })}
      ${renderUiButton("删除当前库", {
        tone: "danger",
        testId: "inventory-delete-library-button",
        attrs: { "data-library-delete-action": "true" },
      })}
      <div class="workspace-library-create-row">
        ${renderCreateLibraryPopover()}
      </div>
    </div>
  `;
}

function renderWorkspaceLibraryToolbar(
  library: LibrarySnapshot | null,
  capabilities: WorkspaceToolbarCapabilities = {}
) {
  const normalizedCapabilities = {
    showNameEditor: false,
    showLibraryActions: false,
    showMetrics: true,
    ...capabilities,
  };
  const readiness = library ? libraryOperationalReadiness(library) : null;

  return `
    <section
      class="library-context-cluster workspace-library-toolbar ${
        normalizedCapabilities.showLibraryActions ? "workspace-library-toolbar-management" : "workspace-library-toolbar-readonly"
      }"
      data-testid="workspace-library-toolbar"
      data-library-context-mode="${normalizedCapabilities.showLibraryActions ? "management" : "readonly"}"
    >
      <div class="workspace-library-toolbar-row">
        ${renderLibrarySelectControl("当前库", "context-rail-field workspace-library-select")}
        ${
          normalizedCapabilities.showNameEditor
            ? renderWorkspaceLibraryNameEditor(library)
            : renderWorkspaceLibraryDisplay(library, normalizedCapabilities)
        }
        ${renderWorkspaceLibraryStatusline(library, readiness)}
        ${
          normalizedCapabilities.showMetrics
            ? renderWorkspaceLibraryMetrics(library, readiness)
            : ""
        }
        ${
          normalizedCapabilities.showLibraryActions
            ? renderWorkspaceLibraryActions(library)
            : ""
        }
      </div>
    </section>
  `;
}

export function renderLibraryContext(options: {
  library: LibrarySnapshot | null;
  variant?: "search-scope" | "workspace-toolbar";
  capabilities?: WorkspaceToolbarCapabilities;
}) {
  const activeLibraries = state.libraries.filter((item) => !libraryIsArchived(item));
  const archivedLibraries = state.libraries.filter((item) => libraryIsArchived(item));
  const variant = options.variant ?? "search-scope";

  if (variant === "search-scope") {
    return renderSearchScopeBar(options.library, activeLibraries, archivedLibraries);
  }

  return renderWorkspaceLibraryToolbar(options.library, options.capabilities);
}
