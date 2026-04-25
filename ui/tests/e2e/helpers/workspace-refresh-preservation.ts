import { expect, test } from "@playwright/test";
import fs from "node:fs";
import {
  createLibrary,
  createTempDocumentSearchFixtures,
  ensureCreateLibraryPopoverOpen,
  importFixturesIntoCurrentLibrary,
  librarySourceCard,
  mockDocumentSearchResults,
  openInventoryWorkspace,
  openSearchWorkspace,
  waitForFirstJobCompleted,
  workspacePollWaitMs,
} from "./fixtures";

export function registerWorkspaceRefreshPreservationScenarios() {
  test("workspace refresh preserves focused editable inputs and drafts", async ({ page }) => {
    const libraryName = `playwright-focus-${Date.now()}`;
    const secondLibraryName = `playwright-focus-next-${Date.now()}`;

    await page.goto("/");
    await expect(page.getByTestId("workspace-shell")).toBeVisible();

    const libraryNameInput = await ensureCreateLibraryPopoverOpen(page);
    await libraryNameInput.click();
    await page.keyboard.type(libraryName);
    await expect(libraryNameInput).toBeFocused();
    await expect(libraryNameInput).toHaveValue(libraryName);

    await page.waitForTimeout(workspacePollWaitMs);
    await expect(page.getByTestId("library-name-input")).toBeFocused();
    await expect(page.getByTestId("library-name-input")).toHaveValue(libraryName);

    await page.getByTestId("create-library-button").click();
    await expect(page.getByTestId("library-select")).toContainText(libraryName);

    const secondLibraryNameInput = await ensureCreateLibraryPopoverOpen(page);
    await secondLibraryNameInput.click();
    await page.keyboard.type(secondLibraryName);
    await expect(page.getByTestId("library-name-input")).toBeFocused();
    await expect(page.getByTestId("library-name-input")).toHaveValue(secondLibraryName);

    await page.waitForTimeout(workspacePollWaitMs);
    await expect(page.getByTestId("library-name-input")).toBeFocused();
    await expect(page.getByTestId("library-name-input")).toHaveValue(secondLibraryName);

    await openSearchWorkspace(page);
    await page.getByTestId("search-mode-document").click();
    const queryDocumentRangeStart = page.getByTestId("query-document-range-start");
    await queryDocumentRangeStart.click();
    await page.keyboard.type("12");
    await expect(page.getByTestId("query-document-range-start")).toBeFocused();
    await expect(page.getByTestId("query-document-range-start")).toHaveValue("12");

    await page.waitForTimeout(workspacePollWaitMs);
    await expect(page.getByTestId("query-document-range-start")).toBeFocused();
    await expect(page.getByTestId("query-document-range-start")).toHaveValue("12");
  });

  test("workspace refresh preserves the open PDF detail preview element", async ({ page }) => {
    const fixtures = createTempDocumentSearchFixtures();
    try {
      await createLibrary(page, "detail-preview-pdf-stability");

      await page.getByTestId("import-paths-input").fill(`${fixtures.imagePath}\n${fixtures.pdfPath}`);
      await page.getByTestId("import-submit-button").click();
      await waitForFirstJobCompleted(page);

      await page.getByTestId("search-mode-document").click();
      await page.getByTestId("query-document-input").setInputFiles(fixtures.pdfPath);
      await expect(page.getByTestId("query-document-preview")).toBeVisible();
      await mockDocumentSearchResults(page, fixtures.pdfPath);

      await page.getByTestId("search-submit-button").click();

      const documentPageResult = page.locator('[data-testid="result-card"][data-kind="document_page"]').first();
      await expect(documentPageResult).toBeVisible({ timeout: 2 * 60 * 1000 });
      await documentPageResult.locator(".result-select").click();

      const preview = page.locator('iframe[data-testid="visual-preview"]');
      await expect(preview).toBeVisible();

      const probeValue = `pdf-preview-probe-${Date.now()}`;
      await preview.evaluate((element, nextProbeValue) => {
        element.setAttribute("data-preview-probe", nextProbeValue);
      }, probeValue);

      await page.waitForTimeout(workspacePollWaitMs);
      await expect(page.locator('iframe[data-testid="visual-preview"]')).toHaveAttribute(
        "data-preview-probe",
        probeValue
      );
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("workspace refresh preserves the inventory PDF preview element", async ({ page }) => {
    const fixtures = createTempDocumentSearchFixtures();
    try {
      await createLibrary(page, "inventory-preview-pdf-stability");
      await importFixturesIntoCurrentLibrary(page, [fixtures.pdfPath]);
      await openInventoryWorkspace(page);

      await librarySourceCard(page, "query-document.pdf").locator(".inventory-source-select").click();
      const preview = page.locator('iframe[data-testid="inventory-detail-preview"]');
      await expect(preview).toBeVisible();

      const probeValue = `inventory-preview-probe-${Date.now()}`;
      await preview.evaluate((element, nextProbeValue) => {
        element.setAttribute("data-preview-probe", nextProbeValue);
      }, probeValue);
      await preview.evaluate((element) => {
        const global = window as Window & {
          __inventoryPreviewRemoved?: boolean;
          __inventoryPreviewObserver?: MutationObserver;
        };
        global.__inventoryPreviewRemoved = false;
        global.__inventoryPreviewObserver?.disconnect();
        global.__inventoryPreviewObserver = new MutationObserver((mutations) => {
          for (const mutation of mutations) {
            for (const removedNode of Array.from(mutation.removedNodes)) {
              if (
                removedNode === element ||
                (removedNode instanceof Element && removedNode.contains(element))
              ) {
                global.__inventoryPreviewRemoved = true;
              }
            }
          }
        });
        global.__inventoryPreviewObserver.observe(document.body, {
          childList: true,
          subtree: true,
        });
      });

      await page.waitForTimeout(workspacePollWaitMs);
      await expect(page.locator('iframe[data-testid="inventory-detail-preview"]')).toHaveAttribute(
        "data-preview-probe",
        probeValue
      );
      expect(
        await page.evaluate(() => {
          const global = window as Window & { __inventoryPreviewRemoved?: boolean };
          return Boolean(global.__inventoryPreviewRemoved);
        })
      ).toBe(false);
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });
}
