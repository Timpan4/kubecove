import { mkdirSync, readFileSync, readdirSync, rmSync, statSync } from "node:fs";
import { extname, join, relative, resolve, sep } from "node:path";
import { gzipSync } from "node:zlib";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import react from "@vitejs/plugin-react";
import { chromium, type Browser } from "playwright-core";
import { build, type PluginOption } from "vite";
import {
	TOPOLOGY_SPIKE_NODE_COUNTS,
	createTopologySpikeGraph,
} from "../src/features/resources/topology-spike/synthetic-topology";
import type { TopologySpikeRunResult } from "../src/features/resources/topology-spike/benchmark-types";

interface AssetSummary {
	rawBytes: number;
	gzipBytes: number;
	largest: Array<{ path: string; rawBytes: number; gzipBytes: number }>;
}

interface FrameworkSummary {
	framework: "react" | "svelte";
	outDir: string;
	assets: AssetSummary;
	browser: TopologySpikeRunResult[];
	browserErrors: string[];
}

interface PureTopologySummary {
	nodeCount: number;
	edgeCount: number;
	layoutMs: number;
	selectionCount: number;
	rssBytes: number;
	heapUsedBytes: number;
}

const rootDir = resolve(import.meta.dir, "..");
const spikeDir = resolve(rootDir, "src/features/resources/topology-spike");
const cacheDir = resolve(rootDir, ".cache/topology-spike");
const browserTimeoutMs = 10_000;
const browserLaunchTimeoutMs = 8_000;

function forceGc() {
	Bun.gc(true);
}

function memorySample() {
	forceGc();
	const usage = process.memoryUsage();
	return {
		rssBytes: usage.rss,
		heapUsedBytes: usage.heapUsed,
	};
}

function walkFiles(dir: string): string[] {
	return readdirSync(dir).flatMap((entry) => {
		const path = join(dir, entry);
		const stat = statSync(path);
		return stat.isDirectory() ? walkFiles(path) : [path];
	});
}

function summarizeAssets(outDir: string): AssetSummary {
	const files = walkFiles(outDir).filter((path) =>
		[".js", ".css", ".html"].includes(extname(path)),
	);
	const largest = files
		.map((path) => {
			const bytes = readFileSync(path);
			return {
				path: relative(outDir, path).split(sep).join("/"),
				rawBytes: bytes.length,
				gzipBytes: gzipSync(bytes).length,
			};
		})
		.sort((a, b) => b.rawBytes - a.rawBytes);

	return {
		rawBytes: largest.reduce((total, file) => total + file.rawBytes, 0),
		gzipBytes: largest.reduce((total, file) => total + file.gzipBytes, 0),
		largest: largest.slice(0, 8),
	};
}

function htmlInput(name: "react" | "svelte"): string {
	return resolve(spikeDir, `${name}.html`);
}

async function buildHarness(
	framework: "react" | "svelte",
	plugins: PluginOption[],
): Promise<string> {
	console.error(`Building ${framework} topology harness...`);
	const outDir = resolve(cacheDir, framework);
	rmSync(outDir, { recursive: true, force: true });
	await build({
		root: spikeDir,
		base: "./",
		plugins,
		logLevel: "warn",
		resolve: {
			alias: {
				"@": resolve(rootDir, "src"),
			},
		},
		build: {
			outDir,
			emptyOutDir: true,
			manifest: true,
			rollupOptions: {
				input: htmlInput(framework),
			},
		},
	});
	return outDir;
}

function pureTopologyBenchmarks(): PureTopologySummary[] {
	return TOPOLOGY_SPIKE_NODE_COUNTS.map((nodeCount) => {
		const graph = createTopologySpikeGraph(nodeCount);
		const memory = memorySample();
		return {
			nodeCount: graph.nodes.length,
			edgeCount: graph.edges.length,
			layoutMs: Number(graph.layoutMs.toFixed(2)),
			selectionCount: graph.selectionIds.length,
			...memory,
		};
	});
}

