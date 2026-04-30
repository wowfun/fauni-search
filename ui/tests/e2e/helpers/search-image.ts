import { expect, test } from "@playwright/test";
import fs from "node:fs";
import {
  createLibrary,
  createMockMatchedUnits,
  createTempDocumentSearchFixtures,
  expectSearchRequiresContent,
  fileSourceUri,
  fixtureImagePath,
  invalidQueryUploadPath,
  importFixtureIntoCurrentLibrary,
  mockImageSearchResults,
  pasteImageIntoQueryTarget,
  prepareSearchableLibrary,
  prepareSearchableSourceRoot,
} from "./fixtures";

export function registerSearchImageScenarios() {
  test("image mode uploads a query image and returns real results", async ({ page }) => {
    await createLibrary(page, "image-search");
    await importFixtureIntoCurrentLibrary(page);

    await page.getByTestId("search-mode-image").click();
    await page.getByTestId("query-image-input").setInputFiles(fixtureImagePath);
    await expect(page.getByTestId("query-image-preview")).toBeVisible();
    await mockImageSearchResults(page);

    await page.getByTestId("search-submit-button").click();

    const firstResult = page.getByTestId("result-card").first();
    await expect(firstResult).toBeVisible({ timeout: 2 * 60 * 1000 });
    await expect(firstResult.getByTestId("result-score")).toBeVisible();
    await expect(page.getByTestId("detail-panel")).toBeVisible();
    await expect(page.getByTestId("visual-preview")).toBeVisible();
  });

  test("image mode can paste a query image like a search box", async ({ page }) => {
    await createLibrary(page, "image-paste-search");
    await importFixtureIntoCurrentLibrary(page);

    await page.getByTestId("search-mode-image").click();
    await pasteImageIntoQueryTarget(page, fixtureImagePath);
    await expect(page.getByTestId("query-image-preview")).toBeVisible();
    await expect(page.getByTestId("query-image-card")).toContainText("待上传");
    await mockImageSearchResults(page);

    await page.getByTestId("search-submit-button").click();

    const firstResult = page.getByTestId("result-card").first();
    await expect(firstResult).toBeVisible({ timeout: 2 * 60 * 1000 });
    await expect(firstResult.getByTestId("result-score")).toBeVisible();
    await expect(page.getByTestId("detail-panel")).toBeVisible();
    await expect(page.getByTestId("visual-preview")).toBeVisible();
  });

  test("image mode can reuse a library image object as the query image", async ({ page }) => {
    await createLibrary(page, "image-library-object");
    await importFixtureIntoCurrentLibrary(page);
    await page.getByTestId("search-mode-image").click();
    await page.getByTestId("query-image-input").setInputFiles(fixtureImagePath);
    await expect(page.getByTestId("query-image-preview")).toBeVisible();
    await mockImageSearchResults(page);
    await page.getByTestId("search-submit-button").click();

    const imageResult = page.locator('[data-testid="result-card"][data-kind="image"]').first();
    await expect(imageResult).toBeVisible({ timeout: 2 * 60 * 1000 });
    await imageResult.getByTestId("use-as-query-image-button").click();

    await expect(page.getByTestId("query-image-card")).toContainText("库内对象");
    await expect(page.getByTestId("query-image-preview")).toBeVisible();

    await page.getByTestId("search-submit-button").click();

    const firstResult = page.getByTestId("result-card").first();
    await expect(firstResult).toBeVisible({ timeout: 2 * 60 * 1000 });
    await expect(firstResult.getByTestId("result-score")).toBeVisible();
    await expect(page.getByTestId("detail-panel")).toBeVisible();
  });

  test("image mode can reuse a library document_page object as the query image", async ({ page }) => {
    const fixtures = createTempDocumentSearchFixtures();
    try {
      await createLibrary(page, "document-page-library-object");
      await prepareSearchableSourceRoot(page, fixtures.tempDir);

      await page.getByTestId("search-mode-image").click();
      await page.getByTestId("query-image-input").setInputFiles(fixtures.imagePath);
      await expect(page.getByTestId("query-image-preview")).toBeVisible();
      await mockImageSearchResults(page, [
        {
          asset_id: "asset_image_document_page_mock_0",
          source_id: "src_image_document_page_mock_0",
          preview: {
            url: "http://127.0.0.1:54210/mock-preview/document-page-image-0.png",
          },
          source_uri: fileSourceUri(fixtures.pdfPath),
          source_type: "pdf",
          asset_type: "document_page",
          locator: {
            page: 1,
            page_label: "1",
          },
          cursor: "search:v1:image-document-page:1",
          score: 100,
          matched_units: createMockMatchedUnits(0),
        },
      ]);
      await page.getByTestId("search-submit-button").click();

      const documentPageResult = page.locator('[data-testid="result-card"][data-kind="document_page"]').first();
      await expect(documentPageResult).toBeVisible({ timeout: 2 * 60 * 1000 });
      await documentPageResult.getByTestId("use-as-query-image-button").click();

      await expect(page.getByTestId("query-image-card")).toContainText("库内对象");
      await expect(page.getByTestId("query-image-preview")).toBeVisible();

      await page.getByTestId("search-submit-button").click();

      const firstResult = page.getByTestId("result-card").first();
      await expect(firstResult).toBeVisible({ timeout: 2 * 60 * 1000 });
      await expect(firstResult.getByTestId("result-score")).toBeVisible();
      await expect(page.getByTestId("detail-panel")).toBeVisible();
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("image mode before import keeps submit disabled and points to import prep", async ({ page }) => {
    await createLibrary(page, "image-not-ready");

    await page.getByTestId("search-mode-image").click();
    await page.getByTestId("query-image-input").setInputFiles(fixtureImagePath);
    await expect(page.getByTestId("query-image-preview")).toBeVisible();
    await expectSearchRequiresContent(page);
  });

  test("image mode rejects non-image query uploads with explicit feedback", async ({ page }) => {
    await createLibrary(page, "image-invalid-upload");
    await prepareSearchableLibrary(page);

    await page.getByTestId("search-mode-image").click();
    await page.getByTestId("query-image-input").setInputFiles(invalidQueryUploadPath);
    await page.getByTestId("search-submit-button").click();

    await expect(page.getByTestId("search-error-notice")).toBeVisible();
    await expect(page.getByTestId("search-error-code")).toHaveText("validation_failed");
    await expect(page.getByTestId("search-error-message")).toContainText(
      "Only common image files are accepted as query images right now."
    );
  });
}
