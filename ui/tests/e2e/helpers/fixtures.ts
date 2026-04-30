import { expect, test } from "@playwright/test";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { SearchResultItem } from "../../../src/types";

export { expect, test };

export const __filename = fileURLToPath(import.meta.url);
export const __dirname = path.dirname(__filename);
export const fixtureImagePath = path.resolve(
  __dirname,
  "../../../../tests/fixtures/tatdqa-page-images/images/tatdqa-page-0001.png"
);
export const fixtureDocumentPath = path.resolve(__dirname, "../../../../data/example/2025年中期报告.pdf");
export const fixtureVideoPath = path.resolve(
  __dirname,
  "../../../../data/example/generate_q2_report_from_csv_bank_data-720-512.mp4"
);
export const invalidQueryUploadPath = path.resolve(__dirname, "../../../../README.md");
export const venvPythonPath = path.resolve(__dirname, "../../../../.venv/bin/python");
export const workspacePollWaitMs = 3_500;

export function fileSourceUri(sourcePath) {
  const value = String(sourcePath);
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(value)) {
    return value;
  }
  return `file://${value}`;
}

export function createMockMatchedUnits(index) {
  return [
    {
      unit_id: `unit_mock_${index}`,
      unit_type: "page_image",
      vector_space_id: "vs_mock_late_interaction",
      rank: index + 1,
      raw_score: 100 - index,
    },
  ];
}

function unitsFromMatchedUnits(matchedUnits) {
  return matchedUnits.map((unit) => ({
    unit_id: unit.unit_id,
    unit_type: unit.unit_type,
  }));
}

