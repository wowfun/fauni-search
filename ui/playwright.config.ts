import os from "node:os";
import path from "node:path";
import { defineConfig } from "@playwright/test";
import { getDevUiUrl } from "./tests/e2e/dev-runtime";

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: false,
  workers: 1,
  timeout: 12 * 60 * 1000,
  expect: {
    timeout: 30 * 1000,
  },
  outputDir: path.join(os.tmpdir(), "fauni-search-playwright-results"),
  reporter: "list",
  globalSetup: "./playwright.global-setup.ts",
  globalTeardown: "./playwright.global-teardown.ts",
  use: {
    baseURL: getDevUiUrl(),
    headless: true,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: {
        browserName: "chromium",
      },
    },
  ],
});
