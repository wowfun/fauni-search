import { escapeHtml } from "../../selectors/common";

export type UiTagTone = "default" | "muted" | "ready" | "pending" | "error" | "score";
export type UiButtonTone = "primary" | "secondary" | "danger";
export type UiNoticeTone = "neutral" | "warning" | "error" | "success";

export function renderUiTag(
  label: string,
  tone: UiTagTone = "muted",
  options: { className?: string; testId?: string } = {}
) {
  const classes = ["ui-tag", `ui-tag-${tone}`, options.className].filter(Boolean).join(" ");
  return `<span class="${escapeHtml(classes)}"${options.testId ? ` data-testid="${escapeHtml(options.testId)}"` : ""}>${escapeHtml(label)}</span>`;
}

export function renderUiTagRow(tags: Array<{ label: string; tone?: UiTagTone; className?: string; testId?: string }>) {
  const renderedTags = tags
    .filter((tag) => tag.label)
    .map((tag) => renderUiTag(tag.label, tag.tone ?? "muted", tag))
    .join("");
  return renderedTags ? `<div class="ui-tag-row">${renderedTags}</div>` : "";
}

export function renderTypeTag(label: string, tone: UiTagTone = "muted", options: { className?: string; testId?: string } = {}) {
  return renderUiTag(label, tone, options);
}

export function renderStatusTag(
  label: string,
  tone: UiTagTone = "muted",
  options: { className?: string; testId?: string } = {}
) {
  return renderUiTag(label, tone, options);
}

export function renderLocatorTag(label: string, options: { className?: string; testId?: string } = {}) {
  return renderUiTag(label, "muted", options);
}

export function renderScoreTag(label: string, options: { className?: string; testId?: string } = {}) {
  return renderUiTag(`相似度 ${label}`, "score", options);
}

export function renderScopeTag(label: string, options: { className?: string; testId?: string } = {}) {
  return renderUiTag(label, "muted", options);
}

export function renderCountTag(label: string | number, options: { className?: string; testId?: string } = {}) {
  return renderUiTag(String(label), "muted", options);
}

export function renderUiButton(
  label: string,
  options: {
    type?: "button" | "submit" | "reset";
    tone?: UiButtonTone;
    className?: string;
    id?: string;
    testId?: string;
    attrs?: Record<string, string | number | boolean | null | undefined>;
    disabled?: boolean;
    form?: string;
    selected?: boolean;
  } = {}
) {
  const toneClass =
    options.tone === "danger"
      ? "ui-button-secondary ui-button-danger"
      : options.tone === "primary" || !options.tone
        ? "ui-button-primary"
        : "ui-button-secondary";
  const isSelectionControl = options.selected !== undefined;
  const classes = [
    "ui-button",
    toneClass,
    isSelectionControl ? "ui-selection-control" : "",
    options.className,
  ]
    .filter(Boolean)
    .join(" ");
  const attrEntries = Object.entries({
    ...(isSelectionControl ? { "data-ui-selected": options.selected ? "true" : "false" } : {}),
    ...(options.attrs ?? {}),
  }) as Array<[string, string | number | boolean | null | undefined]>;
  const attrs = attrEntries
    .filter(([, value]) => value !== false && value !== null && value !== undefined)
    .map(([key, value]) => (value === true ? ` ${escapeHtml(key)}` : ` ${escapeHtml(key)}="${escapeHtml(value)}"`))
    .join("");
  return `
    <button
      type="${escapeHtml(options.type ?? "button")}"
      class="${escapeHtml(classes)}"
      ${options.id ? `id="${escapeHtml(options.id)}"` : ""}
      ${options.testId ? `data-testid="${escapeHtml(options.testId)}"` : ""}
      ${options.form ? `form="${escapeHtml(options.form)}"` : ""}
      ${options.disabled ? "disabled" : ""}
      ${attrs}
    >
      ${escapeHtml(label)}
    </button>
  `;
}

export function renderStatusButton(
  label: string,
  tone: UiTagTone = "muted",
  options: {
    className?: string;
    testId?: string;
    prefixHtml?: string;
    childrenHtml?: string;
    attrs?: Record<string, string | number | boolean | null | undefined>;
    disabled?: boolean;
  } = {}
) {
  const attrs = Object.entries(options.attrs ?? {})
    .filter(([, value]) => value !== false && value !== null && value !== undefined)
    .map(([key, value]) => (value === true ? ` ${escapeHtml(key)}` : ` ${escapeHtml(key)}="${escapeHtml(value)}"`))
    .join("");
  return `
    <button
      type="button"
      class="${escapeHtml(["ui-tag", `ui-tag-${tone}`, options.className].filter(Boolean).join(" "))}"
      ${options.testId ? `data-testid="${escapeHtml(options.testId)}"` : ""}
      ${options.disabled ? "disabled" : ""}
      ${attrs}
    >
      ${options.prefixHtml ?? ""}
      <span>${escapeHtml(label)}</span>
      ${options.childrenHtml ?? ""}
    </button>
  `;
}

export function renderUiActionRow(actionsHtml: string, className = "") {
  const actions = actionsHtml.trim();
  if (!actions) {
    return "";
  }
  return `<div class="${escapeHtml(["ui-action-row", className].filter(Boolean).join(" "))}">${actions}</div>`;
}

export function renderUiMetaList(
  items: Array<{ label: string; value?: string | number | null; valueHtml?: string; valueClassName?: string }>,
  className = "ui-meta-list"
) {
  const rows = items
    .filter((item) => item.valueHtml || item.value !== undefined && item.value !== null && `${item.value}` !== "")
    .map(
      (item) => `
        <div>
          <dt>${escapeHtml(item.label)}</dt>
          <dd class="${escapeHtml(item.valueClassName ?? "")}">${item.valueHtml ?? escapeHtml(item.value ?? "")}</dd>
        </div>
      `
    )
    .join("");
  return rows ? `<dl class="${escapeHtml(className)}">${rows}</dl>` : "";
}

export function renderNotice(options: {
  tone?: UiNoticeTone;
  testId?: string;
  className?: string;
  eyebrow?: string;
  title?: string;
  titleTestId?: string;
  body?: string;
  bodyTestId?: string;
  bodyHtml?: string;
  actionsHtml?: string;
  childrenHtml?: string;
}) {
  const tone = options.tone ?? "neutral";
  const classes = ["ui-notice", `ui-notice-${tone}`, options.className].filter(Boolean).join(" ");
  return `
    <div class="${escapeHtml(classes)}"${options.testId ? ` data-testid="${escapeHtml(options.testId)}"` : ""}>
      ${options.eyebrow ? `<p class="eyebrow">${escapeHtml(options.eyebrow)}</p>` : ""}
      ${options.title ? `<h4${options.titleTestId ? ` data-testid="${escapeHtml(options.titleTestId)}"` : ""}>${escapeHtml(options.title)}</h4>` : ""}
      ${options.body ? `<p${options.bodyTestId ? ` data-testid="${escapeHtml(options.bodyTestId)}"` : ""}>${escapeHtml(options.body)}</p>` : ""}
      ${options.bodyHtml ?? ""}
      ${options.childrenHtml ?? ""}
      ${renderUiActionRow(options.actionsHtml ?? "")}
    </div>
  `;
}

export function renderEmptyState(message: string, options: { testId?: string; className?: string } = {}) {
  const classes = ["ui-empty", options.className].filter(Boolean).join(" ");
  return `<p class="${escapeHtml(classes)}"${options.testId ? ` data-testid="${escapeHtml(options.testId)}"` : ""}>${escapeHtml(message)}</p>`;
}
