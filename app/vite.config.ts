import { defineConfig } from "vite";

// Tauri serves the dev build from a fixed port and loads dist/ when bundled.
export default defineConfig({
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  // three.js is most of the bundle; the app loads it once, offline.
  build: { target: "es2022", outDir: "dist", chunkSizeWarningLimit: 900 },
});