function colorLuminance(color) {
  const match = color.match(/\d+(\.\d+)?/g);
  if (!match || match.length < 3) {
    return 0;
  }
  const [r, g, b] = match.slice(0, 3).map(Number);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function readColorChannels(color) {
  const match = color.match(/\d+(\.\d+)?/g);
  if (!match || match.length < 3) {
    return { r: 0, g: 0, b: 0, a: 1 };
  }
  const [r, g, b, a = 1] = match.map(Number);
  return { r, g, b, a };
}

export async function readSelectionControlStyles(locator) {
  return locator.evaluate((element) => {
    const style = window.getComputedStyle(element);
    return {
      selected: element.getAttribute("data-ui-selected"),
      backgroundColor: style.backgroundColor,
      color: style.color,
      borderColor: style.borderColor,
      boxShadow: style.boxShadow,
    };
  });
}

export async function expectSelectionControlContrast(selectedLocator, inactiveLocator) {
  await expect(selectedLocator).toHaveAttribute("data-ui-selected", "true");
  await expect(inactiveLocator).toHaveAttribute("data-ui-selected", "false");

  const selected = await readSelectionControlStyles(selectedLocator);
  const inactive = await readSelectionControlStyles(inactiveLocator);
  const selectedBackground = readColorChannels(selected.backgroundColor);
  const selectedText = readColorChannels(selected.color);
  const selectedBorder = readColorChannels(selected.borderColor);

  expect(selected.backgroundColor).not.toBe(inactive.backgroundColor);
  expect(selected.color).not.toBe(inactive.color);
  expect(selected.borderColor).not.toBe(inactive.borderColor);

  expect(colorLuminance(selected.backgroundColor)).toBeGreaterThan(230);
  expect(colorLuminance(selected.backgroundColor)).toBeLessThan(colorLuminance(inactive.backgroundColor));

  expect(selectedBackground.g).toBeGreaterThan(selectedBackground.r);
  expect(selectedBackground.g).toBeGreaterThanOrEqual(selectedBackground.b);

  expect(selectedText.r).toBeLessThan(80);
  expect(selectedText.g).toBeLessThan(120);
  expect(selectedText.b).toBeLessThan(120);
  expect(colorLuminance(selected.color)).toBeLessThan(110);

  expect(selectedBorder.g).toBeGreaterThan(selectedBorder.r);
  expect(selectedBorder.g).toBeGreaterThan(selectedBorder.b);
}

export async function pasteImageIntoQueryTarget(page, imagePath) {
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

export async function ensureCreateLibraryPopoverOpen(page) {
  await openInventoryWorkspace(page);
  const libraryNameInput = page.getByTestId("library-name-input");
  if (!(await libraryNameInput.isVisible())) {
    await page.getByTestId("open-create-library-button").click();
  }
  await expect(libraryNameInput).toBeVisible();
  return libraryNameInput;
}

export async function createLibrary(page, suffix) {
  const libraryName = `playwright-${suffix}-${Date.now()}`;
  await page.goto("/");
  await expect(page.getByTestId("workspace-shell")).toBeVisible();
  const libraryNameInput = await ensureCreateLibraryPopoverOpen(page);
  await libraryNameInput.fill(libraryName);
  await page.getByTestId("create-library-button").click();
  await expect(page.getByTestId("library-select")).toContainText(libraryName);
  await openSearchWorkspace(page);
  await expect(page.getByTestId("library-select")).toContainText(libraryName);
  return libraryName;
}

export async function ensureManageLibraryPopoverOpen(page) {
  await openInventoryWorkspace(page);
  const libraryNameInput = page.getByTestId("inventory-manage-library-name-input");
  await expect(libraryNameInput).toBeVisible();
  return libraryNameInput;
}

export async function currentLibraryId(page) {
  await expect(page.getByTestId("library-select")).toBeVisible();
  return await page.getByTestId("library-select").inputValue();
}

export async function openInventoryWorkspace(page) {
  await page.getByTestId("workspace-tab-inventory").click();
  await expect(page.getByTestId("inventory-panel")).toBeVisible();
}

export async function openInventorySourceManagement(page) {
  await openInventoryWorkspace(page);
  const managementPanel = page.getByTestId("inventory-source-management-panel");
  if (!(await managementPanel.isVisible())) {
    await page.getByTestId("inventory-action-manage-source-roots").click();
  }
  await expect(managementPanel).toBeVisible();
}

export async function openInventoryImportPanel(page) {
  await openInventoryWorkspace(page);
  const importPanel = page.getByTestId("inventory-import-panel");
  if (!(await importPanel.isVisible())) {
    await page.getByTestId("inventory-action-import-paths").click();
  }
  await expect(importPanel).toBeVisible();
}

export async function openSearchWorkspace(page) {
  await page.getByTestId("workspace-tab-search").click();
  await expect(page.getByTestId("search-panel")).toBeVisible();
}

export async function openSourcePreparationPanel(page) {
  await openInventorySourceManagement(page);
  const sourceRootPathInput = page.getByTestId("source-root-path-input");
  if (!(await sourceRootPathInput.isVisible())) {
    await page.getByTestId("inventory-source-root-create-button").click();
  }
  await expect(sourceRootPathInput).toBeVisible();
}

export async function openSourceRootAdvancedRules(page) {
  const advancedRulesPanel = page.getByTestId("source-root-advanced-rules-panel");
  if (!(await advancedRulesPanel.isVisible())) {
    await page.getByTestId("source-root-advanced-rules-toggle").click();
  }
  await expect(advancedRulesPanel).toBeVisible();
}

export async function prepareSearchableSourceRoot(page, rootPath) {
  const libraryId = await currentLibraryId(page);
  await openSourcePreparationPanel(page);
  await page.getByTestId("source-root-path-input").fill(rootPath);
  await page.getByTestId("source-root-submit-button").click();

  const rootCard = sourceRootCard(page, rootPath);
  await expect(rootCard).toBeVisible();
  const previousJobId = await latestJobId(page, libraryId);
  await rootCard.locator("[data-source-root-refresh-id]").click();
  await waitForNewLatestJobCompleted(page, libraryId, previousJobId);
  await openSearchWorkspace(page);
  return rootCard;
}

export async function openSettingsWorkspace(page) {
  await page.getByTestId("workspace-tab-settings").click();
  await expect(page.getByTestId("settings-workspace")).toBeVisible();
}

export async function openSettingsSection(page, section) {
  await openSettingsWorkspace(page);
  await page.getByTestId(`settings-nav-${section}`).click();
}

export async function openSettingsDiagnostics(page) {
  if (!(await page.getByTestId("settings-workspace").isVisible())) {
    await page.getByTestId("status-capsule-button").click();
  } else if (!(await page.getByTestId("runtime-status-panel").isVisible())) {
    await openSettingsSection(page, "diagnostics");
  }
  await expect(page.getByTestId("settings-workspace")).toBeVisible();
  await expect(page.getByTestId("runtime-status-panel")).toBeVisible();
}

export async function openDiagnosticsJobs(page) {
  await openSettingsDiagnostics(page);
  const disclosure = page.getByTestId("settings-diagnostics-jobs-disclosure");
  if ((await disclosure.getAttribute("open")) === null) {
    await disclosure.locator("summary").click();
  }
  await expect(disclosure).toHaveAttribute("open", "");
}

export async function waitForFirstJobCompleted(page) {
  const libraryId = await currentLibraryId(page);
  await expect
    .poll(
      async () => {
        const response = await page.request.get("/api/jobs");
        if (!response.ok()) {
          return null;
        }
        const payload = await response.json();
        const latest = (payload?.data?.jobs ?? []).find((job) => job?.library_id === libraryId) ?? null;
        return latest?.status ?? null;
      },
      {
        timeout: 10 * 60 * 1000,
        intervals: [1_000, 2_000, 5_000],
      }
    )
    .toBe("completed");
}

export async function prepareSearchableLibrary(page) {
  await importFixtureIntoCurrentLibrary(page);
}

export async function importFixtureIntoCurrentLibrary(page, importPath = fixtureImagePath) {
  await importFixturesIntoCurrentLibrary(page, [importPath]);
}

export async function importFixturesIntoCurrentLibrary(page, importPaths) {
  const libraryId = await currentLibraryId(page);
  const response = await page.request.post(`/api/libraries/${libraryId}/imports`, {
    data: {
      paths: importPaths,
    },
  });
  expect(response.ok()).toBeTruthy();
  await waitForFirstJobCompleted(page);
  await expect(page.getByTestId("search-submit-button")).toBeEnabled();
}

export async function mockSingleTextSearchResult(
  page,
  queryText = "Revenue 46 percent",
  sourcePath = "/tmp/search-fixtures/formal/report-1.pdf"
) {
  const libraryId = await currentLibraryId(page);
  const result = {
    ...createMockSearchResult(0, sourcePath),
    library_id: libraryId,
  };

  await page.route("**/api/search/text", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }

    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        data: {
          results: [result],
          next_cursor: null,
          debug: {
            backend: "qdrant",
          },
        },
      }),
    });
  });

  await page.route(`**/api/libraries/${libraryId}/assets/${result.asset_id}`, async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        data: {
          asset: {
            asset_id: result.asset_id,
            source_id: result.source_id,
            source_uri: result.source_uri,
            source_type: result.source_type,
            asset_type: result.asset_type,
            locator: result.locator,
          },
          preview: result.preview,
          neighbor_context: null,
          units: unitsFromMatchedUnits(result.matched_units),
          library_id: libraryId,
        },
      }),
    });
  });

  await page.getByTestId("search-text-input").fill(queryText);
  await page.getByTestId("search-submit-button").click();
  return result;
}

