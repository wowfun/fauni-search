import {
  clearRuntimeState,
  ensureDevRuntime,
  writeRuntimeState,
} from "./tests/e2e/dev-runtime";

export default async function globalSetup(): Promise<void> {
  clearRuntimeState();
  const runtime = await ensureDevRuntime();
  writeRuntimeState({
    startedByUs: runtime.startedByUs,
    uiUrl: runtime.uiUrl,
  });
}
