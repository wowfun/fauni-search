import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFile } from "node:child_process";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const REPO_ROOT = path.resolve(__dirname, "../../..");
const DEV_ENV_PATH = path.join(REPO_ROOT, ".env.dev");
const DEV_ENV_EXAMPLE_PATH = path.join(REPO_ROOT, ".env.dev.example");
const RUNTIME_STATE_PATH = path.join(os.tmpdir(), "fauni-search-playwright-dev-runtime.json");

interface SelectedDevEnv {
  envPath: string;
  env: Record<string, string>;
  isConcreteEnv: boolean;
}

interface DevServiceStatus {
  ready?: boolean;
  pid?: number | null;
  pids?: number[] | null;
  url?: string | null;
}

interface DevRuntimeStatus {
  services?: Record<string, DevServiceStatus>;
}

interface DevRuntimeState {
  startedByUs: boolean;
  uiUrl: string;
}

function parseEnvFile(contents: string): Record<string, string> {
  const values: Record<string, string> = {};

  for (const rawLine of contents.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) {
      continue;
    }

    const equalsAt = line.indexOf("=");
    if (equalsAt <= 0) {
      continue;
    }

    const key = line.slice(0, equalsAt).trim();
    let value = line.slice(equalsAt + 1).trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    values[key] = value;
  }

  return values;
}

function readSelectedDevEnv(): SelectedDevEnv {
  const envPath = fs.existsSync(DEV_ENV_PATH) ? DEV_ENV_PATH : DEV_ENV_EXAMPLE_PATH;
  if (!envPath || !fs.existsSync(envPath)) {
    throw new Error("Missing .env.dev and .env.dev.example; bootstrap the repo first.");
  }

  const env = parseEnvFile(fs.readFileSync(envPath, "utf8"));
  for (const key of ["UI_HOST", "UI_PORT"]) {
    if (!env[key]) {
      throw new Error(`Missing ${key} in ${path.relative(REPO_ROOT, envPath)}`);
    }
  }

  return {
    envPath,
    env,
    isConcreteEnv: envPath === DEV_ENV_PATH,
  };
}

export function getDevUiUrl() {
  const { env } = readSelectedDevEnv();
  return `http://${env.UI_HOST}:${env.UI_PORT}/`;
}

async function runLocalScript(scriptName: string, args: string[] = []) {
  const scriptPath = path.join(REPO_ROOT, "scripts/local", scriptName);
  return execFileAsync("bash", [scriptPath, "--dev", ...args], {
    cwd: REPO_ROOT,
    maxBuffer: 10 * 1024 * 1024,
  });
}

export async function getDevStatus(): Promise<DevRuntimeStatus> {
  const { stdout } = await runLocalScript("status.sh", ["--json"]);
  return JSON.parse(stdout);
}

function isReady(service?: DevServiceStatus | null) {
  return Boolean(service?.ready);
}

function hasPid(service?: DevServiceStatus | null) {
  return Boolean(service?.pid || (service?.pids ?? []).length);
}

function hasAnyDevRuntime(status: DevRuntimeStatus) {
  const services = status?.services ?? {};
  return ["app", "sidecar", "ui", "qdrant"].some(
    (name) => isReady(services[name]) || hasPid(services[name])
  );
}

function allDevServicesReady(status: DevRuntimeStatus) {
  const services = status?.services ?? {};
  return ["app", "sidecar", "ui", "qdrant"].every((name) => isReady(services[name]));
}

export async function ensureDevRuntime() {
  const selectedEnv = readSelectedDevEnv();
  if (!selectedEnv.isConcreteEnv) {
    throw new Error("Missing .env.dev; run `bash scripts/local/bootstrap-linux.sh --dev` first.");
  }

  const status = await getDevStatus();
  if (allDevServicesReady(status)) {
    return {
      startedByUs: false,
      uiUrl: status.services.ui.url,
    };
  }

  if (hasAnyDevRuntime(status)) {
    await stopDevRuntime();
  }

  await runLocalScript("prune-dev-qdrant-collections.sh", [
    "--max-count",
    "500",
    "--keep-count",
    "100",
  ]);
  await runLocalScript("run.sh", ["--detach"]);

  const nextStatus = await getDevStatus();
  if (!allDevServicesReady(nextStatus)) {
    throw new Error(
      `The --dev runtime did not become fully ready. Status:\n${JSON.stringify(nextStatus, null, 2)}`
    );
  }

  return {
    startedByUs: true,
    uiUrl: nextStatus.services.ui.url,
  };
}

export async function stopDevRuntime() {
  await runLocalScript("stop.sh", ["--all"]);
}

export function writeRuntimeState(payload: DevRuntimeState) {
  fs.writeFileSync(RUNTIME_STATE_PATH, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
}

export function readRuntimeState(): DevRuntimeState | null {
  if (!fs.existsSync(RUNTIME_STATE_PATH)) {
    return null;
  }

  return JSON.parse(fs.readFileSync(RUNTIME_STATE_PATH, "utf8"));
}

export function clearRuntimeState() {
  fs.rmSync(RUNTIME_STATE_PATH, { force: true });
}
