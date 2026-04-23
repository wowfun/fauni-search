import { expect, test } from "@playwright/test";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { SearchResultItem } from "../../../src/types";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const fixtureImagePath = path.resolve(
  __dirname,
  "../../../../tests/fixtures/tatdqa-page-images/images/tatdqa-page-0001.png"
);
const fixtureDocumentPath = path.resolve(__dirname, "../../../../data/example/2025年中期报告.pdf");
const fixtureVideoPath = path.resolve(
  __dirname,
  "../../../../data/example/generate_q2_report_from_csv_bank_data-720-512.mp4"
);
const invalidQueryUploadPath = path.resolve(__dirname, "../../../../README.md");
const venvPythonPath = path.resolve(__dirname, "../../../../.venv/bin/python");
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

async function ensureCreateLibraryPopoverOpen(page) {
  await openInventoryWorkspace(page);
  const libraryNameInput = page.getByTestId("library-name-input");
  if (!(await libraryNameInput.isVisible())) {
    await page.getByTestId("open-create-library-button").click();
  }
  await expect(libraryNameInput).toBeVisible();
  return libraryNameInput;
}

async function createLibrary(page, suffix) {
  const libraryName = `playwright-${suffix}-${Date.now()}`;
  await page.goto("/");
  await expect(page.getByTestId("workspace-shell")).toBeVisible();
  const libraryNameInput = await ensureCreateLibraryPopoverOpen(page);
  await libraryNameInput.fill(libraryName);
  await page.getByTestId("create-library-button").click();
  await expect(page.getByTestId("current-library-name")).toHaveText(libraryName);
  await openSearchWorkspace(page);
  return libraryName;
}

async function ensureManageLibraryPopoverOpen(page) {
  await openInventoryWorkspace(page);
  const libraryNameInput = page.getByTestId("manage-library-name-input");
  if (!(await libraryNameInput.isVisible())) {
    await page.getByTestId("open-manage-library-button").click();
  }
  await expect(libraryNameInput).toBeVisible();
  return libraryNameInput;
}

async function currentLibraryId(page) {
  await expect(page.getByTestId("library-select")).toBeVisible();
  return await page.getByTestId("library-select").inputValue();
}

export function registerLibraryScenarios() {
  test("library creation shows display name and custom library id separately", async ({ page }) => {
    const displayName = `Invoice Demo ${Date.now()}`;
    const libraryId = `invoice-demo-${Date.now()}`;

    await page.goto("/");
    await expect(page.getByTestId("workspace-shell")).toBeVisible();
    await (await ensureCreateLibraryPopoverOpen(page)).fill(displayName);
    await page.getByTestId("library-id-input").fill(libraryId);
    await page.getByTestId("create-library-button").click();

    await expect(page.getByTestId("current-library-name")).toHaveText(displayName);
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
    await page.getByTestId("rename-library-button").click();

    await expect(page.getByTestId("current-library-name")).toHaveText(renamed);
    await expect(page.getByTestId("library-select")).toContainText(renamed);
    await expect(page.getByTestId("current-library-lifecycle")).toHaveText("活跃");

    await ensureManageLibraryPopoverOpen(page);
    page.once("dialog", (dialog) => dialog.accept());
    await page.getByTestId("toggle-library-archive-button").click();

    await expect(page.getByTestId("current-library-lifecycle")).toHaveText("已归档");
    await expect(page.getByTestId("library-select")).toContainText(`${renamed} (${libraryId}) · 已归档`);

    await ensureManageLibraryPopoverOpen(page);
    await page.getByTestId("toggle-library-archive-button").click();

    await expect(page.getByTestId("current-library-lifecycle")).toHaveText("活跃");
    await expect(page.getByTestId("library-select")).not.toContainText(
      `${renamed} (${libraryId}) · 已归档`
    );

    await ensureManageLibraryPopoverOpen(page);
    page.once("dialog", (dialog) => dialog.accept());
    await page.getByTestId("delete-library-button").click();

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
    await expect(page.getByTestId("inventory-library-management")).toBeVisible();
    await expect(page.getByTestId("inventory-library-name")).toHaveText(originalName);
    await expect(page.getByTestId("inventory-library-lifecycle")).toHaveText("活跃");

    await page.getByTestId("inventory-manage-library-name-input").fill(renamed);
    await page.getByTestId("inventory-rename-library-button").click();

    await expect(page.getByTestId("inventory-library-name")).toHaveText(renamed);
    await expect(page.getByTestId("current-library-name")).toHaveText(renamed);

    page.once("dialog", (dialog) => dialog.accept());
    await page.getByTestId("inventory-toggle-library-archive-button").click();
    await expect(page.getByTestId("inventory-library-lifecycle")).toHaveText("已归档");
    await expect(page.getByTestId("inventory-summary")).toBeVisible();

    await page.getByTestId("inventory-toggle-library-archive-button").click();
    await expect(page.getByTestId("inventory-library-lifecycle")).toHaveText("活跃");

    page.once("dialog", (dialog) => dialog.accept());
    await page.getByTestId("inventory-delete-library-button").click();
    await expect(page.getByTestId("library-select")).not.toHaveValue(libraryId);
    await expect(page.getByTestId("library-select")).not.toContainText(`${renamed} (${libraryId})`);
  });

  test("inventory workspace links current library readiness to settings overrides", async ({
    page,
  }) => {
    await createLibrary(page, "inventory-library-overrides-link");

    await openInventoryWorkspace(page);
    await expect(page.getByTestId("inventory-library-config-strip")).toBeVisible();
    await expect(page.getByTestId("inventory-library-config-list")).toContainText("文本");
    await expect(page.getByTestId("inventory-library-config-list")).toContainText("图片");

    await page.getByTestId("inventory-open-library-overrides-button").click();
    await expect(page.getByTestId("settings-workspace")).toBeVisible();
    await expect(page.getByTestId("settings-stage-title")).toHaveText("当前库覆盖");
    await expect(page.getByTestId("library-content-types-panel")).toBeVisible();
  });

  test("manage library popover stays stable inside inventory across workspace polling", async ({
    page,
  }) => {
    const originalName = await createLibrary(page, "library-manage-popover");
    await openInventoryWorkspace(page);
    const cluster = page.getByTestId("library-context-cluster");
    const clusterBoxBefore = await cluster.boundingBox();
    const manageNameInput = await ensureManageLibraryPopoverOpen(page);
    const manageCard = page.getByTestId("manage-library-card");
    const draftName = `${originalName} draft`;

    await expect(manageCard).toBeVisible();
    const clusterBoxAfterOpen = await cluster.boundingBox();
    expect(clusterBoxBefore).not.toBeNull();
    expect(clusterBoxAfterOpen).not.toBeNull();
    expect(clusterBoxAfterOpen!.height).toBeLessThan(clusterBoxBefore!.height + 20);

    await manageNameInput.fill(draftName);
    await page.getByTestId("current-library-name").click();
    await expect(manageNameInput).not.toBeFocused();

    await page.waitForTimeout(3500);

    await expect(manageCard).toBeVisible();
    await expect(manageNameInput).toHaveValue(draftName);
  });
}

async function openInventoryWorkspace(page) {
  await page.getByTestId("workspace-tab-inventory").click();
  await expect(page.getByTestId("inventory-panel")).toBeVisible();
}

async function openSearchWorkspace(page) {
  await page.getByTestId("workspace-tab-search").click();
  await expect(page.getByTestId("search-panel")).toBeVisible();
}

async function openSourcePreparationPanel(page) {
  await openSearchWorkspace(page);
  const sourceRootPathInput = page.getByTestId("source-root-path-input");
  if (!(await sourceRootPathInput.isVisible())) {
    await page.locator("summary").filter({ hasText: "导入与来源准备" }).click();
  }
  await expect(sourceRootPathInput).toBeVisible();
}

async function openSettingsWorkspace(page) {
  await page.getByTestId("workspace-tab-settings").click();
  await expect(page.getByTestId("settings-workspace")).toBeVisible();
}

async function openSettingsSection(page, section) {
  await openSettingsWorkspace(page);
  await page.getByTestId(`settings-nav-${section}`).click();
}

async function openUtilityDrawerSection(page, section) {
  if (section === "status") {
    await page.getByTestId("utility-drawer-open-status").click();
  } else if (!(await page.getByTestId("utility-drawer").isVisible())) {
    await page.getByTestId("workspace-tab-tools").click();
  }

  await expect(page.getByTestId("utility-drawer")).toBeVisible();
  if (
    section !== "status" &&
    (await page.getByTestId("utility-drawer").getAttribute("data-drawer-section")) !== section
  ) {
    await page.getByTestId(`utility-drawer-tab-${section}`).click();
  }
  await expect(page.getByTestId("utility-drawer")).toHaveAttribute("data-drawer-section", section);
}

async function waitForFirstJobCompleted(page) {
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

async function prepareSearchableLibrary(page) {
  await importFixtureIntoCurrentLibrary(page);
}

async function importFixtureIntoCurrentLibrary(page, importPath = fixtureImagePath) {
  const libraryId = await currentLibraryId(page);
  const response = await page.request.post(`/api/libraries/${libraryId}/imports`, {
    data: {
      paths: [importPath],
    },
  });
  expect(response.ok()).toBeTruthy();
  await waitForFirstJobCompleted(page);
  await expect(page.getByTestId("search-submit-button")).toBeEnabled();
}

async function mockSingleTextSearchResult(
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

  await page.route(`**/api/libraries/${libraryId}/visual-units/${result.visual_unit_id}`, async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        data: {
          visual_unit: {
            visual_unit_id: result.visual_unit_id,
            source_id: result.source_id,
            source_path: result.source_path,
            source_type: result.source_type,
            kind: result.kind,
            locator: result.locator,
          },
          preview: result.preview,
          neighbor_context: null,
          library_id: libraryId,
        },
      }),
    });
  });

  await page.getByTestId("search-text-input").fill(queryText);
  await page.getByTestId("search-submit-button").click();
  return result;
}

