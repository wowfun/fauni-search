import { expect, test } from "@playwright/test";
import fs from "node:fs";
import {
  createLibrary,
  createMockMatchedUnits,
  createMockSearchResult,
  createTempSourceManagementFixtures,
  currentLibraryId,
  expectSelectionControlContrast,
  expectSearchRequiresContent,
  fileSourceUri,
  importFixtureIntoCurrentLibrary,
  mockSingleTextSearchResult,
  openInventoryImportPanel,
  openSearchAdvancedFilters,
  openSourcePreparationPanel,
  workspacePollWaitMs,
} from "./fixtures";

async function mockHealthyRuntimeStatus(page) {
  await page.route("**/api/runtime/status", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        data: {
          app: {
            component_id: "app",
            display_name: "App",
            status: "available",
            message: "ready",
            last_checked_at: "2026-04-26T00:00:00Z",
          },
          qdrant: {
            component_id: "qdrant",
            display_name: "Qdrant",
            status: "available",
            message: "ready",
            last_checked_at: "2026-04-26T00:00:00Z",
          },
          providers: [],
        },
      }),
    });
  });
}

async function mockPartialRuntimeStatus(page) {
  await page.route("**/api/runtime/status", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        data: {
          app: {
            component_id: "app",
            display_name: "App",
            status: "available",
            message: "ready",
            last_checked_at: "2026-04-26T00:00:00Z",
          },
          qdrant: {
            component_id: "qdrant",
            display_name: "Qdrant",
            status: "available",
            message: "ready",
            last_checked_at: "2026-04-26T00:00:00Z",
          },
          providers: [
            {
              provider_id: "dashscope",
              display_name: "DashScope",
              provider_kind: "remote_http",
              enabled: true,
              status: "not_supported",
              message: "not executable in this slice",
              last_probed_at: "2026-04-26T00:00:00Z",
              execution_input_types: [],
              runtime_adapters: [],
            },
          ],
        },
      }),
    });
  });
}

