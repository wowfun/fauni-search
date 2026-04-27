import {
  availableContentTypeKeys,
  canExecuteSettingsModelTest,
  catalogEntriesForProvider,
  contentTypeDisplayName,
  currentDraftProviderSummary,
  endpoints,
  escapeHtml,
  formatBindingSource,
  formatEmbeddingCapabilities,
  formatExecutionInputTypes,
  formatResolvedContentModel,
  formatResolvedContentModelContext,
  libraryDisplayName,
  modelTestFileAccept,
  modelTestFileLabel,
  modelTestModalityDisplayName,
  providerProbePillClass,
  providerSelectionPillClass,
  selectedCatalogEntryForProvider,
  selectedCatalogEntryForSelection,
  selectedGlobalContentTypeBinding,
  selectedGlobalContentTypeKey,
  selectedGlobalModelSelection,
  selectedGlobalTestModalities,
  selectedLibrary,
  selectedLibraryContentTypeBinding,
  selectedLibraryContentTypeHasOverride,
  selectedLibraryContentTypeKey,
  selectedLibraryModelSelection,
  selectedLibraryTestModalities,
  selectedProviderConfig,
  selectedProviderModelSelection,
  selectedProviderTestModalities,
  selectionFromBinding,
  settingsModelTestSupportMessage,
  settingsSectionIcon,
  settingsSectionLabel,
  settingsSectionPill,
  state,
  vectorTypeOptionsForSelection,
  type ApiErrorPayload,
  type LibrarySnapshot,
  type ModelSelectionPayload,
  type ModelTestData,
  type ModelTestModality,
  type SettingsSection,
  type SourceInventoryItem,
} from "../core";
import { renderLibraryContext } from "../render/shared/library-context";
import { renderUiIcon } from "../render/shared/icons";
import { renderJobs } from "../render/shared/jobs";
import { renderEmptyState, renderNotice, renderStatusTag, renderUiButton, renderUiTag } from "../render/shared/primitives";
import { renderModelTestResult, renderSettingsStage } from "../render/shared/settings";

export function renderSettingsModelTestPanel(options: {
  scope: "global" | "library" | "provider";
  selection: ModelSelectionPayload;
  supportedModalities: ModelTestModality[];
  modalityDraft: ModelTestModality | "";
  textDraft: string;
  file: File | null;
  comparisonModalityDraft: ModelTestModality | "";
  comparisonTextDraft: string;
  comparisonFile: File | null;
  result: ModelTestData | null;
  error: ApiErrorPayload | null;
  pending: boolean;
}) {
  const {
    scope,
    selection,
    supportedModalities,
    modalityDraft,
    textDraft,
    file,
    comparisonModalityDraft,
    comparisonTextDraft,
    comparisonFile,
    result,
    error,
    pending,
  } = options;
  const testIdPrefix = `${scope}-model-test`;
  const inputModality = modalityDraft || supportedModalities[0] || "";
  const fileRequired = inputModality === "image";
  const comparisonFileRequired = comparisonModalityDraft === "image";
  const disabled =
    !supportedModalities.length || !canExecuteSettingsModelTest(selection) || pending;
  const catalogEntry = selectedCatalogEntryForSelection(selection);

  return `
    <section class="model-test-panel" data-testid="${testIdPrefix}-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">测试</p>
          <h3>${
            scope === "provider"
              ? "测试当前模型"
              : scope === "global"
                ? "测试当前全局模型"
                : "测试当前库模型"
          }</h3>
        </div>
      </div>
      <p class="helper" data-testid="${testIdPrefix}-draft-summary">
        ${escapeHtml(currentDraftProviderSummary(selection.provider_id))} · ${escapeHtml(selection.model_id)}
      </p>
      <p class="helper" data-testid="${testIdPrefix}-support-message">
        ${escapeHtml(settingsModelTestSupportMessage(selection, supportedModalities))}
      </p>
      ${
        catalogEntry
          ? `<p class="helper" data-testid="${scope}-model-capabilities">${escapeHtml(
              formatEmbeddingCapabilities(catalogEntry.embedding_capabilities, { includePrefix: true })
            )}</p>`
          : ""
      }
      <form id="${testIdPrefix}-form" class="stack-form" data-testid="${testIdPrefix}-form">
        <div class="ui-form-grid settings-form-grid">
          <label>
            <span>主输入模态</span>
            <select
              id="${testIdPrefix}-modality"
              data-testid="${testIdPrefix}-modality"
              ${supportedModalities.length ? "" : "disabled"}
            >
              ${supportedModalities.length
                ? supportedModalities
                    .map(
                      (modality) => `
                        <option value="${escapeHtml(modality)}" ${modality === inputModality ? "selected" : ""}>
                          ${escapeHtml(modelTestModalityDisplayName(modality))}
                        </option>
                      `
                    )
                    .join("")
                : '<option value="" selected>当前不可用</option>'}
            </select>
          </label>
          ${
            fileRequired
              ? `
                <label>
                  <span>${escapeHtml(modelTestFileLabel(inputModality))}</span>
                  <input
                    id="${testIdPrefix}-file"
                    data-testid="${testIdPrefix}-file"
                    type="file"
                    accept="${escapeHtml(modelTestFileAccept(inputModality))}"
                    ${supportedModalities.length ? "" : "disabled"}
                  />
                </label>
              `
              : `
                <label class="model-test-textarea">
                  <span>测试文本</span>
                  <textarea
                    id="${testIdPrefix}-text"
                    data-testid="${testIdPrefix}-text"
                    rows="4"
                    placeholder="输入一段测试文本"
                    ${supportedModalities.length ? "" : "disabled"}
                  >${escapeHtml(textDraft)}</textarea>
                </label>
              `
          }
        </div>
        ${
          fileRequired && file
            ? `<p class="helper" data-testid="${testIdPrefix}-file-name">${escapeHtml(file.name)} · ${escapeHtml(file.type || "application/octet-stream")}</p>`
            : ""
        }
        <div class="ui-form-grid settings-form-grid">
          <label>
            <span>对照输入模态</span>
            <select
              id="${testIdPrefix}-comparison-modality"
              data-testid="${testIdPrefix}-comparison-modality"
              ${supportedModalities.length ? "" : "disabled"}
            >
              <option value="" ${comparisonModalityDraft ? "" : "selected"}>不启用</option>
              ${supportedModalities
                .map(
                  (modality) => `
                    <option value="${escapeHtml(modality)}" ${
                      modality === comparisonModalityDraft ? "selected" : ""
                    }>
                      ${escapeHtml(modelTestModalityDisplayName(modality))}
                    </option>
                  `
                )
                .join("")}
            </select>
          </label>
          ${
            comparisonModalityDraft
              ? comparisonFileRequired
                ? `
                  <label>
                    <span>${escapeHtml(modelTestFileLabel(comparisonModalityDraft))}</span>
                    <input
                      id="${testIdPrefix}-comparison-file"
                      data-testid="${testIdPrefix}-comparison-file"
                      type="file"
                      accept="${escapeHtml(modelTestFileAccept(comparisonModalityDraft))}"
                      ${supportedModalities.length ? "" : "disabled"}
                    />
                  </label>
                `
                : `
                  <label class="model-test-textarea">
                    <span>对照测试文本</span>
                    <textarea
                      id="${testIdPrefix}-comparison-text"
                      data-testid="${testIdPrefix}-comparison-text"
                      rows="4"
                      placeholder="输入第二个用于比较的文本"
                      ${supportedModalities.length ? "" : "disabled"}
                    >${escapeHtml(comparisonTextDraft)}</textarea>
                  </label>
                `
              : ""
          }
        </div>
        ${
          comparisonFileRequired && comparisonFile
            ? `<p class="helper" data-testid="${testIdPrefix}-comparison-file-name">${escapeHtml(comparisonFile.name)} · ${escapeHtml(comparisonFile.type || "application/octet-stream")}</p>`
            : ""
        }
        ${
          error
            ? renderNotice({
                tone: "error",
                testId: `${testIdPrefix}-error`,
                title: error.code,
                body: error.message,
              })
            : ""
        }
        <div class="inline-actions">
          <button
            type="submit"
            data-testid="${testIdPrefix}-submit-button"
            ${disabled ? "disabled" : ""}
          >
            ${pending ? "测试中..." : "测试当前模型"}
          </button>
        </div>
      </form>
      ${renderModelTestResult(testIdPrefix, result)}
    </section>
  `;
}