async function mockDocumentSearchResults(
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

  await page.route(`**/api/libraries/${libraryId}/visual-units/*`, async (route) => {
    const visualUnitId = route.request().url().split("/").pop();
    const result = results.find((entry) => entry.visual_unit_id === visualUnitId) ?? results[0];
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        data: {
          visual_unit: {
            visual_unit_id: result.visual_unit_id,
            source_id: result.source_id,
            source_path: result.source_path,
            source_type: result.source_type,
            kind: result.kind,
            locator: result.locator,
          },
          preview: result.preview,
          neighbor_context: {
            previous: null,
            next: null,
          },
          library_id: libraryId,
        },
      }),
    });
  });

  return results;
}

async function mockImageSearchResults(
  page,
  results?: Array<Omit<SearchResultItem, "library_id">>
) {
  const libraryId = await currentLibraryId(page);
  const resolvedResults = (results ?? [
    {
      visual_unit_id: "vu_image_mock_0",
      source_id: "src_image_mock_0",
      preview: {
        url: "http://127.0.0.1:54210/mock-preview/image-0.png",
      },
      source_path: "/tmp/search-fixtures/formal/query-image.png",
      source_type: "image",
      kind: "image",
      locator: null,
      cursor: "search:v1:image:1",
      score: 100,
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

  await page.route(`**/api/libraries/${libraryId}/visual-units/*`, async (route) => {
    const visualUnitId = route.request().url().split("/").pop();
    const result =
      resolvedResults.find((entry) => entry.visual_unit_id === visualUnitId) ?? resolvedResults[0];
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        data: {
          visual_unit: {
            visual_unit_id: result.visual_unit_id,
            source_id: result.source_id,
            source_path: result.source_path,
            source_type: result.source_type,
            kind: result.kind,
            locator: result.locator,
          },
          preview: result.preview,
          neighbor_context: result.kind === "document_page" ? { previous: null, next: null } : null,
          library_id: libraryId,
        },
      }),
    });
  });

  return resolvedResults;
}

async function expectSearchRequiresContent(page) {
  await expect(page.getByTestId("search-submit-button")).toBeDisabled();
  await expect(page.getByTestId("search-state-strip")).toContainText("尚未接入来源根");
  await expect(page.getByTestId("search-next-step-dock")).toBeVisible();
  await expect(page.getByTestId("search-next-step-dock")).toContainText("接入第一个来源根");
  await expect(page.getByTestId("search-next-step-open-source-prep")).toBeVisible();
}

