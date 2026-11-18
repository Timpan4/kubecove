import { readFile, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { startIdentity, stopGroup } from "../../.agents/skills/verify-kubecove/scripts/processes";
import type { Contract } from "./contract";
import { COVERAGE_RUSTFLAGS, recordGpuiCoverage } from "./gpui-coverage-map";
import { driverSelectionFile } from "./rerun";

type Spawned = { code: number; output: string; signaled: boolean };

export function parityRebuildReason(output: string): string | undefined {
	const dirty = output.match(/target="parity"[^\n]*cargo::compiler::fingerprint:\s+dirty: (.+)/)?.[1];
	if (dirty) return dirty.trim();
	const cause = output.match(/target="parity"[^\n]*cargo::compiler::fingerprint:\s+err: failed to read [^\n]+\n\s*Caused by:[ \t]*\r?\n[ \t]*([^\r\n]+)/)?.[1];
	return cause ? `Cargo could not read the parity fingerprint: ${cause.trim()}` : undefined;
}

/** Runs a child in its own process group and stops the group when `signal` aborts. */
async function spawnGroup(command: string[], env: Record<string, string | undefined>, signal: AbortSignal): Promise<Spawned> {
	// An abort listener added after the abort never fires, so an earlier abort stops here.
	signal.throwIfAborted();
	const niced = process.platform === "win32" ? command : ["nice", ...command];
	const child = Bun.spawn(niced, { env, detached: true, stdout: "pipe", stderr: "pipe" });
	const identity = startIdentity(child.pid);
	const output = Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text()]);
	const cancel = () => { void stopGroup(child.pid, identity); };
	signal.addEventListener("abort", cancel, { once: true });
	try {
		const code = await child.exited;
		return { code, output: (await output).join("\n"), signaled: child.signalCode !== null };
	} finally { signal.removeEventListener("abort", cancel); }
}

/** Builds the parity test binary once and returns its path from cargo's JSON messages. */
async function buildParity(workspace: string, env: Record<string, string | undefined>, artifacts: string, signal: AbortSignal): Promise<string> {
	console.log("Checking the GPUI parity executable with Cargo.");
	const build = await spawnGroup(
		["cargo", "test", "--manifest-path", join(workspace, "Cargo.toml"), "--test", "parity", "--no-run", "--message-format=json-render-diagnostics"],
		{ ...env, CARGO_LOG: env.CARGO_LOG ?? "cargo::compiler::fingerprint=info" },
		signal,
	);
	await writeFile(join(artifacts, "gpui-build.log"), build.output);
	if (build.code !== 0) throw new Error(`GPUI parity build exited ${build.code}`);
	for (const line of build.output.split("\n")) {
		if (!line.startsWith("{")) continue;
		// SAFETY: cargo writes one JSON object per line under --message-format=json. Every field read here is optional in cargo's schema, and each read is a comparison.
		const message = JSON.parse(line) as { reason?: string; executable?: string | null; target?: { name?: string }; fresh?: boolean };
		if (message.reason !== "compiler-artifact" || message.target?.name !== "parity" || !message.executable) continue;
		if (message.fresh) console.log("Reusing the GPUI parity executable. Cargo found its inputs unchanged.");
		else {
			const reason = parityRebuildReason(build.output);
			console.log(reason ? `GPUI parity executable rebuilt: ${reason}` : "GPUI parity executable rebuilt. See gpui-build.log for Cargo diagnostics.");
		}
		return message.executable;
	}
	throw new Error("cargo did not report the parity test executable");
}

/**
 * Runs the selected scenarios in `shards` GPUI test processes. The processes take scenarios
 * from a shared queue (`claim` in tests/parity/selection.rs) and each writes its own report;
 * this merges them into `gpui.json` in contract order. With `coverage`, the binary is
 * instrumented in its own `<target>/coverage` directory, so it never evicts the normal build.
 */
