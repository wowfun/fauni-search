import { escapeHtml } from "../../selectors/common";
import { renderUiMetaList, renderUiTagRow, type UiTagTone } from "./primitives";

export type DetailCardTag = {
  label: string;
  tone?: UiTagTone;
  className?: string;
  testId?: string;
};

export type DetailCardMetaItem = {
  label: string;
  value?: string | number | null;
  valueHtml?: string;
  valueClassName?: string;
};

export function renderDetailCard(options: {
  testId: string;
  title: string;
  previewHtml: string;
  className?: string;
  previewClassName?: string;
  tags?: DetailCardTag[];
  afterHeadHtml?: string;
  actionsHtml?: string;
  metaItems?: DetailCardMetaItem[];
  footerHtml?: string;
}) {
  const className = ["ui-detail-card", options.className].filter(Boolean).join(" ");
  const previewClassName = ["ui-preview-surface", options.previewClassName].filter(Boolean).join(" ");
  const actions = options.actionsHtml?.trim()
    ? `<div class="ui-action-row">${options.actionsHtml}</div>`
    : "";
  const meta = options.metaItems?.length ? renderUiMetaList(options.metaItems) : "";

  return `
    <div class="${escapeHtml(className)}" data-testid="${escapeHtml(options.testId)}">
      <div class="${escapeHtml(previewClassName)}">
        ${options.previewHtml}
      </div>
      <div class="detail-head ui-detail-head">
        ${renderUiTagRow(options.tags ?? [])}
        <h4>${escapeHtml(options.title)}</h4>
      </div>
      ${options.afterHeadHtml ?? ""}
      ${actions}
      ${meta}
      ${options.footerHtml ?? ""}
    </div>
  `;
}