async function openSearchAdvancedFilters(page) {
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

function createMockSearchResult(index, sourcePath) {
  return {
    visual_unit_id: `vu_mock_${index}`,
    source_id: `src_mock_${index}`,
    preview: {
      url: `http://127.0.0.1:54210/mock-preview/${index}.png`,
    },
    source_path: sourcePath,
    source_type: "pdf",
    kind: "document_page",
    locator: {
      page: index + 1,
      page_label: String(index + 1),
    },
    cursor: `search:v1:${index + 1}`,
    score: 100 - index,
  };
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

async function latestJobId(page, libraryId) {
  const response = await page.request.get("/api/jobs");
  expect(response.ok()).toBeTruthy();
  const payload = await response.json();
  const jobs = payload?.data?.jobs ?? [];
  return jobs.find((job) => job?.library_id === libraryId)?.job_id ?? null;
}

async function waitForNewLatestJobCompleted(page, libraryId, previousJobId) {
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

async function setRangeValue(locator, value) {
  await locator.evaluate((element, nextValue) => {
    element.value = String(nextValue);
    element.dispatchEvent(new Event("input", { bubbles: true }));
  }, value);
}

export function registerSearchTextScenarios() {
  test("manual import and submitted search keep results and detail in the same workspace", async ({
    page,
  }) => {
    await createLibrary(page, "smoke");
    await expect(page.getByTestId("search-panel")).toContainText("Search anything you want");
    await expect(page.getByTestId("search-panel")).not.toContainText("Search Stage");
    await importFixtureIntoCurrentLibrary(page);
    await mockSingleTextSearchResult(page, "Revenue 46 percent");

    const firstResult = page.getByTestId("result-card").first();
    await expect(firstResult).toBeVisible({ timeout: 2 * 60 * 1000 });
    await expect(firstResult.getByTestId("result-score")).toBeVisible();
    await expect(firstResult.getByTestId("result-preview")).toBeVisible();

    await expect(page.getByTestId("detail-panel")).toBeVisible();
    await expect(page.getByTestId("visual-unit-detail")).toBeVisible();
    await expect(page.getByTestId("visual-preview")).toBeVisible();
    await expect(page.locator('[data-testid="preview-link"]')).toHaveCount(0);
    await expect(page.locator('[data-testid="detail-use-as-query-document-button"]')).toHaveCount(0);
    await expect(page.getByTestId("detail-technical-content")).not.toBeVisible();
    await page.getByTestId("detail-technical-disclosure").locator("summary").click();
    await expect(page.getByTestId("detail-technical-content")).toBeVisible();
  });

  test("search before source preparation keeps submit disabled and points to the first source root", async ({ page }) => {
    await createLibrary(page, "not-ready");

    await page.getByTestId("search-text-input").fill("operating activities");
    await expectSearchRequiresContent(page);
  });

  test("search readiness distinguishes missing source roots from missing content", async ({ page }) => {
    const fixtures = createTempSourceManagementFixtures();
    try {
      await createLibrary(page, "search-readiness");

      await page.getByTestId("search-text-input").fill("quarterly planning");
      await expectSearchRequiresContent(page);

      await openSourcePreparationPanel(page);
      await page.getByTestId("source-root-path-input").fill(fixtures.tempDir);
      await page.getByTestId("source-root-submit-button").click();

      await expect(page.getByTestId("search-state-strip")).toContainText("等待内容");
      await expect(page.getByTestId("search-next-step-dock")).toContainText("准备第一批内容");
      await expect(page.getByTestId("search-next-step-open-inventory")).toBeVisible();
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });
}

export function registerWorkspaceRegressionScenarios() {
  test("default workspace keeps search first and lets the empty stage own the desktop layout", async ({ page }) => {
    await createLibrary(page, "workspace-default");

    await expect(page.getByTestId("workspace-tab-search")).toBeVisible();
    await expect(page.getByTestId("workspace-tab-inventory")).toBeVisible();
    await expect(page.getByTestId("search-panel")).toBeVisible();
    await expect(page.getByTestId("search-inline-outcome")).toHaveCount(0);
    await expect(page.getByTestId("search-results-column")).toHaveCount(0);
    await expect(page.getByTestId("detail-panel")).toHaveCount(0);
    await expect(page.getByTestId("inventory-panel")).toHaveCount(0);
    await expect(page.getByTestId("inventory-bridge-button")).toBeVisible();
  });

  test("status drawer mirrors search readiness and opens source prep from the same next step", async ({
    page,
  }) => {
    await createLibrary(page, "workspace-status-next-step");

    await openUtilityDrawerSection(page, "status");
    await expect(page.getByTestId("utility-drawer-stage-state")).toContainText("尚未接入来源根");
    await expect(page.getByTestId("utility-drawer-status-next-step")).toContainText("接入第一个来源根");
    await page.getByTestId("utility-drawer-status-open-source-prep").click();

    await expect(page.getByTestId("search-panel")).toBeVisible();
    await expect(page.getByTestId("source-root-form")).toBeVisible();
  });

  test("status and tools share a unified drawer while search results still keep result and detail columns", async ({
    page,
  }) => {
    await createLibrary(page, "shell-status");
    await importFixtureIntoCurrentLibrary(page);
    await mockSingleTextSearchResult(page, "Revenue 46 percent");

    await expect(page.getByTestId("search-results-column")).toBeVisible();
    await expect(page.getByTestId("detail-panel")).toBeVisible();

    await openUtilityDrawerSection(page, "status");
    await expect(page.getByTestId("utility-drawer-section-status")).toBeVisible();
    await expect(page.getByTestId("utility-drawer-runtime-app")).toBeVisible();
    await expect(page.getByTestId("utility-drawer-runtime-qdrant")).toBeVisible();
    await expect(page.getByTestId("utility-drawer-runtime-providers")).toContainText("连接");
    await expect(page.getByTestId("status-capsule")).toHaveCount(0);
    await expect(page.getByTestId("utilities-menu")).toHaveCount(0);
    await page.getByTestId("utility-drawer-open-diagnostics").click();

    await expect(page.getByTestId("settings-workspace")).toBeVisible();
    await expect(page.getByTestId("runtime-health-panel")).toBeVisible();
    await expect(page.getByTestId("context-rail")).toBeVisible();

    await openSearchWorkspace(page);
    await openUtilityDrawerSection(page, "source-prep");
    await expect(page.getByTestId("utility-drawer-section-source-prep")).toBeVisible();
    await page.getByTestId("utility-drawer-focus-search-prep").click();
    await expect(page.getByTestId("search-panel")).toBeVisible();
    await expect(page.getByTestId("source-root-form")).toBeVisible();

    await openUtilityDrawerSection(page, "maintenance");
    await page.getByTestId("utility-drawer-open-runtime-health").click();
    await expect(page.getByTestId("runtime-health-panel")).toBeVisible();
  });

  test("utilities and diagnostics expose rebuild and cleanup maintenance actions", async ({
    page,
  }) => {
    const rebuildRequests = [];
    const maintenanceRequests = [];

    await page.route("**/api/libraries/*/vector-space-diagnostics", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            vector_spaces: [
              {
                vector_space_id: "vs_active",
                lifecycle_state: "active",
                content_types: ["document"],
                provider_id: "local_sidecar",
                model_id: "athrael-soju/colqwen3.5-4.5B-v3",
                model_version: "main",
                vector_type: "multi_vector_late_interaction",
              },
              {
                vector_space_id: "vs_retired",
                lifecycle_state: "retired",
                content_types: [],
                retired_at_ms: Date.now() - 60_000,
              },
            ],
          },
        }),
      });
    });

    await page.route("**/api/libraries/*/rebuild", async (route) => {
      rebuildRequests.push(route.request().url());
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            accepted: [
              {
                source_root_id: "root_000001",
                root_path: "/tmp/demo-root",
                action: "rebuild",
              },
            ],
            rejected: [],
            job_handle: "job_rebuild_001",
            job: {
              job_id: "job_rebuild_001",
              library_id: "playwright-maintenance",
              kind: "rebuild",
              status: "completed",
              phase: "activated",
              progress: {
                completed: 1,
                total: 1,
                unit: "source_root",
              },
              cancelable: false,
              retryable: true,
              current_attempt: {
                attempt: 1,
                status: "completed",
                summary: "Rebuild completed.",
              },
            },
          },
        }),
      });
    });

    await page.route("**/api/libraries/*/maintenance", async (route) => {
      maintenanceRequests.push(route.request().postDataJSON());
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            action: "cleanup_retired_vector_spaces",
            accepted: [
              {
                target_kind: "vector_space",
                target_id: "vs_retired",
                message: "已加入退役执行空间清理队列。",
              },
            ],
            rejected: [],
            job_handle: "job_cleanup_001",
            job: {
              job_id: "job_cleanup_001",
              library_id: "playwright-maintenance",
              kind: "cleanup",
              status: "completed",
              phase: "cleaned",
              progress: {
                completed: 1,
                total: 1,
                unit: "vector_space",
              },
              cancelable: false,
              retryable: true,
              current_attempt: {
                attempt: 1,
                status: "completed",
                summary: "Cleanup completed.",
              },
            },
          },
        }),
      });
    });

    await createLibrary(page, "maintenance-actions");

    await openUtilityDrawerSection(page, "maintenance");
    await expect(page.getByTestId("utility-drawer-rebuild-library")).toBeVisible();
    await expect(page.getByTestId("utility-drawer-cleanup-retired-vector-spaces")).toBeEnabled();
    await page.getByTestId("utility-drawer-rebuild-library").click();
    await expect.poll(() => rebuildRequests.length).toBe(1);

    await openSettingsSection(page, "diagnostics");
    await expect(page.getByTestId("maintenance-actions-panel")).toContainText(
      "1 个退役执行空间可立即清理"
    );
    await expect(page.getByTestId("diagnostics-cleanup-retired-vector-spaces")).toBeEnabled();
    await page.getByTestId("diagnostics-cleanup-retired-vector-spaces").click();
    await expect
      .poll(() => maintenanceRequests.at(0)?.action ?? null)
      .toBe("cleanup_retired_vector_spaces");
  });

  test("job panel exposes a cancel action for cancelable tasks", async ({ page }) => {
    const libraryName = await createLibrary(page, "job-cancel");
    const libraryId = await currentLibraryId(page);
    const cancelRequests = [];
    let cancelRequested = false;

    const currentJobSnapshot = () => ({
      job_id: "job_cancel_001",
      library_id: libraryId,
      kind: "import",
      status: cancelRequested ? "canceled" : "running",
      phase: cancelRequested ? "canceled" : "encode",
      progress: {
        completed: cancelRequested ? 1 : 0,
        total: 1,
        unit: "item",
      },
      cancelable: !cancelRequested,
      retryable: false,
      current_attempt: {
        attempt: 1,
        status: cancelRequested ? "canceled" : "running",
        summary: cancelRequested
          ? "Canceled before the next safe boundary completed."
          : "Encoding 1 accepted path into vector-space embeddings.",
      },
    });

    await page.route("**/api/libraries", async (route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }

      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            libraries: [
              {
                id: libraryId,
                display_name: libraryName,
                lifecycle_state: "active",
                counts: {
                  accepted_items: 1,
                  pending_jobs: 1,
                },
                latest_job_id: "job_cancel_001",
              },
            ],
          },
        }),
      });
    });

    await page.route("**/api/jobs?library_id=*", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            jobs: [currentJobSnapshot()],
          },
        }),
      });
    });

    await page.route("**/api/jobs/job_cancel_001", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: currentJobSnapshot(),
        }),
      });
    });

    await page.route("**/api/jobs/job_cancel_001/cancel", async (route) => {
      cancelRequested = true;
      cancelRequests.push(route.request().url());
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            ...currentJobSnapshot(),
            status: "running",
            phase: "cancel_requested",
            current_attempt: {
              attempt: 1,
              status: "running",
              summary:
                "Cancellation requested during encode. The job will stop at the next safe boundary.",
            },
          },
        }),
      });
    });

    await page.reload();
    await expect(page.getByTestId("workspace-shell")).toBeVisible();

    await openUtilityDrawerSection(page, "jobs");
    await expect(page.getByTestId("job-list")).toBeVisible();
    await expect(page.getByTestId("job-cancel-button")).toBeVisible();
    await page.getByTestId("job-cancel-button").click();

    await expect.poll(() => cancelRequests.length).toBe(1);
    await expect(page.getByTestId("job-card").first()).toHaveAttribute("data-job-status", "canceled");
    await expect(page.getByTestId("job-cancel-button")).toHaveCount(0);
  });

  test("job panel exposes a retry action for retryable terminal tasks", async ({ page }) => {
    const libraryName = await createLibrary(page, "job-retry");
    const libraryId = await currentLibraryId(page);
    const retryRequests = [];
    let retryRequested = false;
    let retriedJobPolls = 0;

    const failedJobSnapshot = {
      job_id: "job_retry_001",
      library_id: libraryId,
      kind: "refresh",
      status: "failed",
      phase: "failed",
      progress: {
        completed: 0,
        total: 1,
        unit: "source_root",
      },
      cancelable: false,
      retryable: true,
      retried_from_job_id: null,
      current_attempt: {
        attempt: 1,
        status: "failed",
        summary: "refresh failed: runtime temporarily unavailable",
      },
    };

    const retriedJobSnapshot = () => ({
      job_id: "job_retry_002",
      library_id: libraryId,
      kind: "refresh",
      status: retriedJobPolls > 0 ? "completed" : "running",
      phase: retriedJobPolls > 0 ? "activated" : "encode",
      progress: {
        completed: retriedJobPolls > 0 ? 1 : 0,
        total: 1,
        unit: "source_root",
      },
      cancelable: retriedJobPolls === 0,
      retryable: true,
      retried_from_job_id: "job_retry_001",
      current_attempt: {
        attempt: 2,
        status: retriedJobPolls > 0 ? "completed" : "running",
        summary:
          retriedJobPolls > 0
            ? "refresh completed."
            : "Encoding 1 visual unit for refresh.",
      },
    });

    await page.route("**/api/libraries", async (route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }

      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            libraries: [
              {
                id: libraryId,
                display_name: libraryName,
                lifecycle_state: "active",
                counts: {
                  accepted_items: 1,
                  pending_jobs: 1,
                },
                latest_job_id: retryRequested ? "job_retry_002" : "job_retry_001",
              },
            ],
          },
        }),
      });
    });

    await page.route("**/api/jobs?library_id=*", async (route) => {
      const jobs = retryRequested
        ? [{ ...retriedJobSnapshot(), status: "completed", phase: "activated", cancelable: false }, failedJobSnapshot]
        : [failedJobSnapshot];
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            jobs,
          },
        }),
      });
    });

    await page.route("**/api/jobs/job_retry_002", async (route) => {
      const snapshot = retriedJobSnapshot();
      retriedJobPolls += 1;
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: snapshot,
        }),
      });
    });

    await page.route("**/api/jobs/job_retry_001/retry", async (route) => {
      retryRequested = true;
      retryRequests.push(route.request().url());
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            job_id: "job_retry_002",
            library_id: libraryId,
            kind: "refresh",
            status: "queued",
            phase: "intake",
            progress: {
              completed: 0,
              total: 1,
              unit: "source_root",
            },
            cancelable: true,
            retryable: true,
            retried_from_job_id: "job_retry_001",
            current_attempt: {
              attempt: 2,
              status: "queued",
              summary:
                "Retry attempt 2 for refresh after job_retry_001; queued across 1 source root(s) via manual trigger.",
            },
          },
        }),
      });
    });

    await page.reload();
    await expect(page.getByTestId("workspace-shell")).toBeVisible();

    await openUtilityDrawerSection(page, "jobs");
    await expect(page.getByTestId("job-list")).toBeVisible();

    const failedJobCard = page.locator('[data-job-id="job_retry_001"]');
    await expect(failedJobCard.getByTestId("job-retry-button")).toBeVisible();
    await failedJobCard.getByTestId("job-retry-button").click();

    await expect.poll(() => retryRequests.length).toBe(1);
    await expect(page.locator('[data-job-id="job_retry_002"]')).toHaveAttribute(
      "data-job-status",
      "completed"
    );
    await expect(page.locator('[data-job-id="job_retry_002"]')).toContainText("第 2 次尝试");
    await expect(page.locator('[data-job-id="job_retry_002"]')).toContainText(
      "重试自 job_retry_001"
    );
  });

  test("job panel exposes a resume action for replayable terminal tasks", async ({ page }) => {
    const libraryName = await createLibrary(page, "job-resume");
    const libraryId = await currentLibraryId(page);
    const resumeRequests = [];
    let resumeRequested = false;
    let resumedJobPolls = 0;

    const failedJobSnapshot = {
      job_id: "job_resume_001",
      library_id: libraryId,
      kind: "import",
      status: "canceled",
      phase: "canceled",
      progress: {
        completed: 0,
        total: 1,
        unit: "item",
      },
      cancelable: false,
      retryable: true,
      retried_from_job_id: null,
      current_attempt: {
        attempt: 1,
        status: "canceled",
        summary: "Import canceled before any vector-space activation.",
      },
    };

    const resumedJobSnapshot = () => ({
      job_id: "job_resume_001",
      library_id: libraryId,
      kind: "import",
      status: resumedJobPolls > 0 ? "completed" : "running",
      phase: resumedJobPolls > 0 ? "activated" : "encode",
      progress: {
        completed: resumedJobPolls > 0 ? 1 : 0,
        total: 1,
        unit: "item",
      },
      cancelable: resumedJobPolls === 0,
      retryable: true,
      retried_from_job_id: null,
      current_attempt: {
        attempt: 2,
        status: resumedJobPolls > 0 ? "completed" : "running",
        summary:
          resumedJobPolls > 0
            ? "Accepted 1 path(s); indexed 1 visual unit(s) across 1 vector space(s) and activated the resulting namespaces."
            : "Encoding batch 1/1 (1 visual unit(s)) for staged vector-space indexing.",
      },
    });

    await page.route("**/api/libraries", async (route) => {
      if (route.request().method() !== "GET") {
        await route.continue();
        return;
      }

      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            libraries: [
              {
                id: libraryId,
                display_name: libraryName,
                lifecycle_state: "active",
                counts: {
                  accepted_items: 1,
                  pending_jobs: 1,
                },
                latest_job_id: "job_resume_001",
              },
            ],
          },
        }),
      });
    });

    await page.route("**/api/jobs?library_id=*", async (route) => {
      const jobs = resumeRequested
        ? [{ ...resumedJobSnapshot(), status: "completed", phase: "activated", cancelable: false }]
        : [failedJobSnapshot];
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            jobs,
          },
        }),
      });
    });

    await page.route("**/api/jobs/job_resume_001", async (route) => {
      const snapshot = resumedJobSnapshot();
      resumedJobPolls += 1;
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: snapshot,
        }),
      });
    });

    await page.route("**/api/jobs/job_resume_001/resume", async (route) => {
      resumeRequested = true;
      resumeRequests.push(route.request().url());
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            job_id: "job_resume_001",
            library_id: libraryId,
            kind: "import",
            status: "queued",
            phase: "intake",
            progress: {
              completed: 0,
              total: 1,
              unit: "item",
            },
            cancelable: true,
            retryable: true,
            retried_from_job_id: null,
            current_attempt: {
              attempt: 2,
              status: "queued",
              summary:
                "Resume attempt 2 for import on existing job; accepted 1 path(s) and requeued them for vector-space indexing.",
            },
          },
        }),
      });
    });

    await page.reload();
    await expect(page.getByTestId("workspace-shell")).toBeVisible();

    await openUtilityDrawerSection(page, "jobs");
    await expect(page.getByTestId("job-list")).toBeVisible();

    const failedJobCard = page.locator('[data-job-id="job_resume_001"]');
    await expect(failedJobCard.getByTestId("job-resume-button")).toBeVisible();
    await failedJobCard.getByTestId("job-resume-button").click();

    await expect.poll(() => resumeRequests.length).toBe(1);
    await expect(page.locator('[data-job-id="job_resume_001"]')).toHaveAttribute(
      "data-job-status",
      "completed"
    );
    await expect(page.locator('[data-job-id="job_resume_001"]')).toContainText("第 2 次尝试");
  });

  test("switching between search and inventory preserves search drafts results and detail selection", async ({ page }) => {
    await createLibrary(page, "workspace-switch-preserve");
    await importFixtureIntoCurrentLibrary(page);
    await mockSingleTextSearchResult(
      page,
      "What is the percentage change in the net cash provided from operating activities?"
    );

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

  test("phone-sized search detail opens as a closable sheet", async ({ page }) => {
    const fixtures = createTempDocumentSearchFixtures();
    try {
      await page.setViewportSize({ width: 390, height: 844 });
      await createLibrary(page, "search-mobile-sheet");
      await page
        .getByTestId("import-paths-input")
        .fill(`${fixtures.imagePath}\n${fixtures.pdfPath}`);
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
    await expect(page.getByTestId("current-library-name")).toHaveText(libraryName);

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

      await page
        .getByTestId("import-paths-input")
        .fill(`${fixtures.imagePath}\n${fixtures.pdfPath}`);
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
}

export function registerSettingsScenarios() {
  test("settings workspace shows exact models and editable provider config", async ({ page }) => {
    const localModelId = "athrael-soju/colqwen3.5-4.5B-v3";

    await createLibrary(page, "provider-settings");
    await expect(page.getByTestId("provider-bridge-summary")).toContainText(localModelId);

    await openSettingsSection(page, "providers");

    await expect(page.getByTestId("settings-stage-title")).toHaveText("连接");
    await expect(page.getByTestId("settings-stage-summary")).toContainText("连接状态");
    await expect(page.getByTestId("settings-stage-metrics")).toContainText("已启用连接");
    await expect(page.getByTestId("provider-configs-panel")).toContainText("Local Sidecar");
    await expect(page.getByTestId("provider-configs-panel")).toContainText("DashScope");
    await expect(page.getByTestId("provider-runtime-summary-local_sidecar")).toContainText(
      localModelId
    );
    await expect(page.getByTestId("provider-runtime-summary-local_sidecar")).toContainText(
      "模型版本 main"
    );
    await expect(page.getByTestId("provider-runtime-summary-local_sidecar")).toContainText(
      "模型修订 main"
    );
    await expect(page.getByTestId("provider-configs-panel")).not.toContainText("qdrant");
    await expect(page.getByTestId("settings-workspace")).not.toContainText("Region");
    await expect(page.getByTestId("settings-workspace")).not.toContainText("Provider profiles");
    await page.getByTestId("provider-config-id").selectOption("local_sidecar");
    await expect(page.getByTestId("provider-editor-runtime-summary")).toContainText(localModelId);
    await expect(page.getByTestId("provider-editor-runtime-summary")).toContainText("模型版本 main");
    await expect(page.getByTestId("provider-editor-runtime-summary")).toContainText("模型修订 main");
    await page.getByTestId("provider-config-id").selectOption("dashscope");
    await page.getByTestId("provider-base-url").fill("https://dashscope.aliyuncs.com");
    await Promise.all([
      page.waitForResponse(
        (response) =>
          response.url().includes("/settings/providers/dashscope") &&
          response.request().method() === "PATCH" &&
          response.ok()
      ),
      page.getByTestId("provider-config-submit-button").click(),
    ]);
    await expect(page.getByTestId("provider-base-url")).toHaveValue(
      "https://dashscope.aliyuncs.com"
    );

    await page.getByTestId("settings-nav-library-overrides").click();
    await expect(page.getByTestId("settings-stage-title")).toHaveText("当前库覆盖");
    await expect(page.getByTestId("settings-stage-metrics")).toContainText("覆盖状态");
    await expect(page.getByTestId("resolved-content-models-panel")).toContainText(localModelId);
    await expect(page.getByTestId("resolved-content-models-panel")).toContainText(
      "全局内容类型"
    );
    await expect(page.getByTestId("resolved-content-models-panel")).not.toContainText("执行空间");
    await expect(page.getByTestId("resolved-content-models-panel")).not.toContainText(
      "vector_space_id"
    );

    await openSearchWorkspace(page);
    await expect(page.getByTestId("provider-bridge-summary")).toContainText(localModelId);
  });

  test("settings workspace tests only native embedding inputs and shows unsupported drafts", async ({
    page,
  }) => {
    const localModelId = "athrael-soju/colqwen3.5-4.5B-v3";
    const dashscopeModelId = "qwen3-vl-embedding";

    await page.route("**/api/settings/model-tests", async (route) => {
    const body = route.request().postDataBuffer()?.toString("latin1") ?? "";
    const providerMatch = body.match(/name="provider_id"\r\n\r\n([a-z_]+)/);
    const providerId = providerMatch?.[1] ?? "local_sidecar";
    const modalityMatch = body.match(/name="input_modality"\r\n\r\n([a-z]+)/);
    const modality = modalityMatch?.[1] ?? "text";
    const comparisonModalityMatch = body.match(
      /name="comparison_input_modality"\r\n\r\n([a-z]+)/
    );
    const comparisonModality = comparisonModalityMatch?.[1] ?? null;
    if (providerId === "local_sidecar") {
      expect(body).not.toContain('name="provider_base_url"');
    }
    const operationKindByModality = {
      text: "query_embedding",
      image: "image_query_embedding",
    };
    const vectorsByModality = {
      text: [[0.1, 0.2, 0.3], [0.4, 0.5, 0.6]],
      image: [[1, 2, 3]],
    };
    const similarityByPair = {
      "text:image": 0.876543,
    };

    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        data: {
          resolved_model: {
            binding_source: "settings_model_test",
            provider_id: "local_sidecar",
            provider_kind: "local_sidecar",
            model_id: localModelId,
            model_revision: "main",
            embedding_capabilities: {
              input_types: ["text", "image"],
              vector_types: ["multi_vector_late_interaction"],
              supports_mixed_inputs: false,
            },
            status: "available",
            message: `Validated settings model test via ${operationKindByModality[modality]}.`,
            last_probed_at: "2026-04-19T00:00:00Z",
          },
          input_modality: modality,
          operation_kind: operationKindByModality[modality],
          vector_shape: [
            vectorsByModality[modality].length,
            vectorsByModality[modality][0].length,
          ],
          vectors: vectorsByModality[modality],
          pooled_vector: vectorsByModality[modality][0],
          input_summary:
            modality === "text"
              ? { kind: "text", text_preview: "Revenue 46 percent", size_bytes: 18 }
              : {
                  kind: "file",
                  original_filename: `query-${modality}`,
                  content_type:
                    modality === "image"
                      ? "image/png"
                      : modality === "video"
                        ? "video/mp4"
                        : "application/pdf",
                  size_bytes: 1234,
                },
          comparison: comparisonModality
            ? {
                input_modality: comparisonModality,
                operation_kind: operationKindByModality[comparisonModality],
                vector_shape: [
                  vectorsByModality[comparisonModality].length,
                  vectorsByModality[comparisonModality][0].length,
                ],
                vectors: vectorsByModality[comparisonModality],
                pooled_vector: vectorsByModality[comparisonModality][0],
                input_summary: {
                  kind: "file",
                  original_filename: `query-${comparisonModality}`,
                  content_type: comparisonModality === "image" ? "image/png" : "application/octet-stream",
                  size_bytes: 4321,
                },
                similarity_to_primary: similarityByPair[`${modality}:${comparisonModality}`] ?? 0.5,
              }
            : null,
        },
      }),
    });
    });

    await createLibrary(page, "provider-settings-model-test");
    await openSettingsSection(page, "model-tests");

    const globalPanel = page.getByTestId("global-model-test-panel");
    await expect(globalPanel).toContainText(localModelId);
    await expect(page.getByTestId("global-model-test-support-message")).toContainText("文本、图片");
    await expect(page.getByTestId("global-model-capabilities")).toContainText("输入 text, image");
    await expect(page.getByTestId("global-model-capabilities")).toContainText("向量 multi_vector_late_interaction");
    await expect(page.locator('[data-testid="global-model-test-modality"] option')).toHaveCount(2);
    await expect(page.locator('[data-testid="global-model-test-modality"] option').nth(0)).toHaveText(
      "文本"
    );
    await expect(page.locator('[data-testid="global-model-test-modality"] option').nth(1)).toHaveText(
      "图片"
    );
    await expect(page.getByTestId("global-model-test-modality")).toHaveValue("text");

    await page.getByTestId("global-model-test-text").fill("Revenue 46 percent");
    await page.getByTestId("global-model-test-submit-button").click();
    await expect(page.getByTestId("global-model-test-shape")).toContainText("[2, 3]");
    await expect(page.getByTestId("global-model-test-vectors")).toContainText("0.1");

    await page.getByTestId("global-model-test-comparison-modality").selectOption("image");
    await page.getByTestId("global-model-test-comparison-file").setInputFiles(fixtureImagePath);
    await page.getByTestId("global-model-test-submit-button").click();
    await expect(page.getByTestId("global-model-test-comparison-shape")).toContainText("[1, 3]");
    await expect(page.getByTestId("global-model-test-comparison-vectors")).toContainText("1");
    await expect(page.getByTestId("global-model-test-similarity")).toContainText("0.876543");

    await page.getByTestId("global-model-test-modality").selectOption("image");
    await expect(page.getByTestId("global-model-test-file")).toBeVisible();
    await page.getByTestId("global-model-test-file").setInputFiles(fixtureImagePath);
    await page.getByTestId("global-model-test-submit-button").click();
    await expect(page.getByTestId("global-model-test-shape")).toContainText("[1, 3]");
    await expect(page.getByTestId("global-model-test-vectors")).toContainText("1");

    await openSettingsSection(page, "library-overrides");
    await page.getByTestId("library-override-mode-override").click();
    await page.getByTestId("library-content-type-provider-id").selectOption("dashscope");
    await page.getByTestId("library-content-type-model-id").selectOption(dashscopeModelId);
    await openSettingsSection(page, "model-tests");
    await expect(page.getByTestId("library-model-test-support-message")).toContainText("not executable");
    await expect(page.getByTestId("library-model-capabilities")).toContainText("输入 text, image");
    await expect(page.getByTestId("library-model-test-submit-button")).toBeDisabled();
  });
}

