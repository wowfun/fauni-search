import {
  clearRuntimeState,
  readRuntimeState,
  stopDevRuntime,
} from "./tests/e2e/dev-runtime.js";

export default async function globalTeardown() {
  const runtime = readRuntimeState();

  try {
    if (runtime?.startedByUs) {
      await stopDevRuntime();
    }
  } finally {
    clearRuntimeState();
  }
}
