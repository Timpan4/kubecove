import { fileURLToPath } from "node:url";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import { frontendBundleReportPlugin } from "./scripts/frontend-bundle-report.ts";

export default defineConfig(({ command }) => {
	const host = process.env.TAURI_DEV_HOST?.trim();
	const releaseChannel = process.env.KUBECOVE_PUBLIC_RELEASE_CHANNEL === "stable" ? "stable" : "dev";
	const profiling = process.env.KUBECOVE_PUBLIC_PROFILE === "true";
	const runtime = process.env.KUBECOVE_PUBLIC_RUNTIME === "web" ? "web" : "native";
	return {
		plugins: [
			svelte(), tailwindcss(), frontendBundleReportPlugin(releaseChannel, profiling),
			{
				name: "native-e2e-bridge",
				transformIndexHtml: {
					order: "pre",
						handler: () => runtime !== "web" && process.env.KUBECOVE_E2E === "1"
						? [{ tag: "script", attrs: { type: "module" }, children: 'import "@wdio/tauri-plugin";', injectTo: "head" }]
						: [],
				},
			},
		],
		resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
		// First opening a deferred screen must not trigger optimizer reloads.
		optimizeDeps: {
			include: [
				...(runtime === "web" ? [] : ["@tauri-apps/plugin-opener"]),
				"@codemirror/lang-yaml",
				"@codemirror/language",
				"@codemirror/lint",
				"@codemirror/state",
				"@codemirror/view",
				"@lezer/highlight",
				"@tanstack/svelte-virtual",
				"diff",
				"yaml",
			],
		},
		define: {
			"process.env.KUBECOVE_PUBLIC_DEV": JSON.stringify(String(command === "serve")),
			"process.env.KUBECOVE_PUBLIC_RUNTIME": JSON.stringify(runtime),
			"process.env.KUBECOVE_PUBLIC_PROFILE": JSON.stringify(String(profiling)),
			"process.env.KUBECOVE_PUBLIC_RELEASE_CHANNEL": JSON.stringify(releaseChannel),
		},
		build: {
			// Preserve native dynamic imports and BigInt on older WebKit releases.
			target: "safari15",
			rolldownOptions: {
				output: {
					codeSplitting: {
						groups: [
							{ name: "vendor-svelte", test: /node_modules[\\/]svelte[\\/]/, entriesAware: true },
							{ name: "vendor-query", test: /node_modules[\\/]@tanstack[\\/]/, entriesAware: true },
							{ name: "vendor-bits", test: /node_modules[\\/]bits-ui[\\/]/, entriesAware: true },
							{ name: "vendor-icons", test: /node_modules[\\/]lucide-svelte[\\/]/, entriesAware: true },
							{ name: "vendor-yaml", test: /node_modules[\\/]yaml[\\/]/, entriesAware: true },
						],
					},
				},
			},
		},
		server: {
			port: 1430,
			strictPort: true,
			host: host || "localhost",
			hmr: host ? { host } : undefined,
			watch: { ignored: ["**/src-tauri/**", "**/.e2e/**", "**/e2e/artifacts/**"] },
		},
		preview: { host: "127.0.0.1", port: Number(process.env.PORT ?? 4173), strictPort: true },
	};
});