export function registerRuntimeHealthScenarios() {
  test("runtime health panel shows native capabilities execution inputs and vector-space diagnostics", async ({
    page,
  }) => {
    const localModelId = "athrael-soju/colqwen3.5-4.5B-v3";

    await createLibrary(page, "runtime-health");
    await importFixtureIntoCurrentLibrary(page);

    await openSettingsSection(page, "diagnostics");

    const runtimeHealthPanel = page.getByTestId("runtime-health-panel");
    await expect(runtimeHealthPanel).toContainText("Local Sidecar");
    await expect(runtimeHealthPanel).toContainText(localModelId);
    await expect(runtimeHealthPanel).toContainText("嵌入能力");
    await expect(runtimeHealthPanel).toContainText("输入 text, image");
    await expect(runtimeHealthPanel).toContainText("执行输入");
    await expect(runtimeHealthPanel).toContainText("text, image, document, video");
    await expect(runtimeHealthPanel).toContainText("运行时适配器");
    await expect(runtimeHealthPanel).toContainText("document_query_via_page_images");
    await expect(runtimeHealthPanel).toContainText("video_query_via_frame_images");

    const vectorSpacesPanel = page.getByTestId("vector-space-diagnostics-panel");
    await expect(vectorSpacesPanel).toContainText("active");
    await expect(vectorSpacesPanel).toContainText(localModelId);
    await expect(vectorSpacesPanel).toContainText("multi_vector_late_interaction");
  });
}

