import { expect, test } from "@playwright/test";
import {
  createLibrary,
  currentLibraryId,
  expectSelectionControlContrast,
  fixtureImagePath,
  openSettingsSection,
} from "./fixtures";

export function registerSettingsScenarios() {
  test("settings diagnostics keeps runtime facts visible and folds jobs by default", async ({
    page,
  }) => {
    await createLibrary(page, "settings-diagnostics");
    await openSettingsSection(page, "diagnostics");

    await expect(page.getByTestId("runtime-status-panel")).toBeVisible();
    await expect(page.getByTestId("vector-space-diagnostics-panel")).toBeVisible();
    await expect(page.getByTestId("settings-diagnostics-jobs-disclosure")).toBeVisible();
    await expect(page.getByTestId("settings-diagnostics-jobs-disclosure")).not.toHaveAttribute(
      "open",
      ""
    );
    await expect(page.getByTestId("settings-open-maintenance-tools")).toHaveCount(0);
  });

  test("settings workspace shows exact models and editable provider config", async ({ page }) => {
    const localModelId = "athrael-soju/colqwen3.5-4.5B-v3";

    await createLibrary(page, "provider-settings");

    await openSettingsSection(page, "providers");

    await expect(page.getByTestId("settings-stage-title")).toHaveText("模型提供方");
    await expect(page.getByTestId("settings-stage-summary")).toHaveCount(0);
    await expect(page.getByTestId("settings-stage-metrics")).toHaveCount(0);
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
    await page.locator('[data-provider-edit-id="local_sidecar"]').click();
    await expect(page.getByTestId("provider-editor-runtime-summary")).toContainText(localModelId);
    await expect(page.getByTestId("provider-editor-runtime-summary")).toContainText("模型版本 main");
    await expect(page.getByTestId("provider-editor-runtime-summary")).toContainText("模型修订 main");
    await page.locator('[data-provider-edit-id="dashscope"]').click();
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
    await expect(page.getByTestId("settings-stage-metrics")).toHaveCount(0);
    await expect(page.getByTestId("resolved-content-models-panel")).toContainText(localModelId);
    await expect(page.getByTestId("resolved-content-models-panel")).toContainText(
      "全局内容类型"
    );
    await expect(page.getByTestId("resolved-content-models-panel")).not.toContainText("执行空间");
    await expect(page.getByTestId("resolved-content-models-panel")).not.toContainText(
      "vector_space_id"
    );
  });

  test("settings content type overlays save and restore through runtime config CRUD", async ({
    page,
  }) => {
    await createLibrary(page, "settings-content-type-overlays");
    const libraryId = await currentLibraryId(page);
    await page.request.delete("/api/settings/content-types/document");
    await page.request.delete(
      `/api/libraries/${encodeURIComponent(libraryId)}/content-types/document`
    );

    await openSettingsSection(page, "content-types");
    await expect(page.getByTestId("settings-stage-title")).toHaveText("内容类型");
    await expect(page.getByTestId("global-content-type-tabs").locator("[data-content-type]"))
      .toHaveCount(4);
    await page.getByTestId("global-content-type-tab-document").click();
    await expect(page.getByTestId("global-content-type-summary")).toContainText("baseline");
    await expect(page.getByTestId("global-content-types-reset-button")).toBeDisabled();

    await Promise.all([
      page.waitForResponse(
        (response) =>
          response.url().includes("/settings/content-types/document") &&
          response.request().method() === "PATCH" &&
          response.ok()
      ),
      page.getByTestId("global-content-types-submit-button").click(),
    ]);
    await expect(page.getByTestId("global-content-type-summary")).toContainText(
      "runtime_overlay"
    );
    await expect(page.getByTestId("global-content-types-reset-button")).toBeEnabled();
    const globalOverlay = await (await page.request.get("/api/settings/content-types")).json();
    expect(globalOverlay.data.origins.document).toMatchObject({
      origin: "runtime_overlay",
      has_runtime_overlay: true,
    });

    await Promise.all([
      page.waitForResponse(
        (response) =>
          response.url().includes("/settings/content-types/document") &&
          response.request().method() === "DELETE" &&
          response.ok()
      ),
      page.getByTestId("global-content-types-reset-button").click(),
    ]);
    await expect(page.getByTestId("global-content-type-summary")).toContainText("baseline");
    const globalRestored = await (await page.request.get("/api/settings/content-types")).json();
    expect(globalRestored.data.origins.document).toMatchObject({
      origin: "baseline",
      has_runtime_overlay: false,
    });

    await openSettingsSection(page, "library-overrides");
    await expect(page.getByTestId("settings-stage-title")).toHaveText("当前库覆盖");
    await expect(page.getByTestId("inventory-toggle-library-archive-button")).toHaveCount(0);
    await expect(page.getByTestId("inventory-delete-library-button")).toHaveCount(0);
    await expect(page.getByTestId("open-create-library-button")).toHaveCount(0);
    await page.getByTestId("library-content-type-tab-document").click();
    await expect(page.getByTestId("library-content-type-summary")).toContainText("inherited");
    await page.getByTestId("library-override-mode-override").click();

    await Promise.all([
      page.waitForResponse(
        (response) =>
          response.url().includes(`/libraries/${encodeURIComponent(libraryId)}/content-types/document`) &&
          response.request().method() === "PATCH" &&
          response.ok()
      ),
      page.getByTestId("library-content-types-submit-button").click(),
    ]);
    await expect(page.getByTestId("library-content-type-summary")).toContainText(
      "runtime_overlay"
    );

    await Promise.all([
      page.waitForResponse(
        (response) =>
          response.url().includes(`/libraries/${encodeURIComponent(libraryId)}/content-types/document`) &&
          response.request().method() === "DELETE" &&
          response.ok()
      ),
      page.getByTestId("library-content-types-reset-button").click(),
    ]);
    await expect(page.getByTestId("library-content-type-summary")).toContainText("inherited");
  });

  test("settings navigation tabs and override switches share the selected-state styling", async ({
    page,
  }) => {
    await createLibrary(page, "settings-selection-controls");
    await openSettingsSection(page, "library-overrides");

    await expectSelectionControlContrast(
      page.getByTestId("settings-nav-library-overrides"),
      page.getByTestId("settings-nav-providers")
    );

    const activeContentTypeTab = page
      .getByTestId("library-content-type-tabs")
      .locator('[data-ui-selected="true"]')
      .first();
    const inactiveContentTypeTab = page
      .getByTestId("library-content-type-tabs")
      .locator('[data-ui-selected="false"]')
      .first();
    await expectSelectionControlContrast(activeContentTypeTab, inactiveContentTypeTab);

    const activeOverrideMode = page
      .getByTestId("library-override-mode-switch")
      .locator('[data-ui-selected="true"]')
      .first();
    const inactiveOverrideMode = page
      .getByTestId("library-override-mode-switch")
      .locator('[data-ui-selected="false"]')
      .first();
    await expectSelectionControlContrast(activeOverrideMode, inactiveOverrideMode);
    await inactiveOverrideMode.click();
    await expectSelectionControlContrast(
      page.getByTestId("library-override-mode-switch").locator('[data-ui-selected="true"]').first(),
      page.getByTestId("library-override-mode-switch").locator('[data-ui-selected="false"]').first()
    );
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
        text: [
          [0.1, 0.2, 0.3],
          [0.4, 0.5, 0.6],
        ],
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
                    content_type:
                      comparisonModality === "image" ? "image/png" : "application/octet-stream",
                    size_bytes: 4321,
                  },
                  similarity_to_primary:
                    similarityByPair[`${modality}:${comparisonModality}`] ?? 0.5,
                }
              : null,
          },
        }),
      });
    });

    await createLibrary(page, "provider-settings-model-test");
    await openSettingsSection(page, "providers");
    await page.locator('[data-provider-edit-id="local_sidecar"]').click();

    const globalPanel = page.getByTestId("provider-model-test-panel");
    await expect(globalPanel).toContainText(localModelId);
    await expect(page.getByTestId("provider-model-test-support-message")).toContainText("文本、图片");
    await expect(page.getByTestId("provider-model-capabilities")).toContainText("输入 text, image");
    await expect(page.getByTestId("provider-model-capabilities")).toContainText("向量 multi_vector_late_interaction");
    await expect(page.locator('[data-testid="provider-model-test-modality"] option')).toHaveCount(2);
    await expect(page.locator('[data-testid="provider-model-test-modality"] option').nth(0)).toHaveText(
      "文本"
    );
    await expect(page.locator('[data-testid="provider-model-test-modality"] option').nth(1)).toHaveText(
      "图片"
    );
    await expect(page.getByTestId("provider-model-test-modality")).toHaveValue("text");

    await page.getByTestId("provider-model-test-text").fill("Revenue 46 percent");
    await page.getByTestId("provider-model-test-submit-button").click();
    await expect(page.getByTestId("provider-model-test-shape")).toContainText("[2, 3]");
    await expect(page.getByTestId("provider-model-test-vectors")).toContainText("0.1");

    await page.getByTestId("provider-model-test-comparison-modality").selectOption("image");
    await page.getByTestId("provider-model-test-comparison-file").setInputFiles(fixtureImagePath);
    await page.getByTestId("provider-model-test-submit-button").click();
    await expect(page.getByTestId("provider-model-test-comparison-shape")).toContainText("[1, 3]");
    await expect(page.getByTestId("provider-model-test-comparison-vectors")).toContainText("1");
    await expect(page.getByTestId("provider-model-test-similarity")).toContainText("0.876543");

    await page.getByTestId("provider-model-test-modality").selectOption("image");
    await expect(page.getByTestId("provider-model-test-file")).toBeVisible();
    await page.getByTestId("provider-model-test-file").setInputFiles(fixtureImagePath);
    await page.getByTestId("provider-model-test-submit-button").click();
    await expect(page.getByTestId("provider-model-test-shape")).toContainText("[1, 3]");
    await expect(page.getByTestId("provider-model-test-vectors")).toContainText("1");

    await openSettingsSection(page, "library-overrides");
    await page.getByTestId("library-override-mode-override").click();
    await page.getByTestId("library-content-type-provider-id").selectOption("dashscope");
    await page.getByTestId("library-content-type-model-id").selectOption(dashscopeModelId);
    await expect(page.getByTestId("settings-nav-model-tests")).toHaveCount(0);
  });
}
