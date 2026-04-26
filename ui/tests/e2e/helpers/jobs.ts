import { expect, test } from "@playwright/test";
import { createLibrary, importFixtureIntoCurrentLibrary, openSettingsSection } from "./fixtures";

export function registerRuntimeStatusScenarios() {
  test("runtime status panel shows native capabilities execution inputs and vector-space diagnostics", async ({
    page,
  }) => {
    const localModelId = "athrael-soju/colqwen3.5-4.5B-v3";

    await createLibrary(page, "runtime-status");
    await importFixtureIntoCurrentLibrary(page);

    await openSettingsSection(page, "diagnostics");

    const runtimeStatusPanel = page.getByTestId("runtime-status-panel");
    await expect(runtimeStatusPanel).toContainText("Local Sidecar");
    await expect(runtimeStatusPanel).toContainText(localModelId);
    await expect(runtimeStatusPanel).toContainText("嵌入能力");
    await expect(runtimeStatusPanel).toContainText("输入 text, image");
    await expect(runtimeStatusPanel).toContainText("执行输入");
    await expect(runtimeStatusPanel).toContainText("text, image, document, video");
    await expect(runtimeStatusPanel).toContainText("运行时适配器");
    await expect(runtimeStatusPanel).toContainText("document_query_via_page_images");
    await expect(runtimeStatusPanel).toContainText("video_query_via_frame_images");

    const vectorSpacesPanel = page.getByTestId("vector-space-diagnostics-panel");
    await expect(vectorSpacesPanel).toContainText("active");
    await expect(vectorSpacesPanel).toContainText(localModelId);
    await expect(vectorSpacesPanel).toContainText("multi_vector_late_interaction");
  });
}
