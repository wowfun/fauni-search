/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly APP_HOST: string;
  readonly APP_PORT: string;
  readonly SIDECAR_HOST: string;
  readonly SIDECAR_PORT: string;
  readonly QDRANT_URL: string;
  readonly UI_HOST: string;
  readonly UI_PORT: string;
  readonly [key: string]: string | boolean | undefined;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
