import type { ModelTestData, SettingsSection, LibrarySnapshot } from "../../../types";
import {
  escapeHtml,
  formatEmbeddingCapabilities,
  formatResolvedModel,
  formatResolvedModelContext,
  modelTestModalityDisplayName,
} from "../../selectors/common";
import { settingsSectionLabel, settingsSectionPill } from "../../selectors/settings";
import { renderStatusTag, renderTypeTag } from "./primitives";

export function formatModelTestShape(shape: number[] | undefined) {
  if (!shape?.length) {
    return "[]";
  }
  return `[${shape.join(", ")}]`;
}

export function renderModelTestResult(testIdPrefix: string, result: ModelTestData | null) {
  if (!result) {
    return "";
  }

  return `
    <div class="model-test-result" data-testid="${testIdPrefix}-result">
      <div class="job-meta">
        ${renderStatusTag(formatResolvedModel(result.resolved_model), "ready", { testId: `${testIdPrefix}-resolved-model` })}
        ${renderTypeTag(result.operation_kind, "muted")}
        ${renderTypeTag(formatModelTestShape(result.vector_shape), "muted", { testId: `${testIdPrefix}-shape` })}
      </div>
      <p class="helper">${escapeHtml(formatResolvedModelContext(result.resolved_model))}</p>
      <p class="helper">${escapeHtml(formatEmbeddingCapabilities(result.resolved_model.embedding_capabilities, { includePrefix: true }))}</p>
      <p class="helper">${escapeHtml(result.resolved_model.message)}</p>
      <div class="detail-grid model-test-grid">
        <div class="detail-block">
          <h5>向量</h5>
          <pre data-testid="${testIdPrefix}-vectors">${escapeHtml(JSON.stringify(result.vectors, null, 2))}</pre>
        </div>
        ${
          result.pooled_vector?.length
            ? `
              <div class="detail-block">
                <h5>池化向量</h5>
                <pre data-testid="${testIdPrefix}-pooled-vector">${escapeHtml(JSON.stringify(result.pooled_vector, null, 2))}</pre>
              </div>
            `
            : ""
        }
      </div>
      <div class="detail-block">
        <h5>输入摘要</h5>
        <pre>${escapeHtml(JSON.stringify(result.input_summary, null, 2))}</pre>
      </div>
      ${
        result.comparison
          ? `
            <div class="detail-block">
              <h5>对照结果</h5>
              <div class="job-meta">
                ${renderTypeTag(result.comparison.operation_kind, "muted")}
                ${renderTypeTag(formatModelTestShape(result.comparison.vector_shape), "muted", { testId: `${testIdPrefix}-comparison-shape` })}
                ${renderStatusTag(result.comparison.similarity_to_primary.toFixed(6), "ready", { testId: `${testIdPrefix}-similarity` })}
              </div>
              <p class="helper">输入模态：${escapeHtml(modelTestModalityDisplayName(result.comparison.input_modality))}</p>
              <div class="detail-grid model-test-grid">
                <div class="detail-block">
                  <h5>对照向量</h5>
                  <pre data-testid="${testIdPrefix}-comparison-vectors">${escapeHtml(
                    JSON.stringify(result.comparison.vectors, null, 2)
                  )}</pre>
                </div>
                ${
                  result.comparison.pooled_vector?.length
                    ? `
                      <div class="detail-block">
                        <h5>对照池化向量</h5>
                        <pre data-testid="${testIdPrefix}-comparison-pooled-vector">${escapeHtml(
                          JSON.stringify(result.comparison.pooled_vector, null, 2)
                        )}</pre>
                      </div>
                    `
                    : ""
                }
              </div>
              <div class="detail-block">
                <h5>对照输入摘要</h5>
                <pre>${escapeHtml(JSON.stringify(result.comparison.input_summary, null, 2))}</pre>
              </div>
            </div>
          `
          : ""
      }
    </div>
  `;
}

export function renderSettingsStage(
  section: SettingsSection,
  library: LibrarySnapshot | null,
  body: string
) {
  const pill = settingsSectionPill(section, library);

  return `
    <section
      class="settings-stage"
      data-testid="settings-stage"
      data-settings-stage="${escapeHtml(section)}"
    >
      <div class="settings-stage-head">
        <h2 data-testid="settings-stage-title">${escapeHtml(settingsSectionLabel(section))}</h2>
        ${renderStatusTag(pill.label, pill.pillClass as any, { testId: "settings-stage-pill" })}
      </div>
      <div class="settings-stage-body">
        ${body}
      </div>
    </section>
  `;
}
