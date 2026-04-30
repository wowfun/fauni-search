import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import {
  createLibrary,
  createTempSourceManagementFixtures,
  currentLibraryId,
  ensureCreateLibraryPopoverOpen,
  ensureManageLibraryPopoverOpen,
  fixtureImagePath,
  latestJobId,
  openInventorySourceManagement,
  librarySourceCard,
  openInventoryWorkspace,
  openSourceRootAdvancedRules,
  openSourcePreparationPanel,
  sourceRootCard,
  waitForNewLatestJobCompleted,
  writeSourceManagementPdf,
} from "./fixtures";

export function registerLibraryScenarios() {
  test("library creation shows display name and custom library id separately", async ({ page }) => {
    const displayName = `Invoice Demo ${Date.now()}`;
    const libraryId = `invoice-demo-${Date.now()}`;

    await page.goto("/");
    await expect(page.getByTestId("workspace-shell")).toBeVisible();
    await (await ensureCreateLibraryPopoverOpen(page)).fill(displayName);
    await page.getByTestId("library-id-input").fill(libraryId);
    await page.getByTestId("create-library-button").click();

    await expect(page.getByTestId("library-select")).toContainText(`${displayName} (${libraryId})`);
  });

  test("library management can rename archive restore and delete the current library", async ({
    page,
  }) => {
    const originalName = await createLibrary(page, "library-manage");
    const libraryId = await currentLibraryId(page);
    const renamed = `${originalName} archive`;

    const manageNameInput = await ensureManageLibraryPopoverOpen(page);
    await manageNameInput.fill(renamed);
    await page.getByTestId("inventory-rename-library-button").click();

    await expect(page.getByTestId("library-select")).toContainText(renamed);
    await expect(page.getByTestId("workspace-library-lifecycle")).toHaveText("活跃");

    await ensureManageLibraryPopoverOpen(page);
    page.once("dialog", (dialog) => dialog.accept());
    await page.getByTestId("inventory-toggle-library-archive-button").click();

    await expect(page.getByTestId("workspace-library-lifecycle")).toHaveText("已归档");
    await expect(page.getByTestId("library-select")).toContainText(`${renamed} (${libraryId}) · 已归档`);

    await ensureManageLibraryPopoverOpen(page);
    await page.getByTestId("inventory-toggle-library-archive-button").click();

    await expect(page.getByTestId("workspace-library-lifecycle")).toHaveText("活跃");
    await expect(page.getByTestId("library-select")).not.toContainText(
      `${renamed} (${libraryId}) · 已归档`
    );

    await ensureManageLibraryPopoverOpen(page);
    page.once("dialog", (dialog) => dialog.accept());
    await page.getByTestId("inventory-delete-library-button").click();

    await expect(page.getByTestId("library-select")).not.toHaveValue(libraryId);
    await expect(page.getByTestId("library-select")).not.toContainText(`${renamed} (${libraryId})`);
  });

  test("inventory workspace keeps current library administration in the same management surface", async ({
    page,
  }) => {
    const originalName = await createLibrary(page, "inventory-library-manage");
    const libraryId = await currentLibraryId(page);
    const renamed = `${originalName} inventory`;

    await openInventoryWorkspace(page);
    await expect(page.getByTestId("workspace-library-toolbar")).toBeVisible();
    await expect(page.getByTestId("library-select")).toContainText(originalName);
    await expect(page.getByTestId("workspace-library-lifecycle")).toHaveText("活跃");

    await page.getByTestId("inventory-manage-library-name-input").fill(renamed);
    await page.getByTestId("inventory-rename-library-button").click();

    await expect(page.getByTestId("library-select")).toContainText(renamed);

    page.once("dialog", (dialog) => dialog.accept());
    await page.getByTestId("inventory-toggle-library-archive-button").click();
    await expect(page.getByTestId("workspace-library-lifecycle")).toHaveText("已归档");
    await expect(page.getByTestId("workspace-library-metrics")).toBeVisible();

    await page.getByTestId("inventory-toggle-library-archive-button").click();
    await expect(page.getByTestId("workspace-library-lifecycle")).toHaveText("活跃");

    page.once("dialog", (dialog) => dialog.accept());
    await page.getByTestId("inventory-delete-library-button").click();
    await expect(page.getByTestId("library-select")).not.toHaveValue(libraryId);
    await expect(page.getByTestId("library-select")).not.toContainText(`${renamed} (${libraryId})`);
  });

  test("settings owns current library overrides instead of duplicating them in inventory", async ({
    page,
  }) => {
    await createLibrary(page, "inventory-library-overrides-link");

    await openInventoryWorkspace(page);
    await expect(page.getByTestId("inventory-library-config-strip")).toHaveCount(0);
    await expect(page.getByTestId("workspace-library-toolbar")).toBeVisible();
    await expect(page.getByTestId("inventory-manage-library-name-input")).toBeVisible();
    await expect(page.getByTestId("current-library-card")).toHaveCount(0);

    await page.getByTestId("workspace-tab-settings").click();
    await page.getByTestId("settings-nav-library-overrides").click();
    await expect(page.getByTestId("settings-workspace")).toBeVisible();
    await expect(page.getByTestId("settings-stage-title")).toHaveText("当前库覆盖");
    await expect(page.getByTestId("workspace-library-toolbar")).toBeVisible();
    await expect(page.getByTestId("workspace-library-name")).toBeVisible();
    await expect(page.getByTestId("workspace-library-readiness")).toBeVisible();
    await expect(page.getByTestId("workspace-library-metrics")).toBeVisible();
    await expect(page.getByTestId("inventory-manage-library-name-input")).toHaveCount(0);
    await expect(page.getByTestId("inventory-toggle-library-archive-button")).toHaveCount(0);
    await expect(page.getByTestId("inventory-delete-library-button")).toHaveCount(0);
    await expect(page.getByTestId("open-create-library-button")).toHaveCount(0);
    await expect(page.getByTestId("current-library-card")).toHaveCount(0);
    await expect(page.getByTestId("library-content-types-panel")).toBeVisible();
  });

  test("manage library popover stays stable inside inventory across workspace polling", async ({
    page,
  }) => {
    const originalName = await createLibrary(page, "library-manage-popover");
    await openInventoryWorkspace(page);
    const manageNameInput = page.getByTestId("inventory-manage-library-name-input");
    const manageCard = page.getByTestId("workspace-library-toolbar");
    const draftName = `${originalName} draft`;

    await expect(manageCard).toBeVisible();
    await manageNameInput.fill(draftName);
    await manageCard.click();
    await expect(manageNameInput).not.toBeFocused();

    await page.waitForTimeout(3500);

    await expect(manageCard).toBeVisible();
    await expect(manageNameInput).toHaveValue(draftName);
  });
}

