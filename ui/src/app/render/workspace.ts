import {
  captureFocusedEditableState,
  currentDetailPanelRenderKey,
  lastRenderedDetailPanelKey,
  restoreFocusedEditableState,
  root,
  selectedLibrary,
  searchDetailSheetIsOpen,
  setLastRenderedDetailPanelKey,
  setQueryVideoDuration,
  state,
} from "../core";
import { renderStatusNotices } from "./shared/status";
import {
  onCancelJob,
  onClearQueryDocument,
  onClearQueryDocumentRange,
  onClearQueryImage,
  onClearQueryVideo,
  onClearQueryVideoRange,
  onClearSearchFilters,
  onCloseMobileSheet,
  onCreateLibrary,
  onCreateLibraryPopoverToggle,
  onDeleteLibrary,
  onDeleteSourceRoot,
  onEditProviderConfig,
  onEditSourceRoot,
  onGlobalContentTypeChange,
  onGlobalContentTypeEnabledChange,
  onGlobalContentTypeModelIdInput,
  onGlobalContentTypeProviderChange,
  onGlobalContentTypeVectorTypeChange,
  onGlobalModelTestComparisonFileInput,
  onGlobalModelTestComparisonModalityChange,
  onGlobalModelTestComparisonTextInput,
  onGlobalModelTestFileInput,
  onGlobalModelTestModalityChange,
  onGlobalModelTestTextInput,
  onImportPaths,
  onImportPathsInput,
  onLibraryContentTypeChange,
  onLibraryContentTypeEnabledChange,
  onLibraryContentTypeModelIdInput,
  onLibraryContentTypeProviderChange,
  onLibraryContentTypeVectorTypeChange,
  onLibraryIdInput,
  onLibraryModelTestComparisonFileInput,
  onLibraryModelTestComparisonModalityChange,
  onLibraryModelTestComparisonTextInput,
  onLibraryModelTestFileInput,
  onLibraryModelTestModalityChange,
  onLibraryModelTestTextInput,
  onLibraryNameInput,
  onLibraryOverrideModeChange,
  onLoadMoreSearchResults,
  onManageLibraryNameInput,
  onOpenHitLibraryContext,
  onOpenSettingsSection,
  onProviderBaseUrlInput,
  onProviderActiveModelChange,
  onProviderConfigSelect,
  onProviderDisplayNameInput,
  onProviderEnabledChange,
  onProviderKindInput,
  onProviderModelBackendInput,
  onProviderModelEnabledChange,
  onProviderModelIdInput,
  onProviderModelInputTypesInput,
  onProviderModelSelect,
  onProviderModelSupportsMixedInputsChange,
  onProviderModelVectorTypesInput,
  onProviderModelVersionInput,
  onDeleteProviderConfig,
  onDeleteProviderModelConfig,
  onQueryDocumentInput,
  onQueryDocumentRangeEndInput,
  onQueryDocumentRangeStartInput,
  onQueryImageInput,
  onQueryImagePaste,
  onQueryVideoInput,
  onQueryVideoPreviewLoadedMetadata,
  onQueryVideoRangeEndInput,
  onQueryVideoRangeStartInput,
  onQueryVideoSourceSelect,
  onRefreshSourceRoot,
  onRenameLibrary,
  onRescanSourceRoot,
  onResetLibraryContentTypes,
  onResetProviderConfigForm,
  onResetSourceRootEditor,
  onResumeJob,
  onRetryJob,
  onSearchFilterKindChange,
  onSearchFilterPathPrefixInput,
  onSearchFilterSourceTypeChange,
  onSearchFilterTimeRangeEndInput,
  onSearchFilterTimeRangeStartInput,
  onSearchSubmit,
  onSearchTextInput,
  onSelectContentTypeTab,
  onSelectInventorySource,
  onSelectLibrary,
  onSelectSearchMode,
  onSelectSearchResultLibraryFocus,
  onSelectSearchScope,
  onSelectSettingsSection,
  onSettingsDiagnosticsJobsToggle,
  onSelectVisualUnit,
  onSelectWorkspace,
  onSourceFilterRootChange,
  onSourceFilterStatusChange,
  onSourceFilterTypeChange,
  onSourceRootEnabledInput,
  onSourceRootExcludeGlobsInput,
  onSourceRootIncludeExtensionsInput,
  onSourceRootIncludeGlobsInput,
  onSourceRootPathInput,
  onSubmitGlobalContentTypes,
  onSubmitGlobalModelTest,
  onSubmitLibraryContentTypes,
  onSubmitLibraryModelTest,
  onSubmitProviderConfig,
  onSubmitProviderModelConfig,
  onSubmitProviderModelTest,
  onResetGlobalContentType,
  onSubmitSourceRoot,
  onToggleInventoryImport,
  onToggleLibraryArchive,
  onToggleInventoryLibraryMaintenance,
  onToggleInventorySourceManagement,
  onToggleSearchFiltersPanel,
  onToggleSourceRootAdvancedRules,
  onToggleSourceRoot,
  onStartCreateSourceRoot,
  onUseAsQueryDocument,
  onUseAsQueryImage,
  onUseAsQueryVideo,
  onUtilitiesAction,
} from "../events";
import {
  renderContextRail,
  renderWorkspaceSwitcher,
} from "./shell";
import { renderLibraryContext } from "./shared/library-context";
import { renderLibrarySourcesPanel } from "../workspaces/inventory";
import {
  renderSearchControls,
  renderSearchLoadingNotice,
  renderSearchNextStepDock,
  renderSearchOutcome,
  renderVisualUnitDetail,
} from "../workspaces/search";
import { renderSettingsPanel } from "../workspaces/settings";

