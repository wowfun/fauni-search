import type { PreviewReference, SearchResultItem } from "../../../types";
import { escapeHtml, sourceName, sourceTypeDisplayName, visualUnitKindDisplayName } from "../../selectors/common";
import { selectedVisualUnitOriginLibraryId } from "../../selectors/library";
import { state } from "../../state/store";

export function renderPreviewSurface(visualUnit, preview, testId = "visual-preview") {
  const title = `${visualUnit.kind} · ${sourceName(visualUnit.source_path)}`;
  const previewIdentity =
    visualUnit.source_id ?? visualUnit.visual_unit_id ?? visualUnit.source_path ?? preview.url;
  const previewKey = `${visualUnit.visual_unit_id ?? visualUnit.source_id ?? visualUnit.source_path}::${preview.url}`;

  if (visualUnit.kind === "image") {
    return `
      <img
        class="preview-image"
        data-testid="${escapeHtml(testId)}"
        data-preview-identity="${escapeHtml(previewIdentity)}"
        data-preview-key="${escapeHtml(previewKey)}"
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
        data-testid="${escapeHtml(testId)}"
        data-preview-identity="${escapeHtml(previewIdentity)}"
        data-preview-key="${escapeHtml(previewKey)}"
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
      data-testid="${escapeHtml(testId)}"
      data-preview-identity="${escapeHtml(previewIdentity)}"
      data-preview-key="${escapeHtml(previewKey)}"
      src="${escapeHtml(preview.url)}"
      title="${escapeHtml(title)}"
      loading="lazy"
    ></iframe>
  `;
}

export function renderSearchResultPreview(result: SearchResultItem) {
  const title = `${visualUnitKindDisplayName(result.kind)} · ${sourceName(result.source_path)}`;

  if (result.kind === "image") {
    return `
      <img
        class="result-preview-image"
        data-testid="result-preview"
        src="${escapeHtml(result.preview.url)}"
        alt="${escapeHtml(title)}"
        loading="lazy"
      />
    `;
  }

  if (result.kind === "video_segment") {
    const startMs = result.locator?.start_ms ?? 0;
    const endMs = result.locator?.end_ms ?? 0;
    return `
      <video
        class="result-preview-video"
        data-testid="result-preview"
        data-preview-kind="video"
        data-start-ms="${escapeHtml(startMs)}"
        data-end-ms="${escapeHtml(endMs)}"
        src="${escapeHtml(result.preview.url)}"
        muted
        playsinline
        preload="metadata"
      ></video>
    `;
  }

  return `
    <div
      class="result-preview-placeholder"
      data-testid="result-preview"
      role="img"
      aria-label="${escapeHtml(title)}"
    >
      <span class="result-preview-placeholder-sheet" aria-hidden="true"></span>
      <span class="result-preview-placeholder-label">${escapeHtml(sourceTypeDisplayName(result.source_type))}</span>
    </div>
  `;
}

export function selectedPreviewSurface() {
  const visualUnit = state.selectedVisualUnit?.visual_unit;
  const preview = state.selectedVisualUnit?.preview;
  if (!visualUnit || !preview) {
    return "";
  }
  return renderPreviewSurface(visualUnit, preview, "visual-preview");
}
