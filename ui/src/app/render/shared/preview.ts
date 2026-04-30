import type { PreviewReference, SearchResultItem } from "../../../types";
import { escapeHtml, sourceName, sourceTypeDisplayName, assetTypeDisplayName } from "../../selectors/common";
import { selectedAssetOriginLibraryId } from "../../selectors/library";
import { state } from "../../state/store";

export function renderPreviewSurface(asset, preview, testId = "visual-preview") {
  const title = `${assetTypeDisplayName(asset.asset_type)} · ${sourceName(asset.source_uri)}`;
  const previewIdentity =
    asset.source_id ?? asset.asset_id ?? asset.source_uri ?? preview.url;
  const previewKey = `${asset.asset_id ?? asset.source_id ?? asset.source_uri}::${preview.url}`;

  if (asset.asset_type === "image") {
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

  if (asset.asset_type === "video_segment") {
    const startMs = asset.locator?.start_ms ?? 0;
    const endMs = asset.locator?.end_ms ?? 0;
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
  const title = `${assetTypeDisplayName(result.asset_type)} · ${sourceName(result.source_uri)}`;
  const previewIdentity =
    result.library_id && result.asset_id
      ? `${result.library_id}:${result.asset_id}`
      : result.asset_id ?? result.source_uri ?? result.preview.url;
  const previewKey = [
    result.library_id ?? "",
    result.asset_id ?? result.source_uri,
    result.preview.url,
    result.locator?.start_ms ?? "",
    result.locator?.end_ms ?? "",
  ].join("::");

  if (result.asset_type === "image") {
    return `
      <img
        class="result-preview-image"
        data-testid="result-preview"
        data-preview-identity="${escapeHtml(previewIdentity)}"
        data-preview-key="${escapeHtml(previewKey)}"
        src="${escapeHtml(result.preview.url)}"
        alt="${escapeHtml(title)}"
        loading="lazy"
      />
    `;
  }

  if (result.asset_type === "video_segment") {
    const startMs = result.locator?.start_ms ?? 0;
    const endMs = result.locator?.end_ms ?? 0;
    return `
      <video
        class="result-preview-video"
        data-testid="result-preview"
        data-preview-identity="${escapeHtml(previewIdentity)}"
        data-preview-key="${escapeHtml(previewKey)}"
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
  const asset = state.selectedAsset?.asset;
  const preview = state.selectedAsset?.preview;
  if (!asset || !preview) {
    return "";
  }
  return renderPreviewSurface(asset, preview, "visual-preview");
}
