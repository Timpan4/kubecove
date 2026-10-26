import { realpathSync, statSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs } from "node:util";
import { parse } from "svelte/compiler";
import {
	getCSSLanguageService,
	newCSSDataProvider,
	TextDocument,
} from "vscode-css-languageservice";

const require = createRequire(import.meta.url);
const css = getCSSLanguageService({
	customDataProviders: [
		newCSSDataProvider({
			version: 1.1,
			pseudoClasses: [{ name: ":global()" }],
		}),
	],
});

async function capture(command) {
	const child = Bun.spawn(command, { stdout: "pipe", stderr: "inherit" });
	const interrupt = () => child.kill("SIGINT");
	const terminate = () => child.kill("SIGTERM");
	process.on("SIGINT", interrupt);
	process.on("SIGTERM", terminate);
	try {
		const [status, output] = await Promise.all([
			child.exited,
			new Response(child.stdout).text(),
		]);
		return { status, output };
	} finally {
		process.off("SIGINT", interrupt);
		process.off("SIGTERM", terminate);
	}
}

async function cssDiagnostics(workspace, records) {
	const modules = join(workspace, "node_modules");
	const cache = statSync(modules, { throwIfNoEntry: false })?.isDirectory()
		? join(modules, ".cache", "svelte-check-native")
		: join(workspace, ".svelte-check");
	const program = await capture([
		process.env.TSGO_BIN,
		"--project",
		join(cache, "tsconfig.json"),
		"--listFilesOnly",
	]);
	if (program.status !== 0)
		throw new Error(
			`Cannot read the Svelte compiler file graph:\n${program.output}`,
		);
	const files = new Set();
	const generated = join(realpathSync(cache), "svelte");
	// Include declaration wrappers: scriptless components need CSS checking too.
	for (const emitted of program.output.split(/\r?\n/)) {
		const script = /\.svelte\.svn\.(ts|js)$/.test(emitted);
		const declaration = emitted.endsWith(".d.svelte.ts");
		if (!script && !declaration) continue;
		// The compiler resolves Windows short paths and workspace symlinks.
		const path = relative(generated, realpathSync(emitted));
		if (isAbsolute(path) || path === ".." || path.startsWith(`..${sep}`))
			continue;
		if (script) files.add(path.replace(/\.svn\.(ts|js)$/, ""));
		else files.add(`${path.slice(0, -".d.svelte.ts".length)}.svelte`);
	}
	const diagnostics = [];
	for (const file of files) {
		const filename = file.split(sep).join("/");
		const source = await Bun.file(resolve(workspace, filename)).text();
		let ast;
		try {
			ast = parse(source, { filename, modern: true });
		} catch (error) {
			// The compiler bridge already reports invalid Svelte syntax.
			if (
				records.some(
					(record) => record.filename === filename && record.type === "ERROR",
				)
			)
				continue;
			throw error;
		}
		if (!ast.css) continue;
		const { start, end } = ast.css.content;
		// Preserve UTF-16 offsets and line endings while hiding the surrounding Svelte.
		const content =
			source.slice(0, start).replace(/[^\r\n]/g, " ") +
			source.slice(start, end);
		const document = TextDocument.create(
			pathToFileURL(resolve(workspace, filename)).href,
			"css",
			0,
			content,
		);
		for (const diagnostic of css.doValidation(
			document,
			css.parseStylesheet(document),
		)) {
			diagnostics.push({
				type: diagnostic.severity === 1 ? "ERROR" : "WARNING",
				filename,
				start: diagnostic.range.start,
				end: diagnostic.range.end,
				message: diagnostic.message,
				code: diagnostic.code,
				source: "css",
			});
		}
	}
	return diagnostics;
}

async function main() {
	const { values } = parseArgs({
		args: process.argv.slice(2),
		options: {
			workspace: { type: "string", default: process.cwd() },
			tsconfig: { type: "string", default: "tsconfig.svelte.json" },
			output: { type: "string", default: "human-verbose" },
			"fail-on-warnings": { type: "boolean", default: false },
		},
	});
	if (!["human-verbose", "machine-verbose"].includes(values.output)) {
		throw new Error(
			"Supported output formats: human-verbose, machine-verbose.",
		);
	}
	const workspace = realpathSync(resolve(values.workspace));
	const entry = join(
		dirname(require.resolve("svelte-check-native/package.json")),
		"bin",
		"svelte-check-native.js",
	);
	const { status, output } = await capture([
		process.execPath,
		entry,
		"--workspace",
		workspace,
		"--tsconfig",
		resolve(workspace, values.tsconfig),
		"--output",
		"machine-verbose",
		"--svelte-warnings",
		"bridge",
	]);
	const completed = output.match(/COMPLETED (\d+) FILES/);
	if (!completed) {
		process.stdout.write(output);
		if (status === 0)
			throw new Error("svelte-check-native exited without a check result.");
		return status;
	}
	const records = output.split(/\r?\n/).flatMap((line) => {
		const match = line.match(/^\d+ (\{.*\})$/);
		return match ? [JSON.parse(match[1])] : [];
	});
	records.push(...(await cssDiagnostics(workspace, records)));
	const errors = records.filter((record) => record.type === "ERROR").length;
	const warnings = records.filter((record) => record.type === "WARNING").length;
	if (values.output === "machine-verbose") {
		console.log(`${Date.now()} START ${JSON.stringify(workspace)}`);
		for (const record of records)
			console.log(`${Date.now()} ${JSON.stringify(record)}`);
		const files = new Set(records.map((record) => record.filename)).size;
		console.log(
			`${Date.now()} COMPLETED ${completed[1]} FILES ${errors} ERRORS ${warnings} WARNINGS ${files} FILES_WITH_PROBLEMS`,
		);
	} else {
		for (const record of records) {
			console.log(
				`${record.filename}:${record.start.line + 1}:${record.start.character + 1}\n${record.type}: ${record.message} (${record.source})\n`,
			);
		}
		console.log(
			`Svelte checks found ${errors} errors and ${warnings} warnings`,
		);
	}
	return (
		status ||
		(errors > 0 || (values["fail-on-warnings"] && warnings > 0) ? 1 : 0)
	);
}

try {
	process.exitCode = await main();
} catch (error) {
	console.error(error instanceof Error ? error.message : error);
	process.exitCode = 1;
}