export function renderProviderOptions(currentValue = "", includeEmpty = false) {
  const emptyOption = includeEmpty
    ? `<option value="" ${!currentValue ? "selected" : ""}>未选择</option>`
    : "";
  const hasCurrentValue =
    !!currentValue && state.providerConfigs.some((provider) => provider.provider_id === currentValue);
  const missingOption =
    currentValue && !hasCurrentValue
      ? `<option value="${escapeHtml(currentValue)}" selected>${escapeHtml(currentValue)} (已配置)</option>`
      : "";
  return `${emptyOption}${missingOption}${state.providerConfigs
    .map(
      (provider) => `
        <option value="${escapeHtml(provider.provider_id)}" ${provider.provider_id === currentValue ? "selected" : ""}>
          ${escapeHtml(provider.display_name)} (${escapeHtml(provider.provider_kind)}${provider.enabled ? "" : " · 已停用"})
        </option>
      `
    )
    .join("")}`;
}

export function renderContentTypeTabs(scope: "global" | "library", selected: string, contentTypes: string[]) {
  return `
    <div class="content-type-tabs" data-testid="${escapeHtml(scope)}-content-type-tabs">
      ${contentTypes
        .map(
          (contentType) => `
            <button
              type="button"
              class="content-type-tab ui-selection-control"
              data-testid="${escapeHtml(scope)}-content-type-tab-${escapeHtml(contentType)}"
              data-content-type-scope="${escapeHtml(scope)}"
              data-content-type="${escapeHtml(contentType)}"
              data-ui-selected="${contentType === selected ? "true" : "false"}"
            >
              <strong>${escapeHtml(contentTypeDisplayName(contentType))}</strong>
            </button>
          `
        )
        .join("")}
    </div>
  `;
}

export function providerRuntimeSnapshot(providerId: string) {
  return state.runtimeHealth?.providers.find((provider) => provider.provider_id === providerId) ?? null;
}

