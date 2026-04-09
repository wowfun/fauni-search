import { expect, test } from "@playwright/test";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const fixtureImagePath = path.resolve(
  __dirname,
  "../../../tests/fixtures/tatdqa-page-images/images/tatdqa-page-0001.png"
);
const invalidQueryUploadPath = path.resolve(__dirname, "../../../README.md");
const venvPythonPath = path.resolve(__dirname, "../../../.venv/bin/python");

async function pasteImageIntoQueryTarget(page, imagePath) {
  const target = page.getByTestId("query-image-paste-target");
  await expect(target).toBeVisible();
  await target.focus();

  const bytes = Array.from(fs.readFileSync(imagePath));
  await target.evaluate(
    (element, payload) => {
      const file = new File([Uint8Array.from(payload.bytes)], payload.name, {
        type: payload.type,
      });
      const dataTransfer = new DataTransfer();
      dataTransfer.items.add(file);
      const event = new Event("paste", { bubbles: true, cancelable: true });
      Object.defineProperty(event, "clipboardData", {
        value: dataTransfer,
      });
      element.dispatchEvent(event);
    },
    {
      name: path.basename(imagePath),
      type: "image/png",
      bytes,
    }
  );
}

async function createLibrary(page, suffix) {
  const libraryName = `playwright-${suffix}-${Date.now()}`;
  await page.goto("/");
  await expect(page.getByTestId("workspace-shell")).toBeVisible();
  await page.getByTestId("library-name-input").fill(libraryName);
  await page.getByTestId("create-library-button").click();
  await expect(page.getByTestId("current-library-name")).toHaveText(libraryName);
  return libraryName;
}

async function waitForFirstJobCompleted(page) {
  const firstJob = page.getByTestId("job-card").first();
  await expect(firstJob).toBeVisible({ timeout: 30_000 });
  await expect
    .poll(async () => firstJob.getAttribute("data-job-status"), {
      timeout: 10 * 60 * 1000,
      intervals: [1_000, 2_000, 5_000],
    })
    .toBe("completed");
}

function createTempPdfFixture() {
  const pdfPath = path.join(os.tmpdir(), `fauni-search-playwright-${Date.now()}.pdf`);
  execFileSync(
    venvPythonPath,
    [
      "-c",
      `
from PIL import Image, ImageDraw
from pathlib import Path

path = Path(${JSON.stringify(pdfPath)})
first_page = Image.new("RGB", (512, 512), "white")
first_draw = ImageDraw.Draw(first_page)
first_draw.rectangle((48, 48, 464, 464), outline="black", width=4)
first_draw.text((80, 220), "Revenue 46 percent", fill="black")

second_page = Image.new("RGB", (512, 512), "white")
second_draw = ImageDraw.Draw(second_page)
second_draw.rectangle((48, 48, 464, 464), outline="black", width=4)
second_draw.text((80, 220), "Operating margin 18 percent", fill="black")

first_page.save(path, "PDF", save_all=True, append_images=[second_page])
      `,
    ],
    { stdio: "pipe" }
  );
  return pdfPath;
}

test("demo import and search closes the current UI happy path", async ({ page }) => {
  await createLibrary(page, "smoke");
  await expect(page.getByTestId("run-demo-button")).toBeEnabled();

  await page.getByTestId("run-demo-button").click();
  await waitForFirstJobCompleted(page);

  const firstResult = page.getByTestId("result-card").first();
  await expect(firstResult).toBeVisible({ timeout: 2 * 60 * 1000 });
  await expect(firstResult.getByTestId("result-score")).toBeVisible();

  await expect(page.getByTestId("detail-panel")).toBeVisible();
  await expect(page.getByTestId("visual-unit-detail")).toBeVisible();
  await expect(page.getByTestId("visual-preview")).toBeVisible();
  await expect(page.getByTestId("preview-link")).toBeVisible();
});

test("search before import shows not_ready instead of an empty result", async ({ page }) => {
  await createLibrary(page, "not-ready");

  await page.getByTestId("search-text-input").fill("operating activities");
  await page.getByTestId("search-submit-button").click();

  await expect(page.getByTestId("search-error-notice")).toBeVisible();
  await expect(page.getByTestId("search-error-code")).toHaveText("not_ready");
  await expect(page.getByTestId("search-error-message")).toContainText("active index");
});

