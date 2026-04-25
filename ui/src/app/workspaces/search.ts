import {
  activeQueryDocumentPreview,
  activeQueryImagePreview,
  activeQueryVideoPreview,
  allLibrariesTextScopeActive,
  contentTypeDisplayName,
  currentQueryDocumentEndPage,
  currentQueryDocumentStartPage,
  currentQueryVideoEndMs,
  currentQueryVideoStartMs,
  currentSearchScopeStageState,
  currentSearchStageState,
  escapeHtml,
  formatScore,
  isDocumentPageQueryImage,
  libraryById,
  libraryDisplayName,
  libraryNeedsPreparation,
  libraryOperationalReadiness,
  pageLabel,
  queryDocumentDisplayName,
  queryDocumentPreviewUrl,
  queryDocumentRangeSummary,
  queryDocumentStatusLabel,
  queryImageDisplayName,
  queryImagePreviewUrl,
  queryImageStatusLabel,
  queryVideoDisplayName,
  queryVideoPreviewUrl,
  queryVideoRangeStep,
  queryVideoRangeSummary,
  queryVideoStatusLabel,
  renderUiIcon,
  searchHasMoreResults,
  searchModeDisplayName,
  searchStageNextAction,
  selectedLibrary,
  selectedVisualUnitId,
  selectedVisualUnitOriginLibraryId,
  sourceName,
  sourceTypeDisplayName,
  state,
  videoLabel,
  visualUnitKindDisplayName,
  type LibrarySnapshot,
  type SearchResultItem,
} from "../core";
import { renderInventoryBridge, renderProviderBridge } from "../render/shared/bridges";
import { renderDetailCard } from "../render/shared/detail";
import { renderObjectListItem } from "../render/shared/list-item";
import { renderPreviewSurface, renderSearchResultPreview } from "../render/shared/preview";
import {
  renderEmptyState,
  renderLocatorTag,
  renderNotice,
  renderScoreTag,
  renderStatusTag,
  renderTypeTag,
  renderUiButton,
} from "../render/shared/primitives";
import { renderSourceRootsPanel } from "../render/shared/source-roots";
import { renderImportReceipt, renderSearchStatusNextStep } from "../render/shared/status";

export function renderSearchStateStrip(library: LibrarySnapshot | null) {
  const stageState = currentSearchStageState(library);
  if (stageState.status === "可搜索") {
    return "";
  }
  return `
    <div class="search-state-strip" data-testid="search-state-strip">
      ${renderStatusTag(stageState.status, stageState.pillClass as any)}
      <p class="helper">${escapeHtml(stageState.summary)}</p>
    </div>
  `;
}

export function renderSearchLoadingNotice() {
  if (!state.searchInFlight) {
    return "";
  }

  return `
    <div class="search-results-loading" data-testid="search-loading-notice">
      <p class="helper">搜索中...</p>
    </div>
  `;
}

export function shouldRenderSearchNextStepDock(library: LibrarySnapshot | null) {
  if (!library) {
    return true;
  }

  return (
    libraryNeedsPreparation(library) ||
    currentSearchScopeStageState(library).nextAction === "jobs" ||
    state.searchPreparationDisclosureOpen ||
    Boolean(state.editingSourceRootId)
  );
}

