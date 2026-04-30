import type { LibrarySnapshot } from "../../../types";
import { allLibrariesTextScopeActive } from "../../selectors/library";
import { currentSearchScopeStageState } from "../../selectors/search";
import { currentStatusCapsule, globalJobsProgressSummary, libraryOperationalReadiness } from "../../selectors/runtime";
import { escapeHtml, assetTypeDisplayName } from "../../selectors/common";
import { state } from "../../state/store";
import { renderEmptyState, renderNotice, renderUiButton } from "./primitives";

export function renderSearchStatusNextStep(
  library: LibrarySnapshot | null,
  context: "utility" | "outcome" = "utility"
) {
  if (!library) {
    return "";
  }

  const allLibrariesScope = allLibrariesTextScopeActive();
  const readiness = libraryOperationalReadiness(library);
  const scopeState = currentSearchScopeStageState(library);
  const nextAction = scopeState.nextAction;
  const actions = [];
  let title = allLibrariesScope ? "可以直接跨库搜索" : "可以直接搜索";
  let summary = allLibrariesScope
    ? scopeState.summary
    : "当前库已经进入可搜索状态，下一步更适合直接发起查询或调整查询方式。";

  if (nextAction === "settings") {
    title = allLibrariesScope ? "先完成一个库的搜索配置" : "检查当前库覆盖";
    summary = scopeState.summary;
    actions.push(`
      ${renderUiButton("前往当前库覆盖", {
        tone: context === "utility" ? "secondary" : "primary",
        testId: context === "utility" ? "utility-drawer-status-open-library-overrides" : "search-error-open-library-overrides",
        attrs: { "data-open-settings-section": "library-overrides" },
      })}
    `);
  } else if (nextAction === "jobs") {
    title = allLibrariesScope ? "等待至少一个库准备完成" : "等待当前任务完成";
    summary = scopeState.summary;
    actions.push(`
      ${renderUiButton("查看任务", {
        tone: context === "utility" ? "secondary" : "primary",
        testId: context === "utility" ? "utility-drawer-status-open-jobs" : "search-outcome-open-jobs",
        attrs: { "data-utilities-action": "focus-search-jobs" },
      })}
    `);
  } else if (nextAction === "source-prep") {
    title = allLibrariesScope
      ? "让至少一个库进入可搜索状态"
      : readiness.status === "尚未接入来源根"
        ? "接入第一个来源根"
        : readiness.status === "来源根已停用"
          ? "恢复一个来源根"
          : readiness.status === "需要关注"
            ? "先检查来源根健康"
            : readiness.status === "观察未稳定"
              ? "恢复来源观察"
              : "准备第一批内容";
    summary = scopeState.summary;
    actions.push(`
      ${renderUiButton("前往库管理", {
        tone: context === "utility" ? "secondary" : "primary",
        testId: context === "utility" ? "utility-drawer-status-open-inventory" : "search-outcome-open-inventory",
        attrs: { "data-utilities-action": "focus-source-prep" },
      })}
    `);
  }

  return `
    <div class="utility-drawer-summary-card search-status-next-step" data-testid="${escapeHtml(
      context === "utility" ? "utility-drawer-status-next-step" : "search-outcome-next-step"
    )}">
      <strong>${escapeHtml(title)}</strong>
      <p class="helper">${escapeHtml(summary)}</p>
      ${actions.length ? `<div class="inline-actions">${actions.join("")}</div>` : ""}
    </div>
  `;
}

export function renderStatusNotices() {
  const blocks = [];

  if (state.globalError) {
    blocks.push(
      renderNotice({
        tone: "error",
        title: state.globalError.code ?? "error",
        body: state.globalError.message ?? String(state.globalError),
      })
    );
  }

  if (state.statusMessage && !globalJobsProgressSummary()) {
    blocks.push(renderNotice({ tone: "success", title: "进行中", body: state.statusMessage }));
  }

  if (!blocks.length) {
    return "";
  }

  return `<section class="status-stack">${blocks.join("")}</section>`;
}

export function renderImportReceipt() {
  if (!state.importReceipt) {
    return renderEmptyState("还没有导入回执。提交路径后会在这里显示接受和拒绝结果。", {
      testId: "import-receipt-empty",
    });
  }

  const accepted = state.importReceipt.accepted.length
    ? `
        <div class="receipt-group" data-testid="import-accepted-group">
          <h4>已接受</h4>
          <ul class="data-list">
            ${state.importReceipt.accepted
              .map(
                (item) => `
                  <li>
                    <div class="list-head">
                      <strong>${escapeHtml(assetTypeDisplayName(item.kind))}</strong>
                      <span class="helper">${(item.assets ?? []).length} 个可搜索对象</span>
                    </div>
                    <span>${escapeHtml(item.normalized_path ?? item.original_path)}</span>
                    ${
                      item.assets?.length
                        ? `<div class="inline-actions">
                            ${item.assets
                              .map(
                                (asset) => `
                                  <button
                                    type="button"
                                    class="ui-button ui-button-secondary"
                                    data-asset-id="${escapeHtml(asset.asset_id)}"
                                  >
                                    查看 ${escapeHtml(assetTypeDisplayName(asset.asset_type))} · ${escapeHtml(asset.asset_id)}
                                  </button>
                                `
                              )
                              .join("")}
                          </div>`
                        : ""
                    }
                  </li>
                `
              )
              .join("")}
          </ul>
        </div>
      `
    : "";

  const rejected = state.importReceipt.rejected.length
    ? `
        <div class="receipt-group" data-testid="import-rejected-group">
          <h4>已拒绝</h4>
          <ul class="data-list">
            ${state.importReceipt.rejected
              .map(
                (item) => `
                  <li data-testid="import-rejected-item" data-reason-code="${escapeHtml(item.reason_code)}">
                    <strong data-testid="import-rejected-reason">${escapeHtml(item.reason_code)}</strong>
                    <span>${escapeHtml(item.original_path)} · ${escapeHtml(item.message)}</span>
                  </li>
                `
              )
              .join("")}
          </ul>
        </div>
      `
    : "";

  const jobSummary = state.importReceipt.job
    ? `<p class="helper" data-testid="import-job-summary">任务 ${escapeHtml(state.importReceipt.job.job_id)} 当前处于 ${escapeHtml(state.importReceipt.job.phase)}。</p>`
    : `<p class="helper" data-testid="import-no-job">这次提交没有创建后台任务。</p>`;

  return `<div data-testid="import-receipt">${accepted}${rejected}${jobSummary}</div>`;
}
