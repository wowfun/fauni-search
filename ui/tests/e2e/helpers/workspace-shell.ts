import { expect, test } from "@playwright/test";
import {
  createLibrary,
  expectSelectionControlContrast,
  importFixtureIntoCurrentLibrary,
  mockSingleTextSearchResult,
  openInventoryWorkspace,
  openSearchWorkspace,
} from "./fixtures";

export function registerWorkspaceShellScenarios() {
  test("default workspace keeps search first and lets the empty stage own the desktop layout", async ({
    page,
  }) => {
    await createLibrary(page, "workspace-default");

    await expect(page.getByTestId("workspace-tab-search")).toBeVisible();
    await expect(page.getByTestId("workspace-tab-inventory")).toBeVisible();
    await expect(page.getByTestId("search-panel")).toBeVisible();
    await expect(page.getByTestId("search-inline-outcome")).toHaveCount(0);
    await expect(page.getByTestId("search-results-column")).toHaveCount(0);
    await expect(page.getByTestId("detail-panel")).toHaveCount(0);
    await expect(page.getByTestId("inventory-panel")).toHaveCount(0);
    await expect(page.getByTestId("workspace-tab-tools")).toHaveCount(0);
    await expect(page.getByTestId("utility-drawer")).toHaveCount(0);
    await expect(page.getByTestId("status-capsule-button")).toBeVisible();
    await expect(page.getByTestId("search-readiness-open-inventory")).toBeVisible();
    await page.evaluate(() => {
      const probe = document.createElement("button");
      probe.type = "button";
      probe.dataset.testid = "ready-status-pill-probe";
      probe.className = "ui-tag ui-tag-ready utility-trigger-pill";
      probe.textContent = "Ready";
      document.body.appendChild(probe);
    });
    await expect(page.getByTestId("ready-status-pill-probe")).toHaveCSS("color", "rgb(27, 111, 75)");
    await page.evaluate(() => {
      document.querySelector('[data-testid="ready-status-pill-probe"]')?.remove();
    });
    await expectSelectionControlContrast(
      page.getByTestId("workspace-tab-search"),
      page.getByTestId("workspace-tab-inventory")
    );
  });

  test("switching between search and inventory preserves search drafts results and detail selection", async ({
    page,
  }) => {
    await createLibrary(page, "workspace-switch-preserve");
    await importFixtureIntoCurrentLibrary(page);
    await mockSingleTextSearchResult(
      page,
      "What is the percentage change in the net cash provided from operating activities?"
    );

    const firstResult = page.getByTestId("result-card").first();
    await expect(firstResult).toBeVisible({ timeout: 2 * 60 * 1000 });
    const assetId = await firstResult.getAttribute("data-asset-id");
    await firstResult.locator(".result-select").click();
    await expect(page.getByTestId("asset-detail")).toBeVisible();
    await expect(page.getByTestId("search-text-input")).toHaveValue(
      "What is the percentage change in the net cash provided from operating activities?"
    );

    await openInventoryWorkspace(page);
    await expect(page.getByTestId("workspace-library-metrics")).toBeVisible();
    await expect(page.getByTestId("library-source-card").first()).toBeVisible();

    await openSearchWorkspace(page);
    await expect(page.getByTestId("search-text-input")).toHaveValue(
      "What is the percentage change in the net cash provided from operating activities?"
    );
    await expect(page.getByTestId("asset-detail")).toBeVisible();
    await expect(
      page.locator(`[data-testid="result-card"][data-asset-id="${assetId}"]`)
    ).toHaveClass(/active/);
  });
}
