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
const workspacePollWaitMs = 3_500;

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

async function openInventoryWorkspace(page) {
  await page.getByTestId("workspace-tab-inventory").click();
  await expect(page.getByTestId("inventory-panel")).toBeVisible();
}

async function openSearchWorkspace(page) {
  await page.getByTestId("workspace-tab-search").click();
  await expect(page.getByTestId("search-panel")).toBeVisible();
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

function createTempDocumentSearchFixtures() {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "fauni-search-document-search-"));
  const imagePath = path.join(tempDir, "report-page.png");
  const pdfPath = path.join(tempDir, "query-document.pdf");

  execFileSync(
    venvPythonPath,
    [
      "-c",
      `
from PIL import Image, ImageDraw
from pathlib import Path

image_path = Path(${JSON.stringify(imagePath)})
pdf_path = Path(${JSON.stringify(pdfPath)})

first_page = Image.new("RGB", (960, 720), "white")
first_draw = ImageDraw.Draw(first_page)
first_draw.rectangle((60, 60, 900, 660), outline="black", width=6)
first_draw.text((120, 170), "Q2 2025 Financial Report", fill="black")
first_draw.text((120, 260), "Revenue 46 percent", fill="black")
first_draw.text((120, 350), "Net income 18 percent", fill="black")
first_draw.text((120, 440), "Cash flow positive", fill="black")

second_page = Image.new("RGB", (960, 720), "white")
second_draw = ImageDraw.Draw(second_page)
second_draw.rectangle((60, 60, 900, 660), outline="black", width=6)
second_draw.text((120, 170), "Q2 2025 Financial Report", fill="black")
second_draw.text((120, 260), "Operating margin 18 percent", fill="black")
second_draw.text((120, 350), "Cash conversion stable", fill="black")
second_draw.text((120, 440), "Forward guidance unchanged", fill="black")

first_page.save(image_path, "PNG")
first_page.save(pdf_path, "PDF", save_all=True, append_images=[second_page])
      `,
    ],
    { stdio: "pipe" }
  );

  return { tempDir, imagePath, pdfPath };
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

function writeSourceManagementPdf(pdfPath, pageCount) {
  execFileSync(
    venvPythonPath,
    [
      "-c",
      `
from PIL import Image, ImageDraw
from pathlib import Path

path = Path(${JSON.stringify(pdfPath)})
pages = []
lines_by_page = [
    ["Q2 2025 Financial Report", "Revenue 46 percent", "Cash flow positive"],
    ["Q2 2025 Financial Report", "Operating margin 18 percent", "Forward guidance unchanged"],
]

for page_index in range(${JSON.stringify(pageCount)}):
    page = Image.new("RGB", (960, 720), "white")
    draw = ImageDraw.Draw(page)
    draw.rectangle((60, 60, 900, 660), outline="black", width=6)
    for line_index, line in enumerate(lines_by_page[page_index % len(lines_by_page)]):
        draw.text((120, 170 + line_index * 90), line, fill="black")
    pages.append(page)

pages[0].save(path, "PDF", save_all=True, append_images=pages[1:])
      `,
    ],
    { stdio: "pipe" }
  );
}

function createTempSourceManagementFixtures() {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "fauni-search-source-management-"));
  const imagePath = path.join(tempDir, "chart.png");
  const addedImagePath = path.join(tempDir, "new-chart.png");
  const pdfPath = path.join(tempDir, "report.pdf");

  fs.copyFileSync(fixtureImagePath, imagePath);
  writeSourceManagementPdf(pdfPath, 2);

  return { tempDir, imagePath, addedImagePath, pdfPath };
}

function sourceRootCard(page, rootPath) {
  return page.getByTestId("source-root-card").filter({ hasText: rootPath });
}

function librarySourceCard(page, sourceName) {
  return page.getByTestId("library-source-card").filter({ hasText: sourceName });
}

async function latestJobId(page) {
  if (!(await page.getByTestId("job-card").count())) {
    return null;
  }
  return page.getByTestId("job-card").first().getAttribute("data-job-id");
}