export async function runGpui(contract: Contract, artifacts: string, fixture: { path: string; exit: Promise<number> }, shards: number, signal: AbortSignal, mark: (phase: string) => void, coverage = false): Promise<string | undefined> {
	const workspace = resolve("src-tauri/gpui");
	// The runner records this driver's selection separately from the web driver's.
	const ids = join(artifacts, driverSelectionFile.gpui);
	// Keep path-crate artifacts specific to this worktree, including environment overrides.
	const env = Object.fromEntries(Object.entries(process.env).filter(([name]) => !["ARGV0", "KUBECONFIG", "KUBECOVE_PARITY_SCENARIO_PREFIX"].includes(name)));
	Object.assign(env, { CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? join(workspace, "target"), CARGO_BUILD_BUILD_DIR: process.env.CARGO_BUILD_BUILD_DIR ?? join(workspace, "build"), KUBECOVE_E2E_ARTIFACTS: artifacts, KUBECOVE_PARITY_FIXTURE: fixture.path, KUBECOVE_PARITY_SCENARIO_IDS: ids });
	const strayProfiles = join(env.CARGO_TARGET_DIR ?? "", "coverage", "profraw");
	if (coverage) {
		const target = join(env.CARGO_TARGET_DIR ?? "", "coverage");
		// RUSTFLAGS replaces `build.rustflags` from cargo config; this repository sets none.
		// Instrumented build scripts and proc macros write profiles into their working
		// directory, and the test processes write one at exit; all go to `strayProfiles`.
		Object.assign(env, { CARGO_TARGET_DIR: target, CARGO_BUILD_BUILD_DIR: target, RUSTFLAGS: `${env.RUSTFLAGS ?? ""} ${COVERAGE_RUSTFLAGS}`.trim(), LLVM_PROFILE_FILE: join(strayProfiles, "%p-%m.profraw") });
	}
	const executable = await buildParity(workspace, env, artifacts, signal);
	mark("gpuiBuild");
	if (await fixture.exit !== 0) throw new Error("Fixture export failed");
	const count = Math.max(1, Math.min(shards, contract.scenarios.length));
	// The GPUI test platform renders without a display.
	const runs = await Promise.all(Array.from({ length: count }, async (_, index) => {
		const run = await spawnGroup([executable, "--nocapture"], { ...env, KUBECOVE_PARITY_SHARD: `${index}/${count}` }, signal);
		mark(`gpuiShard${index}`);
		await writeFile(join(artifacts, `gpui-${index}.log`), run.output);
		return run;
	}));
	await writeFile(join(artifacts, "gpui.log"), runs.map((run, index) => `== shard ${index}/${count} exited ${run.code}\n${run.output}`).join("\n"));
	const results = new Map<string, unknown>();
	const problems: string[] = [];
	for (const [index, run] of runs.entries()) {
		try {
			// SAFETY: the parity test binary writes each shard report as { scenarios }. A report without that array throws in the loop below, and the catch records it as missing.
			const report = JSON.parse(await readFile(join(artifacts, `gpui-${index}.json`), "utf8")) as { scenarios: { id: string; status: string }[] };
			for (const result of report.scenarios) results.set(result.id, result);
			// The native harness writes the report, then fails its test if a scenario failed.
			if (run.signaled || (run.code !== 0 && !report.scenarios.some(({ status }) => status === "failed"))) {
				problems.push(`GPUI shard ${index}/${count} exited ${run.code}`);
			}
		} catch { problems.push(`GPUI shard ${index}/${count} wrote no report`); }
	}
	const scenarios = contract.scenarios.flatMap((scenario) => results.has(scenario.id) ? [results.get(scenario.id)] : []);
	await writeFile(join(artifacts, "gpui.json"), JSON.stringify({ driver: "gpui", scenarios }, null, 2));
	if (coverage) {
		await recordGpuiCoverage(artifacts, executable, env.CARGO_BUILD_BUILD_DIR ?? "", contract.scenarios.map((scenario) => scenario.id), shards, signal);
		await rm(strayProfiles, { recursive: true, force: true });
		mark("gpuiCoverage");
	}
	return problems.length ? problems.join("; ") : undefined;
}