export function renderProviderRuntimeSummary(providerId: string, options: { editor?: boolean } = {}) {
  const runtimeProvider = providerRuntimeSnapshot(providerId);
  const testId = options.editor
    ? "provider-editor-runtime-summary"
    : `provider-runtime-summary-${providerId}`;

  if (!runtimeProvider) {
    return `
      <div class="provider-runtime-summary" data-testid="${escapeHtml(testId)}">
        <p class="helper">当前还没有拿到这个连接的运行时模型快照。</p>
      </div>
    `;
  }

  const facts = [
    runtimeProvider.model_id ? `当前模型 ${runtimeProvider.model_id}` : "当前模型 未解析",
    runtimeProvider.model_version ? `模型版本 ${runtimeProvider.model_version}` : "模型版本 未解析",
    runtimeProvider.model_revision ? `模型修订 ${runtimeProvider.model_revision}` : null,
  ]
    .filter(Boolean)
    .map((value) => `<span class="helper">${escapeHtml(String(value))}</span>`)
    .join("");

  return `
    <div class="provider-runtime-summary" data-testid="${escapeHtml(testId)}">
      ${facts}
    </div>
  `;
}

export function renderProviderConfigsPanel() {
  const editingProvider = selectedProviderConfig();
  const selectedModel = editingProvider?.models.find(
    (model) => model.model_id === state.providerModelIdDraft
  );
  const providerSelection = selectedProviderModelSelection();
  const providerModalities = selectedProviderTestModalities();
  const listMarkup = state.providerConfigs.length
    ? `
      <ul class="provider-profile-list" data-testid="provider-config-list">
        ${state.providerConfigs
          .map(
            (provider) => `
              <li class="provider-profile-row" data-testid="provider-config-row">
                <div class="provider-profile-main">
                  <strong>${escapeHtml(provider.display_name)}</strong>
                  <span class="helper">${escapeHtml(provider.provider_id)} · ${escapeHtml(provider.provider_kind)}</span>
                  ${
                    provider.base_url
                      ? `<span class="helper">${escapeHtml(provider.base_url)}</span>`
                      : ""
                  }
                  ${renderProviderRuntimeSummary(provider.provider_id)}
                </div>
                <div class="provider-profile-meta">
                  ${renderUiTag(provider.origin === "runtime_overlay" ? "已覆盖" : "基线", provider.origin === "runtime_overlay" ? "ready" : "muted")}
                  ${renderStatusTag(provider.probe?.status ?? "unknown", providerProbePillClass(provider.probe?.status) as any)}
                  ${renderUiButton("编辑", {
                    tone: "secondary",
                    attrs: { "data-provider-edit-id": provider.provider_id },
                  })}
                </div>
              </li>
            `
          )
          .join("")}
      </ul>
    `
    : renderEmptyState("当前还没有可用连接。");
  const providerOptions = state.providerConfigs
    .map(
      (provider) => `
        <option value="${escapeHtml(provider.provider_id)}">
          ${escapeHtml(provider.display_name)}
        </option>
      `
    )
    .join("");
  const modelOptions = editingProvider?.models
    .map(
      (model) => `
        <option value="${escapeHtml(model.model_id)}" ${model.model_id === state.providerModelIdDraft ? "selected" : ""}>
          ${escapeHtml(`${model.model_id}@${model.version}`)}
        </option>
      `
    )
    .join("") ?? "";

  return `
    <section class="panel settings-panel" data-testid="provider-configs-panel">
      <div class="provider-configs-layout">
        <div class="provider-config-list-surface">
          ${listMarkup}
        </div>
        <form id="provider-config-form" class="stack-form provider-config-editor" data-testid="provider-config-form">
          <label>
            <span>Provider ID</span>
            <input
              id="provider-config-id"
              data-testid="provider-config-id"
              list="provider-config-id-options"
              value="${escapeHtml(state.editingProviderId)}"
              placeholder="例如：local_sidecar"
            />
            <datalist id="provider-config-id-options">${providerOptions}</datalist>
          </label>
          <div class="ui-form-grid settings-form-grid">
            <label>
              <span>显示名称</span>
              <input
                id="provider-display-name"
                data-testid="provider-display-name"
                value="${escapeHtml(state.providerDisplayNameDraft)}"
                placeholder="Local Sidecar"
              />
            </label>
            <label>
              <span>Provider Kind</span>
              <input
                id="provider-kind"
                data-testid="provider-kind"
                value="${escapeHtml(state.providerKindDraft)}"
                placeholder="local_sidecar"
              />
            </label>
            <label class="checkbox-field">
              <input
                id="provider-enabled"
                data-testid="provider-enabled"
                type="checkbox"
                ${state.providerEnabledDraft ? "checked" : ""}
                ${!editingProvider ? "disabled" : ""}
              />
              <span>启用</span>
            </label>
            <label>
              <span>连接地址</span>
              <input
                id="provider-base-url"
                data-testid="provider-base-url"
                type="url"
                value="${escapeHtml(state.providerBaseUrlDraft)}"
                placeholder="https://dashscope.aliyuncs.com"
              />
            </label>
            <label>
              <span>默认模型</span>
              <select id="provider-active-model" data-testid="provider-active-model" ${editingProvider?.models.length ? "" : "disabled"}>
                ${modelOptions || `<option value="">先添加模型</option>`}
              </select>
            </label>
          </div>
          ${
            editingProvider
              ? `
                  <p class="helper">${escapeHtml(editingProvider.provider_id)} · ${escapeHtml(editingProvider.provider_kind)} · ${escapeHtml(editingProvider.origin)}</p>
                  ${renderProviderRuntimeSummary(editingProvider.provider_id, { editor: true })}
                `
              : `<p class="helper">输入 provider id 后可以创建 runtime overlay provider。</p>`
          }
          <div class="inline-actions">
            <button type="submit" data-testid="provider-config-submit-button" ${state.editingProviderId ? "" : "disabled"}>
              保存 Provider
            </button>
            <button
              type="button"
              id="provider-config-reset-button"
              data-testid="provider-config-reset-button"
              class="ui-button ui-button-secondary"
            >
              重置
            </button>
            ${renderUiButton("恢复基线", {
              tone: "secondary",
              testId: "provider-config-delete-button",
              attrs: { id: "provider-config-delete-button" },
              disabled: !editingProvider || editingProvider.origin !== "runtime_overlay",
            })}
          </div>
        </form>
      </div>
      <form id="provider-model-config-form" class="stack-form provider-model-editor" data-testid="provider-model-config-form">
        <div class="ui-form-grid settings-form-grid">
          <label>
            <span>选择模型</span>
            <select id="provider-model-select" data-testid="provider-model-select" ${editingProvider?.models.length ? "" : "disabled"}>
              ${modelOptions || `<option value="">暂无模型</option>`}
            </select>
          </label>
          <label>
            <span>Model ID</span>
            <input
              id="provider-model-id"
              data-testid="provider-model-id"
              value="${escapeHtml(state.providerModelIdDraft)}"
              placeholder="例如：Qwen/Qwen3-VL-Embedding-2B"
            />
          </label>
          <label class="checkbox-field">
            <input
              id="provider-model-enabled"
              data-testid="provider-model-enabled"
              type="checkbox"
              ${state.providerModelEnabledDraft ? "checked" : ""}
            />
            <span>启用模型</span>
          </label>
          <label>
            <span>版本</span>
            <input
              id="provider-model-version"
              data-testid="provider-model-version"
              value="${escapeHtml(state.providerModelVersionDraft)}"
              placeholder="main"
            />
          </label>
          <label>
            <span>Backend</span>
            <input
              id="provider-model-backend"
              data-testid="provider-model-backend"
              value="${escapeHtml(state.providerModelBackendDraft)}"
              placeholder="colqwen3_5"
            />
          </label>
          <label>
            <span>输入类型</span>
            <input
              id="provider-model-input-types"
              data-testid="provider-model-input-types"
              value="${escapeHtml(state.providerModelInputTypesDraft)}"
              placeholder="text, image"
            />
          </label>
          <label>
            <span>向量类型</span>
            <input
              id="provider-model-vector-types"
              data-testid="provider-model-vector-types"
              value="${escapeHtml(state.providerModelVectorTypesDraft)}"
              placeholder="single_vector"
            />
          </label>
          <label class="checkbox-field">
            <input
              id="provider-model-supports-mixed-inputs"
              data-testid="provider-model-supports-mixed-inputs"
              type="checkbox"
              ${state.providerModelSupportsMixedInputsDraft ? "checked" : ""}
            />
            <span>支持混合输入</span>
          </label>
        </div>
        <p class="helper" data-testid="provider-model-origin">
          ${escapeHtml(selectedModel ? `当前模型 ${selectedModel.origin}` : "输入 model id 后可以创建模型覆盖。")}
        </p>
        <div class="inline-actions">
          <button type="submit" data-testid="provider-model-config-submit-button" ${state.editingProviderId && state.providerModelIdDraft ? "" : "disabled"}>保存模型</button>
          ${renderUiButton("恢复模型基线", {
            tone: "secondary",
            testId: "provider-model-config-delete-button",
            attrs: { id: "provider-model-config-delete-button" },
            disabled: !selectedModel || selectedModel.origin !== "runtime_overlay",
          })}
        </div>
      </form>
      ${
        state.editingProviderId && state.providerModelIdDraft
          ? renderSettingsModelTestPanel({
              scope: "provider",
              selection: providerSelection,
              supportedModalities: providerModalities,
              modalityDraft: state.globalModelTestModalityDraft,
              textDraft: state.globalModelTestTextDraft,
              file: state.globalModelTestFile,
              comparisonModalityDraft: state.globalModelTestComparisonModalityDraft,
              comparisonTextDraft: state.globalModelTestComparisonTextDraft,
              comparisonFile: state.globalModelTestComparisonFile,
              result: state.globalModelTestResult,
              error: state.globalModelTestError,
              pending: state.globalModelTestPending,
            })
          : ""
      }
    </section>
  `;
}