export async function mockDocumentSearchResults(
  page,
  sourcePath = "/tmp/search-fixtures/formal/query-document.pdf"
) {
  const libraryId = await currentLibraryId(page);
  const results = [createMockSearchResult(0, sourcePath), createMockSearchResult(1, sourcePath)].map(
    (result) => ({
      ...result,
      library_id: libraryId,
    })
  );

  await page.route("**/api/search/document", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }

    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        data: {
          results,
          next_cursor: null,
          debug: {
            backend: "qdrant",
          },
        },
      }),
    });
  });

  await page.route(`**/api/libraries/${libraryId}/assets/*`, async (route) => {
    const assetId = route.request().url().split("/").pop();
    const result = results.find((entry) => entry.asset_id === assetId) ?? results[0];
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        data: {
          asset: {
            asset_id: result.asset_id,
            source_id: result.source_id,
            source_uri: result.source_uri,
            source_type: result.source_type,
            asset_type: result.asset_type,
            locator: result.locator,
          },
          preview: result.preview,
          neighbor_context: {
            previous: null,
            next: null,
          },
          units: unitsFromMatchedUnits(result.matched_units),
          library_id: libraryId,
        },
      }),
    });
  });

  return results;
}

export async function mockImageSearchResults(
  page,
  results?: Array<Omit<SearchResultItem, "library_id">>
) {
  const libraryId = await currentLibraryId(page);
  const resolvedResults = (results ?? [
    {
      asset_id: "asset_image_mock_0",
      source_id: "src_image_mock_0",
      preview: {
        url: "http://127.0.0.1:54210/mock-preview/image-0.png",
      },
      source_uri: fileSourceUri("/tmp/search-fixtures/formal/query-image.png"),
      source_type: "image",
      asset_type: "image",
      locator: null,
      cursor: "search:v1:image:1",
      score: 100,
      matched_units: createMockMatchedUnits(0),
    },
  ]).map((result) => ({
    ...result,
    library_id: libraryId,
  }));

  await page.route("**/api/search/image", async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }

    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        data: {
          results: resolvedResults,
          next_cursor: null,
          debug: {
            backend: "qdrant",
          },
        },
      }),
    });
  });

  await page.route(`**/api/libraries/${libraryId}/assets/*`, async (route) => {
    const assetId = route.request().url().split("/").pop();
    const result =
      resolvedResults.find((entry) => entry.asset_id === assetId) ?? resolvedResults[0];
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        data: {
          asset: {
            asset_id: result.asset_id,
            source_id: result.source_id,
            source_uri: result.source_uri,
            source_type: result.source_type,
            asset_type: result.asset_type,
            locator: result.locator,
          },
          preview: result.preview,
          neighbor_context:
            result.asset_type === "document_page" ? { previous: null, next: null } : null,
          units: unitsFromMatchedUnits(result.matched_units),
          library_id: libraryId,
        },
      }),
    });
  });

  return resolvedResults;
}

