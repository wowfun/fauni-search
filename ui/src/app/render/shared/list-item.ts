import { escapeHtml } from "../../selectors/common";

export function renderObjectListItem(options: {
  testId: string;
  className?: string;
  active?: boolean;
  dataAttrs?: Record<string, string | number | boolean | null | undefined>;
  selectClassName?: string;
  selectAttrs?: Record<string, string | number | boolean | null | undefined>;
  visualClassName?: string;
  visualHtml: string;
  bodyClassName?: string;
  topLineHtml?: string;
  title: string;
  titleRowClassName?: string;
  titleClassName?: string;
  titleAfterHtml?: string;
  metaHtml?: string;
  trailingClassName?: string;
  trailingHtml?: string;
  actionsClassName?: string;
  actionsHtml?: string;
}) {
  const classes = ["ui-object-list-item", options.className, options.active ? "active" : ""]
    .filter(Boolean)
    .join(" ");
  const dataAttrs = Object.entries(options.dataAttrs ?? {})
    .filter(([, value]) => value !== false && value !== null && value !== undefined)
    .map(([key, value]) => (value === true ? ` ${escapeHtml(key)}` : ` ${escapeHtml(key)}="${escapeHtml(value)}"`))
    .join("");
  const selectAttrs = Object.entries(options.selectAttrs ?? {})
    .filter(([, value]) => value !== false && value !== null && value !== undefined)
    .map(([key, value]) => (value === true ? ` ${escapeHtml(key)}` : ` ${escapeHtml(key)}="${escapeHtml(value)}"`))
    .join("");

  return `
    <li
      class="${escapeHtml(classes)}"
      data-testid="${escapeHtml(options.testId)}"
      ${dataAttrs}
    >
      <button
        type="button"
        class="${escapeHtml(["ui-object-list-select", options.selectClassName].filter(Boolean).join(" "))}"
        ${selectAttrs}
      >
        <div class="${escapeHtml(["ui-object-list-visual", options.visualClassName].filter(Boolean).join(" "))}">
          ${options.visualHtml}
        </div>
        <div class="${escapeHtml(["ui-object-list-body", options.bodyClassName].filter(Boolean).join(" "))}">
          ${options.topLineHtml ?? ""}
          <div class="${escapeHtml(["ui-object-list-title-row", options.titleRowClassName].filter(Boolean).join(" "))}">
            <strong class="${escapeHtml(["ui-object-list-title", options.titleClassName].filter(Boolean).join(" "))}">
              ${escapeHtml(options.title)}
            </strong>
            ${options.titleAfterHtml ?? ""}
          </div>
          ${options.metaHtml ?? ""}
        </div>
        ${
          options.trailingHtml
            ? `<div class="${escapeHtml(["ui-object-list-trailing", options.trailingClassName].filter(Boolean).join(" "))}">${options.trailingHtml}</div>`
            : ""
        }
      </button>
      ${
        options.actionsHtml
          ? `<div class="${escapeHtml(["ui-action-row", "ui-object-list-actions", options.actionsClassName].filter(Boolean).join(" "))}">${options.actionsHtml}</div>`
          : ""
      }
    </li>
  `;
}
