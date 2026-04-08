import {
  clearRuntimeState,
  ensureDevRuntime,
  writeRuntimeState,
} from "./tests/e2e/dev-runtime.js";

export default async function globalSetup() {
  clearRuntimeState();
  const runtime = await ensureDevRuntime();
  writeRuntimeState({
    startedByUs: runtime.startedByUs,
    uiUrl: runtime.uiUrl,
  });
}
