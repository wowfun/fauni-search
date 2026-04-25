import type { LibrarySnapshot } from "../../../types";
import { escapeHtml } from "../../selectors/common";
import { sourceRootStatusPillClass } from "../../selectors/inventory";
import { state } from "../../state/store";
import { renderEmptyState, renderStatusTag, renderUiButton } from "./primitives";

function multilineDraftToList(value: string) {
  return String(value ?? "")
    .split("\n")
    .map((item) => item.trim())
    .filter(Boolean);
}

function commaDraftToList(value: string) {
  return String(value ?? "")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function sourceRootDraftRules() {
  return {
    include_globs: multilineDraftToList(state.sourceRootIncludeGlobsDraft),
    exclude_globs: multilineDraftToList(state.sourceRootExcludeGlobsDraft),
    include_extensions: commaDraftToList(state.sourceRootIncludeExtensionsDraft),
  };
}

export function renderSourceRootRulesSummary(rules) {
  const parts = [];
  const includeGlobs = rules?.include_globs ?? [];
  const excludeGlobs = rules?.exclude_globs ?? [];
  const includeExtensions = rules?.include_extensions ?? [];

  parts.push(includeGlobs.length ? `包含规则 ${includeGlobs.length}` : "包含全部");
  parts.push(excludeGlobs.length ? `排除规则 ${excludeGlobs.length}` : "不排除");
  parts.push(includeExtensions.length ? includeExtensions.join(", ") : "全部来源类型");
  return parts.join(" · ");
}

export function formatScanTime(lastScanAtMs) {
  if (!lastScanAtMs) {
    return "尚未刷新或重扫";
  }
  return new Date(Number(lastScanAtMs)).toLocaleString();
}

export function renderSourceRootManagementSummary() {
  if (!state.sourceRoots.length) {
    return "当前库还没有来源根。展开后创建第一个本地目录来源根。";
  }

  const enabledCount = state.sourceRoots.filter((sourceRoot) => sourceRoot.enabled).length;
  const degradedCount = state.sourceRoots.filter((sourceRoot) => sourceRoot.status === "degraded").length;
  const disabledCount = state.sourceRoots.filter((sourceRoot) => !sourceRoot.enabled).length;
  const parts = [`已配置 ${state.sourceRoots.length} 个来源根`, `启用 ${enabledCount}`];

  if (degradedCount) {
    parts.push(`需关注 ${degradedCount}`);
  }
  if (disabledCount) {
    parts.push(`停用 ${disabledCount}`);
  }

  return parts.join(" · ");
}

function renderSourceRootAdvancedRules(library: LibrarySnapshot | null) {
  const summary = renderSourceRootRulesSummary(sourceRootDraftRules());

  return `
    <section class="source-root-advanced-rules" data-testid="source-root-advanced-rules">
      <button
        type="button"
        class="source-root-advanced-toggle"
        data-testid="source-root-advanced-rules-toggle"
        data-source-root-advanced-toggle
        aria-expanded="${state.sourceRootAdvancedRulesOpen ? "true" : "false"}"
        ${library ? "" : "disabled"}
      >
        <span>高级规则</span>
        <span class="helper">${escapeHtml(summary)}</span>
      </button>
      ${
        state.sourceRootAdvancedRulesOpen
          ? `
            <div class="source-root-advanced-fields" data-testid="source-root-advanced-rules-panel">
              <label>
                <span>包含规则（globs）</span>
                <textarea
                  id="source-root-include-globs"
                  data-testid="source-root-include-globs-input"
                  rows="3"
                  placeholder="images/**&#10;reports/*.pdf"
                  ${library ? "" : "disabled"}
                >${escapeHtml(state.sourceRootIncludeGlobsDraft)}</textarea>
              </label>
              <label>
                <span>排除规则（globs）</span>
                <textarea
                  id="source-root-exclude-globs"
                  data-testid="source-root-exclude-globs-input"
                  rows="3"
                  placeholder="**/*.tmp&#10;archive/**"
                  ${library ? "" : "disabled"}
                >${escapeHtml(state.sourceRootExcludeGlobsDraft)}</textarea>
              </label>
              <label>
                <span>包含扩展名</span>
                <input
                  id="source-root-include-extensions"
                  data-testid="source-root-include-extensions-input"
                  type="text"
                  placeholder="png, jpg, pdf"
                  value="${escapeHtml(state.sourceRootIncludeExtensionsDraft)}"
                  ${library ? "" : "disabled"}
                />
              </label>
            </div>
          `
          : ""
      }
    </section>
  `;
}

export function renderSourceRootEditorForm(
  library: LibrarySnapshot | null,
  options: { variant?: "search" | "inventory" } = {}
) {
  const variant = options.variant ?? "search";
  const submitLabel = state.editingSourceRootId ? "保存来源根" : "创建来源根";
  const resetLabel = variant === "inventory" ? "取消" : "清空";
  const title = state.editingSourceRootId ? "编辑来源根" : "新建来源根";
  const description = state.editingSourceRootId
    ? "调整目录、启用状态或高级规则。"
    : "添加一个本地目录来源根，并决定是否立即附带高级规则。";

  return `
    <form
      id="source-root-form"
      class="stack-form source-root-form ${variant === "inventory" ? "source-root-form-card" : ""}"
      data-testid="source-root-form"
    >
      ${
        variant === "inventory"
          ? `
            <div class="source-root-form-head" data-testid="inventory-source-root-editor">
              <div>
                <p class="eyebrow">来源根</p>
                <h3>${title}</h3>
              </div>
              <p class="helper">${description}</p>
            </div>
          `
          : ""
      }
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
      ${renderSourceRootAdvancedRules(library)}
      <div class="inline-actions">
        ${renderUiButton(submitLabel, {
          type: "submit",
          testId: "source-root-submit-button",
          disabled: !library,
        })}
        ${renderUiButton(resetLabel, {
          tone: "secondary",
          id: "source-root-reset-button",
          testId: "source-root-reset-button",
          disabled: !library,
        })}
      </div>
    </form>
  `;
}

export function renderSourceRootList() {
  if (!state.sourceRoots.length) {
    return renderEmptyState("当前库还没有来源根。先创建一个本地目录来源根，再触发刷新或重扫。", {
      testId: "source-root-empty",
    });
  }

  return `
    <ul class="data-list source-root-list" data-testid="source-root-list">
      ${state.sourceRoots
        .map(
          (sourceRoot) => `
            <li class="source-root-card" data-testid="source-root-card" data-source-root-id="${escapeHtml(sourceRoot.source_root_id)}">
              <div class="list-head">
                <strong>${escapeHtml(sourceRoot.root_path)}</strong>
                <span class="helper">${escapeHtml(sourceRoot.source_root_id)}</span>
              </div>
              <div class="ui-tag-row compact-row">
                ${renderStatusTag(sourceRoot.status, sourceRootStatusPillClass(sourceRoot.status) as any)}
                ${renderStatusTag(sourceRoot.watch_state, "muted")}
              </div>
              <dl class="ui-meta-list compact-stats">
                <div><dt>已观察</dt><dd>${sourceRoot.coverage_summary?.observed_file_count ?? 0}</dd></div>
                <div><dt>匹配</dt><dd>${sourceRoot.coverage_summary?.matched_file_count ?? 0}</dd></div>
                <div><dt>正常</dt><dd>${sourceRoot.coverage_summary?.active_source_count ?? 0}</dd></div>
                <div><dt>未启用</dt><dd>${sourceRoot.coverage_summary?.inactive_source_count ?? 0}</dd></div>
              </dl>
              <p class="helper">${escapeHtml(renderSourceRootRulesSummary(sourceRoot.rules))}</p>
              <p class="helper">最近扫描：${escapeHtml(formatScanTime(sourceRoot.coverage_summary?.last_scan_at_ms))}</p>
              ${
                sourceRoot.last_action
                  ? `<p class="helper">最近动作：${escapeHtml(sourceRoot.last_action.action)} · ${escapeHtml(sourceRoot.last_action.status)} · ${escapeHtml(sourceRoot.last_action.summary)}</p>`
                  : ""
              }
              <div class="inline-actions">
                ${renderUiButton("编辑", {
                  tone: "secondary",
                  attrs: { "data-source-root-edit-id": sourceRoot.source_root_id },
                })}
                ${renderUiButton("刷新", {
                  attrs: { "data-source-root-refresh-id": sourceRoot.source_root_id },
                  disabled: !sourceRoot.enabled,
                })}
                ${renderUiButton("重扫", {
                  tone: "secondary",
                  attrs: { "data-source-root-rescan-id": sourceRoot.source_root_id },
                  disabled: !sourceRoot.enabled,
                })}
                ${renderUiButton(sourceRoot.enabled ? "停用" : "启用", {
                  tone: "secondary",
                  attrs: { "data-source-root-toggle-id": sourceRoot.source_root_id },
                })}
                ${renderUiButton("删除", {
                  tone: "danger",
                  attrs: { "data-source-root-delete-id": sourceRoot.source_root_id },
                })}
              </div>
            </li>
          `
        )
        .join("")}
    </ul>
  `;
}

export function renderSourceRootsPanel(library: LibrarySnapshot | null) {
  return `
    <section class="panel panel-tight source-root-management" data-testid="source-root-management">
      <div class="panel-head">
        <div>
          <p class="eyebrow">来源</p>
          <h2>来源根管理</h2>
        </div>
      </div>
      <p class="helper source-root-management-note">${escapeHtml(renderSourceRootManagementSummary())}</p>
      ${renderSourceRootEditorForm(library)}
      ${renderSourceRootList()}
    </section>
  `;
}
