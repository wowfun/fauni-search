import {
  escapeHtml,
  inventoryDetailSheetIsOpen,
  pageLabel,
  renderUiIcon,
  selectedInventoryRepresentativePreview,
  selectedInventoryRepresentativeAsset,
  selectedInventorySource,
  sourceName,
  sourceRootInventoryLabel,
  sourceStatusDisplayName,
  sourceStatusPillClass,
  sourceTypeDisplayName,
  state,
  videoLabel,
  assetTypeDisplayName,
  type LibrarySnapshot,
  type SourceInventoryItem,
} from "../core";
import { renderDetailCard } from "../render/shared/detail";
import { renderLibraryContext } from "../render/shared/library-context";
import { renderObjectListItem } from "../render/shared/list-item";
import { renderPreviewSurface } from "../render/shared/preview";
import {
  renderEmptyState,
  renderLocatorTag,
  renderStatusTag,
  renderTypeTag,
  renderUiButton,
  renderUiTag,
} from "../render/shared/primitives";
import { renderInventoryActionRow } from "../render/shared/inventory";
import { renderImportReceipt } from "../render/shared/status";
import {
  renderSourceRootEditorForm,
  renderSourceRootList,
  renderSourceRootManagementSummary,
} from "../render/shared/source-roots";

export function renderInventorySummaryBar() {
  const summaryItems = [
    { label: "来源记录", value: state.inventorySummary.total, testId: "inventory-summary-total" },
    { label: "正常", value: state.inventorySummary.active, testId: "inventory-summary-active" },
    {
      label: "已失效",
      value: state.inventorySummary.invalidated,
      testId: "inventory-summary-invalidated",
    },
    {
      label: "超出范围",
      value: state.inventorySummary.out_of_scope,
      testId: "inventory-summary-out-of-scope",
    },
  ];

  return `
    <div class="inventory-summary-bar" data-testid="inventory-summary">
      ${summaryItems
        .map(
          (item) => `
            <article class="inventory-summary-card" data-testid="${item.testId}">
              <span class="inventory-summary-label">${escapeHtml(item.label)}</span>
              <strong class="inventory-summary-value">${escapeHtml(item.value)}</strong>
            </article>
          `
        )
        .join("")}
    </div>
  `;
}

export function inventoryFilterSummaryItems() {
  const items = [];
  if (state.inventoryFilters.sourceRootId) {
    if (state.inventoryFilters.sourceRootId === "manual") {
      items.push("手动导入");
    } else {
      const sourceRoot = state.sourceRoots.find(
        (item) => item.source_root_id === state.inventoryFilters.sourceRootId
      );
      items.push(sourceRoot?.root_path ?? state.inventoryFilters.sourceRootId);
    }
  }
  if (state.inventoryFilters.sourceType) {
    items.push(sourceTypeDisplayName(state.inventoryFilters.sourceType));
  }
  if (state.inventoryFilters.sourceStatus) {
    items.push(sourceStatusDisplayName(state.inventoryFilters.sourceStatus));
  }
  return items;
}

export function renderInventoryImportPanel(library: LibrarySnapshot | null) {
  if (!state.inventoryImportOpen) {
    return "";
  }

  return `
    <section class="inventory-import-panel" data-testid="inventory-import-panel">
      <div class="inventory-import-head">
        <div>
          <p class="eyebrow">导入</p>
          <h3>导入路径</h3>
        </div>
      </div>
      <form id="import-form" class="stack-form inventory-import-form" data-testid="import-form">
        <label>
          <span>本地路径</span>
          <textarea
            id="import-paths"
            data-testid="import-paths-input"
            rows="5"
            placeholder="/path/to/file.pdf&#10;/path/to/image.png"
            ${library ? "" : "disabled"}
          >${escapeHtml(state.importPathsDraft)}</textarea>
        </label>
        ${renderUiButton("提交导入", { type: "submit", testId: "import-submit-button", disabled: !library })}
      </form>
      ${renderImportReceipt()}
    </section>
  `;
}

export function inventoryRepresentativeKind(source: SourceInventoryItem) {
  return source.representative_asset?.asset_type ?? source.kind;
}

export function inventoryRepresentativeSourceType(source: SourceInventoryItem) {
  return source.representative_asset?.source_type ?? source.source_type;
}

export function inventoryRepresentativeKindIcon(source: SourceInventoryItem) {
  const kind = inventoryRepresentativeKind(source);
  if (kind === "video_segment") {
    return "video";
  }
  if (kind === "document_page") {
    return "document";
  }
  return "image";
}

