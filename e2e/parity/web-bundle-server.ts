import { join } from "node:path";
import { build, loadConfigFromFile, preview } from "vite";

// Separate entry points preserve the exports the driver imports while sharing the app's
// mock instances through the bundler's common chunks. The query entry likewise shares
// the app's focus manager with the stale-read scenario.
const mocks = [
    "mcp-dev-mocks", "settings-integration-mocks", "tauri-dev-mock-argo",
    "tauri-dev-mock-coverage", "tauri-dev-mock-live", "tauri-dev-mock-pages",
    "tauri-dev-mock-session-controls", "tauri-dev-mock-shell",
    "tauri-dev-mock-timing", "tauri-dev-mocks",
];
const [portValue, artifacts] = process.argv.slice(2);
if (!portValue || !artifacts) throw new Error("Expected a port and artifacts directory");
const port = Number(portValue);
process.env.NODE_ENV = "development";
const loaded = await loadConfigFromFile({ command: "build", mode: "development" });
if (!loaded) throw new Error("Vite config not found");
const config = loaded.config;
const outDir = join(artifacts, "web-app");
const imports = new Map(mocks.map(name => [`/src/lib/${name}.ts`, `/assets/${name}.js`]));
imports.set("/node_modules/.vite/deps/@tanstack_svelte-query.js", "/assets/parity-query.js");
const input = Object.fromEntries([["index", join(process.cwd(), "index.html")], ["parity-query", "@tanstack/svelte-query"], ...mocks.map(name => [name, join(process.cwd(), "src/lib", `${name}.ts`)])]);
await build({
    ...config,
    configFile: false,
    mode: "development",
    define: { ...config.define, "process.env.KUBECOVE_PUBLIC_DEV": JSON.stringify("true") },
    plugins: [
        ...(config.plugins ?? []).flat().filter(plugin => !(plugin && typeof plugin === "object" && "name" in plugin && plugin.name === "kubecove-bundle-report")),
    ],
    build: {
        ...config.build, outDir, emptyOutDir: false, minify: false, reportCompressedSize: false,
        rolldownOptions: {
            ...config.build?.rolldownOptions, input, preserveEntrySignatures: "strict",
            output: { ...config.build?.rolldownOptions?.output, entryFileNames: "assets/[name].js" },
        },
    },
});
const server = await preview({
    configFile: false, build: { outDir }, preview: { host: "127.0.0.1", port, strictPort: true },
    plugins: [{
        name: "parity-mock-imports",
        configurePreviewServer(server) {
            server.middlewares.use((request, response, next) => {
                const asset = imports.get(request.url?.split("?")[0] ?? "");
                if (!asset) return next();
                // Re-export the canonical asset. Rewriting the URL would break its relative
                // imports, and copying its body could instantiate mock state twice.
                response.setHeader("Content-Type", "text/javascript");
                response.end(`export * from ${JSON.stringify(asset)};`);
            });
        },
    }],
});
server.printUrls();
