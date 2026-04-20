import {
  clearRuntimeState,
  readRuntimeState,
  stopDevRuntime,
} from "./tests/e2e/dev-runtime";

export default async function globalTeardown(): Promise<void> {
  const runtime = readRuntimeState();

  try {
    if (runtime?.startedByUs) {
      await stopDevRuntime();
    }
  } finally {
    clearRuntimeState();
  }
}