export function renderSearchNextStepDock(library: LibrarySnapshot | null) {
  if (!shouldRenderSearchNextStepDock(library)) {
    return "";
  }

  if (!library) {
    return `
      <aside class="next-step-dock" data-testid="search-next-step-dock">
        <p class="eyebrow">下一步</p>
        <h3>先去库管理</h3>
        <p class="helper">新建库和当前库管理已经移到库管理工作区；先完成库准备，再回到 Search 发起查询。</p>
        <div class="dock-note">
          <strong>主路径</strong>
          <p class="helper">库管理 → 导入内容 → 等待可搜索 → 发起搜索。</p>
        </div>
        <div class="dock-actions">
          <button
            type="button"
            data-testid="search-next-step-open-inventory"
            data-workspace="inventory"
          >
            前往库管理
          </button>
        </div>
      </aside>
    `;
  }

  const allLibrariesScope = allLibrariesTextScopeActive();
  const readiness = libraryOperationalReadiness(library);
  const scopeState = currentSearchScopeStageState(library);
  const nextAction = scopeState.nextAction;
  const supportDisclosures = renderSearchSupportDisclosures(library, nextAction);
  const dockFacts = allLibrariesScope
    ? [
        `覆盖 ${scopeState.totalLibraries} 个库`,
        `可搜索库 ${scopeState.searchableLibraries}`,
        `对象 ${scopeState.searchableUnits}`,
        `准备中 ${scopeState.pendingLibraries}`,
      ]
    : [
        `启用来源根 ${readiness.enabledRoots}`,
        `可搜索 ${readiness.searchableUnits}`,
        `待处理 ${readiness.pendingJobs}`,
      ];
  const readinessNote = `
    <div class="dock-note">
      <strong>${escapeHtml(allLibrariesScope ? "当前范围" : libraryDisplayName(library))}</strong>
      <p class="helper">${escapeHtml(dockFacts.join(" · "))}</p>
      ${
        !allLibrariesScope && readiness.lastActionSummary
          ? `<p class="helper">${escapeHtml(readiness.lastActionSummary)}</p>`
          : ""
      }
    </div>
  `;

  if (nextAction === "settings") {
    return `
      <aside class="next-step-dock" data-testid="search-next-step-dock">
        <p class="eyebrow">下一步</p>
        <h3>${escapeHtml(allLibrariesScope ? "先完成一个库的搜索配置" : "检查当前库覆盖")}</h3>
        <p class="helper">${escapeHtml(scopeState.summary)}</p>
        <div class="dock-note">
          <strong>配置状态</strong>
          <p class="helper">${escapeHtml(
            allLibrariesScope
              ? "所有库范围里还没有可搜索库；先让至少一个库的内容类型与 resolved model 完成就绪。"
              : `${readiness.blockedContentTypes} 个启用内容类型仍未就绪；先确认当前库覆盖、连接与 resolved model。`
          )}</p>
        </div>
        <div class="dock-actions">
          <button
            type="button"
            data-testid="search-next-step-open-library-overrides"
            data-open-settings-section="library-overrides"
          >
            前往当前库覆盖
          </button>
        </div>
        ${renderProviderBridge(library)}
        ${supportDisclosures}
      </aside>
    `;
  }

  if (nextAction === "jobs") {
    return `
      <aside class="next-step-dock" data-testid="search-next-step-dock">
        <p class="eyebrow">下一步</p>
        <h3>${escapeHtml(allLibrariesScope ? "等待至少一个库准备完成" : "等待当前任务完成")}</h3>
        <p class="helper">${escapeHtml(scopeState.summary)}</p>
        ${readinessNote}
        <div class="dock-actions">
          <button
            type="button"
            data-testid="search-next-step-open-jobs"
            data-utilities-action="focus-search-jobs"
          >
            查看任务
          </button>
          <button
            type="button"
            class="ui-button ui-button-secondary"
            data-testid="search-next-step-open-source-prep"
            data-utilities-action="focus-source-prep"
          >
            打开来源准备
          </button>
        </div>
        ${supportDisclosures}
      </aside>
    `;
  }

  if (libraryNeedsPreparation(library)) {
    const title =
      allLibrariesScope
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
    return `
      <aside class="next-step-dock" data-testid="search-next-step-dock">
        <p class="eyebrow">下一步</p>
        <h3>${escapeHtml(title)}</h3>
        <p class="helper">${escapeHtml(scopeState.summary)}</p>
        ${readinessNote}
        <div class="dock-actions">
          <button
            type="button"
            data-testid="search-next-step-open-source-prep"
            data-utilities-action="focus-source-prep"
          >
            打开来源准备
          </button>
          <button
            type="button"
            class="ui-button ui-button-secondary"
            data-testid="search-next-step-open-inventory"
            data-workspace="inventory"
          >
            前往库管理
          </button>
        </div>
        ${renderInventoryBridge(library)}
        ${renderProviderBridge(library)}
        ${supportDisclosures}
      </aside>
    `;
  }

  return `
    <aside class="next-step-dock" data-testid="search-next-step-dock">
      <p class="eyebrow">${escapeHtml(allLibrariesScope ? "当前范围" : "当前库")}</p>
      <h3>${escapeHtml(allLibrariesScope ? "所有库" : libraryDisplayName(library))}</h3>
      <p class="helper">${escapeHtml(
        allLibrariesScope
          ? scopeState.summary
          : `${library.id} · 可搜索 ${library.counts.accepted_items} · 待处理 ${library.counts.pending_jobs}`
      )}</p>
      <div class="dock-note">
        <strong>建议</strong>
        <p class="helper">${escapeHtml(
          allLibrariesScope
            ? "先发起一轮跨库文本查询，再从结果卡或详情面继续下钻到命中库。"
            : "先搜一轮，再从结果卡或详情面直接复用对象作为下一次查询输入。"
        )}</p>
      </div>
      <div class="dock-actions">
        <button
          type="button"
          class="ui-button ui-button-secondary"
          data-testid="search-next-step-open-source-prep"
          data-utilities-action="focus-source-prep"
        >
          打开来源准备
        </button>
      </div>
      ${renderInventoryBridge(library)}
      ${renderProviderBridge(library)}
      ${supportDisclosures}
    </aside>
  `;
}

export function renderSearchSupportDisclosures(library: LibrarySnapshot | null, nextAction = searchStageNextAction(library)) {
  if (!library) {
    return "";
  }

  const preparationOpen =
    state.searchPreparationDisclosureOpen ||
    Boolean(state.editingSourceRootId) ||
    nextAction === "source-prep";

  return `
    <div class="next-step-support">
      <details
        id="search-preparation-disclosure"
        class="support-disclosure support-disclosure-subtle"
        ${preparationOpen ? "open" : ""}
      >
        <summary>导入与来源准备</summary>
        <div class="support-disclosure-body">
          ${renderImportPanel(library)}
          ${renderSourceRootsPanel(library)}
        </div>
      </details>
    </div>
  `;
}

export function renderImportPanel(library: LibrarySnapshot | null) {
  return `
    <section class="panel panel-tight utility-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">导入</p>
          <h2>导入内容</h2>
        </div>
      </div>
      <form id="import-form" class="stack-form" data-testid="import-form">
        <label>
          <span>本地路径</span>
          <textarea
            id="import-paths"
            data-testid="import-paths-input"
            rows="6"
            placeholder="/path/to/file.pdf&#10;/path/to/image.png"
            ${library ? "" : "disabled"}
          >${escapeHtml(state.importPathsDraft)}</textarea>
        </label>
        ${renderUiButton("提交导入", { type: "submit", testId: "import-submit-button", disabled: !library })}
      </form>
      <p class="helper">当前仍以服务器可读的本地路径作为正式导入入口；逐行填写文件或目录路径后即可提交导入。</p>
      ${renderImportReceipt()}
    </section>
  `;
}

export function searchResultLibraryBreakdown() {
  const results = state.searchOutcome?.results ?? [];
  const breakdown = new Map<string, { libraryId: string; label: string; count: number }>();
  results.forEach((item) => {
    const libraryId = item.library_id?.trim();
    if (!libraryId) {
      return;
    }
    const existing = breakdown.get(libraryId);
    if (existing) {
      existing.count += 1;
      return;
    }
    const library = libraryById(libraryId);
    breakdown.set(libraryId, {
      libraryId,
      label: library ? libraryDisplayName(library) : libraryId,
      count: 1,
    });
  });
  return [...breakdown.values()];
}

