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

function createTempVideoSearchFixtures() {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "fauni-search-video-search-"));
  const framePath = path.join(tempDir, "report-frame.png");
  const pdfPath = path.join(tempDir, "report-frame.pdf");
  const videoPath = path.join(tempDir, "query-video.mp4");

  execFileSync(
    venvPythonPath,
    [
      "-c",
      `
from PIL import Image, ImageDraw
from pathlib import Path

frame_path = Path(${JSON.stringify(framePath)})
pdf_path = Path(${JSON.stringify(pdfPath)})
page = Image.new("RGB", (960, 540), "white")
draw = ImageDraw.Draw(page)
draw.rectangle((60, 60, 900, 480), outline="black", width=6)
draw.text((120, 170), "Q2 2025 Financial Report", fill="black")
draw.text((120, 250), "Revenue 46 percent", fill="black")
draw.text((120, 330), "Operating margin 18 percent", fill="black")
page.save(frame_path, "PNG")
page.save(pdf_path, "PDF")
      `,
    ],
    { stdio: "pipe" }
  );

  execFileSync(
    "ffmpeg",
    [
      "-y",
      "-loop",
      "1",
      "-t",
      "4",
      "-i",
      fixtureImagePath,
      "-loop",
      "1",
      "-t",
      "4",
      "-i",
      framePath,
      "-filter_complex",
      [
        "[0:v]scale=960:540:force_original_aspect_ratio=decrease,",
        "pad=960:540:(ow-iw)/2:(oh-ih)/2,setsar=1,format=yuv420p[v0];",
        "[1:v]scale=960:540:force_original_aspect_ratio=decrease,",
        "pad=960:540:(ow-iw)/2:(oh-ih)/2,setsar=1,format=yuv420p[v1];",
        "[v0][v1]concat=n=2:v=1:a=0[v]",
      ].join(""),
      "-map",
      "[v]",
      "-r",
      "30",
      "-c:v",
      "libx264",
      "-pix_fmt",
      "yuv420p",
      videoPath,
    ],
    { stdio: "pipe" }
  );

  return { tempDir, framePath, pdfPath, videoPath };
}

async function setRangeValue(locator, value) {
  await locator.evaluate((element, nextValue) => {
    element.value = String(nextValue);
    element.dispatchEvent(new Event("input", { bubbles: true }));
  }, value);
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

test("video mode uploads a query video and returns real results", async ({ page }) => {
  const fixtures = createTempVideoSearchFixtures();
  try {
    await createLibrary(page, "video-search");

    await page
      .getByTestId("import-paths-input")
      .fill(`${fixtureImagePath}\n${fixtures.pdfPath}\n${fixtures.videoPath}`);
    await page.getByTestId("import-submit-button").click();
    await waitForFirstJobCompleted(page);

    await page.getByTestId("search-mode-video").click();
    await page.getByTestId("query-video-input").setInputFiles(fixtures.videoPath);
    await expect(page.getByTestId("query-video-preview")).toBeVisible();
    await expect(page.getByTestId("query-video-range-start")).toBeVisible();

    await setRangeValue(page.getByTestId("query-video-range-start"), 4_000);
    await setRangeValue(page.getByTestId("query-video-range-end"), 7_000);

    await page.getByTestId("search-submit-button").click();

    const videoResult = page.locator('[data-testid="result-card"][data-kind="video_segment"]').first();
    await expect(videoResult).toBeVisible({ timeout: 2 * 60 * 1000 });
    await expect(videoResult.getByTestId("result-score")).toBeVisible();
    await expect(page.getByTestId("detail-panel")).toBeVisible();
    await expect(page.getByTestId("visual-preview")).toBeVisible();
  } finally {
    fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
  }
});

test("video mode can reuse a library source as the query video and fall back to whole-video search", async ({ page }) => {
  const fixtures = createTempVideoSearchFixtures();
  try {
    await createLibrary(page, "video-library-source");

    await page
      .getByTestId("import-paths-input")
      .fill(`${fixtureImagePath}\n${fixtures.pdfPath}\n${fixtures.videoPath}`);
    await page.getByTestId("import-submit-button").click();
    await waitForFirstJobCompleted(page);

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
    await expect(videoResult.getByTestId("result-score")).toBeVisible();
    await expect(page.getByTestId("detail-panel")).toBeVisible();
    await expect(page.getByTestId("visual-preview")).toBeVisible();
  } finally {
    fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
  }
});

test("video mode can reuse a library video_segment as the query video", async ({ page }) => {
  const fixtures = createTempVideoSearchFixtures();
  try {
    await createLibrary(page, "video-library-object");

    await page
      .getByTestId("import-paths-input")
      .fill(`${fixtureImagePath}\n${fixtures.pdfPath}\n${fixtures.videoPath}`);
    await page.getByTestId("import-submit-button").click();
    await waitForFirstJobCompleted(page);

    await page.getByTestId("search-mode-video").click();
    await page.getByTestId("query-video-input").setInputFiles(fixtures.videoPath);
    await expect(page.getByTestId("query-video-preview")).toBeVisible();
    await page.getByTestId("search-submit-button").click();

    const videoResult = page.locator('[data-testid="result-card"][data-kind="video_segment"]').first();
    await expect(videoResult).toBeVisible({ timeout: 2 * 60 * 1000 });
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

test("video mode before import shows not_ready instead of an empty result", async ({ page }) => {
  const fixtures = createTempVideoSearchFixtures();
  try {
    await createLibrary(page, "video-not-ready");

    await page.getByTestId("search-mode-video").click();
    await page.getByTestId("query-video-input").setInputFiles(fixtures.videoPath);
    await expect(page.getByTestId("query-video-preview")).toBeVisible();

    await page.getByTestId("search-submit-button").click();

    await expect(page.getByTestId("search-error-notice")).toBeVisible();
    await expect(page.getByTestId("search-error-code")).toHaveText("not_ready");
    await expect(page.getByTestId("search-error-message")).toContainText("active index");
  } finally {
    fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
  }
});

test("video mode rejects non-video query uploads with explicit feedback", async ({ page }) => {
  await createLibrary(page, "video-invalid-upload");

  await page.getByTestId("search-mode-video").click();
  await page.getByTestId("query-video-input").setInputFiles(invalidQueryUploadPath);
  await page.getByTestId("search-submit-button").click();

  await expect(page.getByTestId("search-error-notice")).toBeVisible();
  await expect(page.getByTestId("search-error-code")).toHaveText("validation_failed");
  await expect(page.getByTestId("search-error-message")).toContainText(
    "Only mp4, mov, or m4v files are accepted as query videos right now."
  );
});