export async function expectSearchRequiresContent(page) {
  await expect(page.getByTestId("search-submit-button")).toBeDisabled();
  await expect(page.getByTestId("search-state-strip")).toContainText("尚未接入来源根");
  await expect(page.getByTestId("search-readiness-action")).toBeVisible();
  await expect(page.getByTestId("search-readiness-action")).toContainText("接入来源");
  await expect(page.getByTestId("search-readiness-open-inventory")).toBeVisible();
  await expect(page.getByTestId("search-next-step-dock")).toHaveCount(0);
  await expect(page.getByTestId("import-form")).toHaveCount(0);
}

export async function openSearchAdvancedFilters(page) {
  const pathPrefixInput = page.getByTestId("search-filter-path-prefix");
  if (!(await pathPrefixInput.isVisible())) {
    const filterToggleButton = page.getByTestId("search-filter-toggle-button");
    if (await filterToggleButton.isVisible()) {
      await filterToggleButton.click({ timeout: 10_000 });
    } else {
      await page.locator("summary").filter({ hasText: "高级过滤" }).click({ timeout: 10_000 });
    }
  }
  await expect(pathPrefixInput).toBeVisible();
}

export function createTempPdfFixture() {
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

export function createTempDocumentSearchFixtures() {
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

export function createMockSearchResult(index, sourcePath) {
  return {
    asset_id: `asset_mock_${index}`,
    source_id: `src_mock_${index}`,
    preview: {
      url: `http://127.0.0.1:54210/mock-preview/${index}.png`,
    },
    source_uri: fileSourceUri(sourcePath),
    source_type: "pdf",
    asset_type: "document_page",
    locator: {
      page: index + 1,
      page_label: String(index + 1),
    },
    cursor: `search:v1:${index + 1}`,
    score: 100 - index,
    matched_units: createMockMatchedUnits(index),
  };
}

export function createTempVideoSearchFixtures() {
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

export function writeSourceManagementPdf(pdfPath, pageCount) {
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

export function createTempSourceManagementFixtures() {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "fauni-search-source-management-"));
  const imagePath = path.join(tempDir, "chart.png");
  const addedImagePath = path.join(tempDir, "new-chart.png");
  const pdfPath = path.join(tempDir, "report.pdf");

  fs.copyFileSync(fixtureImagePath, imagePath);
  writeSourceManagementPdf(pdfPath, 2);

  return { tempDir, imagePath, addedImagePath, pdfPath };
}

export function sourceRootCard(page, rootPath) {
  return page.getByTestId("source-root-card").filter({ hasText: rootPath });
}

export function librarySourceCard(page, sourceName) {
  return page.getByTestId("library-source-card").filter({ hasText: sourceName });
}

export async function latestJobId(page, libraryId) {
  const response = await page.request.get("/api/jobs");
  expect(response.ok()).toBeTruthy();
  const payload = await response.json();
  const jobs = payload?.data?.jobs ?? [];
  return jobs.find((job) => job?.library_id === libraryId)?.job_id ?? null;
}

export async function waitForNewLatestJobCompleted(page, libraryId, previousJobId) {
  let nextJobId = null;
  await expect
    .poll(
      async () => {
        const response = await page.request.get("/api/jobs");
        if (!response.ok()) {
          return null;
        }
        const payload = await response.json();
        const jobs = payload?.data?.jobs ?? [];
        const jobId = jobs.find((job) => job?.library_id === libraryId)?.job_id ?? null;
        nextJobId = jobId && jobId !== previousJobId ? jobId : null;
        return nextJobId;
      },
      {
        timeout: 2 * 60 * 1000,
        intervals: [1_000, 2_000, 5_000],
      }
    )
    .toBeTruthy();

  await expect
    .poll(async () => {
      if (!nextJobId) {
        return null;
      }
      const response = await page.request.get("/api/jobs");
      if (!response.ok()) {
        return null;
      }
      const payload = await response.json();
      const latest = (payload?.data?.jobs ?? []).find((job) => job?.library_id === libraryId) ?? null;
      return latest?.job_id === nextJobId ? latest.status : null;
    }, {
      timeout: 2 * 60 * 1000,
      intervals: [1_000, 2_000, 5_000],
    })
    .toBe("completed");

  return nextJobId;
}

export async function setRangeValue(locator, value) {
  await locator.evaluate((element, nextValue) => {
    element.value = String(nextValue);
    element.dispatchEvent(new Event("input", { bubbles: true }));
  }, value);
}