async function launchBrowser(): Promise<Browser | null> {
	const channels = ["msedge", "chrome", "chromium"];
	for (const channel of channels) {
		try {
			return await chromium.launch({
				channel,
				headless: true,
				timeout: browserLaunchTimeoutMs,
			});
		} catch {
			// Try next locally installed Chromium channel.
		}
	}
	try {
		return await chromium.launch({
			headless: true,
			timeout: browserLaunchTimeoutMs,
		});
	} catch {
		return null;
	}
}

function contentType(path: string): string {
	switch (extname(path)) {
		case ".html":
			return "text/html; charset=utf-8";
		case ".js":
			return "text/javascript; charset=utf-8";
		case ".css":
			return "text/css; charset=utf-8";
		default:
			return "application/octet-stream";
	}
}

async function withStaticServer<T>(
	outDir: string,
	run: (baseUrl: string) => Promise<T>,
): Promise<T> {
	const server = Bun.serve({
		port: 0,
		fetch(request) {
			const url = new URL(request.url);
			const normalizedPath = url.pathname === "/" ? "/index.html" : url.pathname;
			const filePath = resolve(outDir, `.${decodeURIComponent(normalizedPath)}`);
			if (!filePath.startsWith(outDir)) {
				return new Response("Forbidden", { status: 403 });
			}
			const file = Bun.file(filePath);
			return new Response(file, {
				headers: { "content-type": contentType(filePath) },
			});
		},
	});

	try {
		return await run(`http://127.0.0.1:${server.port}`);
	} finally {
		server.stop(true);
	}
}

async function browserBenchmarks(
	framework: "react" | "svelte",
	outDir: string,
): Promise<{ results: TopologySpikeRunResult[]; errors: string[] }> {
	if (process.env.TOPOLOGY_SPIKE_SKIP_BROWSER === "1") {
		return {
			results: [],
			errors: ["Browser benchmarks skipped by TOPOLOGY_SPIKE_SKIP_BROWSER=1"],
		};
	}

	const browser = await launchBrowser();
	if (!browser) {
		return {
			results: [],
			errors: ["No local Chromium, Chrome, or Edge channel available"],
		};
	}

	return await withStaticServer(outDir, async (baseUrl) => {
		const page = await browser.newPage({ viewport: { width: 1440, height: 920 } });
		const results: TopologySpikeRunResult[] = [];
		const errors: string[] = [];
		page.on("pageerror", (error) => {
			errors.push(`${framework} page error: ${error.message}`);
		});
		page.on("console", (message) => {
			if (message.type() === "error") {
				errors.push(`${framework} console error: ${message.text()}`);
			}
		});
		try {
			for (const nodeCount of TOPOLOGY_SPIKE_NODE_COUNTS) {
				console.error(`Running ${framework} browser topology ${nodeCount} nodes...`);
				try {
					await page.goto(
						`${baseUrl}/${framework}.html?nodes=${nodeCount}&autorun=1`,
						{ waitUntil: "domcontentloaded", timeout: browserTimeoutMs },
					);
					await page.waitForFunction(() => window.__topologySpikeResult, null, {
						timeout: browserTimeoutMs,
					});
					results.push(
						await page.evaluate(
							() => window.__topologySpikeResult as TopologySpikeRunResult,
						),
					);
				} catch (error) {
					errors.push(
						`${framework} ${nodeCount} node browser benchmark failed: ${
							error instanceof Error ? error.message : String(error)
						}`,
					);
				}
			}
			return { results, errors };
		} finally {
			await page.close();
			await browser.close();
		}
	});
}

function percentDelta(before: number, after: number): number {
	if (before === 0) return 0;
	return Number((((after - before) / before) * 100).toFixed(2));
}

function formatMiB(bytes: number | null): string {
	if (bytes === null) return "n/a";
	return `${(bytes / 1024 / 1024).toFixed(2)} MiB`;
}

