import "./style.css";

function requireEnv(name) {
  const value = import.meta.env[name];
  if (!value) {
    throw new Error(`Missing required environment variable ${name}`);
  }
  return value;
}

const endpoints = {
  appHealth: `http://${requireEnv("APP_HOST")}:${requireEnv("APP_PORT")}/health`,
  sidecarHealth: `http://${requireEnv("SIDECAR_HOST")}:${requireEnv("SIDECAR_PORT")}/health`,
  qdrantCollections: `${requireEnv("QDRANT_URL").replace(/\/$/, "")}/collections`,
  uiRoot: `http://${requireEnv("UI_HOST")}:${requireEnv("UI_PORT")}/`,
};

const demoFixture = {
  path: "tests/fixtures/tatdqa-page-images/images/tatdqa-page-0001.png",
  query: "What is the percentage change in the net cash provided from operating activities?",
};

const JOB_POLL_INTERVAL_MS = 1000;
const JOB_POLL_TIMEOUT_MS = 5 * 60 * 1000;

const state = {
  libraries: [],
  jobs: [],
  videoSources: [],
  selectedLibraryId: "",
  importPathsDraft: "",
  searchMode: "text",
  searchTextDraft: "",
  queryImageFile: null,
  queryImageObjectUrl: null,
  queryImageAsset: null,
  queryImageLibraryObject: null,
  queryVideoFile: null,
  queryVideoObjectUrl: null,
  queryVideoAsset: null,
  queryVideoSource: null,
  queryVideoLibraryObject: null,
  queryVideoDurationMs: null,
  queryVideoRange: null,
  queryDocumentFile: null,
  queryDocumentObjectUrl: null,
  queryDocumentAsset: null,
  queryDocumentLibraryObject: null,
  queryDocumentPageCount: null,
  queryDocumentStartPageDraft: "",
  queryDocumentEndPageDraft: "",
  importReceipt: null,
  selectedVisualUnit: null,
  searchOutcome: null,
  globalError: null,
  statusMessage: null,
};

const EDITABLE_TARGET_SELECTOR = 'input, textarea, [contenteditable="true"], [contenteditable=""], select';

const root = document.querySelector("#app");

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function selectedLibrary() {
  return state.libraries.find((library) => library.id === state.selectedLibraryId) ?? null;
}