test("invalid import paths show explicit rejection feedback", async ({ page }) => {
  await createLibrary(page, "invalid-import");

  await page.getByTestId("import-paths-input").fill("README.md");
  await page.getByTestId("import-submit-button").click();

  await expect(page.getByTestId("import-receipt")).toBeVisible();
  const rejected = page.getByTestId("import-rejected-item").first();
  await expect(rejected).toBeVisible();
  await expect(rejected).toHaveAttribute("data-reason-code", "unsupported_type");
  await expect(page.getByTestId("import-no-job")).toBeVisible();
});

test("image mode uploads a query image and returns real results", async ({ page }) => {
  await createLibrary(page, "image-search");
  await expect(page.getByTestId("run-demo-button")).toBeEnabled();
  await page.getByTestId("run-demo-button").click();
  await waitForFirstJobCompleted(page);

  await page.getByTestId("search-mode-image").click();
  await page.getByTestId("query-image-input").setInputFiles(fixtureImagePath);
  await expect(page.getByTestId("query-image-preview")).toBeVisible();

  await page.getByTestId("search-submit-button").click();

  const firstResult = page.getByTestId("result-card").first();
  await expect(firstResult).toBeVisible({ timeout: 2 * 60 * 1000 });
  await expect(firstResult.getByTestId("result-score")).toBeVisible();
  await expect(page.getByTestId("detail-panel")).toBeVisible();
  await expect(page.getByTestId("visual-preview")).toBeVisible();
});

test("image mode can paste a query image like a search box", async ({ page }) => {
  await createLibrary(page, "image-paste-search");
  await expect(page.getByTestId("run-demo-button")).toBeEnabled();
  await page.getByTestId("run-demo-button").click();
  await waitForFirstJobCompleted(page);

  await page.getByTestId("search-mode-image").click();
  await pasteImageIntoQueryTarget(page, fixtureImagePath);
  await expect(page.getByTestId("query-image-preview")).toBeVisible();
  await expect(page.getByTestId("query-image-card")).toContainText("待上传");

  await page.getByTestId("search-submit-button").click();

  const firstResult = page.getByTestId("result-card").first();
  await expect(firstResult).toBeVisible({ timeout: 2 * 60 * 1000 });
  await expect(firstResult.getByTestId("result-score")).toBeVisible();
  await expect(page.getByTestId("detail-panel")).toBeVisible();
  await expect(page.getByTestId("visual-preview")).toBeVisible();
});

test("image mode can reuse a library image object as the query image", async ({ page }) => {
  await createLibrary(page, "image-library-object");
  await expect(page.getByTestId("run-demo-button")).toBeEnabled();
  await page.getByTestId("run-demo-button").click();
  await waitForFirstJobCompleted(page);

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
  const pdfPath = createTempPdfFixture();
  try {
    await createLibrary(page, "document-page-library-object");

    await page.getByTestId("import-paths-input").fill(`${fixtureImagePath}\n${pdfPath}`);
    await page.getByTestId("import-submit-button").click();
    await waitForFirstJobCompleted(page);

    await page.getByTestId("search-mode-image").click();
    await page.getByTestId("query-image-input").setInputFiles(fixtureImagePath);
    await expect(page.getByTestId("query-image-preview")).toBeVisible();
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
    fs.rmSync(pdfPath, { force: true });
  }
});

test("image mode before import shows not_ready instead of an empty result", async ({ page }) => {
  await createLibrary(page, "image-not-ready");

  await page.getByTestId("search-mode-image").click();
  await page.getByTestId("query-image-input").setInputFiles(fixtureImagePath);
  await expect(page.getByTestId("query-image-preview")).toBeVisible();

  await page.getByTestId("search-submit-button").click();

  await expect(page.getByTestId("search-error-notice")).toBeVisible();
  await expect(page.getByTestId("search-error-code")).toHaveText("not_ready");
  await expect(page.getByTestId("search-error-message")).toContainText("active index");
});

test("image mode rejects non-image query uploads with explicit feedback", async ({ page }) => {
  await createLibrary(page, "image-invalid-upload");

  await page.getByTestId("search-mode-image").click();
  await page.getByTestId("query-image-input").setInputFiles(invalidQueryUploadPath);
  await page.getByTestId("search-submit-button").click();

  await expect(page.getByTestId("search-error-notice")).toBeVisible();
  await expect(page.getByTestId("search-error-code")).toHaveText("validation_failed");
  await expect(page.getByTestId("search-error-message")).toContainText(
    "Only common image files are accepted as query images right now."
  );
});