export function renderInventorySourceThumbnail(source: SourceInventoryItem) {
  const kind = inventoryRepresentativeKind(source);
  const preview = source.representative_preview;
  if (kind === "image" && preview?.url) {
    return `
      <img
        class="inventory-source-thumbnail"
        src="${escapeHtml(preview.url)}"
        alt="${escapeHtml(sourceName(source.source_uri))}"
        loading="lazy"
      />
    `;
  }

  return `
    <div class="inventory-source-thumbnail inventory-source-thumbnail-placeholder">
      ${renderUiIcon(inventoryRepresentativeKindIcon(source))}
      <span>${escapeHtml(assetTypeDisplayName(kind))}</span>
    </div>
  `;
}

export function renderInventoryDetailPanel(library: LibrarySnapshot | null) {
  const source = selectedInventorySource();
  const mobileSheetOpen = inventoryDetailSheetIsOpen();
  const mobileSheetClass = mobileSheetOpen ? "mobile-sheet-open" : "mobile-sheet-closed";
  const mobileSheetBackdrop = mobileSheetOpen
    ? `<button
        type="button"
        class="mobile-sheet-backdrop"
        data-testid="inventory-detail-sheet-backdrop"
        data-mobile-sheet-close="inventory"
        aria-label="关闭来源详情"
      ></button>`
    : "";

  if (!library) {
    return `
      ${mobileSheetBackdrop}
      <section
        class="panel inventory-detail-panel mobile-sheet-panel ${mobileSheetClass}"
        data-testid="inventory-detail-panel"
      >
        <div class="mobile-sheet-bar">
          <span class="mobile-sheet-handle" aria-hidden="true"></span>
          <button
            type="button"
            class="ui-button ui-button-secondary mobile-sheet-close"
            data-testid="inventory-detail-sheet-close-button"
            data-mobile-sheet-close="inventory"
          >
            关闭
          </button>
        </div>
        <div class="panel-head">
          <div>
            <p class="eyebrow">详情</p>
            <h2>来源详情</h2>
          </div>
        </div>
        ${renderEmptyState("先选择一个库，再浏览来源。")}
      </section>
    `;
  }

  if (!source) {
    return `
      ${mobileSheetBackdrop}
      <section
        class="panel inventory-detail-panel mobile-sheet-panel ${mobileSheetClass}"
        data-testid="inventory-detail-panel"
      >
        <div class="mobile-sheet-bar">
          <span class="mobile-sheet-handle" aria-hidden="true"></span>
          <button
            type="button"
            class="ui-button ui-button-secondary mobile-sheet-close"
            data-testid="inventory-detail-sheet-close-button"
            data-mobile-sheet-close="inventory"
          >
            关闭
          </button>
        </div>
        <div class="panel-head">
          <div>
            <p class="eyebrow">详情</p>
            <h2>来源详情</h2>
          </div>
        </div>
        ${renderEmptyState("从左侧列表选中一个来源后，这里会显示它的状态、归属和可搜索单元摘要。")}
      </section>
    `;
  }

  const representativeVisual = selectedInventoryRepresentativeAsset(source);
  const representativePreview = selectedInventoryRepresentativePreview(source);
  const page = pageLabel(representativeVisual?.locator);
  const segment = videoLabel(representativeVisual?.locator);
  const pageChip = page ? (page.startsWith("P") ? page : `第 ${page} 页`) : "";

  return `
    ${mobileSheetBackdrop}
    <section
      class="panel inventory-detail-panel mobile-sheet-panel ${mobileSheetClass}"
      data-testid="inventory-detail-panel"
    >
      <div class="mobile-sheet-bar">
        <span class="mobile-sheet-handle" aria-hidden="true"></span>
        <button
          type="button"
          class="ui-button ui-button-secondary mobile-sheet-close"
          data-testid="inventory-detail-sheet-close-button"
          data-mobile-sheet-close="inventory"
        >
          关闭
        </button>
      </div>
      <div class="panel-head">
        <div>
          <p class="eyebrow">详情</p>
          <h2>${escapeHtml(sourceName(source.source_uri))}</h2>
        </div>
      </div>
      ${renderDetailCard({
        testId: "inventory-detail-card",
        className: "inventory-detail-shell",
        previewClassName: "inventory-preview-shell",
        title: sourceName(source.source_uri),
        previewHtml:
          representativeVisual && representativePreview
            ? renderPreviewSurface(
                {
                  ...representativeVisual,
                  source_id: source.source_id,
                  source_uri: source.source_uri,
                },
                representativePreview,
                "inventory-detail-preview"
              )
            : `
              <div class="preview-placeholder" data-testid="inventory-detail-preview">
                <p>当前来源还没有可用的代表性预览。完成一次 refresh / rescan 后，这里会优先显示图像、页预览或视频片段。</p>
              </div>
            `,
        tags: [
          { label: sourceTypeDisplayName(source.source_type), tone: "muted" },
          ...(pageChip ? [{ label: pageChip, tone: "muted" as const }] : []),
          ...(segment ? [{ label: segment, tone: "muted" as const }] : []),
        ],
        actionsHtml: `
          ${
            representativePreview
              ? `<a class="ui-button ui-button-secondary" data-testid="inventory-preview-link" href="${escapeHtml(representativePreview.url)}" target="_blank" rel="noreferrer">打开预览</a>`
              : ""
          }
          ${
            representativeVisual &&
            (representativeVisual.asset_type === "image" || representativeVisual.asset_type === "document_page")
              ? renderUiButton("作为查询图片", {
                  tone: "secondary",
                  testId: "inventory-use-as-query-image-button",
                  attrs: { "data-use-query-asset-id": representativeVisual.asset_id },
                })
              : ""
          }
          ${
            representativeVisual && representativeVisual.asset_type === "document_page"
              ? renderUiButton("作为查询文档", {
                  tone: "secondary",
                  testId: "inventory-use-as-query-document-button",
                  attrs: { "data-use-query-document-asset-id": representativeVisual.asset_id },
                })
              : ""
          }
          ${
            representativeVisual && representativeVisual.asset_type === "video_segment"
              ? renderUiButton("作为查询视频", {
                  tone: "secondary",
                  testId: "inventory-use-as-query-video-button",
                  attrs: { "data-use-query-video-asset-id": representativeVisual.asset_id },
                })
              : ""
          }
        `,
        metaItems: [
          { label: "可搜索对象", value: source.asset_count },
          { label: "来源 URI", value: source.source_uri, valueClassName: "detail-path" },
          { label: "来源根", value: sourceRootInventoryLabel(source) },
          { label: "来源编号", value: source.source_id },
        ],
      })}
    </section>
  `;
}

