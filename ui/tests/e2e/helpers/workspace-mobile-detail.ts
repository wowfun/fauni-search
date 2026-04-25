import { expect, test } from "@playwright/test";
import fs from "node:fs";
import {
  createLibrary,
  createTempDocumentSearchFixtures,
  mockDocumentSearchResults,
  waitForFirstJobCompleted,
} from "./fixtures";

export function registerWorkspaceMobileDetailScenarios() {
  test("phone-sized search detail opens as a closable sheet", async ({ page }) => {
    const fixtures = createTempDocumentSearchFixtures();
    try {
      await page.setViewportSize({ width: 390, height: 844 });
      await createLibrary(page, "search-mobile-sheet");
      await page.getByTestId("import-paths-input").fill(`${fixtures.imagePath}\n${fixtures.pdfPath}`);
      await page.getByTestId("import-submit-button").click();
      await waitForFirstJobCompleted(page);
      await expect(page.getByTestId("detail-panel")).toHaveCount(0);

      await page.getByTestId("search-mode-document").click();
      await page.getByTestId("query-document-input").setInputFiles(fixtures.pdfPath);
      await expect(page.getByTestId("query-document-preview")).toBeVisible();
      await mockDocumentSearchResults(page, fixtures.pdfPath);
      await page.getByTestId("search-submit-button").click();

      const firstResult = page.getByTestId("result-card").first();
      const secondResult = page.getByTestId("result-card").nth(1);
      await expect(firstResult).toBeVisible({ timeout: 2 * 60 * 1000 });
      await expect(secondResult).toBeVisible({ timeout: 2 * 60 * 1000 });
      await expect(page.getByTestId("detail-sheet-close-button")).toBeVisible();
      await expect(firstResult.locator(".result-actions")).toBeVisible();
      await expect(secondResult.locator(".result-actions")).not.toBeVisible();
      await expect(page.getByTestId("detail-panel")).toBeVisible();

      await page.getByTestId("detail-sheet-close-button").click();
      await expect(page.getByTestId("detail-panel")).not.toBeVisible();

      await secondResult.locator(".result-select").click();
      await expect(page.getByTestId("detail-panel")).toBeVisible();
      await expect(secondResult.locator(".result-actions")).toBeVisible();
      await expect(firstResult.locator(".result-actions")).not.toBeVisible();
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });
}