export function patchWorkspaceMarkupPreservingDetail(
  nextMarkup,
  options: { preserveSearchDetailPanel?: boolean } = {}
) {
  const preserveSearchDetailPanel = Boolean(options.preserveSearchDetailPanel);
  if (!(root instanceof HTMLElement)) {
    return false;
  }

  const currentShell = root.querySelector("main.shell");
  const currentShellBar = currentShell?.querySelector(".shell-bar, .hero");
  const currentFrame = currentShell?.querySelector(".workspace-frame");
  const currentSidebar = currentFrame?.querySelector(".app-sidebar");
  const currentDesk = currentFrame?.querySelector(".workspace-desk");
  const currentLeft = currentDesk?.querySelector(".workspace-left");
  const currentRight = currentDesk?.querySelector(".workspace-right");
  if (
    !(currentShell instanceof HTMLElement) ||
    !(currentShellBar instanceof HTMLElement) ||
    !(currentFrame instanceof HTMLElement) ||
    !(currentSidebar instanceof HTMLElement) ||
    !(currentDesk instanceof HTMLElement) ||
    !(currentLeft instanceof HTMLElement)
  ) {
    return false;
  }

  const template = document.createElement("template");
  template.innerHTML = nextMarkup.trim();
  const nextShell = template.content.firstElementChild;
  const nextShellBar = nextShell?.querySelector(".shell-bar");
  const nextStatusStack = nextShell?.querySelector(".status-stack");
  const nextFrame = nextShell?.querySelector(".workspace-frame");
  const nextSidebar = nextFrame?.querySelector(".app-sidebar");
  const nextDesk = nextFrame?.querySelector(".workspace-desk");
  const nextLeft = nextDesk?.querySelector(".workspace-left");
  const nextRight = nextDesk?.querySelector(".workspace-right");
  if (
    !(nextShell instanceof HTMLElement) ||
    !(nextShellBar instanceof HTMLElement) ||
    !(nextFrame instanceof HTMLElement) ||
    !(nextSidebar instanceof HTMLElement) ||
    !(nextDesk instanceof HTMLElement) ||
    !(nextLeft instanceof HTMLElement)
  ) {
    return false;
  }

  const syncOptionalRegion = (parent, selector, nextNode) => {
    const currentNode = parent.querySelector(selector);
    if (currentNode instanceof HTMLElement && nextNode instanceof HTMLElement) {
      currentNode.replaceWith(nextNode);
      return;
    }
    if (currentNode instanceof HTMLElement) {
      currentNode.remove();
      return;
    }
    if (nextNode instanceof HTMLElement) {
      parent.append(nextNode);
    }
  };

  const replaceMatchingChild = (currentParent, nextParent, selector) => {
    const currentNode = currentParent.querySelector(selector);
    const nextNode = nextParent.querySelector(selector);
    if (currentNode instanceof HTMLElement && nextNode instanceof HTMLElement) {
      currentNode.replaceWith(nextNode);
      return;
    }
    if (currentNode instanceof HTMLElement) {
      currentNode.remove();
    }
  };

  const syncElementSiblingsAroundStableChild = (
    currentParent: Element,
    nextParent: Element,
    currentStableChild: Element,
    nextStableChild: Element
  ) => {
    const nextChildren = Array.from(nextParent.children);
    const nextStableIndex = nextChildren.indexOf(nextStableChild);
    if (nextStableIndex < 0) {
      return false;
    }

    Array.from(currentParent.children).forEach((child) => {
      if (child !== currentStableChild) {
        child.remove();
      }
    });
    nextChildren.slice(0, nextStableIndex).forEach((child) => {
      currentParent.insertBefore(child.cloneNode(true), currentStableChild);
    });
    nextChildren.slice(nextStableIndex + 1).forEach((child) => {
      currentParent.append(child.cloneNode(true));
    });
    return true;
  };

  const previewIdentity = (preview) =>
    preview.dataset.previewIdentity ?? preview.dataset.previewKey ?? "";

  const stablePreviewMatches = (currentPreview, nextPreview) =>
    currentPreview instanceof HTMLElement &&
    nextPreview instanceof HTMLElement &&
    previewIdentity(currentPreview) === previewIdentity(nextPreview) &&
    currentPreview.tagName === nextPreview.tagName;

  const transferStablePreviewNodes = (currentRegion, nextRegion, selector) => {
    const currentPreviews = Array.from(currentRegion.querySelectorAll(selector)).filter(
      (preview) => preview instanceof HTMLElement
    );
    nextRegion.querySelectorAll(selector).forEach((nextPreview) => {
      if (!(nextPreview instanceof HTMLElement)) {
        return;
      }
      const currentPreview = currentPreviews.find((preview) =>
        stablePreviewMatches(preview, nextPreview)
      );
      if (stablePreviewMatches(currentPreview, nextPreview)) {
        nextPreview.replaceWith(currentPreview);
      }
    });
  };

  const patchInventoryDetailPanel = (currentRegion, nextRegion) => {
    const currentPanel = currentRegion.querySelector('[data-testid="inventory-detail-panel"]');
    const nextPanel = nextRegion.querySelector('[data-testid="inventory-detail-panel"]');
    if (!(currentPanel instanceof HTMLElement) || !(nextPanel instanceof HTMLElement)) {
      return false;
    }

    const currentPreview = currentPanel.querySelector(
      '[data-testid="inventory-detail-preview"][data-preview-identity], [data-testid="inventory-detail-preview"][data-preview-key]'
    );
    const nextPreview = nextPanel.querySelector(
      '[data-testid="inventory-detail-preview"][data-preview-identity], [data-testid="inventory-detail-preview"][data-preview-key]'
    );
    if (!stablePreviewMatches(currentPreview, nextPreview)) {
      currentPanel.replaceWith(nextPanel);
      return true;
    }

    const currentCard = currentPanel.querySelector('[data-testid="inventory-detail-card"]');
    const nextCard = nextPanel.querySelector('[data-testid="inventory-detail-card"]');
    const currentPreviewWrap = currentPreview.closest(".ui-preview-surface");
    const nextPreviewWrap = nextPreview.closest(".ui-preview-surface");
    if (
      !(currentCard instanceof HTMLElement) ||
      !(nextCard instanceof HTMLElement) ||
      !(currentPreviewWrap instanceof HTMLElement) ||
      !(nextPreviewWrap instanceof HTMLElement)
    ) {
      return false;
    }

    currentPanel.className = nextPanel.className;
    replaceMatchingChild(currentPanel, nextPanel, ".mobile-sheet-bar");
    replaceMatchingChild(currentPanel, nextPanel, ".panel-head");

    currentCard.className = nextCard.className;
    currentPreviewWrap.className = nextPreviewWrap.className;
    return syncElementSiblingsAroundStableChild(
      currentCard,
      nextCard,
      currentPreviewWrap,
      nextPreviewWrap
    );
  };

  const patchInventoryLeftColumn = (currentColumn, nextColumn) => {
    const currentWorkspace = currentColumn.querySelector('[data-testid="inventory-panel"]');
    const nextWorkspace = nextColumn.querySelector('[data-testid="inventory-panel"]');
    const currentLayout = currentWorkspace?.querySelector(".inventory-layout");
    const nextLayout = nextWorkspace?.querySelector(".inventory-layout");
    if (
      !(currentWorkspace instanceof HTMLElement) ||
      !(nextWorkspace instanceof HTMLElement) ||
      !(currentLayout instanceof HTMLElement) ||
      !(nextLayout instanceof HTMLElement)
    ) {
      return false;
    }

    currentColumn.className = nextColumn.className;
    currentWorkspace.className = nextWorkspace.className;
    if (
      !syncElementSiblingsAroundStableChild(
        currentWorkspace,
        nextWorkspace,
        currentLayout,
        nextLayout
      )
    ) {
      return false;
    }

    currentLayout.className = nextLayout.className;
    const currentMain = currentLayout.querySelector(".inventory-panel-main");
    const nextMain = nextLayout.querySelector(".inventory-panel-main");
    if (currentMain instanceof HTMLElement && nextMain instanceof HTMLElement) {
      currentMain.replaceWith(nextMain);
    } else if (currentMain instanceof HTMLElement || nextMain instanceof HTMLElement) {
      return false;
    }

    return patchInventoryDetailPanel(currentLayout, nextLayout);
  };

  const patchSearchLeftColumn = (currentColumn, nextColumn) => {
    transferStablePreviewNodes(
      currentColumn,
      nextColumn,
      '[data-testid="result-preview"][data-preview-identity], [data-testid="result-preview"][data-preview-key]'
    );
    currentColumn.replaceWith(nextColumn);
    return true;
  };

  const patchSearchResultCard = (currentCard, nextCard) => {
    const currentSelect = currentCard.querySelector(".result-select");
    const nextSelect = nextCard.querySelector(".result-select");
    const currentVisual = currentSelect?.querySelector(".result-visual");
    const nextVisual = nextSelect?.querySelector(".result-visual");
    const currentPreview = currentVisual?.querySelector(
      '[data-testid="result-preview"][data-preview-identity], [data-testid="result-preview"][data-preview-key]'
    );
    const nextPreview = nextVisual?.querySelector(
      '[data-testid="result-preview"][data-preview-identity], [data-testid="result-preview"][data-preview-key]'
    );
    if (
      !(currentSelect instanceof HTMLElement) ||
      !(nextSelect instanceof HTMLElement) ||
      !(currentVisual instanceof HTMLElement) ||
      !(nextVisual instanceof HTMLElement) ||
      !stablePreviewMatches(currentPreview, nextPreview)
    ) {
      return false;
    }

    currentCard.className = nextCard.className;
    currentSelect.className = nextSelect.className;
    currentVisual.className = nextVisual.className;
    if (
      !syncElementSiblingsAroundStableChild(currentSelect, nextSelect, currentVisual, nextVisual)
    ) {
      return false;
    }

    syncOptionalRegion(currentCard, ".ui-object-list-actions", nextCard.querySelector(".ui-object-list-actions"));
    return true;
  };

  const patchSearchResultList = (currentSurface, nextSurface) => {
    const currentList = currentSurface.querySelector('[data-testid="result-list"]');
    const nextList = nextSurface.querySelector('[data-testid="result-list"]');
    if (!(currentList instanceof HTMLElement) || !(nextList instanceof HTMLElement)) {
      return false;
    }

    const currentCards = Array.from(currentList.querySelectorAll('[data-testid="result-card"]'));
    const nextCards = Array.from(nextList.querySelectorAll('[data-testid="result-card"]'));
    if (currentCards.length !== nextCards.length) {
      return false;
    }
    const cardIdentity = (card) =>
      `${card.getAttribute("data-kind") ?? ""}:${card.getAttribute("data-visual-unit-id") ?? ""}`;
    if (
      currentCards.some(
        (card, index) =>
          !(card instanceof HTMLElement) ||
          !(nextCards[index] instanceof HTMLElement) ||
          cardIdentity(card) !== cardIdentity(nextCards[index])
      )
    ) {
      return false;
    }

    currentSurface.className = nextSurface.className;
    currentSurface.setAttribute(
      "data-search-results-surface",
      nextSurface.getAttribute("data-search-results-surface") ?? ""
    );
    if (!syncElementSiblingsAroundStableChild(currentSurface, nextSurface, currentList, nextList)) {
      return false;
    }

    currentList.className = nextList.className;
    return currentCards.every((card, index) =>
      patchSearchResultCard(card, nextCards[index])
    );
  };

  const patchSearchCenterRegion = (currentDeskRegion, nextDeskRegion) => {
    const currentCenter = currentDeskRegion.querySelector(".workspace-center");
    const nextCenter = nextDeskRegion.querySelector(".workspace-center");
    if (currentCenter instanceof HTMLElement && nextCenter instanceof HTMLElement) {
      const currentSurface = currentCenter.querySelector('[data-testid="search-results-surface"]');
      const nextSurface = nextCenter.querySelector('[data-testid="search-results-surface"]');
      if (
        currentSurface instanceof HTMLElement &&
        nextSurface instanceof HTMLElement &&
        patchSearchResultList(currentSurface, nextSurface)
      ) {
        const currentPanel = currentSurface.closest(".search-results-panel");
        const nextPanel = nextSurface.closest(".search-results-panel");
        if (!(currentPanel instanceof HTMLElement) || !(nextPanel instanceof HTMLElement)) {
          currentCenter.replaceWith(nextCenter);
          return;
        }

        currentCenter.className = nextCenter.className;
        if (
          !syncElementSiblingsAroundStableChild(currentPanel, nextPanel, currentSurface, nextSurface)
        ) {
          currentCenter.replaceWith(nextCenter);
          return;
        }
        currentPanel.className = nextPanel.className;
        if (
          !syncElementSiblingsAroundStableChild(currentCenter, nextCenter, currentPanel, nextPanel)
        ) {
          currentCenter.replaceWith(nextCenter);
        }
      } else {
        transferStablePreviewNodes(
          currentCenter,
          nextCenter,
          '[data-testid="result-preview"][data-preview-identity], [data-testid="result-preview"][data-preview-key]'
        );
        currentCenter.replaceWith(nextCenter);
      }
      return;
    }
    if (currentCenter instanceof HTMLElement) {
      currentCenter.remove();
      return;
    }
    if (nextCenter instanceof HTMLElement) {
      currentDeskRegion.append(nextCenter);
    }
  };

  const patchRightColumn = () => {
    if (
      preserveSearchDetailPanel &&
      state.activeWorkspace === "search" &&
      currentRight instanceof HTMLElement &&
      nextRight instanceof HTMLElement
    ) {
      currentRight.className = nextRight.className;
      return true;
    }
    if (currentRight instanceof HTMLElement && nextRight instanceof HTMLElement) {
      currentRight.replaceWith(nextRight);
      return true;
    }
    if (currentRight instanceof HTMLElement) {
      currentRight.remove();
      return true;
    }
    if (nextRight instanceof HTMLElement) {
      currentDesk.append(nextRight);
      return true;
    }
    return true;
  };

  currentShellBar.replaceWith(nextShellBar);

  const currentStatusStack = currentShell.querySelector(".status-stack");
  const insertedShellBar = currentShell.querySelector(".shell-bar");
  if (nextStatusStack instanceof HTMLElement) {
    if (currentStatusStack instanceof HTMLElement) {
      currentStatusStack.replaceWith(nextStatusStack);
    } else if (insertedShellBar instanceof HTMLElement) {
      insertedShellBar.after(nextStatusStack);
    } else {
      return false;
    }
  } else if (currentStatusStack instanceof HTMLElement) {
    currentStatusStack.remove();
  }

  currentFrame.className = nextFrame.className;
  currentSidebar.replaceWith(nextSidebar);
  currentDesk.className = nextDesk.className;
  if (state.activeWorkspace === "inventory" && patchInventoryLeftColumn(currentLeft, nextLeft)) {
    syncOptionalRegion(currentDesk, ".workspace-center", nextDesk.querySelector(".workspace-center"));
  } else if (state.activeWorkspace === "search" && patchSearchLeftColumn(currentLeft, nextLeft)) {
    patchSearchCenterRegion(currentDesk, nextDesk);
  } else {
    currentLeft.replaceWith(nextLeft);
    syncOptionalRegion(currentDesk, ".workspace-center", nextDesk.querySelector(".workspace-center"));
  }
  if (!patchRightColumn()) {
    return false;
  }
  return true;
}