function markdownReport(
	pure: PureTopologySummary[],
	reactSummary: FrameworkSummary,
	svelteSummary: FrameworkSummary,
): string {
	const lines = [
		"# Svelte Topology Spike Results",
		"",
		`Generated: ${new Date().toISOString()}`,
		"",
		"## Bundle",
		"",
		"| Framework | Raw | Gzip | Largest asset |",
		"| --- | ---: | ---: | --- |",
		...([reactSummary, svelteSummary] as FrameworkSummary[]).map((summary) => {
			const largest = summary.assets.largest[0];
			return `| ${summary.framework} | ${formatMiB(summary.assets.rawBytes)} | ${formatMiB(summary.assets.gzipBytes)} | ${largest.path} (${formatMiB(largest.rawBytes)}) |`;
		}),
		"",
		"## Browser",
		"",
		"| Nodes | React total | Svelte total | Delta | React heap after | Svelte heap after |",
		"| ---: | ---: | ---: | ---: | ---: | ---: |",
	];

	for (const nodeCount of TOPOLOGY_SPIKE_NODE_COUNTS) {
		const reactResult = reactSummary.browser.find(
			(result) => result.nodeCount === nodeCount,
		);
		const svelteResult = svelteSummary.browser.find(
			(result) => result.nodeCount === nodeCount,
		);
		if (!reactResult || !svelteResult) {
			lines.push(`| ${nodeCount} | n/a | n/a | n/a | n/a | n/a |`);
			continue;
		}
		lines.push(
			`| ${nodeCount} | ${reactResult.totalInteractionMs.toFixed(2)} ms | ${svelteResult.totalInteractionMs.toFixed(2)} ms | ${percentDelta(reactResult.totalInteractionMs, svelteResult.totalInteractionMs)}% | ${formatMiB(reactResult.usedJsHeapAfterBytes)} | ${formatMiB(svelteResult.usedJsHeapAfterBytes)} |`,
		);
	}

	lines.push(
		"",
		"Browser errors:",
		"",
		...reactSummary.browserErrors.map((error) => `- React: ${error}`),
		...svelteSummary.browserErrors.map((error) => `- Svelte: ${error}`),
		...(reactSummary.browserErrors.length === 0 &&
		svelteSummary.browserErrors.length === 0
			? ["- None"]
			: []),
		"",
		"## Shared Layout",
		"",
		"| Nodes | Edges | Layout | Selections | RSS | Heap |",
		"| ---: | ---: | ---: | ---: | ---: | ---: |",
		...pure.map(
			(row) =>
				`| ${row.nodeCount} | ${row.edgeCount} | ${row.layoutMs.toFixed(2)} ms | ${row.selectionCount} | ${formatMiB(row.rssBytes)} | ${formatMiB(row.heapUsedBytes)} |`,
		),
		"",
		"## Recommendation",
		"",
		"Apply the 25% decision rule to the Browser section. If Svelte does not clear that bar, keep React and optimize current topology chunking/render paths first.",
	);

	return `${lines.join("\n")}\n`;
}

async function main() {
	mkdirSync(cacheDir, { recursive: true });
	const pure = pureTopologyBenchmarks();
	const reactOutDir = await buildHarness("react", [react()]);
	const svelteOutDir = await buildHarness("svelte", [svelte()]);
	const reactSummary: FrameworkSummary = {
		framework: "react",
		outDir: reactOutDir,
		assets: summarizeAssets(reactOutDir),
		browser: [],
		browserErrors: [],
	};
	const svelteSummary: FrameworkSummary = {
		framework: "svelte",
		outDir: svelteOutDir,
		assets: summarizeAssets(svelteOutDir),
		browser: [],
		browserErrors: [],
	};
	const reactBrowser = await browserBenchmarks("react", reactOutDir);
	reactSummary.browser = reactBrowser.results;
	reactSummary.browserErrors = reactBrowser.errors;
	const svelteBrowser = await browserBenchmarks("svelte", svelteOutDir);
	svelteSummary.browser = svelteBrowser.results;
	svelteSummary.browserErrors = svelteBrowser.errors;
	const report = markdownReport(pure, reactSummary, svelteSummary);

	console.log(
		JSON.stringify(
			{
				pure,
				react: reactSummary,
				svelte: svelteSummary,
				browserAutomation:
					reactSummary.browser.length > 0 && svelteSummary.browser.length > 0
						? "ok"
						: "partial or skipped; inspect browserErrors",
			},
			null,
			2,
		),
	);
	console.log("\n--- markdown ---\n");
	console.log(report);
}

await main();