export function renderModelIdOptions(providerId: string, currentValue: string, includeEmpty = false) {
  const entries = catalogEntriesForProvider(providerId);
  const hasCurrentValue = !!currentValue && entries.some((entry) => entry.model_id === currentValue);
  const emptyOption = includeEmpty
    ? `<option value="" ${!currentValue ? "selected" : ""}>未选择</option>`
    : "";
  const missingOption =
    currentValue && !hasCurrentValue
      ? `<option value="${escapeHtml(currentValue)}" selected>${escapeHtml(currentValue)} (已配置)</option>`
      : "";
  return `${emptyOption}${missingOption}${entries
    .map(
      (entry) => `
        <option value="${escapeHtml(entry.model_id)}" ${entry.model_id === currentValue ? "selected" : ""}>
          ${escapeHtml(`${entry.model_id}@${entry.model_version}`)}
        </option>
      `
    )
    .join("")}`;
}

export function renderVectorTypeOptions(selection: ModelSelectionPayload, currentValue: string) {
  return vectorTypeOptionsForSelection(selection, currentValue)
    .map(
      (value) => `
        <option value="${escapeHtml(value)}" ${value === currentValue ? "selected" : ""}>
          ${escapeHtml(value)}
        </option>
      `
    )
    .join("");
}

export function renderGlobalContentTypesPanel(includeTestPanel = true) {
  const contentType = selectedGlobalContentTypeKey();
  const binding = selectedGlobalContentTypeBinding();
  const selection = selectionFromBinding(binding);
  const catalogEntry = selectedCatalogEntryForProvider(selection.provider_id, selection.model_id);
  const supportedModalities = selectedGlobalTestModalities();
  const contentTypes = availableContentTypeKeys(state.globalContentTypes);
  const origin = state.globalContentTypeOrigins[contentType];

  return `
    <section class="panel settings-panel" data-testid="global-content-types-panel">
      ${renderContentTypeTabs("global", contentType, contentTypes)}
      <form id="global-content-types-form" class="stack-form" data-testid="global-content-types-form">
        <input id="global-content-type" data-testid="global-content-type" type="hidden" value="${escapeHtml(contentType)}" />
        <div class="ui-form-grid settings-form-grid">
          <label class="checkbox-field">
            <input
              id="global-content-type-enabled"
              data-testid="global-content-type-enabled"
              type="checkbox"
              ${binding.enabled ? "checked" : ""}
            />
            <span>启用</span>
          </label>
        </div>
        <div class="ui-form-grid settings-form-grid">
          <label>
            <span>连接</span>
            <select id="global-content-type-provider-id" data-testid="global-content-type-provider-id">
              ${renderProviderOptions(selection.provider_id)}
            </select>
          </label>
          <label>
            <span>模型</span>
            <select
              id="global-content-type-model-id"
              data-testid="global-content-type-model-id"
            >
              ${renderModelIdOptions(selection.provider_id, selection.model_id)}
            </select>
          </label>
          <label>
            <span>向量类型</span>
            <select
              id="global-content-type-vector-type"
              data-testid="global-content-type-vector-type"
            >
              ${renderVectorTypeOptions(selection, binding.vector_type)}
            </select>
          </label>
        </div>
        ${
          catalogEntry
            ? `
              <p class="helper" data-testid="model-catalog-summary">${escapeHtml(catalogEntry.message)}</p>
              <p class="helper" data-testid="global-model-capabilities">${escapeHtml(
                formatEmbeddingCapabilities(catalogEntry.embedding_capabilities, { includePrefix: true })
              )}</p>
            `
            : ""
        }
        <p class="helper" data-testid="global-content-type-summary">
          ${escapeHtml(
            `${contentTypeDisplayName(contentType)} → ${binding.model || "未配置"} · ${binding.vector_type || "未设置向量类型"} · ${binding.enabled ? "已启用" : "已停用"} · ${origin?.origin ?? "baseline"}`
          )}
        </p>
        <div class="inline-actions">
          <button type="submit" data-testid="global-content-types-submit-button">保存覆盖</button>
          <button
            type="button"
            id="global-content-types-reset-button"
            data-testid="global-content-types-reset-button"
            class="ui-button ui-button-secondary"
            ${origin?.has_runtime_overlay ? "" : "disabled"}
          >
            恢复基线
          </button>
        </div>
      </form>
      ${
        includeTestPanel
          ? renderSettingsModelTestPanel({
              scope: "global",
              selection,
              supportedModalities,
              modalityDraft: state.globalModelTestModalityDraft,
              textDraft: state.globalModelTestTextDraft,
              file: state.globalModelTestFile,
              comparisonModalityDraft: state.globalModelTestComparisonModalityDraft,
              comparisonTextDraft: state.globalModelTestComparisonTextDraft,
              comparisonFile: state.globalModelTestComparisonFile,
              result: state.globalModelTestResult,
              error: state.globalModelTestError,
              pending: state.globalModelTestPending,
            })
          : ""
      }
    </section>
  `;
}

