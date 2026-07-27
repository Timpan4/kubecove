import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

const runtime = (globalThis as {
	process?: { env?: { TAURI_DEV_HOST?: string }; platform?: string };
}).process;
const host = runtime?.env?.TAURI_DEV_HOST;
const usePolling = runtime?.platform === "win32";
const devServerPort = 1430;
const hmrPort = 1431;

// https://vite.dev/config/
export default defineConfig(() => ({
	plugins: [svelte(), tailwindcss()],
	resolve: {
		alias: {
			"@": "/src",
		},
		dedupe: [
			"@codemirror/lang-yaml",
			"@codemirror/language",
			"@codemirror/lint",
			"@codemirror/state",
			"@codemirror/view",
			"@lezer/highlight",
		],
	},
	optimizeDeps: {
		exclude: ["@xyflow/svelte"],
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
			// Windows file events can miss agent/editor writes and leave Vite's module graph stale.
			usePolling,
			interval: 250,
		},
	},
}));
