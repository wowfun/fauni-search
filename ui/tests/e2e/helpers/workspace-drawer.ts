import { expect, test } from "@playwright/test";
import {
  createLibrary,
  importFixtureIntoCurrentLibrary,
  mockSingleTextSearchResult,
  openInventoryWorkspace,
} from "./fixtures";

export function registerWorkspaceDrawerScenarios() {
  test("shell removes the tools drawer and routes the status capsule directly to diagnostics", async ({
    page,
  }) => {
    await createLibrary(page, "shell-diagnostics");
    await importFixtureIntoCurrentLibrary(page);
    await mockSingleTextSearchResult(page, "Revenue 46 percent");

    await expect(page.getByTestId("search-results-column")).toBeVisible();
    await expect(page.getByTestId("detail-panel")).toBeVisible();
    await expect(page.getByTestId("workspace-switch").getByRole("button")).toHaveCount(3);
    await expect(page.getByTestId("workspace-tab-tools")).toHaveCount(0);
    await expect(page.getByTestId("utility-drawer")).toHaveCount(0);

    await page.getByTestId("status-capsule-button").click();

    await expect(page.getByTestId("settings-workspace")).toBeVisible();
    await expect(page.getByTestId("settings-stage-title")).toHaveText("诊断");
    await expect(page.getByTestId("runtime-health-panel")).toBeVisible();
    await expect(page.getByTestId("settings-diagnostics-jobs-disclosure")).toBeVisible();
    await expect(page.getByTestId("settings-diagnostics-jobs-disclosure")).not.toHaveAttribute(
      "open",
      ""
    );
    await expect(page.getByTestId("utility-drawer")).toHaveCount(0);
    await expect(page.getByTestId("settings-open-maintenance-tools")).toHaveCount(0);
  });

  test("library maintenance actions live only in inventory and stay folded by default", async ({
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

    await page.getByTestId("workspace-tab-settings").click();
    await page.getByTestId("settings-nav-diagnostics").click();
    await expect(page.getByTestId("maintenance-actions-panel")).toHaveCount(0);
    await expect(page.getByTestId("settings-open-maintenance-tools")).toHaveCount(0);

    await openInventoryWorkspace(page);
    await expect(page.getByTestId("inventory-action-refresh-library")).toBeVisible();
    await expect(page.getByTestId("inventory-action-rescan-library")).toBeVisible();
    await expect(page.getByTestId("inventory-library-maintenance-panel")).toHaveCount(0);

    await page.getByTestId("inventory-action-library-maintenance").click();
    await expect(page.getByTestId("inventory-library-maintenance-panel")).toBeVisible();
    await expect(page.getByTestId("inventory-library-maintenance-summary")).toContainText(
      "退役执行空间 1"
    );
    await expect(page.getByTestId("inventory-library-maintenance-rebuild")).toBeVisible();
    await expect(page.getByTestId("inventory-library-maintenance-cleanup")).toBeEnabled();

    await page.getByTestId("inventory-library-maintenance-rebuild").click();
    await expect.poll(() => rebuildRequests.length).toBe(1);

    await page.getByTestId("inventory-library-maintenance-cleanup").click();
    await expect
      .poll(() => maintenanceRequests.at(0)?.action ?? null)
      .toBe("cleanup_retired_vector_spaces");
  });
}