function sleep(ms) {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function isTerminalJobStatus(status) {
  return ["completed", "failed", "canceled"].includes(status);
}

function jobPillClass(status) {
  if (status === "completed") {
    return "ready";
  }
  if (status === "failed" || status === "canceled") {
    return "error";
  }
  if (status === "queued" || status === "running") {
    return "pending";
  }
  return "muted";
}

function formatIndexLines(indexLines) {
  if (!indexLines.length) {
    return '<span class="pill muted">No index lines</span>';
  }

  return indexLines
    .map(
      (line) =>
        `<span class="pill ${line.status === "ready" ? "ready" : "pending"}">${escapeHtml(line.index_line)} · ${escapeHtml(line.status)}</span>`
    )
    .join("");
}

function selectedVisualUnitId() {
  return state.selectedVisualUnit?.visual_unit?.visual_unit_id ?? null;
}

function sourceName(path) {
  return String(path).split(/[/\\]/).pop() ?? path;
}

function pageLabel(locator) {
  return locator?.page_label ?? (locator?.page ? `P${locator.page}` : null);
}

function videoLabel(locator) {
  if (typeof locator?.start_ms !== "number" || typeof locator?.end_ms !== "number") {
    return null;
  }
  return `${formatDurationMs(locator.start_ms)} → ${formatDurationMs(locator.end_ms)}`;
}

function formatScore(score) {
  if (typeof score !== "number" || Number.isNaN(score)) {
    return null;
  }
  return score.toFixed(4);
}

function clearQueryImageState() {
  if (state.queryImageObjectUrl) {
    URL.revokeObjectURL(state.queryImageObjectUrl);
  }
  state.queryImageFile = null;
  state.queryImageObjectUrl = null;
  state.queryImageAsset = null;
  state.queryImageLibraryObject = null;
}

function normalizeQueryImageFile(file, fallbackName = "pasted-image.png") {
  if (file.name && file.name.trim()) {
    return file;
  }

  return new File([file], fallbackName, {
    type: file.type || "image/png",
    lastModified: Date.now(),
  });
}

function setPendingQueryImageFile(file) {
  clearQueryImageState();
  state.queryImageFile = normalizeQueryImageFile(file);
  state.queryImageObjectUrl = URL.createObjectURL(state.queryImageFile);
}

function clearQueryVideoState() {
  if (state.queryVideoObjectUrl) {
    URL.revokeObjectURL(state.queryVideoObjectUrl);
  }
  state.queryVideoFile = null;
  state.queryVideoObjectUrl = null;
  state.queryVideoAsset = null;
  state.queryVideoSource = null;
  state.queryVideoLibraryObject = null;
  state.queryVideoDurationMs = null;
  state.queryVideoRange = null;
}

function normalizeQueryVideoFile(file, fallbackName = "query-video.mp4") {
  if (file.name && file.name.trim()) {
    return file;
  }

  return new File([file], fallbackName, {
    type: file.type || "video/mp4",
    lastModified: Date.now(),
  });
}

function setQueryVideoDuration(durationMs) {
  if (typeof durationMs !== "number" || Number.isNaN(durationMs) || durationMs <= 0) {
    return;
  }

  const normalizedDurationMs = Math.max(Math.round(durationMs), 1);
  state.queryVideoDurationMs = normalizedDurationMs;
  if (!state.queryVideoRange) {
    return;
  }

  const startMs = Math.max(
    0,
    Math.min(state.queryVideoRange.start_ms ?? 0, normalizedDurationMs - 1)
  );
  const endMs = Math.max(
    startMs + 1,
    Math.min(state.queryVideoRange.end_ms ?? normalizedDurationMs, normalizedDurationMs)
  );

  if (startMs === 0 && endMs === normalizedDurationMs) {
    state.queryVideoRange = null;
    return;
  }

  state.queryVideoRange = {
    start_ms: startMs,
    end_ms: endMs,
  };
}

function setPendingQueryVideoFile(file) {
  clearQueryVideoState();
  state.queryVideoFile = normalizeQueryVideoFile(file);
  state.queryVideoObjectUrl = URL.createObjectURL(state.queryVideoFile);
}

function clearQueryDocumentState() {
  if (state.queryDocumentObjectUrl) {
    URL.revokeObjectURL(state.queryDocumentObjectUrl);
  }
  state.queryDocumentFile = null;
  state.queryDocumentObjectUrl = null;
  state.queryDocumentAsset = null;
  state.queryDocumentLibraryObject = null;
  state.queryDocumentPageCount = null;
  state.queryDocumentStartPageDraft = "";
  state.queryDocumentEndPageDraft = "";
}

function normalizeQueryDocumentFile(file, fallbackName = "query-document.pdf") {
  if (file.name && file.name.trim()) {
    return file;
  }

  return new File([file], fallbackName, {
    type: file.type || "application/pdf",
    lastModified: Date.now(),
  });
}

function setQueryDocumentPageCount(pageCount) {
  if (typeof pageCount === "number" && Number.isFinite(pageCount) && pageCount > 0) {
    state.queryDocumentPageCount = Math.max(1, Math.round(pageCount));
    return;
  }
  state.queryDocumentPageCount = null;
}

function setPendingQueryDocumentFile(file) {
  clearQueryDocumentState();
  state.queryDocumentFile = normalizeQueryDocumentFile(file);
  state.queryDocumentObjectUrl = URL.createObjectURL(state.queryDocumentFile);
}

function setLibraryQueryDocumentVisualUnit(visualUnit) {
  clearQueryDocumentState();
  const page = Number(visualUnit?.locator?.page ?? 0);
  state.queryDocumentLibraryObject = {
    ...visualUnit,
    locator:
      page > 0
        ? {
            start_page: page,
            end_page: page,
          }
        : null,
  };
}

function setLibraryQueryVideoSource(source) {
  clearQueryVideoState();
  state.queryVideoSource = source;
  setQueryVideoDuration(source?.duration_ms ?? null);
}

function setLibraryQueryVideoVisualUnit(visualUnit) {
  clearQueryVideoState();
  state.queryVideoLibraryObject = visualUnit;
  setQueryVideoDuration(
    visualUnit?.locator?.duration_ms ??
      (typeof visualUnit?.locator?.end_ms === "number" ? visualUnit.locator.end_ms : null)
  );
}

function probeVideoDurationFromUrl(url) {
  return new Promise((resolve, reject) => {
    const video = document.createElement("video");
    video.preload = "metadata";
    video.src = url;
    video.onloadedmetadata = () => {
      if (!Number.isFinite(video.duration) || video.duration <= 0) {
        reject(new Error("video_duration_unavailable"));
        return;
      }
      resolve(Math.round(video.duration * 1000));
    };
    video.onerror = () => reject(new Error("video_metadata_load_failed"));
  });
}

function firstClipboardImageFile(clipboardData) {
  if (!clipboardData) {
    return null;
  }

  const fileList = Array.from(clipboardData.files ?? []);
  const directFile = fileList.find((file) => file?.type?.startsWith("image/"));
  if (directFile) {
    return directFile;
  }

  for (const item of Array.from(clipboardData.items ?? [])) {
    if (item.kind === "file" && item.type?.startsWith("image/")) {
      const file = item.getAsFile();
      if (file) {
        return file;
      }
    }
  }

  return null;
}

function formatDurationMs(durationMs) {
  if (typeof durationMs !== "number" || Number.isNaN(durationMs) || durationMs < 0) {
    return null;
  }

  const totalMs = Math.round(durationMs);
  const hours = Math.floor(totalMs / 3_600_000);
  const minutes = Math.floor((totalMs % 3_600_000) / 60_000);
  const seconds = Math.floor((totalMs % 60_000) / 1000);
  const milliseconds = totalMs % 1000;
  const mm = String(minutes).padStart(hours ? 2 : 1, "0");
  const ss = String(seconds).padStart(2, "0");
  const mmm = String(milliseconds).padStart(3, "0");

  if (hours) {
    return `${hours}:${mm}:${ss}.${mmm}`;
  }
  return `${minutes}:${ss}.${mmm}`;
}

function queryImagePreviewUrl() {
  return (
    state.queryImageObjectUrl ??
    state.queryImageAsset?.preview?.url ??
    state.queryImageLibraryObject?.preview?.url ??
    null
  );
}

function queryImageStatusLabel() {
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

function queryImageDisplayName() {
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

function activeQueryImagePreview() {
  return state.queryImageAsset?.preview ?? state.queryImageLibraryObject?.preview ?? null;
}

function isDocumentPageQueryImage() {
  return state.queryImageLibraryObject?.kind === "document_page";
}

function queryVideoPreviewUrl() {
  return (
    state.queryVideoObjectUrl ??
    state.queryVideoAsset?.preview?.url ??
    state.queryVideoSource?.preview?.url ??
    state.queryVideoLibraryObject?.preview?.url ??
    null
  );
}

function queryVideoStatusLabel() {
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

function queryVideoDisplayName() {
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

function activeQueryVideoPreview() {
  return (
    state.queryVideoAsset?.preview ??
    state.queryVideoSource?.preview ??
    state.queryVideoLibraryObject?.preview ??
    null
  );
}

function currentQueryVideoStartMs() {
  if (state.queryVideoLibraryObject?.locator?.start_ms != null) {
    return state.queryVideoLibraryObject.locator.start_ms;
  }
  return state.queryVideoRange?.start_ms ?? 0;
}

function currentQueryVideoEndMs() {
  if (state.queryVideoLibraryObject?.locator?.end_ms != null) {
    return state.queryVideoLibraryObject.locator.end_ms;
  }
  return state.queryVideoRange?.end_ms ?? state.queryVideoDurationMs ?? 0;
}

function queryVideoRangeSummary() {
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

function queryVideoLocatorPayload() {
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

function queryVideoRangeStep() {
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

function queryDocumentPreviewUrl() {
  return (
    state.queryDocumentObjectUrl ??
    state.queryDocumentAsset?.preview?.url ??
    state.queryDocumentLibraryObject?.preview?.url ??
    null
  );
}

function queryDocumentStatusLabel() {
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

function queryDocumentDisplayName() {
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

function activeQueryDocumentPreview() {
  return state.queryDocumentAsset?.preview ?? state.queryDocumentLibraryObject?.preview ?? null;
}

function currentQueryDocumentStartPage() {
  if (state.queryDocumentLibraryObject?.locator?.start_page != null) {
    return state.queryDocumentLibraryObject.locator.start_page;
  }
  return state.queryDocumentStartPageDraft;
}

function currentQueryDocumentEndPage() {
  if (state.queryDocumentLibraryObject?.locator?.end_page != null) {
    return state.queryDocumentLibraryObject.locator.end_page;
  }
  return state.queryDocumentEndPageDraft;
}

function queryDocumentRangeSummary() {
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

function queryDocumentLocatorPayload() {
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

function renderStatusNotices() {
  const blocks = [];

  if (state.globalError) {
    blocks.push(`
      <div class="notice error">
        <h4>${escapeHtml(state.globalError.code ?? "error")}</h4>
        <p>${escapeHtml(state.globalError.message ?? state.globalError)}</p>
      </div>
    `);
  }

  if (state.statusMessage) {
    blocks.push(`
      <div class="notice success">
        <h4>Working</h4>
        <p>${escapeHtml(state.statusMessage)}</p>
      </div>
    `);
  }

  if (!blocks.length) {
    return "";
  }

  return `<section class="status-stack">${blocks.join("")}</section>`;
}

function renderImportReceipt() {
  if (!state.importReceipt) {
    return '<p class="empty" data-testid="import-receipt-empty">还没有导入回执。提交路径后会在这里显示接受和拒绝结果。</p>';
  }

  const accepted = state.importReceipt.accepted.length
    ? `
        <div class="receipt-group" data-testid="import-accepted-group">
          <h4>Accepted</h4>
          <ul class="data-list">
            ${state.importReceipt.accepted
              .map(
                (item) => `
                  <li>
                    <div class="list-head">
                      <strong>${escapeHtml(item.kind)}</strong>
                      <span class="helper">${(item.visual_units ?? []).length} 个 visual units</span>
                    </div>
                    <span>${escapeHtml(item.normalized_path ?? item.original_path)}</span>
                    ${
                      item.visual_units?.length
                        ? `<div class="inline-actions">
                            ${item.visual_units
                              .map(
                                (visualUnit) => `
                                  <button
                                    type="button"
                                    class="secondary-button"
                                    data-visual-unit-id="${escapeHtml(visualUnit.visual_unit_id)}"
                                  >
                                    查看 ${escapeHtml(visualUnit.kind)} · ${escapeHtml(visualUnit.visual_unit_id)}
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
          <h4>Rejected</h4>
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

function renderVisualPreview() {
  if (!state.selectedVisualUnit) {
    return `
      <div class="preview-placeholder" data-testid="visual-preview">
        <p>选择一个结果或导入项后，这里会显示图片或 PDF 页预览。</p>
      </div>
    `;
  }

  const visualUnit = state.selectedVisualUnit.visual_unit;
  const preview = state.selectedVisualUnit.preview;
  const title = `${visualUnit.kind} · ${sourceName(visualUnit.source_path)}`;

  if (visualUnit.kind === "image") {
    return `
      <img
        class="preview-image"
        data-testid="visual-preview"
        src="${escapeHtml(preview.url)}"
        alt="${escapeHtml(title)}"
        loading="lazy"
      />
    `;
  }

  if (visualUnit.kind === "video_segment") {
    const startMs = visualUnit.locator?.start_ms ?? 0;
    const endMs = visualUnit.locator?.end_ms ?? 0;
    return `
      <video
        class="preview-video"
        data-testid="visual-preview"
        data-preview-kind="video"
        data-start-ms="${escapeHtml(startMs)}"
        data-end-ms="${escapeHtml(endMs)}"
        src="${escapeHtml(preview.url)}"
        controls
        preload="metadata"
      ></video>
    `;
  }

  return `
    <iframe
      class="preview-frame"
      data-testid="visual-preview"
      src="${escapeHtml(preview.url)}"
      title="${escapeHtml(title)}"
      loading="lazy"
    ></iframe>
  `;
}

function renderVisualUnitDetail() {
  if (!state.selectedVisualUnit) {
    return '<p class="empty">从导入回执或搜索结果里选择一个 visual unit，右侧会显示预览、定位信息和上下文。</p>';
  }

  const visualUnit = state.selectedVisualUnit.visual_unit;
  const preview = state.selectedVisualUnit.preview;
  const page = pageLabel(visualUnit.locator);
  const segment = videoLabel(visualUnit.locator);
  return `
    <div class="detail-card" data-testid="visual-unit-detail">
      <div class="detail-preview">
        ${renderVisualPreview()}
      </div>
      <div class="detail-head">
        <div class="job-meta">
          <span class="pill ready">${escapeHtml(visualUnit.kind)}</span>
          ${page ? `<span class="pill muted">${escapeHtml(page)}</span>` : ""}
          ${segment ? `<span class="pill muted">${escapeHtml(segment)}</span>` : ""}
        </div>
        <h4>${escapeHtml(sourceName(visualUnit.source_path))}</h4>
        <p class="helper">${escapeHtml(visualUnit.visual_unit_id)}</p>
      </div>
      <dl class="stats">
        <div><dt>Source type</dt><dd>${escapeHtml(visualUnit.source_type)}</dd></div>
        <div><dt>Source path</dt><dd class="detail-path">${escapeHtml(visualUnit.source_path)}</dd></div>
      </dl>
      <div class="detail-grid">
        <div class="detail-block">
          <h5>Locator</h5>
          <pre>${escapeHtml(JSON.stringify(visualUnit.locator, null, 2))}</pre>
        </div>
        <div class="detail-block">
          <h5>Preview</h5>
          <div class="inline-actions">
            <a data-testid="preview-link" href="${escapeHtml(preview.url)}" target="_blank" rel="noreferrer">打开预览</a>
            ${
              visualUnit.kind === "image" || visualUnit.kind === "document_page"
                ? `<button type="button" class="secondary-button" data-testid="detail-use-as-query-image-button" data-use-query-visual-unit-id="${escapeHtml(visualUnit.visual_unit_id)}">作为查询图片</button>`
                : ""
            }
            ${
              visualUnit.kind === "document_page"
                ? `<button type="button" class="secondary-button" data-testid="detail-use-as-query-document-button" data-use-query-document-visual-unit-id="${escapeHtml(visualUnit.visual_unit_id)}">作为查询文档</button>`
                : ""
            }
            ${
              visualUnit.kind === "video_segment"
                ? `<button type="button" class="secondary-button" data-testid="detail-use-as-query-video-button" data-use-query-video-visual-unit-id="${escapeHtml(visualUnit.visual_unit_id)}">作为查询视频</button>`
                : ""
            }
          </div>
        </div>
      </div>
      <div class="detail-block">
        <h5>Neighbor context</h5>
        <pre>${escapeHtml(JSON.stringify(state.selectedVisualUnit.neighbor_context, null, 2))}</pre>
      </div>
    </div>
  `;
}

function renderJobs() {
  if (!state.selectedLibraryId) {
    return '<p class="empty">先创建或选择一个库，再查看任务。</p>';
  }

  if (!state.jobs.length) {
    return '<p class="empty">当前库还没有任务。</p>';
  }

  return `
    <ul class="job-list" data-testid="job-list">
      ${state.jobs
        .map(
          (job) => `
            <li class="job-card" data-testid="job-card" data-job-id="${escapeHtml(job.job_id)}" data-job-status="${escapeHtml(job.status)}">
              <div class="job-meta">
                <span class="pill ${jobPillClass(job.status)}">${escapeHtml(job.status)}</span>
                <span>${escapeHtml(job.job_id)}</span>
              </div>
              <h4>${escapeHtml(job.kind)} · ${escapeHtml(job.phase)}</h4>
              <p>${escapeHtml(job.current_attempt.summary)}</p>
              <small>${job.progress.completed}/${job.progress.total} ${escapeHtml(job.progress.unit)}</small>
            </li>
          `
        )
        .join("")}
    </ul>
  `;
}

function renderSearchOutcome() {
  if (!state.searchOutcome) {
    return '<p class="empty">结果列表会显示在这里。导入成功后，这里会以统一列表混排 `video_segment`、`image` 和 `document_page`，并可直接打开右侧详情。</p>';
  }

  if (state.searchOutcome.error) {
    const details = state.searchOutcome.error.details?.index_lines ?? [];
    return `
      <div class="notice error" data-testid="search-error-notice">
        <h4 data-testid="search-error-code">${escapeHtml(state.searchOutcome.error.code)}</h4>
        <p data-testid="search-error-message">${escapeHtml(state.searchOutcome.error.message)}</p>
        ${
          details.length
            ? `<ul class="data-list" data-testid="search-error-details">
                ${details
                  .map(
                    (item) => `
                      <li>
                        <strong>${escapeHtml(item.index_line)}</strong>
                        <span>${escapeHtml(item.job?.job_id ?? "no-job")} · ${escapeHtml(item.job?.phase ?? item.status)}</span>
                      </li>
                    `
                  )
                  .join("")}
              </ul>`
            : ""
        }
      </div>
    `;
  }

  if (!state.searchOutcome.results.length) {
    return `
      <div class="notice success">
        <h4>No results</h4>
        <p>当前真实检索链路没有返回匹配结果。可以换一个查询词，或确认目标库已经导入相关内容。</p>
      </div>
    `;
  }

  return `
    <div class="results-summary">
      <div>
        <p class="eyebrow">Results</p>
        <h3>命中 ${state.searchOutcome.results.length} 条结果</h3>
      </div>
      <p class="helper">点击结果卡片后，右侧会更新预览和详情。</p>
    </div>
    <ul class="result-list" data-testid="result-list">
      ${state.searchOutcome.results
        .map(
          (item) => {
            const scoreLabel = formatScore(item.score);
            const page = pageLabel(item.locator);
            const segment = videoLabel(item.locator);
            return `
            <li
              class="result-card ${item.visual_unit_id === selectedVisualUnitId() ? "active" : ""}"
              data-testid="result-card"
              data-kind="${escapeHtml(item.kind)}"
              data-visual-unit-id="${escapeHtml(item.visual_unit_id)}"
            >
              <button
                type="button"
                class="result-select"
                data-visual-unit-id="${escapeHtml(item.visual_unit_id)}"
              >
                <div class="result-topline">
                  <span class="pill ${item.kind === "image" ? "ready" : "pending"}">${escapeHtml(item.kind)}</span>
                  ${page ? `<span class="pill muted">${escapeHtml(page)}</span>` : ""}
                  ${segment ? `<span class="pill muted">${escapeHtml(segment)}</span>` : ""}
                  ${scoreLabel ? `<span class="pill score-pill" data-testid="result-score">score ${escapeHtml(scoreLabel)}</span>` : ""}
                </div>
                <strong>${escapeHtml(sourceName(item.source_path))}</strong>
                <span class="helper">${escapeHtml(item.source_path)}</span>
                <span class="helper">${escapeHtml(item.source_type)} · ${escapeHtml(JSON.stringify(item.locator))}</span>
              </button>
              <div class="inline-actions">
                <button type="button" class="secondary-button" data-visual-unit-id="${escapeHtml(item.visual_unit_id)}">查看详情</button>
                ${
                  item.kind === "image" || item.kind === "document_page"
                    ? `<button type="button" class="secondary-button" data-testid="use-as-query-image-button" data-use-query-visual-unit-id="${escapeHtml(item.visual_unit_id)}">作为查询图片</button>`
                    : ""
                }
                ${
                  item.kind === "document_page"
                    ? `<button type="button" class="secondary-button" data-testid="use-as-query-document-button" data-use-query-document-visual-unit-id="${escapeHtml(item.visual_unit_id)}">作为查询文档</button>`
                    : ""
                }
                ${
                  item.kind === "video_segment"
                    ? `<button type="button" class="secondary-button" data-testid="use-as-query-video-button" data-use-query-video-visual-unit-id="${escapeHtml(item.visual_unit_id)}">作为查询视频</button>`
                    : ""
                }
                <a href="${escapeHtml(item.preview.url)}" target="_blank" rel="noreferrer">Preview</a>
              </div>
            </li>
          `;
          }
        )
        .join("")}
    </ul>
  `;
}

function renderSearchControls(library) {
  const queryPreview = queryImagePreviewUrl();
  const queryVideoPreview = queryVideoPreviewUrl();
  const queryDocumentPreview = queryDocumentPreviewUrl();
  const queryVideoDuration = state.queryVideoDurationMs;
  const queryVideoStartMs = currentQueryVideoStartMs();
  const queryVideoEndMs = currentQueryVideoEndMs();
  return `
    <div class="search-mode-switch" data-testid="search-mode-switch">
      <button
        type="button"
        class="${state.searchMode === "text" ? "" : "secondary-button"}"
        data-testid="search-mode-text"
        data-search-mode="text"
      >
        Text
      </button>
      <button
        type="button"
        class="${state.searchMode === "image" ? "" : "secondary-button"}"
        data-testid="search-mode-image"
        data-search-mode="image"
      >
        Image
      </button>
      <button
        type="button"
        class="${state.searchMode === "video" ? "" : "secondary-button"}"
        data-testid="search-mode-video"
        data-search-mode="video"
      >
        Video
      </button>
      <button
        type="button"
        class="${state.searchMode === "document" ? "" : "secondary-button"}"
        data-testid="search-mode-document"
        data-search-mode="document"
      >
        Document
      </button>
    </div>
    <form id="search-form" class="stack-form search-form" data-testid="search-form">
      ${
        state.searchMode === "text"
          ? `
            <label>
              <span>查询文本</span>
              <input
                id="search-text"
                data-testid="search-text-input"
                type="text"
                value="${escapeHtml(state.searchTextDraft)}"
                placeholder="尝试输入财报页面中的问题或关键词"
                ${library ? "" : "disabled"}
              />
            </label>
          `
          : state.searchMode === "image"
            ? `
            <div class="query-image-panel" data-testid="query-image-panel">
              <label>
                <span>查询图片</span>
                <input
                  id="query-image-input"
                  data-testid="query-image-input"
                  type="file"
                  accept="image/*"
                  ${library ? "" : "disabled"}
                />
              </label>
              <div class="query-image-card" data-testid="query-image-card">
                <div class="job-meta">
                  <span class="pill ${state.queryImageAsset || state.queryImageLibraryObject ? "ready" : "muted"}">${escapeHtml(queryImageStatusLabel())}</span>
                  ${
                    queryImageDisplayName()
                      ? `<span class="helper">${escapeHtml(queryImageDisplayName())}</span>`
                      : ""
                  }
                </div>
                ${
                  queryPreview
                    ? isDocumentPageQueryImage()
                      ? `<iframe class="query-image-preview-frame" data-testid="query-image-preview" src="${escapeHtml(queryPreview)}" title="Query image preview" loading="lazy"></iframe>`
                      : `<img class="query-image-preview" data-testid="query-image-preview" src="${escapeHtml(queryPreview)}" alt="Query image preview" />`
                    : `<p class="empty" data-testid="query-image-empty">选择一张本地图片后，这里会显示查询图片预览。</p>`
                }
                <div class="inline-actions">
                  <button type="button" id="clear-query-image-button" data-testid="clear-query-image-button" class="secondary-button" ${state.queryImageFile || state.queryImageAsset || state.queryImageLibraryObject ? "" : "disabled"}>清除</button>
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
              <label>
                <span>查询视频</span>
                <input
                  id="query-video-input"
                  data-testid="query-video-input"
                  type="file"
                  accept="video/mp4,video/quicktime,video/x-m4v,video/*"
                  ${library ? "" : "disabled"}
                />
              </label>
              <label>
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
              <div class="query-video-card" data-testid="query-video-card">
                <div class="job-meta">
                  <span class="pill ${state.queryVideoAsset || state.queryVideoSource || state.queryVideoLibraryObject ? "ready" : "muted"}">${escapeHtml(queryVideoStatusLabel())}</span>
                  ${
                    queryVideoDisplayName()
                      ? `<span class="helper">${escapeHtml(queryVideoDisplayName())}</span>`
                      : ""
                  }
                </div>
                ${
                  queryVideoPreview
                    ? `<video
                        class="query-video-preview"
                        data-testid="query-video-preview"
                        src="${escapeHtml(queryVideoPreview)}"
                        controls
                        preload="metadata"
                      ></video>`
                    : `<p class="empty" data-testid="query-video-empty">选择一个本地视频或库内视频源后，这里会显示查询视频预览。</p>`
                }
                <div class="query-range-card" data-testid="query-video-range-card">
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
                      class="secondary-button"
                      ${queryVideoDuration && state.queryVideoRange && !state.queryVideoLibraryObject ? "" : "disabled"}
                    >
                      整段视频
                    </button>
                  </div>
                </div>
                <div class="inline-actions">
                  <button
                    type="button"
                    id="clear-query-video-button"
                    data-testid="clear-query-video-button"
                    class="secondary-button"
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
              <label>
                <span>查询文档</span>
                <input
                  id="query-document-input"
                  data-testid="query-document-input"
                  type="file"
                  accept="application/pdf,.pdf"
                  ${library ? "" : "disabled"}
                />
              </label>
              <div class="query-document-card" data-testid="query-document-card">
                <div class="job-meta">
                  <span class="pill ${state.queryDocumentAsset || state.queryDocumentLibraryObject ? "ready" : "muted"}">${escapeHtml(queryDocumentStatusLabel())}</span>
                  ${
                    queryDocumentDisplayName()
                      ? `<span class="helper">${escapeHtml(queryDocumentDisplayName())}</span>`
                      : ""
                  }
                </div>
                ${
                  queryDocumentPreview
                    ? `<iframe class="query-document-preview-frame" data-testid="query-document-preview" src="${escapeHtml(queryDocumentPreview)}" title="Query document preview" loading="lazy"></iframe>`
                    : `<p class="empty" data-testid="query-document-empty">选择一个本地 PDF 或从结果复用 document_page 后，这里会显示查询文档预览。</p>`
                }
                <div class="query-range-card" data-testid="query-document-range-card">
                  <div class="job-meta">
                    <strong>页范围</strong>
                    <span class="helper">${escapeHtml(queryDocumentRangeSummary())}</span>
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
                      class="secondary-button"
                      ${!state.queryDocumentLibraryObject && (state.queryDocumentStartPageDraft || state.queryDocumentEndPageDraft) ? "" : "disabled"}
                    >
                      整份文档
                    </button>
                  </div>
                </div>
                <div class="inline-actions">
                  <button
                    type="button"
                    id="clear-query-document-button"
                    data-testid="clear-query-document-button"
                    class="secondary-button"
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
      <button type="submit" data-testid="search-submit-button" ${library ? "" : "disabled"}>
        ${
          state.searchMode === "text"
            ? "搜索"
            : state.searchMode === "image"
              ? "以图片搜索"
              : state.searchMode === "video"
                ? "以视频搜索"
                : "以文档搜索"
        }
      </button>
    </form>
  `;
}

function renderWorkspace() {
  const library = selectedLibrary();

  root.innerHTML = `
    <main class="shell" data-testid="workspace-shell">
      <section class="hero">
        <p class="eyebrow">FauniSearch</p>
        <h1>Search workspace</h1>
        <p class="summary">
          当前工作台已经接通真实的 ColQwen + Qdrant multivector 导入与检索链路。左侧管理库和导入，中间在统一工作区中执行 Text / Image / Video / Document 搜索并浏览结果，右侧查看预览和对象详情。
        </p>
        <div class="service-strip">
          <a href="${endpoints.uiRoot}" target="_blank" rel="noreferrer">UI</a>
          <a href="${endpoints.appHealth}" target="_blank" rel="noreferrer">App health</a>
          <a href="${endpoints.sidecarHealth}" target="_blank" rel="noreferrer">Sidecar health</a>
          <a href="${endpoints.qdrantCollections}" target="_blank" rel="noreferrer">Qdrant</a>
        </div>
      </section>

      ${renderStatusNotices()}

      <section class="workspace-desk">
        <aside class="workspace-column workspace-left">
          <section class="panel panel-tight">
            <div class="panel-head">
              <div>
                <p class="eyebrow">Library</p>
                <h2>库上下文</h2>
              </div>
            </div>
            <form id="create-library-form" class="stack-form" data-testid="create-library-form">
              <label>
                <span>新库名称</span>
                <input
                  id="library-name"
                  data-testid="library-name-input"
                  name="libraryName"
                  type="text"
                  placeholder="例如：demo-library"
                  required
                />
              </label>
              <button type="submit" data-testid="create-library-button">创建 multivector 库</button>
            </form>
            <label class="stack-form">
              <span>当前库</span>
              <select id="library-select" data-testid="library-select" ${state.libraries.length ? "" : "disabled"}>
                ${
                  state.libraries.length
                    ? state.libraries
                        .map(
                          (item) => `
                            <option value="${escapeHtml(item.id)}" ${item.id === state.selectedLibraryId ? "selected" : ""}>
                              ${escapeHtml(item.name)} (${escapeHtml(item.id)})
                            </option>
                          `
                        )
                        .join("")
                    : `<option value="">还没有库</option>`
                }
              </select>
            </label>
            <div class="context-card" data-testid="current-library-card">
              ${
                library
                  ? `
                    <p class="eyebrow">Current</p>
                    <h3 data-testid="current-library-name">${escapeHtml(library.name)}</h3>
                    <p class="helper">${escapeHtml(library.id)}</p>
                    <div class="pill-row">${formatIndexLines(library.index_lines)}</div>
                    <dl class="stats">
                      <div><dt>Accepted items</dt><dd>${library.counts.accepted_items}</dd></div>
                      <div><dt>Pending jobs</dt><dd>${library.counts.pending_jobs}</dd></div>
                      <div><dt>Latest job</dt><dd>${escapeHtml(library.latest_job_id ?? "none")}</dd></div>
                    </dl>
                  `
                  : `<p class="empty">先创建一个库，再进入导入和搜索步骤。</p>`
              }
            </div>
          </section>

          <section class="panel panel-tight">
            <div class="panel-head">
              <div>
                <p class="eyebrow">Import</p>
                <h2>路径导入</h2>
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
              <button type="submit" data-testid="import-submit-button" ${library ? "" : "disabled"}>提交导入</button>
            </form>
            <div class="quick-card" data-testid="demo-card">
              <p class="eyebrow">Quick demo</p>
              <h3>真实索引和检索</h3>
              <p class="helper">使用仓库内置的 TATDQA 图片 fixture，可直接触发真实 document/image embedding、Qdrant 写入和文本搜索。浏览器文件选择不会暴露服务器可读的绝对路径，所以当前仍以路径输入为主。</p>
              <code>${escapeHtml(demoFixture.path)}</code>
              <div class="inline-actions">
                <button id="fill-demo-button" data-testid="fill-demo-button" type="button" class="secondary-button" ${library ? "" : "disabled"}>填入 demo 路径和查询</button>
                <button id="run-demo-button" data-testid="run-demo-button" type="button" ${library ? "" : "disabled"}>导入并搜索 demo</button>
              </div>
            </div>
            ${renderImportReceipt()}
          </section>

          <section class="panel panel-tight">
            <div class="panel-head">
              <div>
                <p class="eyebrow">Tasks</p>
                <h2>任务面板</h2>
              </div>
            </div>
            ${renderJobs()}
          </section>
        </aside>

        <section class="workspace-column workspace-center">
          <section class="panel search-panel">
            <div class="panel-head">
              <div>
                <p class="eyebrow">Search</p>
                <h2>统一搜索入口</h2>
              </div>
            </div>
            ${renderSearchControls(library)}
            ${renderSearchOutcome()}
          </section>
        </section>

        <aside class="workspace-column workspace-right">
          <section class="panel detail-panel" data-testid="detail-panel">
            <div class="panel-head">
              <div>
                <p class="eyebrow">Detail</p>
                <h2>详情侧栏</h2>
              </div>
            </div>
            ${renderVisualUnitDetail()}
          </section>
        </aside>
      </section>
    </main>
  `;

  document.querySelector("#create-library-form")?.addEventListener("submit", onCreateLibrary);
  document.querySelector("#library-select")?.addEventListener("change", onSelectLibrary);
  document.querySelector("#import-form")?.addEventListener("submit", onImportPaths);
  document.querySelector("#import-paths")?.addEventListener("input", onImportPathsInput);
  document.querySelector("#search-form")?.addEventListener("submit", onSearchSubmit);
  document.querySelector("#search-text")?.addEventListener("input", onSearchTextInput);
  document.querySelector("#query-image-input")?.addEventListener("change", onQueryImageInput);
  document.querySelector("#clear-query-image-button")?.addEventListener("click", onClearQueryImage);
  document.querySelector("#query-image-paste-target")?.addEventListener("paste", onQueryImagePaste);
  document.querySelector("#query-video-input")?.addEventListener("change", onQueryVideoInput);
  document.querySelector("#query-video-source-select")?.addEventListener("change", onQueryVideoSourceSelect);
  document.querySelector("#clear-query-video-button")?.addEventListener("click", onClearQueryVideo);
  document.querySelector("#clear-query-video-range-button")?.addEventListener("click", onClearQueryVideoRange);
  document.querySelector("#query-video-range-start")?.addEventListener("input", onQueryVideoRangeStartInput);
  document.querySelector("#query-video-range-end")?.addEventListener("input", onQueryVideoRangeEndInput);
  document.querySelector("#query-document-input")?.addEventListener("change", onQueryDocumentInput);
  document.querySelector("#clear-query-document-button")?.addEventListener("click", onClearQueryDocument);
  document
    .querySelector("#clear-query-document-range-button")
    ?.addEventListener("click", onClearQueryDocumentRange);
  document
    .querySelector("#query-document-range-start")
    ?.addEventListener("input", onQueryDocumentRangeStartInput);
  document
    .querySelector("#query-document-range-end")
    ?.addEventListener("input", onQueryDocumentRangeEndInput);
  document.querySelector("#fill-demo-button")?.addEventListener("click", onFillDemo);
  document.querySelector("#run-demo-button")?.addEventListener("click", onRunDemo);
  document.querySelectorAll("[data-search-mode]").forEach((button) => {
    button.addEventListener("click", onSelectSearchMode);
  });
  document.querySelectorAll("[data-visual-unit-id]").forEach((button) => {
    button.addEventListener("click", onSelectVisualUnit);
  });
  document.querySelectorAll("[data-use-query-visual-unit-id]").forEach((button) => {
    button.addEventListener("click", onUseAsQueryImage);
  });
  document.querySelectorAll("[data-use-query-video-visual-unit-id]").forEach((button) => {
    button.addEventListener("click", onUseAsQueryVideo);
  });
  document.querySelectorAll("[data-use-query-document-visual-unit-id]").forEach((button) => {
    button.addEventListener("click", onUseAsQueryDocument);
  });

  const queryVideoPreview = document.querySelector("#query-video-preview");
  if (queryVideoPreview instanceof HTMLVideoElement) {
    queryVideoPreview.addEventListener("loadedmetadata", onQueryVideoPreviewLoadedMetadata);
    if (queryVideoPreview.readyState >= 1) {
      syncQueryVideoDurationFromVideoElement(queryVideoPreview);
    }
  }

  const detailVideoPreview = document.querySelector('[data-testid="visual-preview"][data-preview-kind="video"]');
  if (detailVideoPreview instanceof HTMLVideoElement) {
    attachBoundedVideoPlayback(detailVideoPreview);
  }
}

async function apiRequest(path, options = {}) {
  const headers = { ...(options.headers ?? {}) };
  const isFormDataBody = options.body instanceof FormData;
  if (!isFormDataBody && !headers["Content-Type"]) {
    headers["Content-Type"] = "application/json";
  }

  const response = await fetch(`/api${path}`, {
    ...options,
    headers,
  });

  let payload = null;
  try {
    payload = await response.json();
  } catch {
    payload = null;
  }

  if (!response.ok || payload?.error) {
    throw payload?.error ?? {
      code: "request_failed",
      message: `Request failed with status ${response.status}`,
    };
  }

  return payload.data;
}

function syncQueryVideoDurationFromVideoElement(videoElement) {
  if (!(videoElement instanceof HTMLVideoElement) || !Number.isFinite(videoElement.duration)) {
    return;
  }

  const durationMs = Math.max(Math.round(videoElement.duration * 1000), 1);
  if (durationMs === state.queryVideoDurationMs) {
    return;
  }

  setQueryVideoDuration(durationMs);
  renderWorkspace();
}

function attachBoundedVideoPlayback(videoElement) {
  if (!(videoElement instanceof HTMLVideoElement)) {
    return;
  }

  const startMs = Number(videoElement.dataset.startMs ?? "0");
  const endMs = Number(videoElement.dataset.endMs ?? "0");
  if (!Number.isFinite(startMs) || !Number.isFinite(endMs) || endMs <= startMs) {
    return;
  }

  const startSeconds = startMs / 1000;
  const endSeconds = endMs / 1000;
  const syncCurrentTime = () => {
    if (Number.isFinite(videoElement.duration) && videoElement.currentTime < startSeconds) {
      videoElement.currentTime = startSeconds;
    }
  };
  const clampPlayback = () => {
    if (videoElement.currentTime >= endSeconds) {
      videoElement.pause();
      videoElement.currentTime = startSeconds;
    }
  };

  videoElement.addEventListener("loadedmetadata", syncCurrentTime, { once: true });
  videoElement.addEventListener("timeupdate", clampPlayback);
  if (videoElement.readyState >= 1) {
    syncCurrentTime();
  }
}

async function refreshLibraries({ keepSelection = true } = {}) {
  const data = await apiRequest("/libraries");
  state.libraries = data.libraries;

  if (!keepSelection || !state.libraries.some((item) => item.id === state.selectedLibraryId)) {
    state.selectedLibraryId = state.libraries[0]?.id ?? "";
  }
}

async function refreshJobs() {
  if (!state.selectedLibraryId) {
    state.jobs = [];
    return;
  }

  const data = await apiRequest(`/jobs?library_id=${encodeURIComponent(state.selectedLibraryId)}`);
  state.jobs = data.jobs;
}

async function refreshVideoSources() {
  if (!state.selectedLibraryId) {
    state.videoSources = [];
    if (!state.queryVideoFile && !state.queryVideoAsset) {
      clearQueryVideoState();
    }
    return;
  }

  const data = await apiRequest(
    `/libraries/${encodeURIComponent(state.selectedLibraryId)}/video-sources`
  );
  state.videoSources = data.sources;

  if (state.queryVideoSource) {
    const refreshed = state.videoSources.find(
      (source) => source.source_id === state.queryVideoSource.source_id
    );
    if (refreshed) {
      state.queryVideoSource = refreshed;
      setQueryVideoDuration(refreshed.duration_ms ?? null);
    } else if (!state.queryVideoFile && !state.queryVideoAsset) {
      clearQueryVideoState();
    }
  }
}

async function refreshJob(jobId) {
  return apiRequest(`/jobs/${encodeURIComponent(jobId)}`);
}

async function refreshWorkspace(options) {
  await refreshLibraries(options);
  await refreshJobs();
  await refreshVideoSources();
  renderWorkspace();
}

async function onCreateLibrary(event) {
  event.preventDefault();
  const input = document.querySelector("#library-name");
  const name = input?.value?.trim() ?? "";
  if (!name) {
    return;
  }

  try {
    state.globalError = null;
    const library = await apiRequest("/libraries", {
      method: "POST",
      body: JSON.stringify({
        name,
        config: { enabled_index_lines: ["multivector"] },
      }),
    });
    state.selectedLibraryId = library.id;
    state.importPathsDraft = "";
    state.searchTextDraft = "";
    clearQueryImageState();
    clearQueryVideoState();
    clearQueryDocumentState();
    state.importReceipt = null;
    state.selectedVisualUnit = null;
    state.searchOutcome = null;
    state.statusMessage = null;
    input.value = "";
    await refreshWorkspace({ keepSelection: true });
  } catch (error) {
    state.globalError = error;
    renderWorkspace();
  }
}

async function onSelectLibrary(event) {
  state.selectedLibraryId = event.target.value;
  clearQueryImageState();
  clearQueryVideoState();
  clearQueryDocumentState();
  state.importReceipt = null;
  state.selectedVisualUnit = null;
  state.searchOutcome = null;
  state.globalError = null;
  state.statusMessage = null;
  await refreshWorkspace({ keepSelection: true });
}

function onImportPathsInput(event) {
  state.importPathsDraft = event.target.value;
}

function onSearchTextInput(event) {
  state.searchTextDraft = event.target.value;
}

function parseImportPaths(value) {
  return value
    .split("\n")
    .map((item) => item.trim())
    .filter(Boolean);
}

function setDemoDrafts() {
  state.importPathsDraft = demoFixture.path;
  state.searchTextDraft = demoFixture.query;
}

async function onImportPaths(event) {
  event.preventDefault();
  if (!state.selectedLibraryId) {
    return;
  }

  const textarea = document.querySelector("#import-paths");
  state.importPathsDraft = textarea?.value ?? "";
  const paths = parseImportPaths(state.importPathsDraft);
  if (!paths.length) {
    state.globalError = {
      code: "validation_failed",
      message: "请至少输入一个本地路径。",
    };
    renderWorkspace();
    return;
  }

  try {
    state.globalError = null;
    state.statusMessage = "正在导入并建立 multivector 索引...";
    renderWorkspace();
    await importPaths(paths);
    state.importPathsDraft = "";
    state.statusMessage = null;
    renderWorkspace();
  } catch (error) {
    state.globalError = error;
    state.statusMessage = null;
    renderWorkspace();
  }
}

async function importPaths(paths) {
  state.importReceipt = await apiRequest(`/libraries/${state.selectedLibraryId}/imports`, {
    method: "POST",
    body: JSON.stringify({ paths }),
  });
  state.searchOutcome = null;
  await refreshWorkspace({ keepSelection: true });

  const job = state.importReceipt.job;
  if (job && !isTerminalJobStatus(job.status)) {
    const terminalJob = await waitForJobTerminal(job.job_id);
    state.importReceipt.job = terminalJob;
    if (terminalJob.status === "failed" || terminalJob.status === "canceled") {
      state.globalError = {
        code: terminalJob.status,
        message: terminalJob.current_attempt.summary,
      };
      renderWorkspace();
      return state.importReceipt;
    }
  }

  const firstVisualUnit = state.importReceipt.accepted
    .flatMap((item) => item.visual_units ?? [])
    .at(0);
  if (firstVisualUnit) {
    await loadVisualUnit(firstVisualUnit.visual_unit_id);
  }
  return state.importReceipt;
}

async function waitForJobTerminal(jobId) {
  const startedAt = Date.now();

  while (Date.now() - startedAt < JOB_POLL_TIMEOUT_MS) {
    const job = await refreshJob(jobId);
    await refreshWorkspace({ keepSelection: true });

    if (isTerminalJobStatus(job.status)) {
      state.statusMessage = null;
      renderWorkspace();
      return job;
    }

    state.statusMessage = `后台任务 ${job.job_id} 正在 ${job.phase}...`;
    renderWorkspace();
    await sleep(JOB_POLL_INTERVAL_MS);
  }

  throw {
    code: "job_timeout",
    message: `后台任务 ${jobId} 在预期时间内没有进入终态。`,
  };
}

async function onSearchSubmit(event) {
  event.preventDefault();
  if (!state.selectedLibraryId) {
    return;
  }

  try {
    state.globalError = null;
    renderWorkspace();
    if (state.searchMode === "image") {
      await runImageSearch();
    } else if (state.searchMode === "video") {
      await runVideoSearch();
    } else if (state.searchMode === "document") {
      await runDocumentSearch();
    } else {
      await runTextSearch();
    }
    state.statusMessage = null;
    renderWorkspace();
  } catch (error) {
    state.searchOutcome = { error };
    state.statusMessage = null;
    renderWorkspace();
  }
}

async function runTextSearch() {
  const input = document.querySelector("#search-text");
  state.searchTextDraft = input?.value ?? "";
  const text = state.searchTextDraft.trim();
  if (!text) {
    state.searchOutcome = {
      error: {
        code: "validation_failed",
        message: "请输入查询文本。",
      },
    };
    renderWorkspace();
    return;
  }

  state.statusMessage = "正在执行真实 multivector 文本搜索...";
  renderWorkspace();
  await searchText(text);
}

async function runImageSearch() {
  if (!state.queryImageFile && !state.queryImageAsset && !state.queryImageLibraryObject) {
    state.searchOutcome = {
      error: {
        code: "validation_failed",
        message: "请先选择一张查询图片。",
      },
    };
    renderWorkspace();
    return;
  }

  if (state.queryImageFile) {
    state.statusMessage = "正在上传查询图片...";
    renderWorkspace();
    await uploadQueryImage(state.queryImageFile);
  }

  state.statusMessage = "正在执行真实 multivector 图片搜索...";
  renderWorkspace();
  if (state.queryImageAsset) {
    await searchImage({
      kind: "temp_asset",
      temp_asset_id: state.queryImageAsset.temp_asset_id,
    });
    return;
  }

  if (state.queryImageLibraryObject) {
    await searchImage({
      kind: "library_object",
      visual_unit_id: state.queryImageLibraryObject.visual_unit_id,
    });
  }
}

async function runVideoSearch() {
  if (
    !state.queryVideoFile &&
    !state.queryVideoAsset &&
    !state.queryVideoSource &&
    !state.queryVideoLibraryObject
  ) {
    state.searchOutcome = {
      error: {
        code: "validation_failed",
        message: "请先选择一个查询视频。",
      },
    };
    renderWorkspace();
    return;
  }

  if (state.queryVideoFile) {
    state.statusMessage = "正在上传查询视频...";
    renderWorkspace();
    await uploadQueryVideo(state.queryVideoFile);
  }

  state.statusMessage = "正在执行真实 multivector 视频搜索...";
  renderWorkspace();
  const locator = queryVideoLocatorPayload();
  if (state.queryVideoAsset) {
    await searchVideo({
      kind: "temp_asset",
      temp_asset_id: state.queryVideoAsset.temp_asset_id,
      ...(locator ? { locator } : {}),
    });
    return;
  }

  if (state.queryVideoSource) {
    await searchVideo({
      kind: "library_object",
      source_id: state.queryVideoSource.source_id,
      ...(locator ? { locator } : {}),
    });
    return;
  }

  if (state.queryVideoLibraryObject) {
    await searchVideo({
      kind: "library_object",
      visual_unit_id: state.queryVideoLibraryObject.visual_unit_id,
    });
  }
}

async function runDocumentSearch() {
  if (!state.queryDocumentFile && !state.queryDocumentAsset && !state.queryDocumentLibraryObject) {
    state.searchOutcome = {
      error: {
        code: "validation_failed",
        message: "请先选择一个查询文档。",
      },
    };
    renderWorkspace();
    return;
  }

  if (state.queryDocumentFile) {
    state.statusMessage = "正在上传查询文档...";
    renderWorkspace();
    await uploadQueryDocument(state.queryDocumentFile);
  }

  state.statusMessage = "正在执行真实 multivector 文档搜索...";
  renderWorkspace();
  const locator = queryDocumentLocatorPayload();
  if (state.queryDocumentAsset) {
    await searchDocument({
      kind: "temp_asset",
      temp_asset_id: state.queryDocumentAsset.temp_asset_id,
      ...(locator ? { locator } : {}),
    });
    return;
  }

  if (state.queryDocumentLibraryObject) {
    await searchDocument({
      kind: "library_object",
      source_id: state.queryDocumentLibraryObject.source_id,
      ...(locator ? { locator } : {}),
    });
  }
}

async function searchText(text) {
  const data = await apiRequest("/search/text", {
    method: "POST",
    body: JSON.stringify({
      library_id: state.selectedLibraryId,
      text,
      top_k: 5,
      debug: true,
    }),
  });
  state.searchOutcome = data;
  renderWorkspace();
  if (data.results[0]?.visual_unit_id) {
    await loadVisualUnit(data.results[0].visual_unit_id);
  }
  return data;
}

async function uploadQueryImage(file) {
  const formData = new FormData();
  formData.append("file", file);
  const data = await apiRequest(`/libraries/${state.selectedLibraryId}/query-assets/images`, {
    method: "POST",
    body: formData,
  });
  if (state.queryImageObjectUrl) {
    URL.revokeObjectURL(state.queryImageObjectUrl);
  }
  state.queryImageFile = null;
  state.queryImageObjectUrl = null;
  state.queryImageAsset = data;
  renderWorkspace();
  return data;
}

async function uploadQueryVideo(file) {
  const formData = new FormData();
  formData.append("file", file);
  const data = await apiRequest(`/libraries/${state.selectedLibraryId}/query-assets/videos`, {
    method: "POST",
    body: formData,
  });
  if (state.queryVideoObjectUrl) {
    URL.revokeObjectURL(state.queryVideoObjectUrl);
  }
  state.queryVideoFile = null;
  state.queryVideoObjectUrl = null;
  state.queryVideoAsset = data;
  state.queryVideoSource = null;
  setQueryVideoDuration(data.duration_ms ?? state.queryVideoDurationMs);
  renderWorkspace();
  return data;
}

async function uploadQueryDocument(file) {
  const formData = new FormData();
  formData.append("file", file);
  const data = await apiRequest(`/libraries/${state.selectedLibraryId}/query-assets/documents`, {
    method: "POST",
    body: formData,
  });
  if (state.queryDocumentObjectUrl) {
    URL.revokeObjectURL(state.queryDocumentObjectUrl);
  }
  state.queryDocumentFile = null;
  state.queryDocumentObjectUrl = null;
  state.queryDocumentAsset = data;
  state.queryDocumentLibraryObject = null;
  setQueryDocumentPageCount(data.page_count ?? null);
  renderWorkspace();
  return data;
}

async function searchImage(imageInput) {
  const data = await apiRequest("/search/image", {
    method: "POST",
    body: JSON.stringify({
      library_id: state.selectedLibraryId,
      image_input: imageInput,
      top_k: 5,
      debug: true,
    }),
  });
  state.searchOutcome = data;
  renderWorkspace();
  if (data.results[0]?.visual_unit_id) {
    await loadVisualUnit(data.results[0].visual_unit_id);
  }
  return data;
}

async function searchVideo(videoInput) {
  const data = await apiRequest("/search/video", {
    method: "POST",
    body: JSON.stringify({
      library_id: state.selectedLibraryId,
      video_input: videoInput,
      top_k: 5,
      debug: true,
    }),
  });
  state.searchOutcome = data;
  renderWorkspace();
  if (data.results[0]?.visual_unit_id) {
    await loadVisualUnit(data.results[0].visual_unit_id);
  }
  return data;
}

async function searchDocument(documentInput) {
  const data = await apiRequest("/search/document", {
    method: "POST",
    body: JSON.stringify({
      library_id: state.selectedLibraryId,
      document_input: documentInput,
      top_k: 5,
      debug: true,
    }),
  });
  state.searchOutcome = data;
  renderWorkspace();
  if (data.results[0]?.visual_unit_id) {
    await loadVisualUnit(data.results[0].visual_unit_id);
  }
  return data;
}

function onFillDemo() {
  setDemoDrafts();
  state.searchMode = "text";
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

async function onRunDemo() {
  if (!state.selectedLibraryId) {
    return;
  }

  try {
    setDemoDrafts();
    state.globalError = null;
    state.statusMessage = "正在导入 demo fixture，并写入 Qdrant...";
    renderWorkspace();
    await importPaths([demoFixture.path]);
    state.statusMessage = "索引完成，正在执行 demo 查询...";
    renderWorkspace();
    await searchText(demoFixture.query);
    state.statusMessage = null;
    renderWorkspace();
  } catch (error) {
    state.globalError = error;
    state.statusMessage = null;
    renderWorkspace();
  }
}

function onSelectSearchMode(event) {
  state.searchMode = event.currentTarget.dataset.searchMode;
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onQueryImageInput(event) {
  const [file] = event.target.files ?? [];
  if (file) {
    setPendingQueryImageFile(file);
  }
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onQueryDocumentInput(event) {
  const [file] = event.target.files ?? [];
  if (file) {
    setPendingQueryDocumentFile(file);
  }
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

async function onQueryVideoInput(event) {
  const [file] = event.target.files ?? [];
  if (file) {
    setPendingQueryVideoFile(file);
  }
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();

  if (!file || !state.queryVideoObjectUrl) {
    return;
  }

  const previewUrl = state.queryVideoObjectUrl;
  try {
    const durationMs = await probeVideoDurationFromUrl(previewUrl);
    if (state.queryVideoObjectUrl === previewUrl) {
      setQueryVideoDuration(durationMs);
      renderWorkspace();
    }
  } catch {
    if (state.queryVideoObjectUrl === previewUrl) {
      state.globalError = {
        code: "validation_failed",
        message: "当前查询视频的元数据无法读取。",
      };
      renderWorkspace();
    }
  }
}

function onQueryImagePaste(event) {
  if (state.searchMode !== "image" || !state.selectedLibraryId) {
    return;
  }

  const clipboardImage = firstClipboardImageFile(event.clipboardData);
  if (!clipboardImage) {
    const target = event.target;
    if (target instanceof Element && target.matches(EDITABLE_TARGET_SELECTOR)) {
      return;
    }
    state.globalError = {
      code: "validation_failed",
      message: "剪贴板中没有可用的图片。",
    };
    state.statusMessage = null;
    renderWorkspace();
    return;
  }

  event.preventDefault();
  setPendingQueryImageFile(clipboardImage);
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onClearQueryImage() {
  clearQueryImageState();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onClearQueryDocument() {
  clearQueryDocumentState();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onQueryVideoSourceSelect(event) {
  const sourceId = event.target.value;
  if (!sourceId) {
    if (!state.queryVideoLibraryObject) {
      clearQueryVideoState();
    }
  } else {
    const source = state.videoSources.find((item) => item.source_id === sourceId);
    if (source) {
      setLibraryQueryVideoSource(source);
    }
  }
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onClearQueryVideo() {
  clearQueryVideoState();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onClearQueryVideoRange() {
  state.queryVideoRange = null;
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onClearQueryDocumentRange() {
  state.queryDocumentStartPageDraft = "";
  state.queryDocumentEndPageDraft = "";
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onQueryVideoRangeStartInput(event) {
  if (!state.queryVideoDurationMs) {
    return;
  }

  const startMs = Math.max(0, Math.round(Number(event.target.value) || 0));
  const currentEndMs = Math.max(currentQueryVideoEndMs(), startMs + 1);
  state.queryVideoRange = {
    start_ms: Math.min(startMs, state.queryVideoDurationMs - 1),
    end_ms: Math.min(currentEndMs, state.queryVideoDurationMs),
  };
  if (state.queryVideoRange.end_ms <= state.queryVideoRange.start_ms) {
    state.queryVideoRange.end_ms = Math.min(
      state.queryVideoDurationMs,
      state.queryVideoRange.start_ms + queryVideoRangeStep()
    );
  }
  renderWorkspace();
}

function onQueryVideoRangeEndInput(event) {
  if (!state.queryVideoDurationMs) {
    return;
  }

  const endMs = Math.min(
    state.queryVideoDurationMs,
    Math.max(1, Math.round(Number(event.target.value) || state.queryVideoDurationMs))
  );
  const currentStartMs = Math.min(currentQueryVideoStartMs(), endMs - 1);
  state.queryVideoRange = {
    start_ms: Math.max(0, currentStartMs),
    end_ms: endMs,
  };
  if (state.queryVideoRange.start_ms >= state.queryVideoRange.end_ms) {
    state.queryVideoRange.start_ms = Math.max(0, state.queryVideoRange.end_ms - queryVideoRangeStep());
  }
  renderWorkspace();
}

function onQueryVideoPreviewLoadedMetadata(event) {
  syncQueryVideoDurationFromVideoElement(event.currentTarget);
}

function onQueryDocumentRangeStartInput(event) {
  state.queryDocumentStartPageDraft = event.target.value.trim();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onQueryDocumentRangeEndInput(event) {
  state.queryDocumentEndPageDraft = event.target.value.trim();
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function resolveLibraryObjectQueryImage(visualUnitId) {
  const resultItem =
    state.searchOutcome?.results?.find((item) => item.visual_unit_id === visualUnitId) ?? null;
  if (resultItem?.kind === "image") {
    return {
      visual_unit_id: resultItem.visual_unit_id,
      kind: resultItem.kind,
      source_path: resultItem.source_path,
      preview: resultItem.preview,
    };
  }
  if (resultItem?.kind === "document_page") {
    return {
      visual_unit_id: resultItem.visual_unit_id,
      kind: resultItem.kind,
      source_path: resultItem.source_path,
      preview: resultItem.preview,
    };
  }

  const detailVisualUnit = state.selectedVisualUnit?.visual_unit;
  if (
    detailVisualUnit?.visual_unit_id === visualUnitId &&
    (detailVisualUnit.kind === "image" || detailVisualUnit.kind === "document_page")
  ) {
    return {
      visual_unit_id: detailVisualUnit.visual_unit_id,
      kind: detailVisualUnit.kind,
      source_path: detailVisualUnit.source_path,
      preview: state.selectedVisualUnit.preview,
    };
  }

  return null;
}

function resolveLibraryObjectQueryVideo(visualUnitId) {
  const resultItem =
    state.searchOutcome?.results?.find((item) => item.visual_unit_id === visualUnitId) ?? null;
  if (resultItem?.kind === "video_segment") {
    return {
      visual_unit_id: resultItem.visual_unit_id,
      kind: resultItem.kind,
      source_path: resultItem.source_path,
      locator: resultItem.locator,
      preview: resultItem.preview,
    };
  }

  const detailVisualUnit = state.selectedVisualUnit?.visual_unit;
  if (detailVisualUnit?.visual_unit_id === visualUnitId && detailVisualUnit.kind === "video_segment") {
    return {
      visual_unit_id: detailVisualUnit.visual_unit_id,
      kind: detailVisualUnit.kind,
      source_path: detailVisualUnit.source_path,
      locator: detailVisualUnit.locator,
      preview: state.selectedVisualUnit.preview,
    };
  }

  return null;
}

function resolveLibraryObjectQueryDocument(visualUnitId) {
  const resultItem =
    state.searchOutcome?.results?.find((item) => item.visual_unit_id === visualUnitId) ?? null;
  if (resultItem?.kind === "document_page") {
    const page = Number(resultItem.locator?.page ?? 0);
    return {
      visual_unit_id: resultItem.visual_unit_id,
      source_id: resultItem.source_id,
      kind: resultItem.kind,
      source_path: resultItem.source_path,
      locator:
        page > 0
          ? {
              start_page: page,
              end_page: page,
            }
          : null,
      preview: resultItem.preview,
    };
  }

  const detailVisualUnit = state.selectedVisualUnit?.visual_unit;
  if (detailVisualUnit?.visual_unit_id === visualUnitId && detailVisualUnit.kind === "document_page") {
    const page = Number(detailVisualUnit.locator?.page ?? 0);
    return {
      visual_unit_id: detailVisualUnit.visual_unit_id,
      source_id: detailVisualUnit.source_id,
      kind: detailVisualUnit.kind,
      source_path: detailVisualUnit.source_path,
      locator:
        page > 0
          ? {
              start_page: page,
              end_page: page,
            }
          : null,
      preview: state.selectedVisualUnit.preview,
    };
  }

  return null;
}

function onUseAsQueryImage(event) {
  const visualUnitId = event.currentTarget.dataset.useQueryVisualUnitId;
  const libraryObject = resolveLibraryObjectQueryImage(visualUnitId);
  if (!libraryObject) {
    state.globalError = {
      code: "not_supported",
      message: "当前只能把库内 image 或 document_page 对象作为查询图片。",
    };
    renderWorkspace();
    return;
  }

  clearQueryImageState();
  state.queryImageLibraryObject = libraryObject;
  state.searchMode = "image";
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onUseAsQueryVideo(event) {
  const visualUnitId = event.currentTarget.dataset.useQueryVideoVisualUnitId;
  const libraryObject = resolveLibraryObjectQueryVideo(visualUnitId);
  if (!libraryObject) {
    state.globalError = {
      code: "not_supported",
      message: "当前只能把库内 video_segment 对象作为查询视频片段。",
    };
    renderWorkspace();
    return;
  }

  setLibraryQueryVideoVisualUnit(libraryObject);
  state.searchMode = "video";
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

function onUseAsQueryDocument(event) {
  const visualUnitId = event.currentTarget.dataset.useQueryDocumentVisualUnitId;
  const libraryObject = resolveLibraryObjectQueryDocument(visualUnitId);
  if (!libraryObject) {
    state.globalError = {
      code: "not_supported",
      message: "当前只能把库内 document_page 对象作为查询文档。",
    };
    renderWorkspace();
    return;
  }

  setLibraryQueryDocumentVisualUnit(libraryObject);
  state.searchMode = "document";
  state.globalError = null;
  state.statusMessage = null;
  renderWorkspace();
}

async function loadVisualUnit(visualUnitId) {
  if (!state.selectedLibraryId) {
    return;
  }

  try {
    state.globalError = null;
    state.selectedVisualUnit = await apiRequest(
      `/libraries/${state.selectedLibraryId}/visual-units/${encodeURIComponent(visualUnitId)}`
    );
    renderWorkspace();
  } catch (error) {
    state.globalError = error;
    renderWorkspace();
  }
}

async function onSelectVisualUnit(event) {
  const visualUnitId = event.currentTarget.dataset.visualUnitId;
  await loadVisualUnit(visualUnitId);
}

refreshWorkspace({ keepSelection: false }).catch((error) => {
  state.globalError = error;
  renderWorkspace();
});