export function renderInventoryWorkspace(library: LibrarySnapshot | null) {
  const filterSummaryItems = inventoryFilterSummaryItems();
  const sourceManagementBody = state.inventorySourceManagementOpen
    ? `
        <div class="inventory-source-management-panel" data-testid="inventory-source-management-panel">
          <div class="inventory-source-management-head">
            <div>
              <p class="eyebrow">来源根</p>
              <h3>管理来源根</h3>
            </div>
            ${renderUiButton("新建来源根", {
              tone: "secondary",
              testId: "inventory-source-root-create-button",
              attrs: { "data-inventory-source-root-create": true },
              disabled: !library,
            })}
          </div>
          ${
            state.inventorySourceRootEditorOpen
              ? renderSourceRootEditorForm(library, { variant: "inventory" })
              : ""
          }
          ${renderSourceRootList()}
        </div>
      `
    : "";
  const list = state.librarySources.length
    ? `
        <ul class="inventory-source-list" data-testid="library-source-list">
          ${state.librarySources
            .map(
              (source) =>
                renderObjectListItem({
                  testId: "library-source-card",
                  className: "inventory-source-row",
                  active: source.source_id === state.selectedInventorySourceId,
                  dataAttrs: { "data-source-id": source.source_id },
                  selectClassName: "inventory-source-select",
                  selectAttrs: { "data-source-id": source.source_id },
                  visualClassName: "inventory-source-visual",
                  visualHtml: renderInventorySourceThumbnail(source),
                  bodyClassName: "inventory-source-main",
                  title: sourceName(source.source_uri),
                  titleClassName: "inventory-source-name",
                  metaHtml: `<p class="helper inventory-source-path">${escapeHtml(source.source_uri)}</p>`,
                  trailingClassName: "inventory-source-meta",
                  trailingHtml: `
                    ${
                      source.status !== "active" || source.status_reason
                        ? renderStatusTag(sourceStatusDisplayName(source.status), sourceStatusPillClass(source.status) as any)
                        : ""
                    }
                    <strong class="inventory-source-count">${escapeHtml(source.asset_count)} 个对象</strong>
                    ${
                      source.status_reason
                        ? `<span class="helper inventory-source-reason">${escapeHtml(source.status_reason)}</span>`
                        : ""
                    }
                  `,
                })
            )
            .join("")}
        </ul>
      `
    : renderEmptyState("当前筛选条件下没有来源内容。", { testId: "library-source-empty" });

  return `
    <section class="inventory-workspace" data-testid="inventory-panel">
      ${renderLibraryContext({
        library,
        variant: "workspace-toolbar",
        capabilities: {
          showNameEditor: true,
          showLibraryActions: true,
          showMetrics: true,
        },
      })}
      <div class="inventory-layout">
        <section class="panel inventory-panel inventory-panel-main">
          <div class="inventory-filter-dock inventory-source-controls" data-testid="inventory-source-controls">
            <div class="inventory-filter-head">
              <div>
                <p class="eyebrow">来源与过滤</p>
                <h3>筛选当前库来源</h3>
              </div>
              ${renderInventoryActionRow(library)}
            </div>
            <div class="ui-filter-bar inventory-filter-grid">
              <label>
                <span>来源根</span>
                <select id="source-filter-root" data-testid="source-filter-root" ${library ? "" : "disabled"}>
                  <option value="">全部来源根</option>
                  <option value="manual" ${state.inventoryFilters.sourceRootId === "manual" ? "selected" : ""}>手动导入</option>
                  ${state.sourceRoots
                    .map(
                      (sourceRoot) => `
                        <option value="${escapeHtml(sourceRoot.source_root_id)}" ${state.inventoryFilters.sourceRootId === sourceRoot.source_root_id ? "selected" : ""}>
                          ${escapeHtml(sourceRoot.root_path)}
                        </option>
                      `
                    )
                    .join("")}
                </select>
              </label>
              <label>
                <span>来源类型</span>
                <select id="source-filter-type" data-testid="source-filter-type" ${library ? "" : "disabled"}>
                  <option value="">全部类型</option>
                  <option value="image" ${state.inventoryFilters.sourceType === "image" ? "selected" : ""}>图片</option>
                  <option value="pdf" ${state.inventoryFilters.sourceType === "pdf" ? "selected" : ""}>PDF</option>
                  <option value="video" ${state.inventoryFilters.sourceType === "video" ? "selected" : ""}>视频</option>
                </select>
              </label>
              <label>
                <span>来源状态</span>
                <select id="source-filter-status" data-testid="source-filter-status" ${library ? "" : "disabled"}>
                  <option value="">全部状态</option>
                  <option value="active" ${state.inventoryFilters.sourceStatus === "active" ? "selected" : ""}>正常</option>
                  <option value="invalidated" ${state.inventoryFilters.sourceStatus === "invalidated" ? "selected" : ""}>已失效</option>
                  <option value="out_of_scope" ${state.inventoryFilters.sourceStatus === "out_of_scope" ? "selected" : ""}>超出范围</option>
                </select>
              </label>
            </div>
            <p class="helper" data-testid="inventory-filter-summary">
              当前显示 ${state.librarySources.length} / ${state.inventorySummary.total} 条来源记录。
            </p>
            <div class="ui-tag-row inventory-filter-tags" data-testid="inventory-filter-pills">
              ${
                filterSummaryItems.length
                  ? filterSummaryItems
                      .map((item) => renderUiTag(item, "muted"))
                      .join("")
                  : renderUiTag("当前显示全部来源", "ready")
              }
            </div>
            ${renderInventoryImportPanel(library)}
            <section class="inventory-source-management-strip" data-testid="inventory-source-management-strip">
              <div class="inventory-source-management-summary" data-testid="inventory-source-management-summary">
                <div>
                  <p class="eyebrow">来源根管理</p>
                  <p class="helper">${escapeHtml(renderSourceRootManagementSummary())}</p>
                </div>
                ${renderUiTag(state.inventorySourceManagementOpen ? "展开中" : "已折叠", state.inventorySourceManagementOpen ? "ready" : "muted")}
              </div>
              ${sourceManagementBody}
            </section>
          </div>
          ${list}
        </section>
        ${renderInventoryDetailPanel(library)}
      </div>
    </section>
  `;
}

export function renderLibrarySourcesPanel(library) {
  return renderInventoryWorkspace(library);
}
