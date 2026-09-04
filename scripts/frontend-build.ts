import { mkdir, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import tailwindPlugin from "bun-plugin-tailwind";
import { createSveltePlugin } from "./bun-svelte-plugin";
import { frontendBundleReport } from "./frontend-bundle-report";

const outdir = resolve("dist");
await rm(outdir, { recursive: true, force: true });

const releaseChannel =
	process.env.KUBECOVE_PUBLIC_RELEASE_CHANNEL === "stable" ? "stable" : "dev";
const startedAt = performance.now();
const result = await Bun.build({
	entrypoints: [resolve("index.html")],
	outdir,
	target: "browser",
	conditions: ["svelte"],
	minify: true,
	splitting: true,
	sourcemap: "none",
	metafile: true,
	plugins: [
		createSveltePlugin(false),
		tailwindPlugin,
	],
	define: {
		"process.env.KUBECOVE_PUBLIC_DEV": JSON.stringify("false"),
		"process.env.KUBECOVE_PUBLIC_PROFILE": JSON.stringify(process.env.KUBECOVE_PUBLIC_PROFILE === "true" ? "true" : "false"),
		"process.env.KUBECOVE_PUBLIC_RELEASE_CHANNEL":
			JSON.stringify(releaseChannel),
	},
});

for (const log of result.logs) console.error(log);
if (!result.success) process.exit(1);
if (!result.metafile) throw new Error("Bun did not emit bundle metadata");
const buildMs = Math.round(performance.now() - startedAt);
const report = await frontendBundleReport(resolve("."), outdir, result.outputs, result.metafile);
await mkdir(resolve(".e2e", "reports"), { recursive: true });
await writeFile(resolve(".e2e", "reports", "frontend-bundle.json"), `${JSON.stringify({ ...report, releaseChannel, profiling: process.env.KUBECOVE_PUBLIC_PROFILE === "true" }, null, 2)}\n`);

const bytes = result.outputs.reduce((total, output) => total + output.size, 0);
console.log(
	`Bun frontend built ${result.outputs.length} files (${bytes} bytes) in ${buildMs}ms. Bundle report: .e2e/reports/frontend-bundle.json`,
);