export function registerSourceManagementScenarios() {
  test("inventory workspace explains search readiness from source-root health", async ({ page }) => {
    const fixtures = createTempSourceManagementFixtures();
    try {
      await createLibrary(page, "inventory-health");
      const libraryId = await currentLibraryId(page);

      await openInventoryWorkspace(page);
      await expect(page.getByTestId("workspace-library-readiness")).toContainText(
        "尚未接入来源根"
      );
      await expect(page.getByTestId("inventory-source-controls")).toBeVisible();
      await expect(page.getByTestId("inventory-source-management-summary")).toContainText(
        "还没有来源根"
      );
      await expect(page.getByTestId("inventory-source-management-panel")).toHaveCount(0);
      await expect(page.getByTestId("source-root-form")).toHaveCount(0);

      await openInventorySourceManagement(page);
      await expect(page.getByTestId("source-root-form")).toHaveCount(0);
      await expect(page.getByTestId("source-root-empty")).toBeVisible();
      await page.getByTestId("inventory-source-root-create-button").click();
      await expect(page.getByTestId("source-root-form")).toBeVisible();
      await expect(page.getByTestId("source-root-advanced-rules-panel")).toHaveCount(0);
      await page.getByTestId("source-root-path-input").fill(fixtures.tempDir);
      await page.getByTestId("source-root-submit-button").click();
      await expect(page.getByTestId("source-root-form")).toHaveCount(0);

      await expect(page.getByTestId("workspace-library-readiness")).toContainText("等待内容");
      const rootHealthCard = sourceRootCard(page, fixtures.tempDir);
      await expect(rootHealthCard).toBeVisible();
      await expect(rootHealthCard).toContainText("watching");

      const previousJobId = await latestJobId(page, libraryId);
      await rootHealthCard.locator("[data-source-root-refresh-id]").click();
      await waitForNewLatestJobCompleted(page, libraryId, previousJobId);

      await expect(page.getByTestId("workspace-library-readiness")).toContainText("可搜索");
      await expect(page.getByTestId("workspace-library-metrics")).toContainText("来源根");
      await expect(rootHealthCard).toContainText("watching");
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("inventory source preparation keeps its folded editor and advanced rules by default", async ({
    page,
  }) => {
    const fixtures = createTempSourceManagementFixtures();
    try {
      await createLibrary(page, "source-prep-persistence");
      const libraryId = await currentLibraryId(page);

      await openSourcePreparationPanel(page);
      await expect(page.getByTestId("inventory-source-management-panel")).toBeVisible();
      await expect(page.getByTestId("inventory-source-controls")).toBeVisible();
      await expect(page.getByTestId("source-root-advanced-rules-panel")).toHaveCount(0);
      await page.getByTestId("source-root-path-input").fill(fixtures.tempDir);
      await openSourceRootAdvancedRules(page);
      await page.getByTestId("source-root-exclude-globs-input").fill("chart.png");
      await page.getByTestId("source-root-submit-button").click();

      const rootCard = sourceRootCard(page, fixtures.tempDir);
      await expect(rootCard).toBeVisible();
      await expect(rootCard).toContainText("排除规则 1");

      const previousJobId = await latestJobId(page, libraryId);
      await rootCard.locator("[data-source-root-refresh-id]").click();
      await waitForNewLatestJobCompleted(page, libraryId, previousJobId);

      await expect(rootCard).toContainText("最近动作：refresh");
      await rootCard.locator("[data-source-root-edit-id]").click();
      await expect(page.getByTestId("source-root-advanced-rules-panel")).toBeVisible();
      await expect(page.getByTestId("source-root-exclude-globs-input")).toHaveValue("chart.png");
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("source management can create edit toggle refresh rescan and filter inventory", async ({ page }) => {
  const fixtures = createTempSourceManagementFixtures();
  try {
    await createLibrary(page, "source-management");
    const libraryId = await currentLibraryId(page);

    await openInventorySourceManagement(page);
    await expect(page.getByTestId("source-root-form")).toHaveCount(0);
    await page.getByTestId("inventory-source-root-create-button").click();
    await expect(page.getByTestId("source-root-form")).toBeVisible();
    await expect(page.getByTestId("source-root-advanced-rules-panel")).toHaveCount(0);
    await page.getByTestId("source-root-path-input").fill(fixtures.tempDir);
    await page.getByTestId("source-root-submit-button").click();

    const rootCard = sourceRootCard(page, fixtures.tempDir);
    await expect(rootCard).toBeVisible();
    await expect(rootCard).toContainText("watching");

    let previousJobId = await latestJobId(page, libraryId);
    await rootCard.locator("[data-source-root-refresh-id]").click();
    await waitForNewLatestJobCompleted(page, libraryId, previousJobId);
    await expect(rootCard).toContainText("最近动作：refresh");

    await openInventoryWorkspace(page);
    await expect(page.getByTestId("workspace-library-metrics")).toBeVisible();
    await expect(page.getByTestId("inventory-action-manage-source-roots")).toBeVisible();
    await expect(page.getByTestId("inventory-action-refresh-library")).toBeVisible();
    await expect(page.getByTestId("inventory-action-rescan-library")).toBeVisible();
    await expect(page.getByTestId("inventory-action-library-maintenance")).toBeVisible();
    await expect(page.getByTestId("inventory-library-maintenance-panel")).toHaveCount(0);
    await expect(page.getByTestId("library-refresh-button")).toHaveCount(0);
    await expect(page.getByTestId("library-rescan-button")).toHaveCount(0);
    await expect(page.getByTestId("inventory-filter-pills")).toContainText("当前显示全部来源");

    const imageCard = librarySourceCard(page, "chart.png");
    const pdfCard = librarySourceCard(page, "report.pdf");
    await expect(page.getByTestId("library-source-card")).toHaveCount(2);
    await expect(imageCard).toContainText("1 个对象");
    await expect(pdfCard).toContainText("2 个对象");
    await imageCard.locator("button").click();
    await expect(page.getByTestId("inventory-detail-preview")).toBeVisible();
    await expect(page.getByTestId("inventory-detail-card")).toContainText(
      `file://${fixtures.imagePath}`
    );
    await expect(page.getByTestId("inventory-preview-link")).toBeVisible();
    await page.getByTestId("inventory-use-as-query-image-button").click();
    await expect(page.getByTestId("search-panel")).toBeVisible();
    await expect(page.getByTestId("query-image-preview")).toBeVisible();
    await openInventoryWorkspace(page);

    previousJobId = await latestJobId(page, libraryId);
    await rootCard.locator("[data-source-root-rescan-id]").click();
    await waitForNewLatestJobCompleted(page, libraryId, previousJobId);
    await expect(rootCard).toContainText("最近动作：rescan");

    await page.getByTestId("source-filter-type").selectOption("pdf");
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(pdfCard).toBeVisible();

    await page.getByTestId("source-filter-root").selectOption({ label: fixtures.tempDir });
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(pdfCard).toBeVisible();

    await page.getByTestId("source-filter-type").selectOption("");
    await expect(page.getByTestId("library-source-card")).toHaveCount(2);

    await rootCard.locator("[data-source-root-edit-id]").click();
    await expect(page.getByTestId("source-root-advanced-rules-panel")).toHaveCount(0);
    await openSourceRootAdvancedRules(page);
    await page.getByTestId("source-root-exclude-globs-input").fill("chart.png");
    await page.getByTestId("source-root-submit-button").click();
    await expect(page.getByTestId("source-root-form")).toHaveCount(0);
    await expect(rootCard).toContainText("排除规则 1");
    await rootCard.locator("[data-source-root-edit-id]").click();
    await expect(page.getByTestId("source-root-advanced-rules-panel")).toBeVisible();
    await expect(page.getByTestId("source-root-exclude-globs-input")).toHaveValue("chart.png");
    await page.getByTestId("source-root-reset-button").click();
    await expect(page.getByTestId("source-root-form")).toHaveCount(0);

    previousJobId = await latestJobId(page, libraryId);
    await page.getByTestId("inventory-action-refresh-library").click();
    await waitForNewLatestJobCompleted(page, libraryId, previousJobId);
    await expect(rootCard).toContainText("最近动作：refresh");

    await page.getByTestId("source-filter-status").selectOption("active");
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(pdfCard).toContainText("2 个对象");
    await expect(pdfCard).not.toContainText("正常");

    await page.getByTestId("source-filter-status").selectOption("out_of_scope");
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(imageCard).toContainText("rule_excluded");

    await page.getByTestId("source-filter-status").selectOption("");
    await rootCard.locator("[data-source-root-toggle-id]").click();
    await expect(rootCard).toContainText("disabled");
    await expect(rootCard.locator("[data-source-root-refresh-id]")).toBeDisabled();

    await page.getByTestId("source-filter-status").selectOption("out_of_scope");
    await expect(page.getByTestId("library-source-card")).toHaveCount(2);
    await expect(imageCard).toContainText("source_root_disabled");
    await expect(pdfCard).toContainText("source_root_disabled");

    await page.getByTestId("source-filter-status").selectOption("");
    await rootCard.locator("[data-source-root-toggle-id]").click();
    await expect(rootCard).toContainText("watching");
    await expect(rootCard.locator("[data-source-root-refresh-id]")).toBeEnabled();

    previousJobId = await latestJobId(page, libraryId);
    await page.getByTestId("inventory-action-rescan-library").click();
    await waitForNewLatestJobCompleted(page, libraryId, previousJobId);
    await expect(rootCard).toContainText("最近动作：rescan");

    await page.getByTestId("source-filter-status").selectOption("active");
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(pdfCard).toContainText("2 个对象");
    await expect(pdfCard).not.toContainText("正常");

    await page.getByTestId("source-filter-status").selectOption("out_of_scope");
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(imageCard).toContainText("rule_excluded");
  } finally {
    fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
  }
  });

  test("inventory source list gives long file paths a wider desktop reading column", async ({
    page,
  }) => {
    const fixtures = createTempSourceManagementFixtures();
    const longDirectory = path.join(
      fixtures.tempDir,
      "quarterly-board-review-packets",
      "finance-committee-shared-exports"
    );
    const longPdfName =
      "2026-q1-board-review-package-with-supplemental-appendix-and-variance-notes.pdf";
    const longPdfPath = path.join(longDirectory, longPdfName);
    fs.mkdirSync(longDirectory, { recursive: true });
    writeSourceManagementPdf(longPdfPath, 2);

    try {
      await page.setViewportSize({ width: 1600, height: 1200 });
      await createLibrary(page, "inventory-source-width");
      const libraryId = await currentLibraryId(page);

      await openInventorySourceManagement(page);
      await page.getByTestId("inventory-source-root-create-button").click();
      await page.getByTestId("source-root-path-input").fill(fixtures.tempDir);
      await page.getByTestId("source-root-submit-button").click();

      const rootCard = sourceRootCard(page, fixtures.tempDir);
      await expect(rootCard).toBeVisible();

      const previousJobId = await latestJobId(page, libraryId);
      await rootCard.locator("[data-source-root-refresh-id]").click();
      await waitForNewLatestJobCompleted(page, libraryId, previousJobId);

      await openInventoryWorkspace(page);
      const longSourceCard = librarySourceCard(page, longPdfName);
      await expect(longSourceCard).toBeVisible();

      const panelMetrics = await page.evaluate(() => {
        const listPanel = document.querySelector(".inventory-panel-main");
        const detailPanel = document.querySelector(".inventory-detail-panel");
        return {
          listPanelWidth: listPanel?.getBoundingClientRect().width ?? 0,
          detailPanelWidth: detailPanel?.getBoundingClientRect().width ?? 0,
        };
      });
      expect(panelMetrics.listPanelWidth).toBeGreaterThan(panelMetrics.detailPanelWidth * 1.5);

      const cardMetrics = await longSourceCard.locator(".inventory-source-select").evaluate((element) => {
        const main = element.querySelector(".inventory-source-main");
        const path = element.querySelector(".inventory-source-path");
        return {
          selectWidth: element.getBoundingClientRect().width,
          mainWidth: main?.getBoundingClientRect().width ?? 0,
          pathWidth: path?.getBoundingClientRect().width ?? 0,
        };
      });
      expect(cardMetrics.mainWidth).toBeGreaterThan(375);
      expect(cardMetrics.pathWidth).toBeGreaterThan(375);
      expect(cardMetrics.mainWidth).toBeGreaterThan(cardMetrics.selectWidth * 0.76);
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("inventory source management keeps disclosure and advanced rules stable across workspace polling", async ({
    page,
  }) => {
    const fixtures = createTempSourceManagementFixtures();
    try {
      await createLibrary(page, "source-management-polling");

      await openInventorySourceManagement(page);
      await page.getByTestId("inventory-source-root-create-button").click();
      await page.getByTestId("source-root-path-input").fill(fixtures.tempDir);
      await openSourceRootAdvancedRules(page);
      await page.getByTestId("source-root-exclude-globs-input").fill("chart.png");

      await page.waitForTimeout(3500);

      await expect(page.getByTestId("inventory-source-management-panel")).toBeVisible();
      await expect(page.getByTestId("source-root-form")).toBeVisible();
      await expect(page.getByTestId("source-root-advanced-rules-panel")).toBeVisible();
      await expect(page.getByTestId("source-root-path-input")).toHaveValue(fixtures.tempDir);
      await expect(page.getByTestId("source-root-exclude-globs-input")).toHaveValue("chart.png");
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("source management watcher updates inventory for add modify and delete", async ({ page }) => {
  const fixtures = createTempSourceManagementFixtures();
  try {
    await createLibrary(page, "source-management-watcher");
    const libraryId = await currentLibraryId(page);

    await openSourcePreparationPanel(page);
    await page.getByTestId("source-root-path-input").fill(fixtures.tempDir);
    await page.getByTestId("source-root-submit-button").click();

    const rootCard = sourceRootCard(page, fixtures.tempDir);
    await expect(rootCard).toBeVisible();

    let previousJobId = await latestJobId(page, libraryId);
    await rootCard.locator("[data-source-root-refresh-id]").click();
    await waitForNewLatestJobCompleted(page, libraryId, previousJobId);

    await openInventoryWorkspace(page);

    const pdfCard = librarySourceCard(page, "report.pdf");
    const addedImageCard = librarySourceCard(page, "new-chart.png");
    await expect(page.getByTestId("library-source-card")).toHaveCount(2);
    await expect(pdfCard).toContainText("2 个对象");

    previousJobId = await latestJobId(page, libraryId);
    fs.copyFileSync(fixtureImagePath, fixtures.addedImagePath);
    await waitForNewLatestJobCompleted(page, libraryId, previousJobId);
    await expect(page.getByTestId("library-source-card")).toHaveCount(3);
    await expect(addedImageCard).toContainText("1 个对象");

    previousJobId = await latestJobId(page, libraryId);
    writeSourceManagementPdf(fixtures.pdfPath, 1);
    await waitForNewLatestJobCompleted(page, libraryId, previousJobId);
    await expect(pdfCard).toContainText("1 个对象");

    previousJobId = await latestJobId(page, libraryId);
    fs.rmSync(fixtures.addedImagePath, { force: true });
    await waitForNewLatestJobCompleted(page, libraryId, previousJobId);

    await page.getByTestId("source-filter-status").selectOption("invalidated");
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(addedImageCard).toContainText("not_found");
  } finally {
    fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
  }
  });
}

export function registerInventoryWorkspaceScenarios() {
  test("inventory workspace stays usable on narrow screens", async ({ page }) => {
  const fixtures = createTempSourceManagementFixtures();
  try {
    await page.setViewportSize({ width: 390, height: 844 });
    await createLibrary(page, "inventory-narrow");
    await openSourcePreparationPanel(page);
    const libraryId = await currentLibraryId(page);

    await page.getByTestId("source-root-path-input").fill(fixtures.tempDir);
    await page.getByTestId("source-root-submit-button").click();

    const rootCard = sourceRootCard(page, fixtures.tempDir);
    await expect(rootCard).toBeVisible();

    const previousJobId = await latestJobId(page, libraryId);
    await rootCard.locator("[data-source-root-refresh-id]").click();
    await waitForNewLatestJobCompleted(page, libraryId, previousJobId);

    await openInventoryWorkspace(page);
    await expect(page.getByTestId("workspace-library-metrics")).toBeVisible();
    const firstSourceCard = page.getByTestId("library-source-card").first();
    await expect(firstSourceCard).toBeVisible();
    await firstSourceCard.locator("button").click();
    await expect(page.getByTestId("inventory-detail-sheet-close-button")).toBeVisible();
    await expect(page.getByTestId("inventory-detail-preview")).toBeVisible();
    await page.getByTestId("inventory-detail-sheet-close-button").click();
    await expect(page.getByTestId("inventory-detail-panel")).not.toBeVisible();
    await firstSourceCard.locator("button").click();
    await expect(page.getByTestId("inventory-detail-panel")).toBeVisible();
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
    ).toBe(true);
  } finally {
    fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
  }
  });
}
