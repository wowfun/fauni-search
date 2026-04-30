import { expect, test } from "@playwright/test";
import fs from "node:fs";
import {
  createLibrary,
  createTempDocumentSearchFixtures,
  expectSearchRequiresContent,
  invalidQueryUploadPath,
  prepareSearchableLibrary,
  prepareSearchableSourceRoot,
} from "./fixtures";

export function registerSearchDocumentScenarios() {
  test("document mode uploads a query document and returns real mixed results", async ({ page }) => {
    const fixtures = createTempDocumentSearchFixtures();
    try {
      await createLibrary(page, "document-search");
      await prepareSearchableSourceRoot(page, fixtures.tempDir);

      await page.getByTestId("search-mode-document").click();
      await page.getByTestId("query-document-input").setInputFiles(fixtures.pdfPath);
      await expect(page.getByTestId("query-document-preview")).toBeVisible();

      await page.getByTestId("search-submit-button").click();

      const documentPageResult = page.locator('[data-testid="result-card"][data-kind="document_page"]').first();
      const imageResult = page.locator('[data-testid="result-card"][data-kind="image"]').first();
      await expect(documentPageResult).toBeVisible({ timeout: 2 * 60 * 1000 });
      await expect(imageResult).toBeVisible({ timeout: 2 * 60 * 1000 });
      await documentPageResult.locator(".result-select").click();
      await expect(page.getByTestId("result-score").first()).toBeVisible();
      await expect(page.getByTestId("detail-panel")).toBeVisible();
      await expect(page.getByTestId("asset-detail")).toBeVisible();
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("document mode can search a specific page range", async ({ page }) => {
    const fixtures = createTempDocumentSearchFixtures();
    try {
      await createLibrary(page, "document-range-search");
      await prepareSearchableSourceRoot(page, fixtures.tempDir);

      await page.getByTestId("search-mode-document").click();
      await page.getByTestId("query-document-input").setInputFiles(fixtures.pdfPath);
      await expect(page.getByTestId("query-document-preview")).toBeVisible();
      await page.getByTestId("query-document-range-start").fill("2");
      await page.getByTestId("query-document-range-end").fill("2");
      await expect(page.getByTestId("query-document-range-card")).toContainText("P2 → P2");

      await page.getByTestId("search-submit-button").click();

      const firstResult = page.getByTestId("result-card").first();
      await expect(firstResult).toBeVisible({ timeout: 2 * 60 * 1000 });
      await expect(firstResult.getByTestId("result-score")).toBeVisible();
      await expect(page.getByTestId("detail-panel")).toBeVisible();
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("document mode can reuse a library document_page as the query document", async ({ page }) => {
    const fixtures = createTempDocumentSearchFixtures();
    try {
      await createLibrary(page, "document-library-object");
      await prepareSearchableSourceRoot(page, fixtures.tempDir);

      await page.getByTestId("search-mode-document").click();
      await page.getByTestId("query-document-input").setInputFiles(fixtures.pdfPath);
      await expect(page.getByTestId("query-document-preview")).toBeVisible();
      await page.getByTestId("search-submit-button").click();

      const documentPageResult = page.locator('[data-testid="result-card"][data-kind="document_page"]').first();
      await expect(documentPageResult).toBeVisible({ timeout: 2 * 60 * 1000 });
      await documentPageResult.locator(".result-select").click();
      await expect(documentPageResult.getByTestId("use-as-query-document-button")).toBeVisible();
      await documentPageResult.getByTestId("use-as-query-document-button").click();

      await expect(page.getByTestId("query-document-card")).toContainText("库内页面");
      await expect(page.getByTestId("query-document-range-card")).toContainText("固定为该页面对应的单页范围");

      await page.getByTestId("search-submit-button").click();

      const firstResult = page.getByTestId("result-card").first();
      await expect(firstResult).toBeVisible({ timeout: 2 * 60 * 1000 });
      await expect(firstResult.getByTestId("result-score")).toBeVisible();
      await expect(page.getByTestId("detail-panel")).toBeVisible();
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("document mode before import keeps submit disabled and points to import prep", async ({ page }) => {
    const fixtures = createTempDocumentSearchFixtures();
    try {
      await createLibrary(page, "document-not-ready");

      await page.getByTestId("search-mode-document").click();
      await page.getByTestId("query-document-input").setInputFiles(fixtures.pdfPath);
      await expect(page.getByTestId("query-document-preview")).toBeVisible();
      await expectSearchRequiresContent(page);
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("document mode rejects non-pdf query uploads with explicit feedback", async ({ page }) => {
    await createLibrary(page, "document-invalid-upload");
    await prepareSearchableLibrary(page);

    await page.getByTestId("search-mode-document").click();
    await page.getByTestId("query-document-input").setInputFiles(invalidQueryUploadPath);
    await page.getByTestId("search-submit-button").click();

    await expect(page.getByTestId("search-error-notice")).toBeVisible();
    await expect(page.getByTestId("search-error-code")).toHaveText("validation_failed");
    await expect(page.getByTestId("search-error-message")).toContainText(
      "Only PDF files are accepted as query documents right now."
    );
  });
}
