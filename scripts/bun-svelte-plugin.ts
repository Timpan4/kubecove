import type { BunPlugin } from "bun";
import { compile, compileModule, type Warning } from "svelte/compiler";

function forwardWarnings(path: string, warnings: Warning[]): void {
	if (path.includes("node_modules")) return;
	for (const warning of warnings) {
		const position = warning.start
			? `:${warning.start.line}:${warning.start.column}`
			: "";
		console.warn(`${path}${position}: ${warning.message} (${warning.code})`);
	}
}

function browserGlobals(code: string): string {
	// ponytail: Bun 1.4 snapshots these Svelte re-exports before DOM init.
	// Remove when Bun preserves their live bindings through a browser bundle.
	return code
		.replaceAll("$.window", "globalThis.window")
		.replaceAll("$.document", "globalThis.document");
}

export function createSveltePlugin(dev: boolean): BunPlugin {
	const transpiler = new Bun.Transpiler({ loader: "ts", target: "browser" });
	return {
		name: "kubecove-svelte",
		setup(builder) {
			builder.onLoad({ filter: /\.svelte$/ }, async ({ path }) => {
				const result = compile(await Bun.file(path).text(), {
					filename: path,
					generate: "client",
					css: "injected",
					dev,
					hmr: false,
				});
				forwardWarnings(path, result.warnings);
				return { contents: browserGlobals(result.js.code), loader: "js" };
			});

			builder.onLoad({ filter: /\.svelte\.(js|ts)$/ }, async ({ path }) => {
				const source = await Bun.file(path).text();
				const javascript = path.endsWith(".ts")
					? await transpiler.transform(source)
					: source;
				const result = compileModule(javascript, {
					filename: path,
					generate: "client",
					dev,
				});
				forwardWarnings(path, result.warnings);
				return { contents: browserGlobals(result.js.code), loader: "js" };
			});
		},
	};
}

export default createSveltePlugin(true);