export function renderLibraryContentTypesPanel(library: LibrarySnapshot | null, includeTestPanel = true) {
  if (!library) {
    return `
      <section class="panel settings-panel" data-testid="library-content-types-panel">
        ${renderEmptyState("先选择一个库，再编辑库级内容类型绑定。")}
      </section>
    `;
  }

  const contentType = selectedLibraryContentTypeKey();
  const binding = selectedLibraryContentTypeBinding();
  const selection = selectionFromBinding(binding);
  const catalogEntry = selectedCatalogEntryForProvider(selection.provider_id, selection.model_id);
  const supportedModalities = selectedLibraryTestModalities();
  const hasOverride = selectedLibraryContentTypeHasOverride();
  const origin = state.libraryContentTypeOrigins[contentType];
  const contentTypes = availableContentTypeKeys(
    state.globalContentTypes,
    state.libraryContentTypes,
    state.resolvedContentModels ? { content_types: state.resolvedContentModels.content_types } : null
  );

  return `
    <section class="panel settings-panel" data-testid="library-content-types-panel">
      ${renderContentTypeTabs("library", contentType, contentTypes)}
      <form id="library-content-types-form" class="stack-form" data-testid="library-content-types-form">
        <input id="library-content-type" data-testid="library-content-type" type="hidden" value="${escapeHtml(contentType)}" />
        <div class="override-mode-switch" data-testid="library-override-mode-switch">
          <button
            type="button"
            class="ui-button ui-button-secondary ui-selection-control"
            data-testid="library-override-mode-inherit"
            data-library-override-mode="inherit"
            data-ui-selected="${!hasOverride ? "true" : "false"}"
          >
            继承默认
          </button>
          <button
            type="button"
            class="ui-button ui-button-secondary ui-selection-control"
            data-testid="library-override-mode-override"
            data-library-override-mode="override"
            data-ui-selected="${hasOverride ? "true" : "false"}"
          >
            覆盖当前库
          </button>
        </div>
        <div class="override-mode-summary ${hasOverride ? "override-mode-summary-override" : ""}">
          ${renderUiTag(hasOverride ? "覆盖当前库" : "继承全局默认", hasOverride ? "ready" : "muted")}
          <span class="helper">${escapeHtml(contentTypeDisplayName(contentType))}</span>
        </div>
        <div class="ui-form-grid settings-form-grid">
          <label class="checkbox-field">
            <input
              id="library-content-type-enabled"
              data-testid="library-content-type-enabled"
              type="checkbox"
              ${binding.enabled ? "checked" : ""}
              ${hasOverride ? "" : "disabled"}
            />
            <span>启用</span>
          </label>
        </div>
        <div class="ui-form-grid settings-form-grid">
          <label>
            <span>连接</span>
            <select id="library-content-type-provider-id" data-testid="library-content-type-provider-id" ${hasOverride ? "" : "disabled"}>
              ${renderProviderOptions(selection.provider_id)}
            </select>
          </label>
          <label>
            <span>模型</span>
            <select
              id="library-content-type-model-id"
              data-testid="library-content-type-model-id"
              ${hasOverride ? "" : "disabled"}
            >
              ${renderModelIdOptions(selection.provider_id, selection.model_id)}
            </select>
          </label>
          <label>
            <span>向量类型</span>
            <select
              id="library-content-type-vector-type"
              data-testid="library-content-type-vector-type"
              ${hasOverride ? "" : "disabled"}
            >
              ${renderVectorTypeOptions(selection, binding.vector_type)}
            </select>
          </label>
        </div>
        ${
          catalogEntry
            ? `<p class="helper" data-testid="library-model-capabilities">${escapeHtml(
                formatEmbeddingCapabilities(catalogEntry.embedding_capabilities, { includePrefix: true })
              )}</p>`
            : ""
        }
        <p class="helper" data-testid="library-content-type-summary">
          ${escapeHtml(
            `${contentTypeDisplayName(contentType)} → ${binding.model || "未配置"} · ${binding.vector_type || "未设置向量类型"} · ${binding.enabled ? "已启用" : "已停用"} · ${origin?.origin ?? "inherited"}`
          )}
        </p>
        <div class="inline-actions">
          <button type="submit" data-testid="library-content-types-submit-button" ${hasOverride ? "" : "disabled"}>保存覆盖</button>
          <button
            type="button"
            id="library-content-types-reset-button"
            data-testid="library-content-types-reset-button"
            class="ui-button ui-button-secondary"
            ${origin?.has_runtime_overlay || hasOverride ? "" : "disabled"}
          >
            恢复继承
          </button>
        </div>
      </form>
      ${
        includeTestPanel
          ? renderSettingsModelTestPanel({
              scope: "library",
              selection,
              supportedModalities,
              modalityDraft: state.libraryModelTestModalityDraft,
              textDraft: state.libraryModelTestTextDraft,
              file: state.libraryModelTestFile,
              comparisonModalityDraft: state.libraryModelTestComparisonModalityDraft,
              comparisonTextDraft: state.libraryModelTestComparisonTextDraft,
              comparisonFile: state.libraryModelTestComparisonFile,
              result: state.libraryModelTestResult,
              error: state.libraryModelTestError,
              pending: state.libraryModelTestPending,
            })
          : ""
      }
    </section>
  `;
}