export function activeSearchResultLibraryFocus() {
  if (!allLibrariesTextScopeActive()) {
    return null;
  }
  const libraryId = state.searchResultLibraryFocusId.trim();
  if (!libraryId) {
    return null;
  }
  return searchResultLibraryBreakdown().find((item) => item.libraryId === libraryId) ?? null;
}

export function visibleSearchResults() {
  const results = state.searchOutcome?.results ?? [];
  const activeFocus = activeSearchResultLibraryFocus();
  if (!activeFocus) {
    return results;
  }
  return results.filter((item) => item.library_id === activeFocus.libraryId);
}

export function groupedSearchResults(results: SearchResultItem[]) {
  const groups = new Map<
    string,
    { libraryId: string; label: string; count: number; items: SearchResultItem[] }
  >();
  results.forEach((item) => {
    const libraryId = item.library_id?.trim() || "unknown";
    const existing = groups.get(libraryId);
    if (existing) {
      existing.items.push(item);
      existing.count += 1;
      return;
    }
    const library = libraryById(libraryId);
    groups.set(libraryId, {
      libraryId,
      label: library ? libraryDisplayName(library) : libraryId,
      count: 1,
      items: [item],
    });
  });
  return [...groups.values()];
}

export function searchResultGroupSummary(libraryId: string, count: number) {
  if (libraryId === state.selectedLibraryId) {
    return `当前工作库 · ${count} 条结果`;
  }
  return `${count} 条结果 · 可留在 Search 里先聚焦这一组，或直接进入库管理。`;
}

export function renderSearchResultCard(
  item: SearchResultItem,
  layout: "default" | "grouped" | "focused" = "default"
) {
  const scoreLabel = formatScore(item.score);
  const page = pageLabel(item.locator);
  const segment = videoLabel(item.locator);
  const actions = `
    ${renderUiButton("查看详情", {
      tone: "secondary",
      attrs: {
        "data-visual-unit-id": item.visual_unit_id,
        "data-visual-unit-library-id": item.library_id,
      },
    })}
    ${
      item.kind === "image" || item.kind === "document_page"
        ? renderUiButton("作为查询图片", {
            tone: "secondary",
            testId: "use-as-query-image-button",
            attrs: {
              "data-use-query-visual-unit-id": item.visual_unit_id,
              "data-use-query-library-id": item.library_id,
            },
          })
        : ""
    }
    ${
      item.kind === "document_page"
        ? renderUiButton("作为查询文档", {
            tone: "secondary",
            testId: "use-as-query-document-button",
            attrs: {
              "data-use-query-document-visual-unit-id": item.visual_unit_id,
              "data-use-query-library-id": item.library_id,
            },
          })
        : ""
    }
    ${
      item.kind === "video_segment"
        ? renderUiButton("作为查询视频", {
            tone: "secondary",
            testId: "use-as-query-video-button",
            attrs: {
              "data-use-query-video-visual-unit-id": item.visual_unit_id,
              "data-use-query-library-id": item.library_id,
            },
          })
        : ""
    }
    <a class="ui-button ui-button-secondary" href="${escapeHtml(item.preview.url)}" target="_blank" rel="noreferrer">打开预览</a>
  `;
  return renderObjectListItem({
    testId: "result-card",
    className: `result-card result-card-${layout}`,
    active: `${item.library_id}:${item.visual_unit_id}` === selectedVisualUnitId(),
    dataAttrs: {
      "data-kind": item.kind,
      "data-visual-unit-id": item.visual_unit_id,
    },
    selectClassName: "result-select",
    selectAttrs: {
      "data-visual-unit-id": item.visual_unit_id,
      "data-visual-unit-library-id": item.library_id,
    },
    visualClassName: "result-visual",
    visualHtml: renderSearchResultPreview(item),
    bodyClassName: `result-body result-body-${layout}`,
    topLineHtml: `
      <div class="result-topline">
        ${renderTypeTag(visualUnitKindDisplayName(item.kind), item.kind === "image" ? "ready" : "pending")}
        ${page ? renderLocatorTag(page) : ""}
        ${segment ? renderLocatorTag(segment) : ""}
      </div>
    `,
    title: sourceName(item.source_path),
    titleRowClassName: "result-title-row",
    titleClassName: "result-title",
    titleAfterHtml: scoreLabel ? renderScoreTag(scoreLabel, { testId: "result-score" }) : "",
    metaHtml: `<span class="helper result-path">${escapeHtml(item.source_path)}</span>`,
    actionsClassName: "result-actions",
    actionsHtml: actions,
  });
}

export function renderSearchResultGroup(group: {
  libraryId: string;
  label: string;
  count: number;
  items: SearchResultItem[];
}) {
  return `
    <section
      class="result-library-group"
      data-testid="search-result-library-group"
      data-library-id="${escapeHtml(group.libraryId)}"
    >
      <div class="result-library-group-header">
        <div class="result-library-group-copy">
          <div class="result-library-group-meta">
            <span class="scope-label">${escapeHtml(group.libraryId === state.selectedLibraryId ? "当前库" : "命中库")}</span>
            <span class="helper" data-testid="search-result-library-group-count">${escapeHtml(`${group.count} 条结果`)}</span>
          </div>
          <strong data-testid="search-result-library-group-heading">${escapeHtml(group.label)}</strong>
          <p class="helper result-library-group-summary" data-testid="search-result-library-group-summary">${escapeHtml(
            searchResultGroupSummary(group.libraryId, group.count)
          )}</p>
        </div>
        <div class="inline-actions result-library-group-actions">
          ${renderUiButton("仅看这个库", {
            tone: "secondary",
            testId: `search-result-library-group-focus-${group.libraryId}`,
            attrs: { "data-search-result-library-focus": group.libraryId },
          })}
          ${renderUiButton("在库管理查看", {
            tone: "secondary",
            testId: `search-result-library-group-open-inventory-${group.libraryId}`,
            attrs: { "data-open-hit-library-id": group.libraryId },
          })}
        </div>
      </div>
      <ul class="result-list result-group-list">
        ${group.items.map((item) => renderSearchResultCard(item, "grouped")).join("")}
      </ul>
    </section>
  `;
}

