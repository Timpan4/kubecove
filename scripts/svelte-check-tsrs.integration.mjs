import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const launcher = fileURLToPath(new URL("./tsrs.ts", import.meta.url));
const modules = fileURLToPath(new URL("../node_modules", import.meta.url));

// Run with Node's test runner. Bun's default test timeout also includes source builds.
// Failure modes: native compiler discovery rejects tsrs, mapped type/prop errors
// disappear or point into generated files, Svelte/CSS warnings vanish, or a failed
// compiler process is reported as a successful check.
test("tsrs checks Svelte components and preserves mapped diagnostics", () => {
	const workspace = mkdtempSync(join(tmpdir(), "kubecove-svelte-tsrs-"));
	const config = join(workspace, "tsconfig.json");
	const app = join(workspace, "App.svelte");
	const run = (env = {}) => {
		const result = spawnSync(
			"bun",
			[
				launcher,
				"--svelte",
				"--workspace",
				workspace,
				"--tsconfig",
				config,
				"--tsgo",
				"--output",
				"machine-verbose",
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

		const failed = run({ TSRS_BINARY: join(workspace, "missing-tsrs") });
		assert.notEqual(failed.status, 0, failed.output);
		assert.match(failed.output, /TSRS_BINARY.*does not exist/, failed.output);
		assert.doesNotMatch(failed.output, /COMPLETED/);
	} finally {
		rmSync(workspace, { recursive: true, force: true });
	}
});
