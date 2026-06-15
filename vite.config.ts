import { defineConfig } from "vite";
import react, { reactCompilerPreset } from "@vitejs/plugin-react";
import babel from "@rolldown/plugin-babel";
import tailwindcss from "@tailwindcss/vite";

const host = (globalThis as { process?: { env?: { TAURI_DEV_HOST?: string } } })
  .process?.env?.TAURI_DEV_HOST;
const devServerPort = 1430;
const hmrPort = 1431;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), babel({ presets: [reactCompilerPreset()] }), tailwindcss()],
  resolve: {
    alias: {
      "@": "/src",
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: devServerPort,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: hmrPort,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
