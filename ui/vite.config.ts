import { defineConfig, loadEnv } from "vite";

function requireEnv(env: Record<string, string>, name: string): string {
  const value = env[name];
  if (!value) {
    throw new Error(`Missing required environment variable ${name}`);
  }
  return value;
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, "..", "");
  const appTarget = `http://${requireEnv(env, "APP_HOST")}:${requireEnv(env, "APP_PORT")}`;

  return {
    envDir: "..",
    envPrefix: ["APP_", "SIDECAR_", "UI_", "QDRANT_", "FAUNI_", "VITE_"],
    server: {
      host: requireEnv(env, "UI_HOST"),
      port: Number(requireEnv(env, "UI_PORT")),
      proxy: {
        "/api": {
          target: appTarget,
          changeOrigin: true,
          rewrite: (requestPath: string) => requestPath.replace(/^\/api/, ""),
        },
      },
    },
  };
});