export function renderResolvedContentModelsPanel(library: LibrarySnapshot | null) {
  if (!library) {
    return "";
  }

  const rows = Object.entries(state.resolvedContentModels?.content_types ?? {})
    .map(
      ([contentType, selection]) => `
        <li class="provider-resolution-row">
          <div>
            <strong>${escapeHtml(contentTypeDisplayName(contentType))}</strong>
            <span class="helper">${escapeHtml(formatResolvedContentModel(selection))} · ${escapeHtml(formatBindingSource(selection.binding_source))}</span>
            <span class="helper">${escapeHtml(formatResolvedContentModelContext(selection))}</span>
            <span class="helper">${escapeHtml(`向量类型 ${selection.vector_type}`)}</span>
            <span class="helper">${escapeHtml(
              formatEmbeddingCapabilities(selection.embedding_capabilities, { includePrefix: true })
            )}</span>
            <span class="helper">${escapeHtml(selection.message)}</span>
          </div>
          ${renderStatusTag(selection.status, providerSelectionPillClass(selection.status) as any)}
        </li>
      `
    )
    .join("");

  return `
    <section class="panel settings-panel" data-testid="resolved-content-models-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">当前生效结果</p>
          <h2>${escapeHtml(libraryDisplayName(library))} 的当前生效模型</h2>
        </div>
      </div>
      <ul class="provider-resolution-list">${rows || `<li>${renderEmptyState("暂无当前生效模型。")}</li>`}</ul>
    </section>
  `;
}

