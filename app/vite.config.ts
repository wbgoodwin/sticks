import { readFileSync } from "node:fs";
import { defineConfig, loadEnv } from "vite";

// One version for the whole app: the one Tauri bundles with.
const version: string = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")).version;

// Tauri serves the dev build from a fixed port and loads dist/ when bundled.
export default defineConfig(({ mode }) => {
  // TYPESAFE_API_KEY from the shell or app/.env.local. It stays on the dev
  // server: the proxy below adds it to calls the page makes to /jev.
  const env = loadEnv(mode, process.cwd(), "TYPESAFE_");
  const key = env.TYPESAFE_API_KEY ?? "";
  return {
    clearScreen: false,
    define: { __APP_VERSION__: JSON.stringify(version) },
    server: {
      port: 5173,
      strictPort: true,
      proxy: {
        "/jev": {
          target: env.TYPESAFE_BASE_URL || "https://api.typesafe.ai",
          changeOrigin: true,
          rewrite: (path) => path.replace(/^\/jev/, ""),
          headers: { Authorization: `Bearer ${key}` },
        },
      },
    },
    // three.js is most of the bundle; the app loads it once, offline.
    build: { target: "es2022", outDir: "dist", chunkSizeWarningLimit: 900 },
  };
});
