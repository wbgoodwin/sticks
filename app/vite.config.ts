import { defineConfig } from "vite";

// Tauri serves the dev build from a fixed port and loads dist/ when bundled.
export default defineConfig({
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "es2022", outDir: "dist" },
});