export function renderVisualPreview() {
  if (!state.selectedVisualUnit) {
    return `
      <div class="preview-placeholder" data-testid="visual-preview">
        <p>选择一个结果或导入项后，这里会显示图片或 PDF 页预览。</p>
      </div>
    `;
  }

  const visualUnit = state.selectedVisualUnit.visual_unit;
  const preview = state.selectedVisualUnit.preview;
  return renderPreviewSurface(visualUnit, preview, "visual-preview");
}

export function renderVisualUnitDetail() {
  if (!state.selectedVisualUnit) {
    return renderEmptyState("从结果列表选择一个对象后，这里会显示预览与来源信息。");
  }

  const visualUnit = state.selectedVisualUnit.visual_unit;
  const originLibraryId = selectedVisualUnitOriginLibraryId();
  const originLibrary = libraryById(originLibraryId);
  const page = pageLabel(visualUnit.locator);
  const segment = videoLabel(visualUnit.locator);
  const showCrossLibraryContext = allLibrariesTextScopeActive() && originLibraryId;
  const crossLibraryContext = showCrossLibraryContext
    ? `
      <section class="detail-library-context" data-testid="detail-library-context">
        <div class="detail-library-context-copy">
          <span class="scope-label">命中库</span>
          <strong data-testid="detail-hit-library-name">${escapeHtml(
            originLibrary ? libraryDisplayName(originLibrary) : originLibraryId
          )}</strong>
          <p class="helper" data-testid="detail-hit-library-summary">${escapeHtml(
            originLibraryId === state.selectedLibraryId
              ? "你当前已经在这个库的上下文里阅读结果。"
              : `当前选中库仍是 ${libraryDisplayName(selectedLibrary()) || state.selectedLibraryId}；继续管理来源或复用结果时会自动切到命中库，如需先核对 readiness，也可以直接进入库管理。`
          )}</p>
        </div>
        <div class="inline-actions detail-library-context-actions">
          ${renderUiButton("在库管理查看", {
            tone: "secondary",
            testId: "detail-open-hit-library-inventory",
            attrs: { "data-open-hit-library-id": originLibraryId },
          })}
        </div>
      </section>
    `
    : "";
  const technicalInfo = `
    <details class="detail-technical-disclosure" data-testid="detail-technical-disclosure">
        <summary>技术信息</summary>
        <div class="detail-technical-content" data-testid="detail-technical-content">
          <div class="detail-grid">
            <div class="detail-block">
              <h5>定位信息</h5>
              <pre>${escapeHtml(JSON.stringify(visualUnit.locator, null, 2))}</pre>
            </div>
            <div class="detail-block">
              <h5>阅读提示</h5>
              <p class="helper">当前详情面会在后台轮询期间保持预览挂载不变，方便继续阅读和复用结果。</p>
            </div>
          </div>
          <div class="detail-block">
            <h5>邻近上下文</h5>
            <pre>${escapeHtml(JSON.stringify(state.selectedVisualUnit.neighbor_context, null, 2))}</pre>
          </div>
        </div>
      </details>
  `;

  return renderDetailCard({
    testId: "visual-unit-detail",
    title: sourceName(visualUnit.source_path),
    previewHtml: renderVisualPreview(),
    tags: [
      ...(state.searchScope === "all_libraries" && originLibraryId
        ? [{ label: originLibrary ? libraryDisplayName(originLibrary) : originLibraryId, tone: "muted" as const }]
        : []),
      { label: visualUnitKindDisplayName(visualUnit.kind), tone: "ready" },
      ...(page ? [{ label: page, tone: "muted" as const }] : []),
      ...(segment ? [{ label: segment, tone: "muted" as const }] : []),
    ],
    afterHeadHtml: crossLibraryContext,
    metaItems: [
      ...(originLibraryId
        ? [
            {
              label: "命中库",
              value: originLibrary ? `${libraryDisplayName(originLibrary)} (${originLibraryId})` : originLibraryId,
            },
          ]
        : []),
      { label: "对象编号", value: visualUnit.visual_unit_id },
      { label: "来源类型", value: sourceTypeDisplayName(visualUnit.source_type) },
      { label: "来源路径", value: visualUnit.source_path, valueClassName: "detail-path" },
    ],
    footerHtml: technicalInfo,
  });
}

