import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
	existsSync,
	mkdtempSync,
	readFileSync,
	realpathSync,
	rmSync,
	symlinkSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const launcher = fileURLToPath(new URL("./tsrs.ts", import.meta.url));
const modules = fileURLToPath(new URL("../node_modules", import.meta.url));

// Run with Node's test runner. Bun's default test timeout also includes source builds.
// Failure modes: native compiler discovery rejects tsrs, mapped type/prop errors
// disappear or point into generated files, Svelte/CSS warnings vanish, or a failed
// compiler process is reported as a successful check, or deleted components
// remain in the generated compiler inputs, or path aliases hide CSS diagnostics.
function checkSvelteDiagnostics(alias) {
	const directory = mkdtempSync(join(tmpdir(), "kubecove-svelte-tsrs-"));
	const workspace = alias ? join(directory, "workspace-alias") : directory;
	const config = join(workspace, "tsconfig.json");
	const app = join(workspace, "App.svelte");
	const run = (env = {}, args = []) => {
		const result = spawnSync(
			"bun",
			[
				launcher,
				"--svelte",
				"--workspace",
				workspace,
				"--tsconfig",
				config,
				"--output",
				"machine-verbose",
				...args,
			],
			{
				encoding: "utf8",
				env: { ...process.env, ...env },
			},
		);
		assert.ifError(result.error);
		return { status: result.status, output: result.stdout + result.stderr };
	};
	try {
		if (alias)
			symlinkSync(
				directory,
				workspace,
				process.platform === "win32" ? "junction" : "dir",
			);
		symlinkSync(
			modules,
			join(workspace, "node_modules"),
			process.platform === "win32" ? "junction" : "dir",
		);
		writeFileSync(
			join(workspace, "svelte.config.cjs"),
			"module.exports = {};\n",
		);
		writeFileSync(
			config,
			JSON.stringify({
				compilerOptions: {
					strict: true,
					target: "ES2020",
					module: "ESNext",
					moduleResolution: "bundler",
					skipLibCheck: true,
					noEmit: true,
					types: [],
				},
				include: ["*.svelte"],
				exclude: ["Ignored.svelte"],
			}),
		);
		writeFileSync(
			join(workspace, "Child.svelte"),
			'<script lang="ts">\nlet { count }: { count: number } = $props();\n</script>\n<p>{count}</p>\n',
		);
		writeFileSync(
			app,
			'<script lang="ts">\nimport Child from "./Child.svelte";\nconst count: number = 1;\n</script>\n<Child {count} />\n',
		);
		const valid = run();
		assert.equal(valid.status, 0, valid.output);
		assert.match(valid.output, /COMPLETED 2 FILES 0 ERRORS 0 WARNINGS/);

		writeFileSync(
			app,
			'<script lang="ts">\nimport Child from "./Child.svelte";\nconst count: string = 1;\n</script>\n<Child count={count} />\n<img src="/example.png" />\n<p>Example</p>\n<style>p { colour: red; }</style>\n',
		);
		const invalid = run();
		assert.equal(invalid.status, 1, invalid.output);
		const records = invalid.output.split(/\r?\n/).flatMap((line) => {
			const start = line.indexOf("{");
			return start === -1 ? [] : [JSON.parse(line.slice(start))];
		});
		const errors = records.filter(
			(record) => record.type === "ERROR" && record.code === 2322,
		);
		assert.deepEqual(
			errors.map((record) => record.start.line).sort((a, b) => a - b),
			[2, 4],
			invalid.output,
		);
		assert.ok(
			errors.every((record) => record.filename === "App.svelte"),
			invalid.output,
		);
		assert.ok(
			records.some(
				(record) =>
					record.type === "WARNING" && record.code === "a11y_missing_attribute",
			),
			invalid.output,
		);
		assert.ok(
			records.some(
				(record) => record.type === "WARNING" && record.source === "css",
			),
			invalid.output,
		);
		const css = records.find((record) => record.code === "unknownProperties");
		assert.deepEqual(css.start, { line: 7, character: 11 }, invalid.output);
		assert.deepEqual(css.end, { line: 7, character: 17 }, invalid.output);

		writeFileSync(
			join(workspace, "model.ts"),
			"export interface RemovedModel { count: number }\n",
		);
		writeFileSync(
			join(workspace, "Child.svelte"),
			'<script lang="ts">\nimport type { RemovedModel } from "./model";\nlet { count }: RemovedModel = $props();\n</script>\n<p>{count}</p>\n',
		);
		writeFileSync(
			app,
			'<script lang="ts">\nimport Child from "./Child.svelte";\n</script>\n<Child count={1} />\n',
		);
		const beforeDeletion = run();
		assert.equal(beforeDeletion.status, 0, beforeDeletion.output);
		rmSync(join(workspace, "Child.svelte"));
		rmSync(join(workspace, "model.ts"));
		writeFileSync(app, "<p>After deletion</p>\n");
		const afterDeletion = run();
		assert.equal(afterDeletion.status, 0, afterDeletion.output);
		assert.match(afterDeletion.output, /COMPLETED 1 FILES 0 ERRORS 0 WARNINGS/);

		writeFileSync(
			app,
			"<p>Example</p>\n<style>\n.unused { colour: red; }\n</style>\n",
		);
		const warnings = run({}, ["--fail-on-warnings"]);
		assert.equal(warnings.status, 1, warnings.output);
		assert.match(warnings.output, /"code":"css_unused_selector"/);
		assert.match(warnings.output, /"code":"unknownProperties"/);
		assert.match(warnings.output, /COMPLETED 1 FILES 0 ERRORS 2 WARNINGS/);

		writeFileSync(
			join(workspace, "Ignored.svelte"),
			'<script lang="ts">const ignored: number = "bad";</script>\n<p>{ignored}</p>\n<style>p { colour: red; }</style>\n',
		);
		writeFileSync(
			app,
			"<p>🌊</p>\r\n<style>\r\np { colour: red; }\r\n</style>\r\n",
		);
		const mappedCss = run({}, ["--fail-on-warnings"]);
		assert.equal(mappedCss.status, 1, mappedCss.output);
		const cssRecord = mappedCss.output
			.split(/\r?\n/)
			.find((line) => line.includes('"code":"unknownProperties"'));
		const mapped = JSON.parse(cssRecord.slice(cssRecord.indexOf("{")));
		assert.equal(mapped.filename, "App.svelte", mappedCss.output);
		assert.deepEqual(mapped.start, { line: 2, character: 4 }, mappedCss.output);
		assert.deepEqual(mapped.end, { line: 2, character: 10 }, mappedCss.output);
		assert.match(mappedCss.output, /0 ERRORS 1 WARNINGS/);
		assert.doesNotMatch(mappedCss.output, /Ignored\.svelte/);

		const missingBinary = join(workspace, "missing-tsrs-sensitive-marker");
		const failed = run({ TSRS_BINARY: missingBinary });
		assert.notEqual(failed.status, 0, failed.output);
		assert.match(failed.output, /TSRS_BINARY.*does not exist/, failed.output);
		assert.ok(!failed.output.includes(missingBinary), failed.output);
		assert.doesNotMatch(failed.output, /COMPLETED/);
	} catch (error) {
		const cache = join(
			workspace,
			"node_modules",
			".cache",
			"svelte-check-native",
		);
		const overlay = join(cache, "tsconfig.json");
		if (existsSync(overlay)) {
			const graph = spawnSync(
				"bun",
				[launcher, "--project", overlay, "--listFilesOnly"],
				{
					encoding: "utf8",
					env: process.env,
				},
			);
			console.error(
				"Svelte compiler failure evidence:",
				JSON.stringify({
					workspace,
					canonicalWorkspace: realpathSync(workspace),
					canonicalCache: realpathSync(cache),
					config: JSON.parse(readFileSync(overlay, "utf8")),
					graphStatus: graph.status,
					graph: graph.stdout,
					graphErrors: graph.stderr,
				}),
			);
		}
		throw error;
	} finally {
		rmSync(directory, { recursive: true, force: true });
	}
}

test("tsrs preserves mapped Svelte diagnostics", () =>
	checkSvelteDiagnostics(false));
test("tsrs preserves mapped Svelte diagnostics through a workspace alias", () =>
	checkSvelteDiagnostics(true));
