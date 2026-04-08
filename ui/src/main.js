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
  selectedLibraryId: "",
  importPathsDraft: "",
  searchTextDraft: "",
  importReceipt: null,
  selectedVisualUnit: null,
  searchOutcome: null,
  globalError: null,
  statusMessage: null,
};

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
    return '<p class="empty">还没有导入回执。提交路径后会在这里显示接受和拒绝结果。</p>';
  }

  const accepted = state.importReceipt.accepted.length
    ? `
        <div class="receipt-group">
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
        <div class="receipt-group">
          <h4>Rejected</h4>
          <ul class="data-list">
            ${state.importReceipt.rejected
              .map(
                (item) => `
                  <li>
                    <strong>${escapeHtml(item.reason_code)}</strong>
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
    ? `<p class="helper">任务 ${escapeHtml(state.importReceipt.job.job_id)} 当前处于 ${escapeHtml(state.importReceipt.job.phase)}。</p>`
    : `<p class="helper">这次提交没有创建后台任务。</p>`;

  return `${accepted}${rejected}${jobSummary}`;
}

function renderVisualPreview() {
  if (!state.selectedVisualUnit) {
    return `
      <div class="preview-placeholder">
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
        src="${escapeHtml(preview.url)}"
        alt="${escapeHtml(title)}"
        loading="lazy"
      />
    `;
  }

  return `
    <iframe
      class="preview-frame"
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
  return `
    <div class="detail-card">
      <div class="detail-preview">
        ${renderVisualPreview()}
      </div>
      <div class="detail-head">
        <div class="job-meta">
          <span class="pill ready">${escapeHtml(visualUnit.kind)}</span>
          ${page ? `<span class="pill muted">${escapeHtml(page)}</span>` : ""}
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
            <a href="${escapeHtml(preview.url)}" target="_blank" rel="noreferrer">打开预览</a>
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
    <ul class="job-list">
      ${state.jobs
        .map(
          (job) => `
            <li class="job-card">
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
    return '<p class="empty">结果列表会显示在这里。导入成功后，这里会以统一列表混排 `image` 和 `document_page`，并可直接打开右侧详情。</p>';
  }

  if (state.searchOutcome.error) {
    const details = state.searchOutcome.error.details?.index_lines ?? [];
    return `
      <div class="notice error">
        <h4>${escapeHtml(state.searchOutcome.error.code)}</h4>
        <p>${escapeHtml(state.searchOutcome.error.message)}</p>
        ${
          details.length
            ? `<ul class="data-list">
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
    <ul class="result-list">
      ${state.searchOutcome.results
        .map(
          (item) => `
            <li class="result-card ${item.visual_unit_id === selectedVisualUnitId() ? "active" : ""}">
              <button
                type="button"
                class="result-select"
                data-visual-unit-id="${escapeHtml(item.visual_unit_id)}"
              >
                <div class="result-topline">
                  <span class="pill ${item.kind === "image" ? "ready" : "pending"}">${escapeHtml(item.kind)}</span>
                  ${pageLabel(item.locator) ? `<span class="pill muted">${escapeHtml(pageLabel(item.locator))}</span>` : ""}
                </div>
                <strong>${escapeHtml(sourceName(item.source_path))}</strong>
                <span class="helper">${escapeHtml(item.source_path)}</span>
                <span class="helper">${escapeHtml(item.source_type)} · ${escapeHtml(JSON.stringify(item.locator))}</span>
              </button>
              <div class="inline-actions">
                <button type="button" class="secondary-button" data-visual-unit-id="${escapeHtml(item.visual_unit_id)}">查看详情</button>
                <a href="${escapeHtml(item.preview.url)}" target="_blank" rel="noreferrer">Preview</a>
              </div>
            </li>
          `
        )
        .join("")}
    </ul>
  `;
}

function renderWorkspace() {
  const library = selectedLibrary();

  root.innerHTML = `
    <main class="shell">
      <section class="hero">
        <p class="eyebrow">FauniSearch</p>
        <h1>100-text-search workspace</h1>
        <p class="summary">
          当前工作台已经接通真实的 ColQwen + Qdrant multivector 导入和文本搜索链路。左侧管理库和导入，中间执行搜索并浏览结果，右侧查看预览和对象详情。
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
            <form id="create-library-form" class="stack-form">
              <label>
                <span>新库名称</span>
                <input id="library-name" name="libraryName" type="text" placeholder="例如：demo-library" required />
              </label>
              <button type="submit">创建 multivector 库</button>
            </form>
            <label class="stack-form">
              <span>当前库</span>
              <select id="library-select" ${state.libraries.length ? "" : "disabled"}>
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
            <div class="context-card">
              ${
                library
                  ? `
                    <p class="eyebrow">Current</p>
                    <h3>${escapeHtml(library.name)}</h3>
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
            <form id="import-form" class="stack-form">
              <label>
                <span>本地路径</span>
                <textarea id="import-paths" rows="6" placeholder="/path/to/file.pdf&#10;/path/to/image.png" ${library ? "" : "disabled"}>${escapeHtml(state.importPathsDraft)}</textarea>
              </label>
              <button type="submit" ${library ? "" : "disabled"}>提交导入</button>
            </form>
            <div class="quick-card">
              <p class="eyebrow">Quick demo</p>
              <h3>真实索引和检索</h3>
              <p class="helper">使用仓库内置的 TATDQA 图片 fixture，可直接触发真实 document/image embedding、Qdrant 写入和文本搜索。浏览器文件选择不会暴露服务器可读的绝对路径，所以当前仍以路径输入为主。</p>
              <code>${escapeHtml(demoFixture.path)}</code>
              <div class="inline-actions">
                <button id="fill-demo-button" type="button" class="secondary-button" ${library ? "" : "disabled"}>填入 demo 路径和查询</button>
                <button id="run-demo-button" type="button" ${library ? "" : "disabled"}>导入并搜索 demo</button>
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
                <h2>文本搜索</h2>
              </div>
            </div>
            <form id="search-form" class="stack-form search-form">
              <label>
                <span>查询文本</span>
                <input id="search-text" type="text" value="${escapeHtml(state.searchTextDraft)}" placeholder="尝试输入财报页面中的问题或关键词" ${library ? "" : "disabled"} />
              </label>
              <button type="submit" ${library ? "" : "disabled"}>搜索</button>
            </form>
            ${renderSearchOutcome()}
          </section>
        </section>

        <aside class="workspace-column workspace-right">
          <section class="panel detail-panel">
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
  document.querySelector("#search-form")?.addEventListener("submit", onSearchText);
  document.querySelector("#search-text")?.addEventListener("input", onSearchTextInput);
  document.querySelector("#fill-demo-button")?.addEventListener("click", onFillDemo);
  document.querySelector("#run-demo-button")?.addEventListener("click", onRunDemo);
  document.querySelectorAll("[data-visual-unit-id]").forEach((button) => {
    button.addEventListener("click", onSelectVisualUnit);
  });
}

async function apiRequest(path, options = {}) {
  const response = await fetch(`/api${path}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...(options.headers ?? {}),
    },
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

async function refreshJob(jobId) {
  return apiRequest(`/jobs/${encodeURIComponent(jobId)}`);
}

async function refreshWorkspace(options) {
  await refreshLibraries(options);
  await refreshJobs();
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

async function onSearchText(event) {
  event.preventDefault();
  if (!state.selectedLibraryId) {
    return;
  }

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

  try {
    state.globalError = null;
    state.statusMessage = "正在执行真实 multivector 文本搜索...";
    renderWorkspace();
    await searchText(text);
    state.statusMessage = null;
    renderWorkspace();
  } catch (error) {
    state.searchOutcome = { error };
    state.statusMessage = null;
    renderWorkspace();
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

function onFillDemo() {
  setDemoDrafts();
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