export function registerSearchTextScenarios() {
  test("search task next step opens settings diagnostics and expands jobs", async ({ page }) => {
    const libraryName = await createLibrary(page, "search-jobs-deeplink");
    const libraryId = await currentLibraryId(page);
    await mockHealthyRuntimeStatus(page);

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
                  accepted_items: 0,
                  pending_jobs: 1,
                },
                latest_job_id: "job_search_jobs_001",
              },
            ],
          },
        }),
      });
    });

    await page.route("**/api/libraries/*/source-roots", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            source_roots: [
              {
                source_root_id: "root_000001",
                root_path: "/tmp/search-root",
                enabled: true,
                watch_state: {
                  status: "watching",
                  reason: null,
                },
                rules: {
                  include_globs: [],
                  exclude_globs: [],
                  include_extensions: [],
                },
                last_action: {
                  action: "refresh",
                  summary: "Refresh queued.",
                },
              },
            ],
          },
        }),
      });
    });

    await page.route("**/api/jobs**", async (route) => {
      const url = new URL(route.request().url());
      const currentLibraryJob = {
        job_id: "job_search_jobs_001",
        library_id: libraryId,
        kind: "refresh",
        status: "running",
        phase: "encode",
        progress: {
          completed: 0,
          total: 1,
          unit: "source_root",
        },
        cancelable: true,
        retryable: true,
        current_attempt: {
          attempt: 1,
          status: "running",
          summary: "Encoding 1 source root.",
        },
      };
      const otherLibraryJob = {
        ...currentLibraryJob,
        job_id: "job_search_jobs_other_001",
        library_id: "other-library",
        progress: {
          completed: 1,
          total: 3,
          unit: "source_root",
        },
      };
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            jobs: url.searchParams.has("library_id")
              ? [currentLibraryJob]
              : [currentLibraryJob, otherLibraryJob],
          },
        }),
      });
    });

    await page.reload();
    await expect(page.getByTestId("status-capsule-button")).toContainText("准备中 · 2");
    await expect(page.getByTestId("status-capsule-progress")).toHaveAttribute(
      "data-progress-kind",
      "determinate"
    );
    await expect(page.getByTestId("status-capsule-progress-label")).toHaveText("1/4 source_root");
    await expect(page.locator(".status-stack .ui-notice-success")).toHaveCount(0);
    await expect(page.getByTestId("search-next-step-dock")).toHaveCount(0);
    await expect(page.getByTestId("import-form")).toHaveCount(0);
    await expect(page.getByTestId("search-readiness-action")).toContainText("后台任务完成后即可搜索");
    await page.getByTestId("search-readiness-open-jobs").click();

    await expect(page.getByTestId("settings-workspace")).toBeVisible();
    await expect(page.getByTestId("settings-stage-title")).toHaveText("诊断");
    await expect(page.getByTestId("settings-diagnostics-jobs-disclosure")).toHaveAttribute(
      "open",
      ""
    );
    await expect(page.getByTestId("job-list")).toBeVisible();
    await expect(page.getByTestId("job-card")).toHaveCount(1);
    await expect(page.getByTestId("job-progress")).toHaveAttribute(
      "data-progress-kind",
      "determinate"
    );
  });

  test("status capsule handles global jobs without computable totals", async ({ page }) => {
    await createLibrary(page, "job-progress-indeterminate");
    await mockHealthyRuntimeStatus(page);

    await page.route("**/api/jobs**", async (route) => {
      const url = new URL(route.request().url());
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            jobs: url.searchParams.has("library_id")
              ? []
              : [
                  {
                    job_id: "job_waiting_001",
                    library_id: "another-library",
                    kind: "import",
                    status: "queued",
                    phase: "intake",
                    progress: {
                      completed: 0,
                      total: 0,
                      unit: "item",
                    },
                    cancelable: true,
                    retryable: false,
                    current_attempt: {
                      attempt: 1,
                      status: "queued",
                      summary: "Waiting for execution.",
                    },
                  },
                ],
          },
        }),
      });
    });

    await page.reload();
    await expect(page.getByTestId("status-capsule-button")).toContainText("准备中 · 1");
    await expect(page.getByTestId("status-capsule-progress")).toHaveAttribute(
      "data-progress-kind",
      "indeterminate"
    );
    await expect(page.getByTestId("status-capsule-progress-label")).toHaveText("等待开始");
    await expect(page.getByTestId("status-capsule-progress-label")).not.toContainText("NaN");
  });

  test("video result thumbnails keep their DOM node across workspace polling", async ({ page }) => {
    const libraryId = "video-result-preview-stability";
    const libraryName = "Video result preview stability";

    await mockHealthyRuntimeStatus(page);
    await page.route("**/api/settings/providers", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ data: { providers: [] } }),
      });
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
                  pending_jobs: 0,
                },
                latest_job_id: null,
              },
            ],
          },
        }),
      });
    });
    await page.route(`**/api/libraries/${libraryId}/content-types`, async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ data: { content_types: { content_types: {} } } }),
      });
    });
    await page.route(`**/api/libraries/${libraryId}/resolved-content-models`, async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ data: { content_types: {} } }),
      });
    });
    await page.route(`**/api/libraries/${libraryId}/vector-space-diagnostics`, async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ data: { vector_spaces: [] } }),
      });
    });
    await page.route(`**/api/libraries/${libraryId}/video-sources`, async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ data: { sources: [] } }),
      });
    });
    await page.route("**/api/libraries/*/source-roots", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            source_roots: [
              {
                source_root_id: "root_video_preview",
                root_path: "/tmp/video-preview",
                enabled: true,
                status: "ready",
                watch_state: "watching",
                rules: {
                  include_globs: [],
                  exclude_globs: [],
                  include_extensions: [],
                },
                last_action: null,
              },
            ],
          },
        }),
      });
    });
    await page.route("**/api/jobs**", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ data: { jobs: [] } }),
      });
    });
    await page.route("**/api/search/text", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            results: [
              {
                ...createMockSearchResult(0, "/tmp/video-preview/clip.mp4"),
                library_id: libraryId,
                preview: {
                  url: "http://127.0.0.1:54210/mock-preview/clip.mp4",
                },
                source_type: "video",
                asset_type: "video_segment",
                locator: {
                  start_ms: 42_000,
                  end_ms: 50_000,
                },
              },
            ],
            next_cursor: null,
            unsupported_content_types: [],
          },
        }),
      });
    });
    await page.route(`**/api/libraries/${libraryId}/assets/asset_mock_0`, async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            asset: {
              asset_id: "asset_mock_0",
              source_id: "src_mock_0",
              source_uri: fileSourceUri("/tmp/video-preview/clip.mp4"),
              source_type: "video",
              asset_type: "video_segment",
              locator: {
                start_ms: 42_000,
                end_ms: 50_000,
              },
            },
            preview: {
              url: "http://127.0.0.1:54210/mock-preview/clip.mp4",
            },
            neighbor_context: null,
            units: createMockMatchedUnits(0).map((unit) => ({
              unit_id: unit.unit_id,
              unit_type: unit.unit_type,
            })),
            library_id: libraryId,
          },
        }),
      });
    });

    await page.goto("/");
    await expect(page.getByTestId("workspace-shell")).toBeVisible();
    await expect(page.getByTestId("library-select")).toContainText(libraryName);
    await page.reload();
    await page.getByTestId("search-text-input").fill("terminal screen");
    await page.getByTestId("search-submit-button").click();

    const videoResult = page.locator('[data-testid="result-card"][data-kind="video_segment"]').first();
    await expect(videoResult).toBeVisible();
    const resultPreview = videoResult.getByTestId("result-preview");
    await expect(resultPreview).toHaveAttribute("data-preview-kind", "video");
    await videoResult.evaluate((node) => {
      (node as HTMLElement).dataset.cardStableMarker = "kept";
      node.querySelector<HTMLElement>('[data-testid="result-preview"]')!.dataset.pollStableMarker =
        "kept";
    });

    await page.waitForTimeout(workspacePollWaitMs);
    await expect(videoResult).toHaveAttribute("data-card-stable-marker", "kept");
    await expect(resultPreview).toHaveAttribute("data-poll-stable-marker", "kept");

    await videoResult.locator(".result-select").click();
    await expect(page.getByTestId("asset-detail")).toBeVisible();
    await expect(page.getByTestId("asset-detail")).toContainText("clip.mp4");
    await expect(videoResult).toHaveAttribute("data-card-stable-marker", "kept");
    await expect(resultPreview).toHaveAttribute("data-poll-stable-marker", "kept");
  });

  test("status capsule keeps job activity visible while runtime is partially limited", async ({
    page,
  }) => {
    await createLibrary(page, "job-progress-partial-runtime");
    await mockPartialRuntimeStatus(page);

    await page.route("**/api/jobs**", async (route) => {
      const url = new URL(route.request().url());
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            jobs: url.searchParams.has("library_id")
              ? []
              : [
                  {
                    job_id: "job_partial_runtime_001",
                    library_id: "another-library",
                    kind: "refresh",
                    status: "running",
                    phase: "stage_write",
                    progress: {
                      completed: 13,
                      total: 57,
                      unit: "asset",
                    },
                    cancelable: true,
                    retryable: false,
                    current_attempt: {
                      attempt: 1,
                      status: "running",
                      summary:
                        "Writing batch 13/57 (8 assets(s)) into staged vector-space storage.",
                    },
                  },
                ],
          },
        }),
      });
    });

    await page.reload();
    await expect(page.getByTestId("status-capsule-button")).toContainText("部分受限");
    await expect(page.getByTestId("status-capsule-progress")).toHaveAttribute(
      "data-progress-kind",
      "determinate"
    );
    await expect(page.getByTestId("status-capsule-progress-label")).toHaveText("13/57 asset");
    await expect(
      page.getByTestId("status-capsule-button").locator(".status-dot")
    ).toHaveCSS("animation-name", "status-capsule-dot-pulse");
  });

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
    await expect(firstResult).toHaveAttribute("data-kind", "document_page");
    await expect(firstResult).toContainText("file:///tmp/search-fixtures/formal/report-1.pdf");
    await expect(firstResult.getByTestId("result-score")).toBeVisible();
    await expect(firstResult.getByTestId("result-preview")).toBeVisible();

    await expect(page.getByTestId("detail-panel")).toBeVisible();
    await expect(page.getByTestId("asset-detail")).toBeVisible();
    await expect(page.getByTestId("asset-detail")).toContainText(
      "file:///tmp/search-fixtures/formal/report-1.pdf"
    );
    await expect(page.getByTestId("visual-preview")).toBeVisible();
    await expect(page.locator('[data-testid="preview-link"]')).toHaveCount(0);
    await expect(page.locator('[data-testid="detail-use-as-query-document-button"]')).toHaveCount(0);
    await expect(page.getByTestId("detail-technical-content")).not.toBeVisible();
    await page.getByTestId("detail-technical-disclosure").locator("summary").click();
    await expect(page.getByTestId("detail-technical-content")).toBeVisible();
  });

  test("search before source preparation keeps submit disabled and points to the first source root", async ({
    page,
  }) => {
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
      await page.getByTestId("workspace-tab-search").click();

      await expect(page.getByTestId("search-state-strip")).toContainText("等待内容");
      await expect(page.getByTestId("search-readiness-action")).toContainText("接入来源或导入内容");
      await expect(page.getByTestId("search-readiness-open-inventory")).toBeVisible();
      await page.getByTestId("search-readiness-open-inventory").click();
      await expect(page.getByTestId("inventory-panel")).toBeVisible();
      await expect(page.getByTestId("inventory-import-panel")).toBeVisible();
    } finally {
      fs.rmSync(fixtures.tempDir, { recursive: true, force: true });
    }
  });
}

