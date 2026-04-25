export function renderUiIcon(
  kind:
    | "search"
    | "library"
    | "tools"
    | "settings"
    | "content-types"
    | "override"
    | "providers"
    | "experiment"
    | "diagnostics"
    | "filter"
    | "image"
    | "video"
    | "document"
) {
  const path =
    kind === "search"
      ? '<circle cx="11" cy="11" r="6.5"></circle><path d="m16 16 5 5"></path>'
      : kind === "library"
        ? '<path d="M4 5.5h16"></path><path d="M6 5.5v13.5a1 1 0 0 0 1 1h10a1 1 0 0 0 1-1V5.5"></path><path d="M9 5.5V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v1.5"></path>'
        : kind === "tools"
          ? '<path d="M12 3v4"></path><path d="M12 17v4"></path><path d="M3 12h4"></path><path d="M17 12h4"></path><path d="m5.6 5.6 2.8 2.8"></path><path d="m15.6 15.6 2.8 2.8"></path><path d="m18.4 5.6-2.8 2.8"></path><path d="m8.4 15.6-2.8 2.8"></path>'
          : kind === "settings"
            ? '<circle cx="12" cy="12" r="3.2"></circle><path d="M19.4 15a1 1 0 0 0 .2 1.1l.1.1a1 1 0 0 1 0 1.4l-1.1 1.1a1 1 0 0 1-1.4 0l-.1-.1a1 1 0 0 0-1.1-.2 1 1 0 0 0-.6.9v.3a1 1 0 0 1-1 1h-1.6a1 1 0 0 1-1-1v-.2a1 1 0 0 0-.7-1 1 1 0 0 0-1.1.2l-.1.1a1 1 0 0 1-1.4 0l-1.1-1.1a1 1 0 0 1 0-1.4l.1-.1a1 1 0 0 0 .2-1.1 1 1 0 0 0-.9-.6H4a1 1 0 0 1-1-1v-1.6a1 1 0 0 1 1-1h.2a1 1 0 0 0 1-.7 1 1 0 0 0-.2-1.1l-.1-.1a1 1 0 0 1 0-1.4L6 5.3a1 1 0 0 1 1.4 0l.1.1a1 1 0 0 0 1.1.2H9a1 1 0 0 0 .6-.9V4.4a1 1 0 0 1 1-1h1.6a1 1 0 0 1 1 1v.2a1 1 0 0 0 .7 1 1 1 0 0 0 1.1-.2l.1-.1a1 1 0 0 1 1.4 0L19 6.4a1 1 0 0 1 0 1.4l-.1.1a1 1 0 0 0-.2 1.1V9c0 .4.2.8.6.9h.3a1 1 0 0 1 1 1v1.6a1 1 0 0 1-1 1h-.2a1 1 0 0 0-1 .7z"></path>'
            : kind === "content-types"
              ? '<rect x="4" y="4" width="6" height="6" rx="1.2"></rect><rect x="14" y="4" width="6" height="6" rx="1.2"></rect><rect x="4" y="14" width="6" height="6" rx="1.2"></rect><rect x="14" y="14" width="6" height="6" rx="1.2"></rect>'
              : kind === "override"
                ? '<path d="M12 4 4 8l8 4 8-4-8-4Z"></path><path d="m4 12 8 4 8-4"></path><path d="m4 16 8 4 8-4"></path>'
                : kind === "providers"
                  ? '<path d="M9 7h6"></path><path d="M7.5 10.5h9"></path><path d="M6.5 14h11"></path><path d="M8 18h8"></path><path d="M5 7h.01"></path><path d="M19 10.5h.01"></path><path d="M5 14h.01"></path><path d="M19 18h.01"></path>'
                  : kind === "experiment"
                    ? '<path d="M10 3v5l-4.5 7.5A3 3 0 0 0 8 20h8a3 3 0 0 0 2.5-4.5L14 8V3"></path><path d="M8.5 3h7"></path><path d="M8 14h8"></path>'
                    : kind === "diagnostics"
                      ? '<path d="M4 13h3l2-5 4 9 2-4h5"></path><path d="M4 5.5h16"></path><path d="M4 18.5h16"></path>'
                      : kind === "filter"
                        ? '<path d="M4 6h16"></path><path d="M7 12h10"></path><path d="M10 18h4"></path>'
                        : kind === "image"
                          ? '<rect x="4" y="5" width="16" height="14" rx="2"></rect><path d="m7.5 15.5 3.2-3.6 2.8 2.8 2.5-2.7L19 15.5"></path><circle cx="9" cy="9" r="1.2"></circle>'
                          : kind === "video"
                            ? '<rect x="3.5" y="6" width="17" height="12" rx="2"></rect><path d="m10 9 5 3-5 3z"></path>'
                            : '<path d="M8 3.5h6l4 4V20a1 1 0 0 1-1 1H8a1 1 0 0 1-1-1V4.5a1 1 0 0 1 1-1z"></path><path d="M14 3.5V8h4"></path>';

  return `<svg class="ui-icon" viewBox="0 0 24 24" aria-hidden="true">${path}</svg>`;
}