export function renderVectorSpaceDiagnosticsPanel(library: LibrarySnapshot | null) {
  if (!library) {
    return "";
  }

  const rows = (state.vectorSpaceDiagnostics?.vector_spaces ?? [])
    .map((vectorSpace) => {
      const details = [
        vectorSpace.provider_id && vectorSpace.model_id
          ? `${vectorSpace.provider_id}/${vectorSpace.model_id}`
          : null,
        vectorSpace.model_version ? `版本 ${vectorSpace.model_version}` : null,
        vectorSpace.vector_type ? `向量类型 ${vectorSpace.vector_type}` : null,
        vectorSpace.content_types.length
          ? `内容类型 ${vectorSpace.content_types.map((contentType) => contentTypeDisplayName(contentType)).join("、")}`
          : null,
        typeof vectorSpace.retired_at_ms === "number"
          ? `停用时间 ${new Date(vectorSpace.retired_at_ms).toLocaleString()}`
          : null,
      ]
        .filter(Boolean)
        .map((value) => `<span class="helper">${escapeHtml(String(value))}</span>`)
        .join("");

      return `
        <li class="provider-resolution-row">
          <div>
            <strong>${escapeHtml(vectorSpace.vector_space_id)}</strong>
            ${details}
          </div>
          ${renderStatusTag(
            vectorSpace.lifecycle_state,
            providerSelectionPillClass(vectorSpace.lifecycle_state === "active" ? "available" : "degraded") as any
          )}
        </li>
      `;
    })
    .join("");

  return `
    <section class="panel settings-panel" data-testid="vector-space-diagnostics-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">诊断</p>
          <h2>${escapeHtml(libraryDisplayName(library))} 的执行空间</h2>
        </div>
      </div>
      <ul class="provider-resolution-list">${rows || `<li>${renderEmptyState("暂无执行空间诊断。")}</li>`}</ul>
    </section>
  `;
}

export function renderDiagnosticsJobsPanel(library: LibrarySnapshot | null) {
  const pendingJobs = library?.counts.pending_jobs ?? 0;
  const summaryLabel = !library
    ? "未选择库"
    : pendingJobs > 0
      ? `${pendingJobs} 进行中`
      : "无进行中任务";
  const summaryTone = pendingJobs > 0 ? "pending" : "muted";

  return `
    <section class="panel settings-panel" data-testid="settings-diagnostics-jobs-panel">
      <details
        id="settings-diagnostics-jobs-disclosure"
        class="support-disclosure support-disclosure-subtle settings-diagnostics-jobs-disclosure"
        data-testid="settings-diagnostics-jobs-disclosure"
        ${state.settingsDiagnosticsJobsOpen ? "open" : ""}
      >
        <summary>
          <span>任务</span>
          ${renderStatusTag(summaryLabel, summaryTone as any, {
            className: "settings-diagnostics-jobs-tag",
            testId: "settings-diagnostics-jobs-tag",
          })}
        </summary>
        <div class="support-disclosure-body">
          <div class="settings-diagnostics-jobs-body">
            <p class="helper">当前库的后台任务、回执与重试 / 继续动作统一收口在这里。</p>
            ${renderJobs()}
          </div>
        </div>
      </details>
    </section>
  `;
}

export function renderRuntimeHealthPanel() {
  const runtimeHealth = state.runtimeHealth;
  const processRows = runtimeHealth
    ? [runtimeHealth.app, runtimeHealth.qdrant]
        .map((snapshot) => {
          const details = Object.entries(snapshot.details ?? {})
            .map(
              ([key, value]) =>
                `<span class="helper">${escapeHtml(`${key} ${String(value)}`)}</span>`
            )
            .join("");
          return `
            <li class="provider-resolution-row">
              <div>
                <strong>${escapeHtml(snapshot.display_name)}</strong>
                <span class="helper">${escapeHtml(snapshot.message)}</span>
                <span class="helper">${escapeHtml(`最近检查 ${snapshot.last_checked_at}`)}</span>
                ${details}
              </div>
              ${renderStatusTag(snapshot.status, providerSelectionPillClass(snapshot.status) as any)}
            </li>
          `;
        })
        .join("")
    : "";
  const providerRows = runtimeHealth
    ? runtimeHealth.providers
        .map((provider) => {
          const details = [
            provider.model_id ? `${provider.provider_id}/${provider.model_id}` : provider.provider_id,
            provider.model_version ? `版本 ${provider.model_version}` : null,
            provider.model_revision ? `修订 ${provider.model_revision}` : null,
            provider.last_probed_at ? `最近探测 ${provider.last_probed_at}` : null,
          ]
            .filter(Boolean)
            .map((value) => `<span class="helper">${escapeHtml(String(value))}</span>`)
            .join("");
          const capabilities = provider.embedding_capabilities
            ? `<span class="helper">${escapeHtml(
                formatEmbeddingCapabilities(provider.embedding_capabilities, { includePrefix: true })
              )}</span>`
            : "";
          const executionInputs = provider.execution_input_types.length
            ? `<span class="helper" data-testid="runtime-provider-execution-input-types">${escapeHtml(
                formatExecutionInputTypes(provider.execution_input_types, { includePrefix: true })
              )}</span>`
            : "";
          const adapters = provider.runtime_adapters.length
            ? `<span class="helper">${escapeHtml(
                `运行时适配器 ${provider.runtime_adapters.join(", ")}`
              )}</span>`
            : "";

          return `
            <li class="provider-resolution-row">
              <div>
                <strong>${escapeHtml(provider.display_name)}</strong>
                <span class="helper">${escapeHtml(provider.message)}</span>
                ${details}
                ${capabilities}
                ${executionInputs}
                ${adapters}
              </div>
              ${renderStatusTag(provider.status, providerSelectionPillClass(provider.status) as any)}
            </li>
          `;
        })
        .join("")
    : "";

  return `
    <section class="panel settings-panel" data-testid="runtime-status-panel">
      <div class="panel-head">
        <div>
          <p class="eyebrow">运行时</p>
          <h2>运行时状态</h2>
        </div>
      </div>
      <ul class="provider-resolution-list">
        ${processRows || `<li>${renderEmptyState("暂无运行时状态快照。")}</li>`}
      </ul>
      <div class="inline-actions">
        <a href="${endpoints.appHealth}" target="_blank" rel="noreferrer">App 健康</a>
        <a href="${endpoints.sidecarHealth}" target="_blank" rel="noreferrer">Sidecar 健康</a>
        <a href="${endpoints.qdrantCollections}" target="_blank" rel="noreferrer">Qdrant</a>
      </div>
      <ul class="provider-resolution-list">
        ${providerRows || `<li>${renderEmptyState("暂无连接运行时诊断。")}</li>`}
      </ul>
    </section>
  `;
}

