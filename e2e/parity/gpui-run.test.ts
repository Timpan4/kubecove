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

// Export must reject cross-target config even when Cargo emits a usable host artifact first.
test.skipIf(process.platform === "win32")("GPUI export only publishes host artifacts without passing --target", async () => {
	await mkdir("e2e/artifacts", { recursive: true });
	const directory = await mkdtemp(resolve("e2e/artifacts/gpui-export-test-"));
	const host = "x86_64-unknown-linux-gnu";
	const cross = "aarch64-unknown-linux-gnu";
	try {
		await mkdir(join(directory, "rust", "gpui"), { recursive: true });
		await Promise.all([
			writeFile(join(directory, "git"), `#!/bin/sh
if [ "$1" = rev-parse ]; then echo ${"a".repeat(40)}; fi
`, { mode: 0o755 }),
			writeFile(join(directory, "rustc"), `#!/bin/sh
echo "host: ${host}"
`, { mode: 0o755 }),
			writeFile(join(directory, "cargo"), `#!/usr/bin/env bun
import { join } from "node:path";
if (process.argv.includes("--target")) throw new Error("Explicit target passed to Cargo");
for (const target of process.env.TEST_TARGETS.split(",")) {
	const intermediate = process.env.TEST_LAYOUT === "build";
	const root = intermediate ? process.env.CARGO_BUILD_BUILD_DIR : process.env.CARGO_TARGET_DIR;
	const executable = join(root, target, process.env.TEST_PROFILE, ...(intermediate ? ["build", "kubecove-gpui", "hash", "out"] : ["deps"]), "parity-test");
	await Bun.write(executable, "parity binary");
	console.log(JSON.stringify({ reason: "compiler-artifact", target: { name: "parity" }, executable, features: [] }));
}
`, { mode: 0o755 }),
		]);
		const module = JSON.stringify(join(import.meta.dir, "gpui-run.ts"));
		for (const [mode, layout, targets, envTarget, profile, error] of [
			["native", "deps", "", "", "debug", ""],
			["host-config", "deps", host, host, "debug", ""],
			["env-cross", "deps", "", cross, "debug", "CARGO_BUILD_TARGET"],
			["cross-artifact", "deps", `,${cross}`, "", "debug", "artifact path"],
			["build-native", "build", "", "", "debug", ""],
			["build-host-config", "build", host, host, "debug", ""],
			["build-cross-artifact", "build", `,${cross}`, "", "debug", "artifact path"],
			["build-outside-root", "build", "../other", "", "debug", "artifact path"],
			["build-wrong-profile", "build", "", "", "release", "artifact path"],
		]) {
			const output = join(directory, mode);
			const child = Bun.spawn([process.execPath, "-e", `
import { exportGpuiParity } from ${module};
await exportGpuiParity(process.env.TEST_OUTPUT, false, new AbortController().signal);
`], {
				cwd: directory,
				env: {
					...process.env, PATH: [directory, process.env.PATH].join(delimiter), RUSTC: join(directory, "rustc"),
					CARGO_BUILD_TARGET: envTarget || undefined, CARGO_TARGET_DIR: join(directory, "target"),
					CARGO_BUILD_BUILD_DIR: join(directory, "build"), TEST_LAYOUT: layout, TEST_PROFILE: profile,
					TEST_TARGETS: targets, TEST_OUTPUT: output,
				},
				stdout: "pipe",
				stderr: "pipe",
			});
			const [code, stderr] = await Promise.all([child.exited, new Response(child.stderr).text()]);
			const manifest = Bun.file(join(output, "manifest.json"));
			if (error) {
				expect(code).not.toBe(0);
				expect(stderr).toContain(error);
				expect(await manifest.exists()).toBe(false);
			} else {
				if (code !== 0) throw new Error(`GPUI export failed: ${stderr}`);
				expect((await manifest.json()).target).toBe(host);
			}
		}
	} finally {
		await rm(directory, { recursive: true, force: true });
	}
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
const ids = (await Bun.file(process.env.KUBECOVE_PARITY_SCENARIO_IDS).text()).split("\\n").filter(Boolean);
const [shard] = process.env.KUBECOVE_PARITY_SHARD.split("/");
const artifacts = process.env.KUBECOVE_E2E_ARTIFACTS;
const reportPath = artifacts + "/gpui-" + shard + ".json";
const mode = process.env.TEST_EXIT;
const claim = (position) => Bun.write(artifacts + "/gpui-claims/" + position, shard);
if (mode === "malformed-duplicate" || mode === "misplaced") {
	await Promise.all((shard === "0" ? [0, 2] : [1]).map(claim));
	const first = mode === "misplaced" ? { id: ids[1], status: "failed" } : { id: ids[0], status: "passed" };
	const scenarios = shard === "0"
		? [first, { id: ids[2], status: "passed" }]
		: [{ id: ids[0], status: "passed" }, ...(mode === "misplaced" ? [] : [{ id: ids[1], status: "invalid" }])];
	await Bun.write(reportPath, JSON.stringify({ driver: "gpui", scenarios }));
	process.exit((mode === "misplaced") === (shard === "0") ? 101 : 0);
}
await Promise.all(ids.flatMap((_, position) => mode === "missing-claim" && position === 0 ? [] : [claim(position)]));
const scenarios = ids.map((id) => ({ id, status: "passed" }));
if (process.env.TEST_EXIT !== "all-passed") scenarios[1].status = "failed";
if (process.env.TEST_EXIT === "invalid-status") scenarios[0].status = "invalid";
if (process.env.TEST_EXIT === "incomplete") scenarios.pop();
if (process.env.TEST_EXIT === "duplicate") scenarios.push({ id: ids[0], status: "failed" });
const driver = process.env.TEST_EXIT === "wrong-driver" ? "web" : "gpui";
await Bun.write(reportPath, JSON.stringify({ driver, scenarios }));
if (process.env.TEST_EXIT === "killed") process.kill(process.pid, "SIGKILL");
process.exit(101);
`, { mode: 0o755 }),
		]);
		for (const { exit, driverFailed, ids, passing } of [
			{ exit: "scenario-failed", driverFailed: false, ids: ["two"], passing: ["one", "three"] },
			{ exit: "all-passed", driverFailed: true, ids: ["one", "two", "three"], passing: ["one", "two", "three"] },
			{ exit: "killed", driverFailed: true, ids: ["one", "two", "three"], passing: ["one", "three"] },
			{ exit: "wrong-driver", driverFailed: true, ids: ["one", "two", "three"], passing: [] },
			{ exit: "invalid-status", driverFailed: true, ids: ["one", "two", "three"], passing: ["three"] },
			{ exit: "incomplete", driverFailed: true, ids: ["one", "two", "three"], passing: ["one"] },
			{ exit: "duplicate", driverFailed: true, ids: ["one", "two", "three"], passing: ["three"] },
			{ exit: "malformed-duplicate", driverFailed: true, ids: ["one", "two", "three"], passing: ["three"] },
			{ exit: "misplaced", driverFailed: true, ids: ["one", "two", "three"], passing: ["three"] },
			{ exit: "missing-claim", driverFailed: true, ids: ["one", "two", "three"], passing: ["three"] },
		]) {
			const artifacts = join(directory, exit);
			await mkdir(artifacts);
			const child = Bun.spawn([process.execPath, "-e", `
import { join } from "node:path";
import { runGpui } from "./e2e/parity/gpui-run";
import { inspectDriverResults } from "./e2e/parity/merge-reports";
import { DRIVER_SELECTION_FORMAT, rerunSelections, selectionHash } from "./e2e/parity/rerun";

const artifacts = process.env.TEST_ARTIFACTS;
const scenarios = ["one", "two", "three"].map((id) => ({ id, title: id, steps: [] }));
const gpui = scenarios.map(({ id }) => id).join("\\n");
await Bun.write(join(artifacts, "gpui-scenarios.txt"), gpui);
await Bun.write(join(artifacts, "web-scenarios.txt"), "");
const shards = ["malformed-duplicate", "misplaced"].includes(process.env.TEST_EXIT) ? 2 : 1;
const problem = await runGpui({ scenarios }, artifacts, { path: "", exit: Promise.resolve(0) }, shards, new AbortController().signal, () => {});
await Bun.write(join(artifacts, "result.json"), JSON.stringify({
	driver: "gpui", status: "failed", failedDrivers: problem ? ["gpui"] : [],
	selectionFormat: DRIVER_SELECTION_FORMAT,
	selectionHashes: { web: selectionHash(""), gpui: selectionHash(gpui) },
}));
const selected = await rerunSelections(artifacts);
const report = await Bun.file(join(artifacts, "gpui.json")).json();
const passing = inspectDriverResults({ scenarios }, "gpui", report).results.filter(({ status }) => status === "passed").map(({ id }) => id);
console.log(JSON.stringify({ driverFailed: Boolean(problem), ids: [...selected.gpui], passing }));
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
			expect(stdout.trimEnd().split("\n").at(-1)).toBe(JSON.stringify({ driverFailed, ids, passing }));
		}
	} finally {
		await rm(directory, { recursive: true });
	}
});
