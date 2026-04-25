import { expect, test } from "@playwright/test";
import { createLibrary, importFixtureIntoCurrentLibrary, openSettingsSection } from "./fixtures";

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
