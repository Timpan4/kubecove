import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { delimiter, join, resolve } from "node:path";
import { parityRebuildReason } from "./gpui-run";

const fingerprint = 'INFO prepare_target{target="parity"}: cargo::compiler::fingerprint:';

test("reports the parity target's own dirty fingerprint", () => {
	const dependency = 'INFO prepare_target{target="gpui"}: cargo::compiler::fingerprint: dirty: dependency source changed';
	const parity = `${fingerprint}     dirty: the file tests/parity.rs has changed`;
	expect(parityRebuildReason(`${dependency}\n${parity}`)).toBe("the file tests/parity.rs has changed");
	expect(parityRebuildReason(dependency)).toBeUndefined();
});

test("reports the fingerprint read cause as Cargo logged it", () => {
	const failed = `${fingerprint}     err: failed to read fingerprint/test-integration-test-parity`;
	const unix = "No such file or directory (os error 2)";
	const windows = "The system cannot find the path specified. (os error 3)";
	expect(parityRebuildReason(`${failed}\n\nCaused by:\n    ${unix}`)).toBe(`Cargo could not read the parity fingerprint: ${unix}`);
	expect(parityRebuildReason(`${failed}\r\n\r\nCaused by:\r\n    ${windows}`)).toBe(`Cargo could not read the parity fingerprint: ${windows}`);
	expect(parityRebuildReason(failed)).toBeUndefined();
});

test.skipIf(process.platform === "win32")("GPUI shard exits determine which scenarios are retried", async () => {
	const directory = await mkdtemp(join(import.meta.dir, ".gpui-exit-test-"));
	const executable = join(directory, "parity");
	try {
		await Promise.all([
			writeFile(join(directory, "cargo"), `#!/usr/bin/env bun
console.log(JSON.stringify({
	reason: "compiler-artifact", target: { name: "parity" }, executable: process.env.TEST_PARITY, fresh: true,
}));
`, { mode: 0o755 }),
			writeFile(executable, `#!/usr/bin/env bun
const scenarios = [
	{ id: "one", status: "passed" },
	{ id: "two", status: process.env.TEST_EXIT === "all-passed" ? "passed" : "failed" },
];
await Bun.write(process.env.KUBECOVE_E2E_ARTIFACTS + "/gpui-0.json", JSON.stringify({ scenarios }));
if (process.env.TEST_EXIT === "killed") process.kill(process.pid, "SIGKILL");
process.exit(101);
`, { mode: 0o755 }),
		]);
		for (const { exit, driverFailed, ids } of [
			{ exit: "scenario-failed", driverFailed: false, ids: ["two"] },
			{ exit: "all-passed", driverFailed: true, ids: ["one", "two"] },
			{ exit: "killed", driverFailed: true, ids: ["one", "two"] },
		]) {
			const artifacts = join(directory, exit);
			await mkdir(artifacts);
			const child = Bun.spawn([process.execPath, "-e", `
import { join } from "node:path";
import { runGpui } from "./e2e/parity/gpui-run";
import { rerunSelections } from "./e2e/parity/rerun";

const artifacts = process.env.TEST_ARTIFACTS;
const scenarios = ["one", "two"].map((id) => ({ id, title: id, steps: [] }));
await Bun.write(join(artifacts, "scenarios.txt"), "one\\ntwo");
const problem = await runGpui({ scenarios }, artifacts, { path: "", exit: Promise.resolve(0) }, 1, new AbortController().signal, () => {});
await Bun.write(join(artifacts, "result.json"), JSON.stringify({
	driver: "gpui", status: "failed", failedDrivers: problem ? ["gpui"] : [],
}));
const selected = await rerunSelections(artifacts);
console.log(JSON.stringify({ driverFailed: Boolean(problem), ids: [...selected.gpui] }));
`], {
				cwd: resolve(import.meta.dir, "../.."),
				env: {
					...process.env,
					PATH: [directory, process.env.PATH].join(delimiter),
					TEST_ARTIFACTS: artifacts,
					TEST_EXIT: exit,
					TEST_PARITY: executable,
				},
				stdout: "pipe",
				stderr: "pipe",
			});
			const [code, stdout, stderr] = await Promise.all([
				child.exited,
				new Response(child.stdout).text(),
				new Response(child.stderr).text(),
			]);
			if (code !== 0) throw new Error(`GPUI exit probe "${exit}" failed: ${stderr}`);
			expect(stdout.trimEnd().split("\n").at(-1)).toBe(JSON.stringify({ driverFailed, ids }));
		}
	} finally {
		await rm(directory, { recursive: true });
	}
});