export function registerSearchTextControlScenarios() {
  test("search workspace supports shared filters and load more pagination", async ({ page }) => {
  const searchRequests = [];
  let searchCallCount = 0;
  const sourcePathPrefix = "/tmp/search-fixtures/set-a";

  const searchRoutePattern = "**/api/search/text";
  const detailRoutePattern = "**/api/libraries/*/visual-units/*";

  await page.route(searchRoutePattern, async (route) => {
    const payload = route.request().postDataJSON();
    searchRequests.push(payload);
    searchCallCount += 1;

    if (searchCallCount === 1) {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            results: Array.from({ length: 5 }, (_, index) =>
              createMockSearchResult(index, `${sourcePathPrefix}/report-${index + 1}.pdf`)
            ),
            next_cursor: "search:v1:5",
            debug: {
              backend: "qdrant",
            },
          },
        }),
      });
      return;
    }

    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        data: {
          results: Array.from({ length: 2 }, (_, index) =>
            createMockSearchResult(index + 5, `${sourcePathPrefix}/report-${index + 6}.pdf`)
          ),
          next_cursor: null,
          debug: {
            backend: "qdrant",
          },
        },
      }),
    });
  });

  await page.route(detailRoutePattern, async (route) => {
    const visualUnitId = route.request().url().split("/").pop();
    const index = Number(String(visualUnitId).replace("vu_mock_", "")) || 0;
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        data: {
          visual_unit: {
            visual_unit_id: `vu_mock_${index}`,
            source_id: `src_mock_${index}`,
            source_path: `${sourcePathPrefix}/report-${index + 1}.pdf`,
            source_type: "pdf",
            kind: "document_page",
            locator: {
              page: index + 1,
              page_label: String(index + 1),
            },
          },
          preview: {
            url: `http://127.0.0.1:54210/mock-preview/${index}.png`,
          },
          neighbor_context: null,
        },
      }),
    });
  });

  try {
    await createLibrary(page, "search-controls");
    await importFixtureIntoCurrentLibrary(page);
    searchRequests.length = 0;
    searchCallCount = 0;

    await page.getByTestId("search-text-input").fill("Revenue 46 percent");
    await openSearchAdvancedFilters(page);
    await page.getByTestId("search-filter-kind").selectOption("document_page");
    await page.getByTestId("search-filter-source-type").selectOption("pdf");
    await page.getByTestId("search-filter-path-prefix").fill(sourcePathPrefix);
    await page.getByTestId("search-submit-button").click();

    await expect(page.getByTestId("result-card")).toHaveCount(5);
    await expect(page.getByTestId("search-load-more-button")).toBeVisible();
    await expect(page.getByTestId("search-results-summary")).toContainText("命中 5 条结果");

    await page.getByTestId("search-load-more-button").click();
    await expect(page.getByTestId("result-card")).toHaveCount(7);
    await expect(page.getByTestId("search-load-more-button")).toHaveCount(0);

    expect(searchRequests).toHaveLength(2);
    expect(searchRequests[0]).toMatchObject({
      text: "Revenue 46 percent",
      top_k: 5,
      filters: {
        "visual_unit.kind": "document_page",
        source_type: "pdf",
        path_prefix: sourcePathPrefix,
      },
    });
    expect(searchRequests[1]).toMatchObject({
      text: "Revenue 46 percent",
      cursor: "search:v1:5",
      filters: {
        "visual_unit.kind": "document_page",
        source_type: "pdf",
        path_prefix: sourcePathPrefix,
      },
    });
  } finally {
    await page.unroute(searchRoutePattern);
    await page.unroute(detailRoutePattern);
  }
  });

  test("search workspace rejects invalid time range filters before sending the request", async ({ page }) => {
  let searchRequestCount = 0;
  let lastSearchRequestBody = null;
  const requestListener = (request) => {
    if (!request.url().includes("/api/search/text") || request.method() !== "POST") {
      return;
    }
    searchRequestCount += 1;
    lastSearchRequestBody = request.postDataJSON();
  };

  try {
    await createLibrary(page, "search-invalid-time-range");
    await importFixtureIntoCurrentLibrary(page);
    page.on("request", requestListener);
    searchRequestCount = 0;
    lastSearchRequestBody = null;

    await page.getByTestId("search-text-input").fill("Revenue 46 percent");
    await openSearchAdvancedFilters(page);
    await page.getByTestId("search-filter-time-range-start").fill("1000");
    await page.getByTestId("search-filter-time-range-end").fill("1000");
    await page.getByTestId("search-submit-button").click();

    await expect.poll(() => searchRequestCount).toBe(0);
    expect(lastSearchRequestBody).toBeNull();
    await expect(page.getByTestId("search-error-notice")).toBeVisible();
    await expect(page.getByTestId("search-error-code")).toHaveText("validation_failed");
    await expect(page.getByTestId("search-error-message")).toContainText("时间范围过滤器");
  } finally {
    page.off("request", requestListener);
  }
  });

  test("all-libraries text scope stays searchable even when the current library is still empty", async ({
    page,
  }) => {
    const readyLibraryName = await createLibrary(page, "search-scope-ready");
    const readyLibraryId = await currentLibraryId(page);
    await createLibrary(page, "search-scope-empty");
    const emptyLibraryId = await currentLibraryId(page);
    const librariesResponse = await page.request.get("/api/libraries");
    expect(librariesResponse.ok()).toBeTruthy();
    const librariesPayload = await librariesResponse.json();
    const scopedLibraries = (librariesPayload?.data?.libraries ?? [])
      .map((library) => {
        if (library.id === readyLibraryId) {
          return {
            ...library,
            counts: {
              ...(library.counts ?? {}),
              accepted_items: 6,
              pending_jobs: 0,
            },
          };
        }
        if (library.id === emptyLibraryId) {
          return {
            ...library,
            counts: {
              ...(library.counts ?? {}),
              accepted_items: 0,
              pending_jobs: 0,
            },
          };
        }
        return library;
      })
      .sort((left, right) => {
        if (left.id === emptyLibraryId) {
          return -1;
        }
        if (right.id === emptyLibraryId) {
          return 1;
        }
        return 0;
      });
    const searchRequests = [];
    const librariesRoutePattern = "**/api/libraries";
    const searchRoutePattern = "**/api/search/text";
    const detailRoutePattern = "**/api/libraries/*/visual-units/*";

    await page.route(librariesRoutePattern, async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            libraries: scopedLibraries,
          },
        }),
      });
    });
    await page.route(searchRoutePattern, async (route) => {
      const payload = route.request().postDataJSON();
      searchRequests.push(payload);
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            results: [
              {
                ...createMockSearchResult(0, "/tmp/search-fixtures/set-b/ready-report.pdf"),
                library_id: readyLibraryId,
              },
            ],
            next_cursor: null,
            debug: {
              backend: "qdrant",
            },
          },
        }),
      });
    });

    await page.route(detailRoutePattern, async (route) => {
      const pathname = new URL(route.request().url()).pathname;
      const parts = pathname.split("/");
      const libraryId = parts[parts.indexOf("libraries") + 1] ?? readyLibraryId;
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            visual_unit: {
              visual_unit_id: "vu_mock_0",
              source_id: "src_mock_0",
              source_path: "/tmp/search-fixtures/set-b/ready-report.pdf",
              source_type: "pdf",
              kind: "document_page",
              locator: {
                page: 1,
                page_label: "1",
              },
            },
            preview: {
              url: "http://127.0.0.1:54210/mock-preview/0.png",
            },
            neighbor_context: null,
            library_id: libraryId,
          },
        }),
      });
    });

    try {
      await page.reload();
      await expect(page.getByTestId("library-select")).toHaveValue(emptyLibraryId);
      await expect(page.getByTestId("search-submit-button")).toBeDisabled();

      await page.getByTestId("search-scope-all-libraries").click();

      await expect(page.getByTestId("search-submit-button")).toBeEnabled();
      await expect(page.getByTestId("search-state-strip")).toHaveCount(0);
      await expect(page.getByTestId("search-scope-bar")).not.toContainText("管理当前库");

      await page.getByTestId("search-text-input").fill("Revenue 46 percent");
      await page.getByTestId("search-submit-button").click();

      await expect(page.getByTestId("result-card")).toHaveCount(1);
      await expect(page.getByTestId("search-result-library-strip")).toContainText(readyLibraryName);
      await expect(page.getByTestId("search-result-card-library-pill")).toHaveCount(0);
      await expect(page.getByTestId("detail-library-context")).toContainText(readyLibraryName);
      await expect(page.getByTestId("detail-panel")).toContainText(readyLibraryName);
      await expect(page.getByTestId("search-results-column")).toContainText("来自 1 个库");

      await page.getByTestId("detail-open-hit-library-inventory").click();
      await expect(page.getByTestId("inventory-panel")).toBeVisible();
      await expect(page.getByTestId("library-select")).toHaveValue(readyLibraryId);

      expect(searchRequests).toHaveLength(1);
      expect(searchRequests[0]).toMatchObject({
        text: "Revenue 46 percent",
        search_scope: {
          kind: "all_libraries",
        },
      });
      expect(searchRequests[0].search_scope.library_id).toBeUndefined();
    } finally {
      await page.unroute(librariesRoutePattern);
      await page.unroute(searchRoutePattern);
      await page.unroute(detailRoutePattern);
    }
  });

  test("all-libraries result focus chips regroup the reading flow without leaving search", async ({
    page,
  }) => {
    const firstReadyLibraryName = await createLibrary(page, "cross-library-focus-a");
    const firstReadyLibraryId = await currentLibraryId(page);
    const secondReadyLibraryName = await createLibrary(page, "cross-library-focus-b");
    const secondReadyLibraryId = await currentLibraryId(page);
    await createLibrary(page, "cross-library-focus-empty");
    const emptyLibraryId = await currentLibraryId(page);
    const librariesResponse = await page.request.get("/api/libraries");
    expect(librariesResponse.ok()).toBeTruthy();
    const librariesPayload = await librariesResponse.json();
    const scopedLibraries = (librariesPayload?.data?.libraries ?? [])
      .map((library) => {
        if (library.id === firstReadyLibraryId) {
          return {
            ...library,
            counts: {
              ...(library.counts ?? {}),
              accepted_items: 6,
              pending_jobs: 0,
            },
          };
        }
        if (library.id === secondReadyLibraryId) {
          return {
            ...library,
            counts: {
              ...(library.counts ?? {}),
              accepted_items: 4,
              pending_jobs: 0,
            },
          };
        }
        if (library.id === emptyLibraryId) {
          return {
            ...library,
            counts: {
              ...(library.counts ?? {}),
              accepted_items: 0,
              pending_jobs: 0,
            },
          };
        }
        return library;
      })
      .sort((left, right) => {
        if (left.id === emptyLibraryId) {
          return -1;
        }
        if (right.id === emptyLibraryId) {
          return 1;
        }
        return 0;
      });
    const librariesRoutePattern = "**/api/libraries";
    const searchRoutePattern = "**/api/search/text";
    const detailRoutePattern = "**/api/libraries/*/visual-units/*";

    await page.route(librariesRoutePattern, async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            libraries: scopedLibraries,
          },
        }),
      });
    });
    await page.route(searchRoutePattern, async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            results: [
              {
                ...createMockSearchResult(0, `/tmp/search-fixtures/${secondReadyLibraryId}/hit-0.pdf`),
                library_id: secondReadyLibraryId,
              },
              {
                ...createMockSearchResult(1, `/tmp/search-fixtures/${firstReadyLibraryId}/hit-1.pdf`),
                library_id: firstReadyLibraryId,
              },
              {
                ...createMockSearchResult(2, `/tmp/search-fixtures/${firstReadyLibraryId}/hit-2.pdf`),
                library_id: firstReadyLibraryId,
              },
            ],
            next_cursor: null,
            debug: {
              backend: "qdrant",
            },
          },
        }),
      });
    });
    await page.route(detailRoutePattern, async (route) => {
      const pathname = new URL(route.request().url()).pathname;
      const parts = pathname.split("/");
      const libraryId = parts[parts.indexOf("libraries") + 1] ?? secondReadyLibraryId;
      const visualUnitId = parts.at(-1) ?? "vu_mock_0";
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            visual_unit: {
              visual_unit_id: visualUnitId,
              source_id: `src_${libraryId}`,
              source_path: `/tmp/search-fixtures/${libraryId}/focused-report.pdf`,
              source_type: "pdf",
              kind: "document_page",
              locator: {
                page: 1,
                page_label: "1",
              },
            },
            preview: {
              url: `http://127.0.0.1:54210/mock-preview/${libraryId}.png`,
            },
            neighbor_context: null,
            library_id: libraryId,
          },
        }),
      });
    });

    try {
      await page.reload();
      await page.getByTestId("search-scope-all-libraries").click();
      await page.getByTestId("search-text-input").fill("Revenue 46 percent");
      await page.getByTestId("search-submit-button").click();

      await expect(page.getByTestId("search-result-library-focus-all")).toHaveClass(/active/);
      await expect(page.getByTestId("result-card")).toHaveCount(3);
      await expect(page.getByTestId("search-result-library-group")).toHaveCount(2);
      await expect(page.getByTestId("search-result-library-group-heading").nth(0)).toHaveText(
        secondReadyLibraryName
      );
      await expect(page.getByTestId("search-result-library-group-heading").nth(1)).toHaveText(
        firstReadyLibraryName
      );
      await expect(page.getByTestId("search-result-library-group-summary").nth(0)).toContainText(
        "留在 Search 里先聚焦这一组"
      );
      await expect(page.getByTestId("search-result-card-library-pill")).toHaveCount(0);
      await expect(
        page.getByTestId(`search-result-library-group-focus-${firstReadyLibraryId}`)
      ).toBeVisible();
      await expect(
        page.getByTestId(`search-result-library-group-open-inventory-${secondReadyLibraryId}`)
      ).toBeVisible();
      await expect(page.getByTestId("detail-hit-library-name")).toHaveText(secondReadyLibraryName);

      await page.getByTestId(`search-result-library-group-focus-${firstReadyLibraryId}`).click();

      await expect(page.getByTestId("result-card")).toHaveCount(2);
      await expect(page.getByTestId("search-result-library-group")).toHaveCount(0);
      await expect(page.getByTestId("search-result-card-library-pill")).toHaveCount(0);
      await expect(page.getByTestId("search-results-summary")).toContainText(firstReadyLibraryName);
      await expect(page.getByTestId("detail-hit-library-name")).toHaveText(firstReadyLibraryName);
      await expect(page.getByTestId(`search-result-library-focus-${firstReadyLibraryId}`)).toHaveClass(
        /active/
      );
      await expect(page.getByTestId("search-result-library-focus-all")).not.toHaveClass(/active/);

      await page.getByTestId("search-result-library-focus-all").click();

      await expect(page.getByTestId("result-card")).toHaveCount(3);
      await expect(page.getByTestId("search-result-library-group")).toHaveCount(2);
      await expect(page.getByTestId("search-result-library-focus-all")).toHaveClass(/active/);
      await expect(page.getByTestId("search-results-summary")).toContainText("来自 2 个库");
    } finally {
      await page.unroute(librariesRoutePattern);
      await page.unroute(searchRoutePattern);
      await page.unroute(detailRoutePattern);
    }
  });

  test("cross-library result reuse switches back into the hit library before opening a library-bound query mode", async ({
    page,
  }) => {
    const readyLibraryName = await createLibrary(page, "cross-library-reuse-ready");
    const readyLibraryId = await currentLibraryId(page);
    await createLibrary(page, "cross-library-reuse-empty");
    const emptyLibraryId = await currentLibraryId(page);
    const librariesResponse = await page.request.get("/api/libraries");
    expect(librariesResponse.ok()).toBeTruthy();
    const librariesPayload = await librariesResponse.json();
    const scopedLibraries = (librariesPayload?.data?.libraries ?? [])
      .map((library) => {
        if (library.id === readyLibraryId) {
          return {
            ...library,
            counts: {
              ...(library.counts ?? {}),
              accepted_items: 6,
              pending_jobs: 0,
            },
          };
        }
        if (library.id === emptyLibraryId) {
          return {
            ...library,
            counts: {
              ...(library.counts ?? {}),
              accepted_items: 0,
              pending_jobs: 0,
            },
          };
        }
        return library;
      })
      .sort((left, right) => {
        if (left.id === emptyLibraryId) {
          return -1;
        }
        if (right.id === emptyLibraryId) {
          return 1;
        }
        return 0;
      });
    const librariesRoutePattern = "**/api/libraries";
    const searchRoutePattern = "**/api/search/text";
    const detailRoutePattern = "**/api/libraries/*/visual-units/*";

    await page.route(librariesRoutePattern, async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            libraries: scopedLibraries,
          },
        }),
      });
    });
    await page.route(searchRoutePattern, async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            results: [
              {
                ...createMockSearchResult(0, "/tmp/search-fixtures/set-b/ready-report.pdf"),
                library_id: readyLibraryId,
              },
            ],
            next_cursor: null,
            debug: {
              backend: "qdrant",
            },
          },
        }),
      });
    });

    await page.route(detailRoutePattern, async (route) => {
      const pathname = new URL(route.request().url()).pathname;
      const parts = pathname.split("/");
      const libraryId = parts[parts.indexOf("libraries") + 1] ?? readyLibraryId;
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            visual_unit: {
              visual_unit_id: "vu_mock_0",
              source_id: "src_mock_0",
              source_path: "/tmp/search-fixtures/set-b/ready-report.pdf",
              source_type: "pdf",
              kind: "document_page",
              locator: {
                page: 1,
                page_label: "1",
              },
            },
            preview: {
              url: "http://127.0.0.1:54210/mock-preview/0.png",
            },
            neighbor_context: null,
            library_id: libraryId,
          },
        }),
      });
    });

    try {
      await page.reload();
      await expect(page.getByTestId("library-select")).toHaveValue(emptyLibraryId);

      await page.getByTestId("search-scope-all-libraries").click();
      await page.getByTestId("search-text-input").fill("Revenue 46 percent");
      await page.getByTestId("search-submit-button").click();

      await expect(page.getByTestId("detail-library-context")).toContainText(readyLibraryName);
      await expect(page.getByTestId("detail-hit-library-summary")).toContainText(
        "复用结果时会自动切到命中库"
      );

      await page.getByTestId("result-card").first().getByTestId("use-as-query-document-button").click();

      await expect(page.getByTestId("library-select")).toHaveValue(readyLibraryId);
      await expect(page.getByTestId("search-scope-library")).toHaveClass(/active/);
      await expect(page.getByTestId("search-scope-all-libraries")).not.toHaveClass(/active/);
      await expect(page.getByTestId("search-mode-document")).toHaveClass(/active/);
      await expect(page.getByTestId("query-document-preview")).toBeVisible();
      await expect(page.getByTestId("query-document-range-card")).toContainText(
        "当前使用库内 document_page；查询范围固定为该页面对应的单页范围。"
      );
    } finally {
      await page.unroute(librariesRoutePattern);
      await page.unroute(searchRoutePattern);
      await page.unroute(detailRoutePattern);
    }
  });

  test("search failure with content-type details points directly to current library overrides", async ({
    page,
  }) => {
    const searchRoutePattern = "**/search/text";

    await createLibrary(page, "search-config-failure");
    await importFixtureIntoCurrentLibrary(page);

    await page.route(searchRoutePattern, async (route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }

      await route.fulfill({
        status: 503,
        json: {
          error: {
            code: "content_types_unavailable",
            message: "部分内容类型当前未完成配置。",
            details: {
              content_types: [
                {
                  content_type: "image",
                  status: "runtime_unavailable",
                  job: {
                    job_id: "job_config_wait",
                    phase: "resolve_models",
                  },
                },
              ],
            },
          },
        },
      });
    });

    try {
      await page.getByTestId("search-text-input").fill("Revenue 46 percent");
      await page.getByTestId("search-submit-button").click();

      await expect(page.getByTestId("search-error-notice")).toBeVisible();
      await expect(page.getByTestId("search-error-details")).toContainText("图片");
      await expect(page.getByTestId("search-error-open-library-overrides")).toBeVisible();
      await page.getByTestId("search-error-open-library-overrides").click();

      await expect(page.getByTestId("settings-workspace")).toBeVisible();
      await expect(page.getByTestId("settings-stage-title")).toHaveText("当前库覆盖");
      await expect(page.getByTestId("library-content-types-panel")).toBeVisible();
    } finally {
      await page.unroute(searchRoutePattern);
    }
  });

  test("empty search results stay in the stage and read as a miss instead of a prep failure", async ({
    page,
  }) => {
    const searchRoutePattern = "**/search/text";

    await createLibrary(page, "search-empty-miss");
    await importFixtureIntoCurrentLibrary(page);

    await page.route(searchRoutePattern, async (route) => {
      if (route.request().method() !== "POST") {
        await route.continue();
        return;
      }

      await route.fulfill({
        status: 200,
        json: {
          data: {
            results: [],
            next_cursor: null,
            unsupported_content_types: [],
          },
        },
      });
    });

    try {
      await page.getByTestId("search-text-input").fill("no matching snippet");
      await page.getByTestId("search-submit-button").click();

      await expect(page.getByTestId("search-empty-notice")).toBeVisible();
      await expect(page.getByTestId("search-empty-notice")).toContainText(
        "当前库可搜索，但本次没有返回结果"
      );
      await expect(page.getByTestId("search-results-column")).toHaveCount(0);
    } finally {
      await page.unroute(searchRoutePattern);
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
}

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
  const pdfPath = createTempPdfFixture();
  try {
    await createLibrary(page, "document-page-library-object");

    await page.getByTestId("import-paths-input").fill(`${fixtureImagePath}\n${pdfPath}`);
    await page.getByTestId("import-submit-button").click();
    await waitForFirstJobCompleted(page);

    await page.getByTestId("search-mode-image").click();
    await page.getByTestId("query-image-input").setInputFiles(fixtureImagePath);
    await expect(page.getByTestId("query-image-preview")).toBeVisible();
    await mockImageSearchResults(page, [
      {
        visual_unit_id: "vu_image_document_page_mock_0",
        source_id: "src_image_document_page_mock_0",
        preview: {
          url: "http://127.0.0.1:54210/mock-preview/document-page-image-0.png",
        },
        source_path: pdfPath,
        source_type: "pdf",
        kind: "document_page",
        locator: {
          page: 1,
          page_label: "1",
        },
        cursor: "search:v1:image-document-page:1",
        score: 100,
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
    fs.rmSync(pdfPath, { force: true });
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

export function registerSearchVideoScenarios() {
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

export function registerSearchDocumentScenarios() {
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

export function registerSourceManagementScenarios() {
  test("inventory workspace explains search readiness from source-root health", async ({ page }) => {
    const fixtures = createTempSourceManagementFixtures();
    try {
      await createLibrary(page, "inventory-health");
      const libraryId = await currentLibraryId(page);

      await openInventoryWorkspace(page);
      await expect(page.getByTestId("inventory-library-readiness")).toContainText(
        "尚未接入来源根"
      );
      await expect(page.getByTestId("inventory-library-root-strip")).toContainText("还没有来源根");

      await openSourcePreparationPanel(page);
      await page.getByTestId("source-root-path-input").fill(fixtures.tempDir);
      await page.getByTestId("source-root-submit-button").click();

      await openInventoryWorkspace(page);
      await expect(page.getByTestId("inventory-library-readiness")).toContainText("等待内容");
      const rootHealthCard = page
        .getByTestId("inventory-library-root-card")
        .filter({ hasText: fixtures.tempDir });
      await expect(rootHealthCard).toContainText("监视中");

      const previousJobId = await latestJobId(page, libraryId);
      await openSourcePreparationPanel(page);
      await sourceRootCard(page, fixtures.tempDir).locator("[data-source-root-refresh-id]").click();
      await waitForNewLatestJobCompleted(page, libraryId, previousJobId);

      await openInventoryWorkspace(page);
      await expect(page.getByTestId("inventory-library-readiness")).toContainText("可搜索");
      await expect(page.getByTestId("inventory-library-metrics")).toContainText("启用来源根");
      await expect(rootHealthCard).toContainText("监视中");
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("source preparation stays open after the first refresh makes the library searchable", async ({
    page,
  }) => {
    const fixtures = createTempSourceManagementFixtures();
    try {
      await createLibrary(page, "source-prep-persistence");
      const libraryId = await currentLibraryId(page);

      await openSourcePreparationPanel(page);
      await page.getByTestId("source-root-path-input").fill(fixtures.tempDir);
      await page.getByTestId("source-root-submit-button").click();

      const rootCard = sourceRootCard(page, fixtures.tempDir);
      await expect(rootCard).toBeVisible();

      const previousJobId = await latestJobId(page, libraryId);
      await rootCard.locator("[data-source-root-refresh-id]").click();
      await waitForNewLatestJobCompleted(page, libraryId, previousJobId);

      await expect(page.getByTestId("source-root-form")).toBeVisible();
      await expect(rootCard).toContainText("最近动作：refresh");
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });

  test("source management can create edit toggle refresh rescan and filter inventory", async ({ page }) => {
  const fixtures = createTempSourceManagementFixtures();
  try {
    await createLibrary(page, "source-management");
    const libraryId = await currentLibraryId(page);

    await openSourcePreparationPanel(page);
    await page.getByTestId("source-root-path-input").fill(fixtures.tempDir);
    await page.getByTestId("source-root-submit-button").click();

    const rootCard = sourceRootCard(page, fixtures.tempDir);
    await expect(rootCard).toBeVisible();
    await expect(rootCard).toContainText("watching");
    await expect(page.getByTestId("library-refresh-button")).toBeEnabled();
    await expect(page.getByTestId("library-rescan-button")).toBeEnabled();

    let previousJobId = await latestJobId(page, libraryId);
    await rootCard.locator("[data-source-root-refresh-id]").click();
    await waitForNewLatestJobCompleted(page, libraryId, previousJobId);
    await expect(rootCard).toContainText("最近动作：refresh");

    await openInventoryWorkspace(page);
    await expect(page.getByTestId("inventory-summary")).toBeVisible();
    await expect(page.getByTestId("inventory-action-focus-source-prep")).toBeVisible();
    await expect(page.getByTestId("inventory-action-refresh-library")).toBeVisible();
    await expect(page.getByTestId("inventory-filter-pills")).toContainText("当前显示全部来源");

    const imageCard = librarySourceCard(page, "chart.png");
    const pdfCard = librarySourceCard(page, "report.pdf");
    await expect(page.getByTestId("library-source-card")).toHaveCount(2);
    await expect(imageCard).toContainText("1 个对象");
    await expect(pdfCard).toContainText("2 个对象");
    await imageCard.locator("button").click();
    await expect(page.getByTestId("inventory-detail-preview")).toBeVisible();
    await expect(page.getByTestId("inventory-preview-link")).toBeVisible();
    await page.getByTestId("inventory-use-as-query-image-button").click();
    await expect(page.getByTestId("search-panel")).toBeVisible();
    await expect(page.getByTestId("query-image-preview")).toBeVisible();
    await openInventoryWorkspace(page);

    await openSourcePreparationPanel(page);
    previousJobId = await latestJobId(page, libraryId);
    await rootCard.locator("[data-source-root-rescan-id]").click();
    await waitForNewLatestJobCompleted(page, libraryId, previousJobId);
    await expect(rootCard).toContainText("最近动作：rescan");

    await openInventoryWorkspace(page);
    await page.getByTestId("source-filter-type").selectOption("pdf");
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(pdfCard).toBeVisible();

    await page.getByTestId("source-filter-root").selectOption({ label: fixtures.tempDir });
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(pdfCard).toBeVisible();

    await page.getByTestId("source-filter-type").selectOption("");
    await expect(page.getByTestId("library-source-card")).toHaveCount(2);

    await openSourcePreparationPanel(page);
    await rootCard.locator("[data-source-root-edit-id]").click();
    await page.getByTestId("source-root-exclude-globs-input").fill("chart.png");
    await page.getByTestId("source-root-submit-button").click();
    await expect(rootCard).toContainText("排除规则 1");

    previousJobId = await latestJobId(page, libraryId);
    await page.getByTestId("library-refresh-button").click();
    await waitForNewLatestJobCompleted(page, libraryId, previousJobId);
    await expect(rootCard).toContainText("最近动作：refresh");

    await openInventoryWorkspace(page);
    await page.getByTestId("source-filter-status").selectOption("active");
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(pdfCard).toContainText("正常");

    await page.getByTestId("source-filter-status").selectOption("out_of_scope");
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(imageCard).toContainText("rule_excluded");

    await page.getByTestId("source-filter-status").selectOption("");
    await openSourcePreparationPanel(page);
    await rootCard.locator("[data-source-root-toggle-id]").click();
    await expect(rootCard).toContainText("disabled");
    await expect(rootCard.locator("[data-source-root-refresh-id]")).toBeDisabled();

    await openInventoryWorkspace(page);
    await page.getByTestId("source-filter-status").selectOption("out_of_scope");
    await expect(page.getByTestId("library-source-card")).toHaveCount(2);
    await expect(imageCard).toContainText("source_root_disabled");
    await expect(pdfCard).toContainText("source_root_disabled");

    await page.getByTestId("source-filter-status").selectOption("");
    await openSourcePreparationPanel(page);
    await rootCard.locator("[data-source-root-toggle-id]").click();
    await expect(rootCard).toContainText("watching");
    await expect(rootCard.locator("[data-source-root-refresh-id]")).toBeEnabled();

    previousJobId = await latestJobId(page, libraryId);
    await page.getByTestId("library-rescan-button").click();
    await waitForNewLatestJobCompleted(page, libraryId, previousJobId);
    await expect(rootCard).toContainText("最近动作：rescan");

    await openInventoryWorkspace(page);
    await page.getByTestId("source-filter-status").selectOption("active");
    await expect(page.getByTestId("library-source-card")).toHaveCount(1);
    await expect(pdfCard).toContainText("正常");

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
    const libraryId = await currentLibraryId(page);

    await page.getByTestId("source-root-path-input").fill(fixtures.tempDir);
    await page.getByTestId("source-root-submit-button").click();

    const rootCard = sourceRootCard(page, fixtures.tempDir);
    await expect(rootCard).toBeVisible();

    const previousJobId = await latestJobId(page, libraryId);
    await rootCard.locator("[data-source-root-refresh-id]").click();
    await waitForNewLatestJobCompleted(page, libraryId, previousJobId);

    await openInventoryWorkspace(page);
    await expect(page.getByTestId("inventory-summary")).toBeVisible();
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
