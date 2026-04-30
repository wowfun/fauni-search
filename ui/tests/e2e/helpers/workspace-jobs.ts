import { expect, test } from "@playwright/test";
import { createLibrary, currentLibraryId, openDiagnosticsJobs } from "./fixtures";

export function registerWorkspaceJobScenarios() {
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

    await openDiagnosticsJobs(page);
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
            : "Encoding 1 assets for refresh.",
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

    await openDiagnosticsJobs(page);
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
            ? "Accepted 1 path(s); indexed 1 assets(s) across 1 vector space(s) and activated the resulting namespaces."
            : "Encoding batch 1/1 (1 assets(s)) for staged vector-space indexing.",
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

    await openDiagnosticsJobs(page);
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
}
