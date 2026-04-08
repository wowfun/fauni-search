import { expect, test } from "@playwright/test";

test("demo import and search closes the current UI happy path", async ({ page }) => {
  const libraryName = `playwright-smoke-${Date.now()}`;

  await page.goto("/");

  await expect(page.getByTestId("workspace-shell")).toBeVisible();

  await page.getByTestId("library-name-input").fill(libraryName);
  await page.getByTestId("create-library-button").click();

  await expect(page.getByTestId("current-library-name")).toHaveText(libraryName);
  await expect(page.getByTestId("run-demo-button")).toBeEnabled();

  await page.getByTestId("run-demo-button").click();

  const firstJob = page.getByTestId("job-card").first();
  await expect(firstJob).toBeVisible({ timeout: 30_000 });
  await expect
    .poll(async () => firstJob.getAttribute("data-job-status"), {
      timeout: 10 * 60 * 1000,
      intervals: [1_000, 2_000, 5_000],
    })
    .toBe("completed");

  const firstResult = page.getByTestId("result-card").first();
  await expect(firstResult).toBeVisible({ timeout: 2 * 60 * 1000 });
  await expect(firstResult.getByTestId("result-score")).toBeVisible();

  await expect(page.getByTestId("detail-panel")).toBeVisible();
  await expect(page.getByTestId("visual-unit-detail")).toBeVisible();
  await expect(page.getByTestId("visual-preview")).toBeVisible();
  await expect(page.getByTestId("preview-link")).toBeVisible();
});