export function bindClickListeners(selector, handler, skipWithin = null) {
  document.querySelectorAll(selector).forEach((button) => {
    if (skipWithin instanceof HTMLElement && skipWithin.contains(button)) {
      return;
    }
    button.addEventListener("click", handler);
  });
}

export function renderWorkspace() {
  const library = selectedLibrary();
  const searchDetailSheetOpen = searchDetailSheetIsOpen();
  const isSearchWorkspace = state.activeWorkspace === "search";
  const searchMobileSheetViewport = window.matchMedia("(max-width: 720px)").matches;
  const searchReadinessAction = isSearchWorkspace ? renderSearchNextStepDock(library) : "";
  const searchHasResults = Boolean((state.searchOutcome?.results ?? []).length);
  const shouldShowSearchResultsColumn = isSearchWorkspace && (searchHasResults || state.searchInFlight);
  const shouldRenderSearchDetailPanel =
    isSearchWorkspace &&
    searchHasResults &&
    (!searchMobileSheetViewport || searchDetailSheetOpen);
  const searchLayoutClass = shouldShowSearchResultsColumn
    ? "workspace-desk-search workspace-desk-search-results"
    : "workspace-desk-search workspace-desk-search-stage-only";
  const searchStagePanelClass = searchHasResults
    ? "panel search-stage-panel search-stage-panel-reading"
    : "panel search-stage-panel";
  const focusedEditableState = captureFocusedEditableState();
  const detailPanelKey = currentDetailPanelRenderKey();
  const previousDetailPanel = root?.querySelector('[data-testid="detail-panel"]') ?? null;
  const shouldPreserveDetailPanel =
    isSearchWorkspace &&
    previousDetailPanel instanceof HTMLElement &&
    detailPanelKey !== null &&
    detailPanelKey === lastRenderedDetailPanelKey;

  const nextMarkup = `
    <main class="shell" data-testid="workspace-shell">
      <section class="shell-bar">
        ${renderContextRail(library)}
      </section>

      ${renderStatusNotices()}

      <section class="workspace-frame workspace-frame-main-only">
        <aside class="panel panel-tight app-sidebar" data-testid="app-sidebar">
          ${renderWorkspaceSwitcher()}
        </aside>
        <section class="workspace-desk ${
          isSearchWorkspace
            ? searchLayoutClass
            : state.activeWorkspace === "inventory"
              ? "workspace-desk-inventory"
              : "workspace-desk-settings"
        }">
          <aside class="workspace-column workspace-left">
            ${
              isSearchWorkspace
                ? `
                  <section class="${searchStagePanelClass}" data-testid="search-panel">
                    <div class="search-stage-layout search-stage-layout-single">
                      <div class="search-stage-main">
                        ${
                          searchHasResults
                            ? ""
                            : `
                              <div class="search-stage-head">
                                <h2>Search anything you want</h2>
                              </div>
                            `
                        }
                        ${renderSearchControls(library, searchHasResults)}
                        ${renderLibraryContext({ library, variant: "search-scope" })}
                        ${searchReadinessAction}
                        ${
                          searchHasResults || !state.searchOutcome
                            ? ""
                            : `
                              <div class="search-stage-inline-outcome" data-testid="search-inline-outcome">
                                ${renderSearchOutcome()}
                              </div>
                            `
                        }
                      </div>
                    </div>
                  </section>
                `
                : state.activeWorkspace === "inventory"
                  ? renderLibrarySourcesPanel(library)
                  : renderSettingsPanel(library)
            }
          </aside>

          ${
            shouldShowSearchResultsColumn
              ? `
                <section class="workspace-column workspace-center" data-testid="search-results-column">
                  <section class="panel search-results-panel">
                    ${renderSearchLoadingNotice()}
                    ${renderSearchOutcome()}
                  </section>
                </section>
              `
              : ""
          }

          ${
            shouldRenderSearchDetailPanel
              ? `
                <aside class="workspace-column workspace-right">
                  ${
                    searchDetailSheetOpen
                      ? `<button
                          type="button"
                          class="mobile-sheet-backdrop"
                          data-testid="detail-sheet-backdrop"
                          data-mobile-sheet-close="search"
                          aria-label="关闭结果详情"
                        ></button>`
                      : ""
                  }
                  <section
                    class="panel detail-panel mobile-sheet-panel ${searchDetailSheetOpen ? "mobile-sheet-open" : "mobile-sheet-closed"}"
                    data-testid="detail-panel"
                  >
                    <div class="mobile-sheet-bar">
                      <span class="mobile-sheet-handle" aria-hidden="true"></span>
                      <button
                        type="button"
                        class="ui-button ui-button-secondary mobile-sheet-close"
                        data-testid="detail-sheet-close-button"
                        data-mobile-sheet-close="search"
                      >
                        关闭
                      </button>
                    </div>
                    ${renderVisualUnitDetail()}
                  </section>
                </aside>
              `
              : ""
          }
        </section>
      </section>
    </main>
  `;

  const shouldPatchWorkspace =
    shouldPreserveDetailPanel ||
    state.activeWorkspace === "inventory" ||
    (isSearchWorkspace && searchHasResults);
  const patchedWorkspace =
    shouldPatchWorkspace &&
    patchWorkspaceMarkupPreservingDetail(nextMarkup, {
      preserveSearchDetailPanel: shouldPreserveDetailPanel,
    });
  if (!patchedWorkspace) {
    root.innerHTML = nextMarkup;
  }
  const preservedDetailPanel =
    shouldPreserveDetailPanel && patchedWorkspace ? previousDetailPanel : null;

  document.querySelectorAll("[data-workspace]").forEach((button) => {
    button.addEventListener("click", onSelectWorkspace);
  });
  document.querySelectorAll("[data-settings-section]").forEach((button) => {
    button.addEventListener("click", onSelectSettingsSection);
  });
  document.querySelectorAll("[data-open-settings-section]").forEach((button) => {
    button.addEventListener("click", onOpenSettingsSection);
  });
  document.querySelectorAll("[data-open-hit-library-id]").forEach((button) => {
    button.addEventListener("click", onOpenHitLibraryContext);
  });
  document.querySelectorAll("[data-content-type-scope]").forEach((button) => {
    button.addEventListener("click", onSelectContentTypeTab);
  });
  document.querySelectorAll("[data-library-override-mode]").forEach((button) => {
    button.addEventListener("click", onLibraryOverrideModeChange);
  });
  document.querySelector("#create-library-form")?.addEventListener("submit", onCreateLibrary);
  document.querySelectorAll("[data-library-rename-form]").forEach((form) => {
    form.addEventListener("submit", onRenameLibrary);
  });
  document.querySelector("#library-name")?.addEventListener("input", onLibraryNameInput);
  document.querySelector("#library-id")?.addEventListener("input", onLibraryIdInput);
  document.querySelectorAll("[data-library-management-display-name-input]").forEach((input) => {
    input.addEventListener("input", onManageLibraryNameInput);
  });
  document
    .querySelector('[data-testid="create-library-popover"]')
    ?.addEventListener("toggle", onCreateLibraryPopoverToggle);
  document.querySelectorAll("[data-library-archive-action]").forEach((button) => {
    button.addEventListener("click", onToggleLibraryArchive);
  });
  document.querySelectorAll("[data-library-delete-action]").forEach((button) => {
    button.addEventListener("click", onDeleteLibrary);
  });
  document.querySelector("#library-select")?.addEventListener("change", onSelectLibrary);
  document.querySelectorAll("[data-job-cancel-id]").forEach((button) => {
    button.addEventListener("click", onCancelJob);
  });
  document.querySelectorAll("[data-job-retry-id]").forEach((button) => {
    button.addEventListener("click", onRetryJob);
  });
  document.querySelectorAll("[data-job-resume-id]").forEach((button) => {
    button.addEventListener("click", onResumeJob);
  });
  document
    .querySelector("#provider-config-form")
    ?.addEventListener("submit", onSubmitProviderConfig);
  document
    .querySelector("#provider-config-reset-button")
    ?.addEventListener("click", onResetProviderConfigForm);
  document
    .querySelector("#provider-config-delete-button")
    ?.addEventListener("click", onDeleteProviderConfig);
  document
    .querySelector("#provider-config-id")
    ?.addEventListener("change", onProviderConfigSelect);
  document
    .querySelector("#provider-config-id")
    ?.addEventListener("input", onProviderConfigSelect);
  document
    .querySelector("#provider-display-name")
    ?.addEventListener("input", onProviderDisplayNameInput);
  document
    .querySelector("#provider-kind")
    ?.addEventListener("input", onProviderKindInput);
  document
    .querySelector("#provider-enabled")
    ?.addEventListener("change", onProviderEnabledChange);
  document
    .querySelector("#provider-base-url")
    ?.addEventListener("input", onProviderBaseUrlInput);
  document
    .querySelector("#provider-active-model")
    ?.addEventListener("change", onProviderActiveModelChange);
  document
    .querySelector("#provider-model-config-form")
    ?.addEventListener("submit", onSubmitProviderModelConfig);
  document
    .querySelector("#provider-model-config-delete-button")
    ?.addEventListener("click", onDeleteProviderModelConfig);
  document
    .querySelector("#provider-model-select")
    ?.addEventListener("change", onProviderModelSelect);
  document
    .querySelector("#provider-model-id")
    ?.addEventListener("input", onProviderModelIdInput);
  document
    .querySelector("#provider-model-enabled")
    ?.addEventListener("change", onProviderModelEnabledChange);
  document
    .querySelector("#provider-model-version")
    ?.addEventListener("input", onProviderModelVersionInput);
  document
    .querySelector("#provider-model-backend")
    ?.addEventListener("input", onProviderModelBackendInput);
  document
    .querySelector("#provider-model-input-types")
    ?.addEventListener("input", onProviderModelInputTypesInput);
  document
    .querySelector("#provider-model-vector-types")
    ?.addEventListener("input", onProviderModelVectorTypesInput);
  document
    .querySelector("#provider-model-supports-mixed-inputs")
    ?.addEventListener("change", onProviderModelSupportsMixedInputsChange);
  document
    .querySelector("#global-content-types-form")
    ?.addEventListener("submit", onSubmitGlobalContentTypes);
  document
    .querySelector("#global-content-types-reset-button")
    ?.addEventListener("click", onResetGlobalContentType);
  document
    .querySelector("#global-content-type")
    ?.addEventListener("change", onGlobalContentTypeChange);
  document
    .querySelector("#global-content-type-enabled")
    ?.addEventListener("change", onGlobalContentTypeEnabledChange);
  document
    .querySelector("#global-content-type-provider-id")
    ?.addEventListener("change", onGlobalContentTypeProviderChange);
  document
    .querySelector("#global-content-type-model-id")
    ?.addEventListener("change", onGlobalContentTypeModelIdInput);
  document
    .querySelector("#global-content-type-vector-type")
    ?.addEventListener("change", onGlobalContentTypeVectorTypeChange);
  document
    .querySelector("#global-model-test-form")
    ?.addEventListener("submit", onSubmitGlobalModelTest);
  document
    .querySelector("#provider-model-test-form")
    ?.addEventListener("submit", onSubmitProviderModelTest);
  document
    .querySelector("#global-model-test-modality")
    ?.addEventListener("change", onGlobalModelTestModalityChange);
  document
    .querySelector("#global-model-test-text")
    ?.addEventListener("input", onGlobalModelTestTextInput);
  document
    .querySelector("#global-model-test-file")
    ?.addEventListener("change", onGlobalModelTestFileInput);
  document
    .querySelector("#global-model-test-comparison-modality")
    ?.addEventListener("change", onGlobalModelTestComparisonModalityChange);
  document
    .querySelector("#global-model-test-comparison-text")
    ?.addEventListener("input", onGlobalModelTestComparisonTextInput);
  document
    .querySelector("#global-model-test-comparison-file")
    ?.addEventListener("change", onGlobalModelTestComparisonFileInput);
  document
    .querySelector("#provider-model-test-modality")
    ?.addEventListener("change", onGlobalModelTestModalityChange);
  document
    .querySelector("#provider-model-test-text")
    ?.addEventListener("input", onGlobalModelTestTextInput);
  document
    .querySelector("#provider-model-test-file")
    ?.addEventListener("change", onGlobalModelTestFileInput);
  document
    .querySelector("#provider-model-test-comparison-modality")
    ?.addEventListener("change", onGlobalModelTestComparisonModalityChange);
  document
    .querySelector("#provider-model-test-comparison-text")
    ?.addEventListener("input", onGlobalModelTestComparisonTextInput);
  document
    .querySelector("#provider-model-test-comparison-file")
    ?.addEventListener("change", onGlobalModelTestComparisonFileInput);
  document
    .querySelector("#library-content-types-form")
    ?.addEventListener("submit", onSubmitLibraryContentTypes);
  document
    .querySelector("#library-content-types-reset-button")
    ?.addEventListener("click", onResetLibraryContentTypes);
  document
    .querySelector("#library-content-type")
    ?.addEventListener("change", onLibraryContentTypeChange);
  document
    .querySelector("#library-content-type-enabled")
    ?.addEventListener("change", onLibraryContentTypeEnabledChange);
  document
    .querySelector("#library-content-type-provider-id")
    ?.addEventListener("change", onLibraryContentTypeProviderChange);
  document
    .querySelector("#library-content-type-model-id")
    ?.addEventListener("change", onLibraryContentTypeModelIdInput);
  document
    .querySelector("#library-content-type-vector-type")
    ?.addEventListener("change", onLibraryContentTypeVectorTypeChange);
  document
    .querySelector("#library-model-test-form")
    ?.addEventListener("submit", onSubmitLibraryModelTest);
  document
    .querySelector("#library-model-test-modality")
    ?.addEventListener("change", onLibraryModelTestModalityChange);
  document
    .querySelector("#library-model-test-text")
    ?.addEventListener("input", onLibraryModelTestTextInput);
  document
    .querySelector("#library-model-test-file")
    ?.addEventListener("change", onLibraryModelTestFileInput);
  document
    .querySelector("#library-model-test-comparison-modality")
    ?.addEventListener("change", onLibraryModelTestComparisonModalityChange);
  document
    .querySelector("#library-model-test-comparison-text")
    ?.addEventListener("input", onLibraryModelTestComparisonTextInput);
  document
    .querySelector("#library-model-test-comparison-file")
    ?.addEventListener("change", onLibraryModelTestComparisonFileInput);
  document.querySelector("#source-root-form")?.addEventListener("submit", onSubmitSourceRoot);
  document.querySelector("#source-root-reset-button")?.addEventListener("click", onResetSourceRootEditor);
  document
    .querySelector("#settings-diagnostics-jobs-disclosure")
    ?.addEventListener("toggle", onSettingsDiagnosticsJobsToggle);
  document.querySelector("#source-root-path")?.addEventListener("input", onSourceRootPathInput);
  document
    .querySelector("#source-root-enabled")
    ?.addEventListener("change", onSourceRootEnabledInput);
  document
    .querySelector("#source-root-include-globs")
    ?.addEventListener("input", onSourceRootIncludeGlobsInput);
  document
    .querySelector("#source-root-exclude-globs")
    ?.addEventListener("input", onSourceRootExcludeGlobsInput);
  document
    .querySelector("#source-root-include-extensions")
    ?.addEventListener("input", onSourceRootIncludeExtensionsInput);
  document
    .querySelector("[data-inventory-source-management-toggle]")
    ?.addEventListener("click", onToggleInventorySourceManagement);
  document
    .querySelector("[data-inventory-import-toggle]")
    ?.addEventListener("click", onToggleInventoryImport);
  document
    .querySelector("[data-inventory-library-maintenance-toggle]")
    ?.addEventListener("click", onToggleInventoryLibraryMaintenance);
  document
    .querySelector("[data-inventory-source-root-create]")
    ?.addEventListener("click", onStartCreateSourceRoot);
  document
    .querySelector("[data-source-root-advanced-toggle]")
    ?.addEventListener("click", onToggleSourceRootAdvancedRules);
  document.querySelector("#source-filter-root")?.addEventListener("change", onSourceFilterRootChange);
  document.querySelector("#source-filter-type")?.addEventListener("change", onSourceFilterTypeChange);
  document
    .querySelector("#source-filter-status")
    ?.addEventListener("change", onSourceFilterStatusChange);
  document.querySelectorAll("[data-source-root-edit-id]").forEach((button) => {
    button.addEventListener("click", onEditSourceRoot);
  });
  document.querySelectorAll("[data-source-root-refresh-id]").forEach((button) => {
    button.addEventListener("click", onRefreshSourceRoot);
  });
  document.querySelectorAll("[data-source-root-rescan-id]").forEach((button) => {
    button.addEventListener("click", onRescanSourceRoot);
  });
  document.querySelectorAll("[data-source-root-toggle-id]").forEach((button) => {
    button.addEventListener("click", onToggleSourceRoot);
  });
  document.querySelectorAll("[data-source-root-delete-id]").forEach((button) => {
    button.addEventListener("click", onDeleteSourceRoot);
  });
  document.querySelectorAll("[data-provider-edit-id]").forEach((button) => {
    button.addEventListener("click", onEditProviderConfig);
  });
  document.querySelectorAll("[data-utilities-action]").forEach((button) => {
    button.addEventListener("click", onUtilitiesAction);
  });
  document.querySelectorAll(".inventory-source-select[data-source-id]").forEach((button) => {
    button.addEventListener("click", onSelectInventorySource);
  });
  document.querySelector("#import-form")?.addEventListener("submit", onImportPaths);
  document.querySelector("#import-paths")?.addEventListener("input", onImportPathsInput);
  document.querySelector("#search-form")?.addEventListener("submit", onSearchSubmit);
  document
    .querySelector("#search-filter-toggle-button")
    ?.addEventListener("click", onToggleSearchFiltersPanel);
  document.querySelectorAll("[data-search-scope]").forEach((button) => {
    button.addEventListener("click", onSelectSearchScope);
  });
  document.querySelectorAll("[data-search-result-library-focus]").forEach((button) => {
    button.addEventListener("click", onSelectSearchResultLibraryFocus);
  });
  document.querySelector("#search-text")?.addEventListener("input", onSearchTextInput);
  document.querySelector("#search-filter-kind")?.addEventListener("change", onSearchFilterKindChange);
  document
    .querySelector("#search-filter-source-type")
    ?.addEventListener("change", onSearchFilterSourceTypeChange);
  document
    .querySelector("#search-filter-path-prefix")
    ?.addEventListener("input", onSearchFilterPathPrefixInput);
  document
    .querySelector("#search-filter-time-range-start")
    ?.addEventListener("input", onSearchFilterTimeRangeStartInput);
  document
    .querySelector("#search-filter-time-range-end")
    ?.addEventListener("input", onSearchFilterTimeRangeEndInput);
  document
    .querySelector("#clear-search-filters-button")
    ?.addEventListener("click", onClearSearchFilters);
  document
    .querySelector("#search-load-more-button")
    ?.addEventListener("click", onLoadMoreSearchResults);
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
  document.querySelectorAll("[data-search-mode]").forEach((button) => {
    button.addEventListener("click", onSelectSearchMode);
  });
  document.querySelectorAll("[data-visual-unit-id]").forEach((button) => {
    button.addEventListener("click", onSelectVisualUnit);
  });
  bindClickListeners("[data-use-query-visual-unit-id]", onUseAsQueryImage, preservedDetailPanel);
  bindClickListeners("[data-use-query-video-visual-unit-id]", onUseAsQueryVideo, preservedDetailPanel);
  bindClickListeners(
    "[data-use-query-document-visual-unit-id]",
    onUseAsQueryDocument,
    preservedDetailPanel
  );
  document.querySelectorAll("[data-mobile-sheet-close]").forEach((button) => {
    button.addEventListener("click", onCloseMobileSheet);
  });

  const queryVideoPreview = document.querySelector("#query-video-preview");
  if (queryVideoPreview instanceof HTMLVideoElement) {
    queryVideoPreview.addEventListener("loadedmetadata", onQueryVideoPreviewLoadedMetadata);
    if (queryVideoPreview.readyState >= 1) {
      syncQueryVideoDurationFromVideoElement(queryVideoPreview);
    }
  }

  document.querySelectorAll('[data-preview-kind="video"]').forEach((previewElement) => {
    if (previewElement instanceof HTMLVideoElement) {
      attachBoundedVideoPlayback(previewElement);
    }
  });

  setLastRenderedDetailPanelKey(detailPanelKey);
  restoreFocusedEditableState(focusedEditableState);
}

export function syncQueryVideoDurationFromVideoElement(videoElement) {
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

export function attachBoundedVideoPlayback(videoElement) {
  if (!(videoElement instanceof HTMLVideoElement)) {
    return;
  }
  if (videoElement.dataset.boundedPlaybackAttached === "true") {
    return;
  }
  videoElement.dataset.boundedPlaybackAttached = "true";

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