export function renderSearchOutcome() {
  const library = selectedLibrary();

  if (!state.searchOutcome) {
    return "";
  }

  if (state.searchOutcome.error) {
    const details = state.searchOutcome.error.details?.content_types ?? [];
    return renderNotice({
      tone: "error",
      testId: "search-error-notice",
      eyebrow: "这次查询没有完成",
      title: state.searchOutcome.error.code,
      titleTestId: "search-error-code",
      body: state.searchOutcome.error.message,
      bodyTestId: "search-error-message",
      childrenHtml: `
        ${
          details.length
            ? `<p class="helper">部分内容类型当前没有完成准备或配置；这次失败不是“没有命中结果”，可以直接检查当前库覆盖。</p>`
            : ""
        }
        ${
          details.length
            ? `<ul class="data-list" data-testid="search-error-details">
                ${details
                  .map(
                    (item) => `
                      <li>
                        <strong>${escapeHtml(contentTypeDisplayName(item.content_type ?? "unknown"))}</strong>
                        <span>${escapeHtml(item.job?.job_id ?? "no-job")} · ${escapeHtml(item.job?.phase ?? item.status)}</span>
                      </li>
                    `
                  )
                  .join("")}
              </ul>`
            : ""
        }
      `,
      actionsHtml: details.length
        ? renderUiButton("前往当前库覆盖", {
            testId: "search-error-open-library-overrides",
            attrs: { "data-open-settings-section": "library-overrides" },
          })
        : "",
    });
  }

  const allResults = state.searchOutcome.results ?? [];
  const results = visibleSearchResults();
  const unsupportedContentTypes = state.searchOutcome.unsupported_content_types ?? [];
  const resultLibraryCount = new Set(allResults.map((item) => item.library_id).filter(Boolean)).size;
  const libraryBreakdown = searchResultLibraryBreakdown();
  const activeLibraryFocus = activeSearchResultLibraryFocus();
  const groupedResults = groupedSearchResults(results);
  const showLibraryGroupedResults =
    allLibrariesTextScopeActive() && !activeLibraryFocus && groupedResults.length > 1;
  const resultsSurfaceMode = showLibraryGroupedResults
    ? "grouped"
    : activeLibraryFocus
      ? "focused"
      : "default";
  if (!results.length) {
    return renderNotice({
      tone: "neutral",
      testId: "search-empty-notice",
      eyebrow: "这次查询没有命中",
      title: allLibrariesTextScopeActive()
        ? "当前范围可搜索，但本次没有返回结果"
        : "当前库可搜索，但本次没有返回结果",
      body: allLibrariesTextScopeActive()
        ? "可以换一个查询词、放宽过滤器，或确认当前范围里的相关内容已经导入到至少一个库。"
        : "可以换一个查询词、放宽过滤器，或确认当前范围里的相关内容已经导入并进入当前库。",
      childrenHtml: `
        ${
          unsupportedContentTypes.length
            ? `<p class="helper">另外有部分内容类型在这次查询里被跳过；如果这不是预期，可以检查当前库覆盖。</p>
               <ul class="data-list" data-testid="search-unsupported-content-types">
                ${unsupportedContentTypes
                  .map(
                    (item) => `
                      <li>
                        <strong>${escapeHtml(contentTypeDisplayName(item.content_type))}</strong>
                        <span>${escapeHtml(item.model)} · ${escapeHtml(item.reason)}</span>
                      </li>
                    `
                  )
                  .join("")}
              </ul>
              ${renderSearchStatusNextStep(library, "outcome")}`
            : ""
        }
      `,
    });
  }

  return `
    <div
      class="search-results-surface search-results-surface-${resultsSurfaceMode}"
      data-testid="search-results-surface"
      data-search-results-surface="${resultsSurfaceMode}"
    >
    ${
      unsupportedContentTypes.length
        ? renderNotice({
            tone: "warning",
            testId: "search-unsupported-content-types",
            title: "部分内容类型已跳过",
            childrenHtml: `<ul class="data-list">
              ${unsupportedContentTypes
                .map(
                  (item) => `
                    <li>
                      <strong>${escapeHtml(contentTypeDisplayName(item.content_type))}</strong>
                      <span>${escapeHtml(item.model)} · ${escapeHtml(item.reason)}</span>
                    </li>
                  `
                )
                .join("")}
            </ul>`,
          })
        : ""
    }
    <div class="results-summary">
      <h3 data-testid="search-results-summary">${
        activeLibraryFocus
          ? `当前查看 ${escapeHtml(activeLibraryFocus.label)} · ${results.length} 条结果`
          : `命中 ${results.length} 条结果${allLibrariesTextScopeActive() && resultLibraryCount ? ` · 来自 ${resultLibraryCount} 个库` : ""}`
      }</h3>
    </div>
    ${
      allLibrariesTextScopeActive() && libraryBreakdown.length
        ? `
          <section class="results-library-strip" data-testid="search-result-library-strip">
            <span class="scope-label">命中库分布</span>
            <div class="results-library-chips">
              ${renderUiButton(`全部结果 · ${allResults.length}`, {
                tone: "secondary",
                className: "result-library-chip",
                testId: "search-result-library-focus-all",
                attrs: { "data-search-result-library-focus": "" },
                selected: !activeLibraryFocus,
              })}
              ${libraryBreakdown
                .map(
                  (item) =>
                    renderUiButton(`${item.label} · ${item.count}`, {
                      tone: "secondary",
                      className: "result-library-chip",
                      testId: `search-result-library-focus-${item.libraryId}`,
                      attrs: { "data-search-result-library-focus": item.libraryId },
                      selected: activeLibraryFocus?.libraryId === item.libraryId,
                    })
                )
                .join("")}
            </div>
          </section>
        `
        : ""
    }
    ${
      showLibraryGroupedResults
        ? `
          <div class="result-library-groups" data-testid="search-result-library-groups">
            ${groupedResults.map((group) => renderSearchResultGroup(group)).join("")}
          </div>
        `
        : `
          <ul class="result-list" data-testid="result-list">
            ${results
              .map((item) =>
                renderSearchResultCard(item, activeLibraryFocus ? "focused" : "default")
              )
              .join("")}
          </ul>
        `
    }
    ${
      searchHasMoreResults()
        ? `
          <div class="results-footer">
            ${renderUiButton("加载更多", {
              tone: "secondary",
              id: "search-load-more-button",
              testId: "search-load-more-button",
            })}
          </div>
        `
        : ""
    }
    </div>
  `;
}

