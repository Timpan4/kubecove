import { rm } from "node:fs/promises";
import { resolve } from "node:path";
import tailwindPlugin from "bun-plugin-tailwind";
import { createSveltePlugin } from "./bun-svelte-plugin";

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
	plugins: [
		createSveltePlugin(false),
		tailwindPlugin,
	],
	define: {
		"process.env.KUBECOVE_PUBLIC_DEV": JSON.stringify("false"),
		"process.env.KUBECOVE_PUBLIC_RELEASE_CHANNEL":
			JSON.stringify(releaseChannel),
	},
});

for (const log of result.logs) console.error(log);
if (!result.success) process.exit(1);

const bytes = result.outputs.reduce((total, output) => total + output.size, 0);
console.log(
	`Bun frontend built ${result.outputs.length} files (${bytes} bytes) in ${Math.round(performance.now() - startedAt)}ms.`,
);