export function registerSearchTextControlScenarios() {
  test("search scope filter and mode toggles use the shared selected-state contract", async ({
    page,
  }) => {
    await createLibrary(page, "search-selection-controls");

    await expectSelectionControlContrast(
      page.getByTestId("search-scope-library"),
      page.getByTestId("search-scope-all-libraries")
    );

    await page.getByTestId("search-filter-toggle-button").click();
    await expectSelectionControlContrast(
      page.getByTestId("search-filter-toggle-button"),
      page.getByTestId("search-mode-image")
    );

    await page.getByTestId("search-mode-document").click();
    await expectSelectionControlContrast(
      page.getByTestId("search-mode-document"),
      page.getByTestId("search-mode-image")
    );
    await expect(page.getByTestId("search-mode-text")).toHaveAttribute("data-ui-selected", "false");
  });

  test("search workspace supports shared filters and load more pagination", async ({ page }) => {
    const searchRequests = [];
    let searchCallCount = 0;
    const sourcePathPrefix = "/tmp/search-fixtures/set-a";

    const searchRoutePattern = "**/api/search/text";
    const detailRoutePattern = "**/api/libraries/*/assets/*";

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
      const assetId = route.request().url().split("/").pop();
      const index = Number(String(assetId).replace("asset_mock_", "")) || 0;
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            asset: {
              asset_id: `asset_mock_${index}`,
              source_id: `src_mock_${index}`,
              source_uri: fileSourceUri(`${sourcePathPrefix}/report-${index + 1}.pdf`),
              source_type: "pdf",
              asset_type: "document_page",
              locator: {
                page: index + 1,
                page_label: String(index + 1),
              },
            },
            preview: {
              url: `http://127.0.0.1:54210/mock-preview/${index}.png`,
            },
            neighbor_context: null,
            units: createMockMatchedUnits(index).map((unit) => ({
              unit_id: unit.unit_id,
              unit_type: unit.unit_type,
            })),
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
          "asset_type": "document_page",
          source_type: "pdf",
          path_prefix: sourcePathPrefix,
        },
      });
      expect(searchRequests[1]).toMatchObject({
        text: "Revenue 46 percent",
        cursor: "search:v1:5",
        filters: {
          "asset_type": "document_page",
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
    const detailRoutePattern = "**/api/libraries/*/assets/*";

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
            asset: {
              asset_id: "asset_mock_0",
              source_id: "src_mock_0",
              source_uri: fileSourceUri("/tmp/search-fixtures/set-b/ready-report.pdf"),
              source_type: "pdf",
              asset_type: "document_page",
              locator: {
                page: 1,
                page_label: "1",
              },
            },
            preview: {
              url: "http://127.0.0.1:54210/mock-preview/0.png",
            },
            neighbor_context: null,
            units: createMockMatchedUnits(0).map((unit) => ({
              unit_id: unit.unit_id,
              unit_type: unit.unit_type,
            })),
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
    const detailRoutePattern = "**/api/libraries/*/assets/*";

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
      const assetId = parts.at(-1) ?? "asset_mock_0";
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            asset: {
              asset_id: assetId,
              source_id: `src_${libraryId}`,
              source_uri: fileSourceUri(`/tmp/search-fixtures/${libraryId}/focused-report.pdf`),
              source_type: "pdf",
              asset_type: "document_page",
              locator: {
                page: 1,
                page_label: "1",
              },
            },
            preview: {
              url: `http://127.0.0.1:54210/mock-preview/${libraryId}.png`,
            },
            neighbor_context: null,
            units: createMockMatchedUnits(0).map((unit) => ({
              unit_id: unit.unit_id,
              unit_type: unit.unit_type,
            })),
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

      await expect(page.getByTestId("search-result-library-focus-all")).toHaveAttribute(
        "data-ui-selected",
        "true"
      );
      await expectSelectionControlContrast(
        page.getByTestId("search-result-library-focus-all"),
        page.getByTestId(`search-result-library-focus-${firstReadyLibraryId}`)
      );
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
      await expect(page.getByTestId(`search-result-library-focus-${firstReadyLibraryId}`)).toHaveAttribute(
        "data-ui-selected",
        "true"
      );
      await expect(page.getByTestId("search-result-library-focus-all")).toHaveAttribute(
        "data-ui-selected",
        "false"
      );
      await expectSelectionControlContrast(
        page.getByTestId(`search-result-library-focus-${firstReadyLibraryId}`),
        page.getByTestId("search-result-library-focus-all")
      );

      await page.getByTestId("search-result-library-focus-all").click();

      await expect(page.getByTestId("result-card")).toHaveCount(3);
      await expect(page.getByTestId("search-result-library-group")).toHaveCount(2);
      await expect(page.getByTestId("search-result-library-focus-all")).toHaveAttribute(
        "data-ui-selected",
        "true"
      );
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
    const detailRoutePattern = "**/api/libraries/*/assets/*";

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
            asset: {
              asset_id: "asset_mock_0",
              source_id: "src_mock_0",
              source_uri: fileSourceUri("/tmp/search-fixtures/set-b/ready-report.pdf"),
              source_type: "pdf",
              asset_type: "document_page",
              locator: {
                page: 1,
                page_label: "1",
              },
            },
            preview: {
              url: "http://127.0.0.1:54210/mock-preview/0.png",
            },
            neighbor_context: null,
            units: createMockMatchedUnits(0).map((unit) => ({
              unit_id: unit.unit_id,
              unit_type: unit.unit_type,
            })),
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
      await expect(page.getByTestId("search-scope-library")).toHaveAttribute("data-ui-selected", "true");
      await expect(page.getByTestId("search-scope-all-libraries")).toHaveAttribute(
        "data-ui-selected",
        "false"
      );
      await expect(page.getByTestId("search-mode-document")).toHaveAttribute("data-ui-selected", "true");
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

    await openInventoryImportPanel(page);
    await page.getByTestId("import-paths-input").fill("README.md");
    await page.getByTestId("import-submit-button").click();

    await expect(page.getByTestId("import-receipt")).toBeVisible();
    const rejected = page.getByTestId("import-rejected-item").first();
    await expect(rejected).toBeVisible();
    await expect(rejected).toHaveAttribute("data-reason-code", "unsupported_type");
    await expect(page.getByTestId("import-no-job")).toBeVisible();
  });
}