export function renderModelTestsSection(library: LibrarySnapshot | null) {
  const globalSelection = selectedGlobalModelSelection();
  const librarySelection = selectedLibraryModelSelection();

  return `
    <div class="settings-stack">
      ${renderSettingsModelTestPanel({
        scope: "global",
        selection: globalSelection,
        supportedModalities: selectedGlobalTestModalities(),
        modalityDraft: state.globalModelTestModalityDraft,
        textDraft: state.globalModelTestTextDraft,
        file: state.globalModelTestFile,
        comparisonModalityDraft: state.globalModelTestComparisonModalityDraft,
        comparisonTextDraft: state.globalModelTestComparisonTextDraft,
        comparisonFile: state.globalModelTestComparisonFile,
        result: state.globalModelTestResult,
        error: state.globalModelTestError,
        pending: state.globalModelTestPending,
      })}
      ${
        library
          ? renderSettingsModelTestPanel({
              scope: "library",
              selection: librarySelection,
              supportedModalities: selectedLibraryTestModalities(),
              modalityDraft: state.libraryModelTestModalityDraft,
              textDraft: state.libraryModelTestTextDraft,
              file: state.libraryModelTestFile,
              comparisonModalityDraft: state.libraryModelTestComparisonModalityDraft,
              comparisonTextDraft: state.libraryModelTestComparisonTextDraft,
              comparisonFile: state.libraryModelTestComparisonFile,
              result: state.libraryModelTestResult,
              error: state.libraryModelTestError,
              pending: state.libraryModelTestPending,
            })
          : ""
      }
    </div>
  `;
}

export function renderSettingsNavRail() {
  const sections: SettingsSection[] = [
    "providers",
    "content-types",
    "library-overrides",
    "diagnostics",
  ];

  return `
    <nav class="settings-nav-rail" data-testid="settings-nav-rail" aria-label="设置章节">
      <div class="settings-nav-rail-head">
        <p class="eyebrow">设置</p>
      </div>
      ${sections
        .map(
          (section) => {
            const pill = settingsSectionPill(section, selectedLibrary());
            return `
            <button
              type="button"
              class="settings-nav-button ui-selection-control"
              data-testid="settings-nav-${escapeHtml(section)}"
              data-settings-section="${escapeHtml(section)}"
              data-ui-selected="${state.selectedSettingsSection === section ? "true" : "false"}"
            >
              <span class="settings-nav-icon" data-ui-selection-icon="true">${renderUiIcon(settingsSectionIcon(section))}</span>
              <span class="settings-nav-copy">
                <strong>${escapeHtml(settingsSectionLabel(section))}</strong>
              </span>
              ${renderStatusTag(pill.label, pill.pillClass as any, { className: "settings-nav-tag" })}
            </button>
          `;
          }
        )
        .join("")}
    </nav>
  `;
}

export function renderSettingsPanel(library: LibrarySnapshot | null) {
  let activeSurface = "";
  if (state.selectedSettingsSection === "providers") {
    activeSurface = renderProviderConfigsPanel();
  } else if (state.selectedSettingsSection === "library-overrides") {
    activeSurface = `
      <div class="settings-dual-surface" data-testid="library-overrides-surface">
        ${renderLibraryContentTypesPanel(library, false)}
        ${renderResolvedContentModelsPanel(library)}
      </div>
    `;
  } else if (state.selectedSettingsSection === "diagnostics") {
    activeSurface = `
      <div class="settings-stack">
        ${renderRuntimeHealthPanel()}
        ${renderDiagnosticsJobsPanel(library)}
        ${renderVectorSpaceDiagnosticsPanel(library)}
      </div>
    `;
  } else {
    activeSurface = renderGlobalContentTypesPanel(false);
  }

  return `
    <section class="settings-workspace" data-testid="settings-workspace">
      ${renderLibraryContext({
        library,
        variant: "workspace-toolbar",
        capabilities: {
          showMetrics: true,
        },
      })}
      <div class="settings-layout">
        ${renderSettingsNavRail()}
        <div class="settings-active-surface">
          ${renderSettingsStage(state.selectedSettingsSection, library, activeSurface)}
        </div>
      </div>
    </section>
  `;
}
