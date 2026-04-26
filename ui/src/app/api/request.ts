import type { ApiErrorPayload } from "../../types";

export interface ApiSuccessEnvelope<T> {
  data: T;
}

export interface ApiErrorEnvelope {
  error: ApiErrorPayload;
}

export type ApiEnvelope<T> = ApiSuccessEnvelope<T> | ApiErrorEnvelope;

export interface EndpointConfig {
  appHealth: string;
  sidecarHealth: string;
  qdrantCollections: string;
  uiRoot: string;
}

export function requireEnv(name: string): string {
  const value = import.meta.env[name];
  if (!value) {
    throw new Error(`Missing required environment variable ${name}`);
  }
  return String(value);
}

export const endpoints: EndpointConfig = {
  appHealth: `http://${requireEnv("APP_HOST")}:${requireEnv("APP_PORT")}/health`,
  sidecarHealth: `http://${requireEnv("SIDECAR_HOST")}:${requireEnv("SIDECAR_PORT")}/health`,
  qdrantCollections: `${requireEnv("QDRANT_URL").replace(/\/$/, "")}/collections`,
  uiRoot: `http://${requireEnv("UI_HOST")}:${requireEnv("UI_PORT")}/`,
};

export function toApiError(error: unknown): ApiErrorPayload {
  if (typeof error === "string") {
    return {
      code: "request_failed",
      message: error,
    };
  }

  if (error && typeof error === "object") {
    const candidate = error as Partial<ApiErrorPayload>;
    return {
      code: typeof candidate.code === "string" ? candidate.code : "request_failed",
      message:
        typeof candidate.message === "string" ? candidate.message : "Unexpected request failure.",
      details:
        candidate.details && typeof candidate.details === "object"
          ? candidate.details
          : undefined,
      retryable: typeof candidate.retryable === "boolean" ? candidate.retryable : undefined,
    };
  }

  return {
    code: "request_failed",
    message: "Unexpected request failure.",
  };
}

function apiFetchPath(path: string): string {
  return `${import.meta.env.DEV ? "/api" : ""}${path}`;
}

export async function apiRequest<T = any>(path: string, options: RequestInit = {}): Promise<T> {
  const headers = new Headers(options.headers);
  const isFormDataBody = options.body instanceof FormData;
  if (!isFormDataBody && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  const response = await fetch(apiFetchPath(path), {
    ...options,
    headers,
  });

  let payload: ApiEnvelope<T> | null = null;
  try {
    payload = (await response.json()) as ApiEnvelope<T>;
  } catch {
    payload = null;
  }

  if (!response.ok || (payload && "error" in payload)) {
    throw toApiError((payload && "error" in payload ? payload.error : null) ?? {
      code: "request_failed",
      message: `Request failed with status ${response.status}`,
    });
  }

  if (!payload || !("data" in payload)) {
    throw toApiError({
      code: "request_failed",
      message: "Expected a successful JSON payload but did not receive one.",
    });
  }

  return payload.data;
}