async function waitForNewLatestJobCompleted(page, previousJobId) {
  await expect
    .poll(
      async () => {
        if (!(await page.getByTestId("job-card").count())) {
          return null;
        }
        const jobId = await page.getByTestId("job-card").first().getAttribute("data-job-id");
        return jobId && jobId !== previousJobId ? jobId : null;
      },
      {
        timeout: 2 * 60 * 1000,
        intervals: [1_000, 2_000, 5_000],
      }
    )
    .toBeTruthy();

  const firstJob = page.getByTestId("job-card").first();
  await expect
    .poll(async () => firstJob.getAttribute("data-job-status"), {
      timeout: 2 * 60 * 1000,
      intervals: [1_000, 2_000, 5_000],
    })
    .toBe("completed");

  return firstJob.getAttribute("data-job-id");
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

test("default workspace keeps search first and moves inventory out of the center flow", async ({ page }) => {
  await createLibrary(page, "workspace-default");

  await expect(page.getByTestId("workspace-tab-search")).toBeVisible();
  await expect(page.getByTestId("workspace-tab-inventory")).toBeVisible();
  await expect(page.getByTestId("search-panel")).toBeVisible();
  await expect(page.getByTestId("inventory-panel")).toHaveCount(0);
  await expect(page.getByTestId("inventory-bridge-button")).toBeVisible();
});

test("switching between search and inventory preserves search drafts results and detail selection", async ({ page }) => {
  await createLibrary(page, "workspace-switch-preserve");
  await page.getByTestId("run-demo-button").click();
  await waitForFirstJobCompleted(page);

  const firstResult = page.getByTestId("result-card").first();
  await expect(firstResult).toBeVisible({ timeout: 2 * 60 * 1000 });
  const visualUnitId = await firstResult.getAttribute("data-visual-unit-id");
  await firstResult.locator(".result-select").click();
  await expect(page.getByTestId("visual-unit-detail")).toBeVisible();
  await expect(page.getByTestId("search-text-input")).toHaveValue(
    "What is the percentage change in the net cash provided from operating activities?"
  );

  await openInventoryWorkspace(page);
  await expect(page.getByTestId("inventory-summary")).toBeVisible();
  await expect(page.getByTestId("library-source-card").first()).toBeVisible();

  await openSearchWorkspace(page);
  await expect(page.getByTestId("search-text-input")).toHaveValue(
    "What is the percentage change in the net cash provided from operating activities?"
  );
  await expect(page.getByTestId("visual-unit-detail")).toBeVisible();
  await expect(
    page.locator(`[data-testid="result-card"][data-visual-unit-id="${visualUnitId}"]`)
  ).toHaveClass(/active/);
});

test("workspace refresh preserves focused editable inputs and drafts", async ({ page }) => {
  const libraryName = `playwright-focus-${Date.now()}`;
  const secondLibraryName = `playwright-focus-next-${Date.now()}`;

  await page.goto("/");
  await expect(page.getByTestId("workspace-shell")).toBeVisible();

  const libraryNameInput = page.getByTestId("library-name-input");
  await libraryNameInput.click();
  await page.keyboard.type(libraryName);
  await expect(libraryNameInput).toBeFocused();
  await expect(libraryNameInput).toHaveValue(libraryName);

  await page.waitForTimeout(workspacePollWaitMs);
  await expect(page.getByTestId("library-name-input")).toBeFocused();
  await expect(page.getByTestId("library-name-input")).toHaveValue(libraryName);

  await page.getByTestId("create-library-button").click();
  await expect(page.getByTestId("current-library-name")).toHaveText(libraryName);

  const secondLibraryNameInput = page.getByTestId("library-name-input");
  await secondLibraryNameInput.click();
  await page.keyboard.type(secondLibraryName);
  await expect(page.getByTestId("library-name-input")).toBeFocused();
  await expect(page.getByTestId("library-name-input")).toHaveValue(secondLibraryName);

  await page.waitForTimeout(workspacePollWaitMs);
  await expect(page.getByTestId("library-name-input")).toBeFocused();
  await expect(page.getByTestId("library-name-input")).toHaveValue(secondLibraryName);

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

    await page
      .getByTestId("import-paths-input")
      .fill(`${fixtures.imagePath}\n${fixtures.pdfPath}`);
    await page.getByTestId("import-submit-button").click();
    await waitForFirstJobCompleted(page);

    await page.getByTestId("search-mode-document").click();
    await page.getByTestId("query-document-input").setInputFiles(fixtures.pdfPath);
    await expect(page.getByTestId("query-document-preview")).toBeVisible();

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

test("document mode uploads a query document and returns real mixed results", async ({ page }) => {
  const fixtures = createTempDocumentSearchFixtures();
  try {
    await createLibrary(page, "document-search");

    await page
      .getByTestId("import-paths-input")
      .fill(`${fixtures.imagePath}\n${fixtures.pdfPath}`);
    await page.getByTestId("import-submit-button").click();
    await waitForFirstJobCompleted(page);

    await page.getByTestId("search-mode-document").click();
    await page.getByTestId("query-document-input").setInputFiles(fixtures.pdfPath);
    await expect(page.getByTestId("query-document-preview")).toBeVisible();

    await page.getByTestId("search-submit-button").click();

    const documentPageResult = page.locator('[data-testid="result-card"][data-kind="document_page"]').first();
    const imageResult = page.locator('[data-testid="result-card"][data-kind="image"]').first();
    await expect(documentPageResult).toBeVisible({ timeout: 2 * 60 * 1000 });
    await expect(imageResult).toBeVisible({ timeout: 2 * 60 * 1000 });
    await expect(page.getByTestId("result-score").first()).toBeVisible();
    await expect(page.getByTestId("detail-panel")).toBeVisible();
    await expect(page.getByTestId("visual-preview")).toBeVisible();
  } finally {
    fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
  }
});

test("document mode can search a specific page range", async ({ page }) => {
  const fixtures = createTempDocumentSearchFixtures();
  try {
    await createLibrary(page, "document-range-search");

    await page
      .getByTestId("import-paths-input")
      .fill(`${fixtures.imagePath}\n${fixtures.pdfPath}`);
    await page.getByTestId("import-submit-button").click();
    await waitForFirstJobCompleted(page);

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

    await page
      .getByTestId("import-paths-input")
      .fill(`${fixtures.imagePath}\n${fixtures.pdfPath}`);
    await page.getByTestId("import-submit-button").click();
    await waitForFirstJobCompleted(page);

    await page.getByTestId("search-mode-document").click();
    await page.getByTestId("query-document-input").setInputFiles(fixtures.pdfPath);
    await expect(page.getByTestId("query-document-preview")).toBeVisible();
    await page.getByTestId("search-submit-button").click();

    const documentPageResult = page.locator('[data-testid="result-card"][data-kind="document_page"]').first();
    await expect(documentPageResult).toBeVisible({ timeout: 2 * 60 * 1000 });
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

test("document mode before import shows not_ready instead of an empty result", async ({ page }) => {
  const fixtures = createTempDocumentSearchFixtures();
  try {
    await createLibrary(page, "document-not-ready");

    await page.getByTestId("search-mode-document").click();
    await page.getByTestId("query-document-input").setInputFiles(fixtures.pdfPath);
    await expect(page.getByTestId("query-document-preview")).toBeVisible();

    await page.getByTestId("search-submit-button").click();

    await expect(page.getByTestId("search-error-notice")).toBeVisible();
    await expect(page.getByTestId("search-error-code")).toHaveText("not_ready");
    await expect(page.getByTestId("search-error-message")).toContainText("active index");
  } finally {
    fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
  }
});

test("document mode rejects non-pdf query uploads with explicit feedback", async ({ page }) => {
  await createLibrary(page, "document-invalid-upload");

  await page.getByTestId("search-mode-document").click();
  await page.getByTestId("query-document-input").setInputFiles(invalidQueryUploadPath);
  await page.getByTestId("search-submit-button").click();

  await expect(page.getByTestId("search-error-notice")).toBeVisible();
  await expect(page.getByTestId("search-error-code")).toHaveText("validation_failed");
  await expect(page.getByTestId("search-error-message")).toContainText(
    "Only PDF files are accepted as query documents right now."
  );
});

test("source management can create edit toggle refresh rescan and filter inventory", async ({ page }) => {
  const fixtures = createTempSourceManagementFixtures();
  try {
    await createLibrary(page, "source-management");

    await page.getByTestId("source-root-path-input").fill(fixtures.tempDir);
    await page.getByTestId("source-root-submit-button").click();

    const rootCard = sourceRootCard(page, fixtures.tempDir);
    await expect(rootCard).toBeVisible();
    await expect(rootCard).toContainText("watching");
    await expect(page.getByTestId("library-refresh-button")).toBeEnabled();
    await expect(page.getByTestId("library-rescan-button")).toBeEnabled();

    let previousJobId = await latestJobId(page);
    await rootCard.locator("[data-source-root-refresh-id]").click();
    await waitForNewLatestJobCompleted(page, previousJobId);
    await expect(rootCard).toContainText("Last action: refresh");

    await openInventoryWorkspace(page);
    await expect(page.getByTestId("inventory-summary")).toBeVisible();

    const imageCard = librarySourceCard(page, "chart.png");
    const pdfCard = librarySourceCard(page, "report.pdf");
    await expect(page.getByTestId("library-source-card")).toHaveCount(2);
    await expect(imageCard).toContainText("visual units 1");
    await expect(pdfCard).toContainText("visual units 2");

    previousJobId = await latestJobId(page);
    await rootCard.locator("[data-source-root-rescan-id]").click();
    await waitForNewLatestJobCompleted(page, previousJobId);
    await expect(rootCard).toContainText("Last action: rescan");

    await page.getByTestId("source-filter-type").selectOption("pdf");
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(pdfCard).toBeVisible();

    await page.getByTestId("source-filter-root").selectOption({ label: fixtures.tempDir });
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(pdfCard).toBeVisible();

    await page.getByTestId("source-filter-type").selectOption("");
    await expect(page.getByTestId("library-source-card")).toHaveCount(2);

    await rootCard.locator("[data-source-root-edit-id]").click();
    await page.getByTestId("source-root-exclude-globs-input").fill("chart.png");
    await page.getByTestId("source-root-submit-button").click();
    await expect(rootCard).toContainText("exclude 1");

    previousJobId = await latestJobId(page);
    await page.getByTestId("library-refresh-button").click();
    await waitForNewLatestJobCompleted(page, previousJobId);
    await expect(rootCard).toContainText("Last action: refresh");

    await page.getByTestId("source-filter-status").selectOption("active");
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(pdfCard).toContainText("active");

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

    previousJobId = await latestJobId(page);
    await page.getByTestId("library-rescan-button").click();
    await waitForNewLatestJobCompleted(page, previousJobId);
    await expect(rootCard).toContainText("Last action: rescan");

    await page.getByTestId("source-filter-status").selectOption("active");
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(pdfCard).toContainText("active");

    await page.getByTestId("source-filter-status").selectOption("out_of_scope");
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(imageCard).toContainText("rule_excluded");
  } finally {
    fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
  }
});

test("source management watcher updates inventory for add modify and delete", async ({ page }) => {
  const fixtures = createTempSourceManagementFixtures();
  try {
    await createLibrary(page, "source-management-watcher");

    await page.getByTestId("source-root-path-input").fill(fixtures.tempDir);
    await page.getByTestId("source-root-submit-button").click();

    const rootCard = sourceRootCard(page, fixtures.tempDir);
    await expect(rootCard).toBeVisible();

    let previousJobId = await latestJobId(page);
    await rootCard.locator("[data-source-root-refresh-id]").click();
    await waitForNewLatestJobCompleted(page, previousJobId);

    await openInventoryWorkspace(page);

    const pdfCard = librarySourceCard(page, "report.pdf");
    const addedImageCard = librarySourceCard(page, "new-chart.png");
    await expect(page.getByTestId("library-source-card")).toHaveCount(2);
    await expect(pdfCard).toContainText("visual units 2");

    previousJobId = await latestJobId(page);
    fs.copyFileSync(fixtureImagePath, fixtures.addedImagePath);
    await waitForNewLatestJobCompleted(page, previousJobId);
    await expect(rootCard).toContainText("Last action: refresh");
    await expect(page.getByTestId("library-source-card")).toHaveCount(3);
    await expect(addedImageCard).toContainText("visual units 1");

    previousJobId = await latestJobId(page);
    writeSourceManagementPdf(fixtures.pdfPath, 1);
    await waitForNewLatestJobCompleted(page, previousJobId);
    await expect(pdfCard).toContainText("visual units 1");

    previousJobId = await latestJobId(page);
    fs.rmSync(fixtures.addedImagePath, { force: true });
    await waitForNewLatestJobCompleted(page, previousJobId);

    await page.getByTestId("source-filter-status").selectOption("invalidated");
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(addedImageCard).toContainText("not_found");
  } finally {
    fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
  }
});

test("inventory workspace stays usable on narrow screens", async ({ page }) => {
  const fixtures = createTempSourceManagementFixtures();
  try {
    await page.setViewportSize({ width: 820, height: 1280 });
    await createLibrary(page, "inventory-narrow");

    await page.getByTestId("source-root-path-input").fill(fixtures.tempDir);
    await page.getByTestId("source-root-submit-button").click();

    const rootCard = sourceRootCard(page, fixtures.tempDir);
    await expect(rootCard).toBeVisible();

    const previousJobId = await latestJobId(page);
    await rootCard.locator("[data-source-root-refresh-id]").click();
    await waitForNewLatestJobCompleted(page, previousJobId);

    await openInventoryWorkspace(page);
    await expect(page.getByTestId("inventory-summary")).toBeVisible();
    await expect(page.getByTestId("library-source-card").first()).toBeVisible();
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)
    ).toBe(true);
  } finally {
    fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
  }
});
