import { expect, test } from "@playwright/test";
import fs from "node:fs";
import {
  createLibrary,
  createTempVideoSearchFixtures,
  expectSearchRequiresContent,
  invalidQueryUploadPath,
  prepareSearchableLibrary,
  prepareSearchableSourceRoot,
  setRangeValue,
} from "./fixtures";

export function registerSearchVideoScenarios() {
  test("video mode uploads a query video and returns real results", async ({ page }) => {
    const fixtures = createTempVideoSearchFixtures();
    try {
      await createLibrary(page, "video-search");
      await prepareSearchableSourceRoot(page, fixtures.tempDir);

      await page.getByTestId("search-mode-video").click();
      await page.getByTestId("query-video-input").setInputFiles(fixtures.videoPath);
      await expect(page.getByTestId("query-video-preview")).toBeVisible();
      await expect(page.getByTestId("query-video-range-start")).toBeVisible();

      await setRangeValue(page.getByTestId("query-video-range-start"), 4_000);
      await setRangeValue(page.getByTestId("query-video-range-end"), 7_000);

      await page.getByTestId("search-submit-button").click();

      const videoResult = page.locator('[data-testid="result-card"][data-kind="video_segment"]').first();
      await expect(videoResult).toBeVisible({ timeout: 2 * 60 * 1000 });
      await videoResult.locator(".result-select").click();
      await expect(videoResult.getByTestId("result-score")).toBeVisible();
      await expect(page.getByTestId("detail-panel")).toBeVisible();
      await expect(page.getByTestId("visual-unit-detail")).toBeVisible();
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("video mode can reuse a library source as the query video and fall back to whole-video search", async ({
    page,
  }) => {
    const fixtures = createTempVideoSearchFixtures();
    try {
      await createLibrary(page, "video-library-source");
      await prepareSearchableSourceRoot(page, fixtures.tempDir);

      await page.getByTestId("search-mode-video").click();
      await page.getByTestId("query-video-source-select").selectOption({ index: 1 });
      await expect(page.getByTestId("query-video-card")).toContainText("库内视频");
      await expect(page.getByTestId("query-video-preview")).toBeVisible();
      await expect(page.getByTestId("query-video-range-card")).toContainText("整段视频");

      await setRangeValue(page.getByTestId("query-video-range-start"), 1_000);
      await setRangeValue(page.getByTestId("query-video-range-end"), 6_000);
      await expect(page.getByTestId("query-video-range-card")).toContainText("0:01.000 → 0:06.001");

      await page.getByTestId("clear-query-video-range-button").click();
      await expect(page.getByTestId("query-video-range-card")).toContainText("整段视频 · 0 →");

      await page.getByTestId("search-submit-button").click();

      const videoResult = page.locator('[data-testid="result-card"][data-kind="video_segment"]').first();
      await expect(videoResult).toBeVisible({ timeout: 2 * 60 * 1000 });
      await videoResult.locator(".result-select").click();
      await expect(videoResult.getByTestId("result-score")).toBeVisible();
      await expect(page.getByTestId("detail-panel")).toBeVisible();
      await expect(page.getByTestId("visual-unit-detail")).toBeVisible();
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("video mode can reuse a library video_segment as the query video", async ({ page }) => {
    const fixtures = createTempVideoSearchFixtures();
    try {
      await createLibrary(page, "video-library-object");
      await prepareSearchableSourceRoot(page, fixtures.tempDir);

      await page.getByTestId("search-mode-video").click();
      await page.getByTestId("query-video-input").setInputFiles(fixtures.videoPath);
      await expect(page.getByTestId("query-video-preview")).toBeVisible();
      await page.getByTestId("search-submit-button").click();

      const videoResult = page.locator('[data-testid="result-card"][data-kind="video_segment"]').first();
      await expect(videoResult).toBeVisible({ timeout: 2 * 60 * 1000 });
      await videoResult.locator(".result-select").click();
      await expect(videoResult.getByTestId("use-as-query-video-button")).toBeVisible();
      await videoResult.getByTestId("use-as-query-video-button").click();

      await expect(page.getByTestId("query-video-card")).toContainText("库内片段");
      await expect(page.getByTestId("query-video-range-card")).toContainText("固定为该片段自身的时间范围");

      await page.getByTestId("search-submit-button").click();

      const nextVideoResult = page.locator('[data-testid="result-card"][data-kind="video_segment"]').first();
      await expect(nextVideoResult).toBeVisible({ timeout: 2 * 60 * 1000 });
      await expect(nextVideoResult.getByTestId("result-score")).toBeVisible();
      await expect(page.getByTestId("detail-panel")).toBeVisible();
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("video mode before import keeps submit disabled and points to import prep", async ({ page }) => {
    const fixtures = createTempVideoSearchFixtures();
    try {
      await createLibrary(page, "video-not-ready");

      await page.getByTestId("search-mode-video").click();
      await page.getByTestId("query-video-input").setInputFiles(fixtures.videoPath);
      await expect(page.getByTestId("query-video-preview")).toBeVisible();
      await expectSearchRequiresContent(page);
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("video mode rejects non-video query uploads with explicit feedback", async ({ page }) => {
    await createLibrary(page, "video-invalid-upload");
    await prepareSearchableLibrary(page);

    await page.getByTestId("search-mode-video").click();
    await page.getByTestId("query-video-input").setInputFiles(invalidQueryUploadPath);
    await page.getByTestId("search-submit-button").click();

    await expect(page.getByTestId("search-error-notice")).toBeVisible();
    await expect(page.getByTestId("search-error-code")).toHaveText("validation_failed");
    await expect(page.getByTestId("search-error-message")).toContainText(
      "Only mp4, mov, or m4v files are accepted as query videos right now."
    );
  });
}
