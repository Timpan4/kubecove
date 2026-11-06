import { readFile, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { startIdentity, stopGroup } from "../../.agents/skills/verify-kubecove/scripts/processes";
import type { Contract } from "./contract";

type Spawned = { code: number; output: string };

/** Runs a child in its own process group and stops the group when `signal` aborts. */
async function spawnGroup(command: string[], env: Record<string, string | undefined>, signal: AbortSignal): Promise<Spawned> {
	const niced = process.platform === "win32" ? command : ["nice", ...command];
	const child = Bun.spawn(niced, { env, detached: true, stdout: "pipe", stderr: "pipe" });
	const identity = startIdentity(child.pid);
	const output = Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text()]);
	const cancel = () => { void stopGroup(child.pid, identity); };
	signal.addEventListener("abort", cancel, { once: true });
	try {
		const code = await child.exited;
		return { code, output: (await output).join("\n") };
	} finally { signal.removeEventListener("abort", cancel); }
}

/** Builds the parity test binary once and returns its path from cargo's JSON messages. */
async function buildParity(workspace: string, env: Record<string, string | undefined>, artifacts: string, signal: AbortSignal): Promise<string> {
	const build = await spawnGroup(["cargo", "test", "--manifest-path", join(workspace, "Cargo.toml"), "--test", "parity", "--no-run", "--message-format=json-render-diagnostics"], env, signal);
	await writeFile(join(artifacts, "gpui-build.log"), build.output);
	if (build.code !== 0) throw new Error(`GPUI parity build exited ${build.code}`);
	for (const line of build.output.split("\n")) {
		if (!line.startsWith("{")) continue;
		const message = JSON.parse(line) as { reason?: string; executable?: string | null; target?: { name?: string } };
		if (message.reason === "compiler-artifact" && message.target?.name === "parity" && message.executable) return message.executable;
	}
	throw new Error("cargo did not report the parity test executable");
}

/**
 * Runs the selected scenarios in `shards` GPUI test processes. Each process takes every
 * `shards`-th scenario (`KUBECOVE_PARITY_SHARD` in tests/parity.rs) and writes its own report;
 * this merges them into `gpui.json` in contract order.
 */
export async function runGpui(contract: Contract, artifacts: string, fixture: string, shards: number, signal: AbortSignal): Promise<string | undefined> {
	const workspace = resolve("src-tauri/gpui");
	// scripts/e2e-parity.ts writes the selected IDs here before either driver starts.
	const ids = join(artifacts, "scenarios.txt");
	// Keep path-crate artifacts specific to this worktree, including environment overrides.
	const env: Record<string, string | undefined> = { ...process.env, CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? join(workspace, "target"), CARGO_BUILD_BUILD_DIR: process.env.CARGO_BUILD_BUILD_DIR ?? join(workspace, "build"), KUBECOVE_E2E_ARTIFACTS: artifacts, KUBECOVE_PARITY_FIXTURE: fixture, KUBECOVE_PARITY_SCENARIO_IDS: ids, CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS ?? "1" };
	delete env.ARGV0; delete env.KUBECONFIG; delete env.KUBECOVE_PARITY_SCENARIO_PREFIX;
	const executable = await buildParity(workspace, env, artifacts, signal);
	const count = Math.max(1, Math.min(shards, contract.scenarios.length));
	// The GPUI test platform renders without a display.
	const runs = await Promise.all(Array.from({ length: count }, async (_, index) => {
		const run = await spawnGroup([executable, "--nocapture"], { ...env, KUBECOVE_PARITY_SHARD: `${index}/${count}` }, signal);
		await writeFile(join(artifacts, `gpui-${index}.log`), run.output);
		return run;
	}));
	await writeFile(join(artifacts, "gpui.log"), runs.map((run, index) => `== shard ${index}/${count} exited ${run.code}\n${run.output}`).join("\n"));
	const results = new Map<string, unknown>();
	const problems: string[] = [];
	for (const [index, run] of runs.entries()) {
		if (run.code !== 0) problems.push(`GPUI shard ${index}/${count} exited ${run.code}`);
		try {
			const report = JSON.parse(await readFile(join(artifacts, `gpui-${index}.json`), "utf8")) as { scenarios: { id: string }[] };
			for (const result of report.scenarios) results.set(result.id, result);
		} catch { problems.push(`GPUI shard ${index}/${count} wrote no report`); }
	}
	const scenarios = contract.scenarios.flatMap((scenario) => results.has(scenario.id) ? [results.get(scenario.id)] : []);
	await writeFile(join(artifacts, "gpui.json"), JSON.stringify({ driver: "gpui", scenarios }, null, 2));
	return problems.length ? problems.join("; ") : undefined;
}