export function renderSearchControls(library, readingMode = false) {
  const queryPreview = queryImagePreviewUrl();
  const queryVideoPreview = queryVideoPreviewUrl();
  const queryDocumentPreview = queryDocumentPreviewUrl();
  const queryVideoDuration = state.queryVideoDurationMs;
  const queryVideoStartMs = currentQueryVideoStartMs();
  const queryVideoEndMs = currentQueryVideoEndMs();
  const hasAdvancedFilters =
    Boolean(state.searchFilters.pathPrefix.trim()) ||
    Boolean(state.searchFilters.timeRangeStartMsDraft.trim()) ||
    Boolean(state.searchFilters.timeRangeEndMsDraft.trim());
  const hasFilterSelections =
    Boolean(state.searchFilters.visualUnitKind) ||
    Boolean(state.searchFilters.sourceType) ||
    hasAdvancedFilters;
  const filterPanelOpen = state.searchFiltersPanelOpen || hasFilterSelections;
  const activeModeLabel = searchModeDisplayName(state.searchMode);
  const modeActionButtons = `
    <button
      type="button"
      class="ui-button ui-button-secondary ui-selection-control search-filter-button"
      id="search-filter-toggle-button"
      data-testid="search-filter-toggle-button"
      aria-expanded="${filterPanelOpen ? "true" : "false"}"
      data-ui-selected="${filterPanelOpen ? "true" : "false"}"
    >
      ${renderUiIcon("filter")}
      <span>过滤</span>
    </button>
    ${
      state.searchMode !== "text"
        ? `
          <button
            type="button"
            class="ui-button ui-button-secondary ui-selection-control search-mode-text-button"
            data-testid="search-mode-text"
            data-search-mode="text"
            data-ui-selected="false"
          >
            文本
          </button>
        `
        : ""
    }
    <button
      type="button"
      class="ui-button ui-button-secondary ui-selection-control search-mode-icon-button"
      data-testid="search-mode-image"
      data-search-mode="image"
      aria-label="图片查询"
      title="图片查询"
      data-ui-selected="${state.searchMode === "image" ? "true" : "false"}"
    >
      ${renderUiIcon("image")}
    </button>
    <button
      type="button"
      class="ui-button ui-button-secondary ui-selection-control search-mode-icon-button"
      data-testid="search-mode-video"
      data-search-mode="video"
      aria-label="视频查询"
      title="视频查询"
      data-ui-selected="${state.searchMode === "video" ? "true" : "false"}"
    >
      ${renderUiIcon("video")}
    </button>
    <button
      type="button"
      class="ui-button ui-button-secondary ui-selection-control search-mode-icon-button"
      data-testid="search-mode-document"
      data-search-mode="document"
      aria-label="文档查询"
      title="文档查询"
      data-ui-selected="${state.searchMode === "document" ? "true" : "false"}"
    >
      ${renderUiIcon("document")}
    </button>
  `;
  return `
    <form
      id="search-form"
      class="stack-form search-form ${readingMode ? "search-form-reading" : ""}"
      data-testid="search-form"
    >
      <div class="search-stage-card">
        <div class="search-composer-shell">
          <div class="search-composer-main ${state.searchMode === "text" ? "search-composer-main-text" : "search-composer-main-object"}">
            ${
              state.searchMode === "text"
                ? `
                  <label class="search-main-input query-text-card search-composer-input-shell">
                    <span class="search-input-row">
                      <span class="search-lens" aria-hidden="true"></span>
                      <input
                        id="search-text"
                        data-testid="search-text-input"
                        type="text"
                        value="${escapeHtml(state.searchTextDraft)}"
                        placeholder="Type, paste, or upload to search"
                        ${library ? "" : "disabled"}
                      />
                    </span>
                  </label>
                `
                : `
                  <div class="search-mode-copy">
                    ${renderTypeTag(`${activeModeLabel}查询`, "ready")}
                    <p class="helper">
                      ${
                        state.searchMode === "image"
                          ? "上传、粘贴或复用图片作为查询输入。"
                          : state.searchMode === "video"
                            ? "上传视频、选择库内视频源，或复用结果片段作为查询输入。"
                            : "上传 PDF 或复用结果页作为查询输入。"
                      }
                    </p>
                  </div>
                `
            }
          </div>
          <div class="search-composer-actions" data-testid="search-mode-switch">
            ${modeActionButtons}
          </div>
        </div>
        ${
          state.searchMode === "text"
            ? ""
            : state.searchMode === "image"
              ? `
            <div class="query-image-panel" data-testid="query-image-panel">
              <label class="query-source-field">
                <span>查询图片</span>
                <input
                  id="query-image-input"
                  data-testid="query-image-input"
                  type="file"
                  accept="image/*"
                  ${library ? "" : "disabled"}
                />
              </label>
              <div class="query-image-card query-surface-card" data-testid="query-image-card">
                <div class="job-meta query-surface-meta">
                  <div class="query-surface-copy">
                    ${renderStatusTag(queryImageStatusLabel(), state.queryImageAsset || state.queryImageLibraryObject ? "ready" : "muted")}
                    ${
                      queryImageDisplayName()
                        ? `<span class="helper query-surface-name">${escapeHtml(queryImageDisplayName())}</span>`
                        : `<span class="helper query-surface-placeholder">当前还没有查询图片。</span>`
                    }
                  </div>
                </div>
                <div class="query-preview-surface">
                  ${
                    queryPreview
                      ? isDocumentPageQueryImage()
                        ? `<iframe class="query-image-preview-frame" data-testid="query-image-preview" src="${escapeHtml(queryPreview)}" title="查询图片预览" loading="lazy"></iframe>`
                        : `<img class="query-image-preview" data-testid="query-image-preview" src="${escapeHtml(queryPreview)}" alt="查询图片预览" />`
                      : renderEmptyState("选择一张本地图片后，这里会显示查询图片预览。", { testId: "query-image-empty", className: "query-preview-empty" })
                  }
                </div>
                <div class="inline-actions query-surface-actions">
                  <button type="button" id="clear-query-image-button" data-testid="clear-query-image-button" class="ui-button ui-button-secondary" ${state.queryImageFile || state.queryImageAsset || state.queryImageLibraryObject ? "" : "disabled"}>清除</button>
                  ${
                    activeQueryImagePreview()
                      ? `<a data-testid="query-image-preview-link" href="${escapeHtml(activeQueryImagePreview().url)}" target="_blank" rel="noreferrer">打开查询图片预览</a>`
                      : ""
                  }
                </div>
                <button
                  type="button"
                  class="paste-target"
                  id="query-image-paste-target"
                  data-testid="query-image-paste-target"
                  ${library ? "" : "disabled"}
                >
                  点击这里后按 Ctrl/Cmd+V 粘贴图片
                </button>
              </div>
            </div>
          `
              : state.searchMode === "video"
                ? `
            <div class="query-video-panel" data-testid="query-video-panel">
              <label class="query-source-field">
                <span>查询视频</span>
                <input
                  id="query-video-input"
                  data-testid="query-video-input"
                  type="file"
                  accept="video/mp4,video/quicktime,video/x-m4v,video/*"
                  ${library ? "" : "disabled"}
                />
              </label>
              <label class="query-source-field">
                <span>或复用库内视频源</span>
                <select
                  id="query-video-source-select"
                  data-testid="query-video-source-select"
                  ${library && state.videoSources.length ? "" : "disabled"}
                >
                  <option value="">不使用库内视频源</option>
                  ${state.videoSources
                    .map(
                      (source) => `
                        <option
                          value="${escapeHtml(source.source_id)}"
                          ${state.queryVideoSource?.source_id === source.source_id ? "selected" : ""}
                        >
                          ${escapeHtml(sourceName(source.source_path))} (${escapeHtml(source.source_id)})
                        </option>
                      `
                    )
                    .join("")}
                </select>
              </label>
              <div class="query-video-card query-surface-card" data-testid="query-video-card">
                <div class="job-meta query-surface-meta">
                  <div class="query-surface-copy">
                    ${renderStatusTag(queryVideoStatusLabel(), state.queryVideoAsset || state.queryVideoSource || state.queryVideoLibraryObject ? "ready" : "muted")}
                    ${
                      queryVideoDisplayName()
                        ? `<span class="helper query-surface-name">${escapeHtml(queryVideoDisplayName())}</span>`
                        : `<span class="helper query-surface-placeholder">当前还没有查询视频。</span>`
                    }
                  </div>
                </div>
                <div class="query-preview-surface">
                  ${
                    queryVideoPreview
                      ? `<video
                          class="query-video-preview"
                          data-testid="query-video-preview"
                          src="${escapeHtml(queryVideoPreview)}"
                          controls
                          preload="metadata"
                        ></video>`
                      : renderEmptyState("选择一个本地视频或库内视频源后，这里会显示查询视频预览。", { testId: "query-video-empty", className: "query-preview-empty" })
                  }
                </div>
                <div class="query-range-card query-surface-subcard" data-testid="query-video-range-card">
                  <div class="job-meta">
                    <strong>时间范围</strong>
                    <span class="helper">${escapeHtml(queryVideoRangeSummary())}</span>
                  </div>
                  ${
                    state.queryVideoLibraryObject
                      ? `<p class="helper">当前使用库内 video_segment；查询范围固定为该片段自身的时间范围。</p>`
                      : queryVideoDuration
                        ? `
                          <div class="range-grid">
                            <label>
                              <span>开始时间</span>
                              <input
                                id="query-video-range-start"
                                data-testid="query-video-range-start"
                                type="range"
                                min="0"
                                max="${escapeHtml(Math.max(queryVideoDuration - 1, 0))}"
                                step="${escapeHtml(queryVideoRangeStep())}"
                                value="${escapeHtml(queryVideoStartMs)}"
                              />
                            </label>
                            <label>
                              <span>结束时间</span>
                              <input
                                id="query-video-range-end"
                                data-testid="query-video-range-end"
                                type="range"
                                min="1"
                                max="${escapeHtml(queryVideoDuration)}"
                                step="${escapeHtml(queryVideoRangeStep())}"
                                value="${escapeHtml(Math.max(queryVideoEndMs, 1))}"
                              />
                            </label>
                          </div>
                        `
                        : `<p class="helper">视频元数据加载后即可通过时间轴拖选查询片段；不拖选时默认整段视频。</p>`
                  }
                  <div class="inline-actions">
                    <button
                      type="button"
                      id="clear-query-video-range-button"
                      data-testid="clear-query-video-range-button"
                      class="ui-button ui-button-secondary"
                      ${queryVideoDuration && state.queryVideoRange && !state.queryVideoLibraryObject ? "" : "disabled"}
                    >
                      整段视频
                    </button>
                  </div>
                </div>
                <div class="inline-actions query-surface-actions">
                  <button
                    type="button"
                    id="clear-query-video-button"
                    data-testid="clear-query-video-button"
                    class="ui-button ui-button-secondary"
                    ${state.queryVideoFile || state.queryVideoAsset || state.queryVideoSource || state.queryVideoLibraryObject ? "" : "disabled"}
                  >
                    清除
                  </button>
                  ${
                    activeQueryVideoPreview()
                      ? `<a data-testid="query-video-preview-link" href="${escapeHtml(activeQueryVideoPreview().url)}" target="_blank" rel="noreferrer">打开查询视频预览</a>`
                      : ""
                  }
                </div>
              </div>
            </div>
          `
                : `
            <div class="query-document-panel" data-testid="query-document-panel">
              <label class="query-source-field">
                <span>查询文档</span>
                <input
                  id="query-document-input"
                  data-testid="query-document-input"
                  type="file"
                  accept="application/pdf,.pdf"
                  ${library ? "" : "disabled"}
                />
              </label>
              <div class="query-document-card query-surface-card" data-testid="query-document-card">
                <div class="job-meta query-surface-meta">
                  <div class="query-surface-copy">
                    ${renderStatusTag(queryDocumentStatusLabel(), state.queryDocumentAsset || state.queryDocumentLibraryObject ? "ready" : "muted")}
                    ${
                      queryDocumentDisplayName()
                        ? `<span class="helper query-surface-name">${escapeHtml(queryDocumentDisplayName())}</span>`
                        : `<span class="helper query-surface-placeholder">当前还没有查询文档。</span>`
                    }
                  </div>
                </div>
                <div class="query-preview-surface">
                  ${
                    queryDocumentPreview
                      ? `<iframe class="query-document-preview-frame" data-testid="query-document-preview" src="${escapeHtml(queryDocumentPreview)}" title="查询文档预览" loading="lazy"></iframe>`
                      : renderEmptyState("选择一个本地 PDF 或从结果复用 document_page 后，这里会显示查询文档预览。", { testId: "query-document-empty", className: "query-preview-empty" })
                  }
                </div>
                <div class="query-range-card query-surface-subcard" data-testid="query-document-range-card">
                  <div class="job-meta">
                    <strong>页范围</strong>
                    <span class="helper" id="query-document-range-summary">${escapeHtml(queryDocumentRangeSummary())}</span>
                  </div>
                  ${
                    state.queryDocumentLibraryObject
                      ? `<p class="helper">当前使用库内 document_page；查询范围固定为该页面对应的单页范围。</p>`
                      : `
                          <div class="range-grid range-grid-pages">
                            <label>
                              <span>起始页</span>
                              <input
                                id="query-document-range-start"
                                data-testid="query-document-range-start"
                                type="number"
                                inputmode="numeric"
                                min="1"
                                step="1"
                                value="${escapeHtml(currentQueryDocumentStartPage())}"
                                placeholder="留空表示整份文档"
                              />
                            </label>
                            <label>
                              <span>结束页</span>
                              <input
                                id="query-document-range-end"
                                data-testid="query-document-range-end"
                                type="number"
                                inputmode="numeric"
                                min="1"
                                step="1"
                                value="${escapeHtml(currentQueryDocumentEndPage())}"
                                placeholder="只填起始页表示单页"
                              />
                            </label>
                          </div>
                        `
                  }
                  <div class="inline-actions">
                    <button
                      type="button"
                      id="clear-query-document-range-button"
                      data-testid="clear-query-document-range-button"
                      class="ui-button ui-button-secondary"
                      ${!state.queryDocumentLibraryObject && (state.queryDocumentStartPageDraft || state.queryDocumentEndPageDraft) ? "" : "disabled"}
                    >
                      整份文档
                    </button>
                  </div>
                </div>
                <div class="inline-actions query-surface-actions">
                  <button
                    type="button"
                    id="clear-query-document-button"
                    data-testid="clear-query-document-button"
                    class="ui-button ui-button-secondary"
                    ${state.queryDocumentFile || state.queryDocumentAsset || state.queryDocumentLibraryObject ? "" : "disabled"}
                  >
                    清除
                  </button>
                  ${
                    activeQueryDocumentPreview()
                      ? `<a data-testid="query-document-preview-link" href="${escapeHtml(activeQueryDocumentPreview().url)}" target="_blank" rel="noreferrer">打开查询文档预览</a>`
                      : ""
                  }
                </div>
              </div>
            </div>
          `
        }
      </div>
      ${
        filterPanelOpen
          ? `
            <section class="search-filter-panel" data-testid="search-filter-dock">
              <div class="search-common-filters">
                <label>
                  <span>视觉对象类型</span>
                  <select id="search-filter-kind" data-testid="search-filter-kind" ${library ? "" : "disabled"}>
                    <option value="">全部</option>
                    <option value="image" ${state.searchFilters.visualUnitKind === "image" ? "selected" : ""}>图片</option>
                    <option value="document_page" ${state.searchFilters.visualUnitKind === "document_page" ? "selected" : ""}>文档页</option>
                    <option value="video_segment" ${state.searchFilters.visualUnitKind === "video_segment" ? "selected" : ""}>视频片段</option>
                  </select>
                </label>
                <label>
                  <span>来源类型</span>
                  <select id="search-filter-source-type" data-testid="search-filter-source-type" ${library ? "" : "disabled"}>
                    <option value="">全部</option>
                    <option value="image" ${state.searchFilters.sourceType === "image" ? "selected" : ""}>图片</option>
                    <option value="pdf" ${state.searchFilters.sourceType === "pdf" ? "selected" : ""}>PDF</option>
                    <option value="video" ${state.searchFilters.sourceType === "video" ? "selected" : ""}>视频</option>
                  </select>
                </label>
              </div>
              <div class="search-advanced-grid">
                <label>
                  <span>路径前缀</span>
                  <input
                    id="search-filter-path-prefix"
                    data-testid="search-filter-path-prefix"
                    type="text"
                    value="${escapeHtml(state.searchFilters.pathPrefix)}"
                    placeholder="/abs/path/prefix"
                    ${library ? "" : "disabled"}
                  />
                </label>
                <label>
                  <span>起始时间（ms）</span>
                  <input
                    id="search-filter-time-range-start"
                    data-testid="search-filter-time-range-start"
                    type="number"
                    inputmode="numeric"
                    min="0"
                    step="1"
                    value="${escapeHtml(state.searchFilters.timeRangeStartMsDraft)}"
                    placeholder="仅作用于视频时间命中"
                    ${library ? "" : "disabled"}
                  />
                </label>
                <label>
                  <span>结束时间（ms）</span>
                  <input
                    id="search-filter-time-range-end"
                    data-testid="search-filter-time-range-end"
                    type="number"
                    inputmode="numeric"
                    min="0"
                    step="1"
                    value="${escapeHtml(state.searchFilters.timeRangeEndMsDraft)}"
                    placeholder="仅作用于视频时间命中"
                    ${library ? "" : "disabled"}
                  />
                </label>
                <div class="inline-actions">
                  <button
                    type="button"
                    id="clear-search-filters-button"
                    data-testid="clear-search-filters-button"
                    class="ui-button ui-button-secondary"
                    ${library ? "" : "disabled"}
                  >
                    清除过滤器
                  </button>
                </div>
              </div>
            </section>
          `
          : ""
      }
      ${renderSearchStateStrip(library)}
    </form>
  `;
}
