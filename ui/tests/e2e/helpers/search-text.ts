import { expect, test } from "@playwright/test";
import fs from "node:fs";
import {
  createLibrary,
  createMockSearchResult,
  createTempSourceManagementFixtures,
  currentLibraryId,
  expectSelectionControlContrast,
  expectSearchRequiresContent,
  importFixtureIntoCurrentLibrary,
  mockSingleTextSearchResult,
  openSearchAdvancedFilters,
  openSourcePreparationPanel,
} from "./fixtures";

export function registerSearchTextScenarios() {
  test("search task next step opens settings diagnostics and expands jobs", async ({ page }) => {
    const libraryName = await createLibrary(page, "search-jobs-deeplink");
    const libraryId = await currentLibraryId(page);

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

    await page.route("**/api/jobs?library_id=*", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          data: {
            jobs: [
              {
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
              },
            ],
          },
        }),
      });
    });

    await page.reload();
    await expect(page.getByTestId("search-next-step-dock")).toContainText("等待当前任务完成");
    await page.getByTestId("search-next-step-open-jobs").click();

    await expect(page.getByTestId("settings-workspace")).toBeVisible();
    await expect(page.getByTestId("settings-stage-title")).toHaveText("诊断");
    await expect(page.getByTestId("settings-diagnostics-jobs-disclosure")).toHaveAttribute(
      "open",
      ""
    );
    await expect(page.getByTestId("job-list")).toBeVisible();
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

      await expect(page.getByTestId("search-state-strip")).toContainText("等待内容");
      await expect(page.getByTestId("search-next-step-dock")).toContainText("准备第一批内容");
      await expect(page.getByTestId("search-next-step-open-inventory")).toBeVisible();
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

    await page.getByTestId("import-paths-input").fill("README.md");
    await page.getByTestId("import-submit-button").click();

    await expect(page.getByTestId("import-receipt")).toBeVisible();
    const rejected = page.getByTestId("import-rejected-item").first();
    await expect(rejected).toBeVisible();
    await expect(rejected).toHaveAttribute("data-reason-code", "unsupported_type");
    await expect(page.getByTestId("import-no-job")).toBeVisible();
  });
}
