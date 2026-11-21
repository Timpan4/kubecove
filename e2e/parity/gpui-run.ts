import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { startIdentity, stopGroup } from "../../.agents/skills/verify-kubecove/scripts/processes";
import type { Contract } from "./contract";
import { object, string } from "./contract-values";
import { compiledFiles, COVERAGE_RUSTFLAGS, recordGpuiCoverage } from "./gpui-coverage-map";
import { type ParityBuild, parityBuild, verifyPrebuiltParity, writePrebuiltParity } from "./gpui-prebuilt";
import { driverReportRows } from "./merge-reports";
import { driverSelectionFile } from "./rerun";

type Spawned = { code: number; output: string; signaled: boolean };

// Rust libtest's ERROR_EXIT_CODE for a failed test.
const LIBTEST_FAILURE = 101;

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
async function buildParity(workspace: string, env: Record<string, string | undefined>, artifacts: string, signal: AbortSignal, expected?: ParityBuild): Promise<string> {
	console.log("Checking the GPUI parity executable with Cargo.");
	const buildArgs = expected ? ["--target", expected.target, "--profile", expected.profile] : [];
	const build = await spawnGroup(
		["cargo", "test", "--manifest-path", join(workspace, "Cargo.toml"), "--test", "parity", "--no-run", "--message-format=json-render-diagnostics", ...buildArgs],
		{ ...env, CARGO_LOG: env.CARGO_LOG ?? "cargo::compiler::fingerprint=info" },
		signal,
	);
	await writeFile(join(artifacts, "gpui-build.log"), build.output);
	if (build.code !== 0) throw new Error(`GPUI parity build exited ${build.code}`);
	for (const line of build.output.split("\n")) {
		if (!line.startsWith("{")) continue;
		// SAFETY: cargo writes one JSON object per line under --message-format=json. Every field read here is optional in cargo's schema, and each read is a comparison.
		const message = JSON.parse(line) as { reason?: string; executable?: string | null; target?: { name?: string }; features?: string[]; fresh?: boolean };
		if (message.reason !== "compiler-artifact" || message.target?.name !== "parity" || !message.executable) continue;
		if (expected && JSON.stringify(message.features?.toSorted()) !== JSON.stringify(expected.features)) throw new Error("Cargo built the parity executable with unexpected features");
		if (message.fresh) console.log("Reusing the GPUI parity executable. Cargo found its inputs unchanged.");
		else {
			const reason = parityRebuildReason(build.output);
			console.log(reason ? `GPUI parity executable rebuilt: ${reason}` : "GPUI parity executable rebuilt. See gpui-build.log for Cargo diagnostics.");
		}
		return message.executable;
	}
	throw new Error("cargo did not report the parity test executable");
}

function gpuiEnvironment(workspace: string, coverage: boolean) {
	// Keep path-crate artifacts specific to this worktree, including environment overrides.
	const env = Object.fromEntries(Object.entries(process.env).filter(([name]) => !["ARGV0", "KUBECONFIG", "KUBECOVE_PARITY_SCENARIO_PREFIX"].includes(name)));
	const targetDir = process.env.CARGO_TARGET_DIR ?? join(workspace, "target");
	const strayProfiles = resolve(targetDir, "coverage", "profraw");
	const buildDir = coverage ? resolve(targetDir, "coverage") : process.env.CARGO_BUILD_BUILD_DIR ?? join(workspace, "build");
	Object.assign(env, { CARGO_TARGET_DIR: coverage ? buildDir : targetDir, CARGO_BUILD_BUILD_DIR: buildDir });
	if (coverage) {
		const encoded = env.CARGO_ENCODED_RUSTFLAGS;
		if (encoded !== undefined) env.CARGO_ENCODED_RUSTFLAGS = [encoded, ...COVERAGE_RUSTFLAGS.split(" ")].filter(Boolean).join("\x1f");
		else env.RUSTFLAGS = `${env.RUSTFLAGS ?? env.CARGO_BUILD_RUSTFLAGS ?? ""} ${COVERAGE_RUSTFLAGS}`.trim();
		env.LLVM_PROFILE_FILE = join(strayProfiles, "%p-%m.profraw");
	}
	return { env, buildDir, strayProfiles };
}

export async function exportGpuiParity(directory: string, coverage: boolean, signal: AbortSignal): Promise<string> {
	const expected = await parityBuild(coverage);
	const workspace = join(expected.sourceRoot, "src-tauri", "gpui");
	const { env, buildDir, strayProfiles } = gpuiEnvironment(workspace, coverage);
	const inheritedFlags = env.CARGO_ENCODED_RUSTFLAGS ?? env.RUSTFLAGS ?? env.CARGO_BUILD_RUSTFLAGS ?? "";
	if (!coverage && /instrument-coverage|kubecove_coverage/.test(inheritedFlags)) {
		throw new Error("Uninstrumented GPUI parity cannot inherit coverage flags");
	}
	await mkdir(dirname(directory), { recursive: true });
	await mkdir(directory);
	try {
		const executable = await buildParity(workspace, env, directory, signal, expected);
		const compiled = coverage ? await compiledFiles(buildDir, workspace, expected.sourceRoot) : undefined;
		const current = await parityBuild(coverage);
		if (current.gitSha !== expected.gitSha) throw new Error("Git SHA changed during the GPUI parity build");
		signal.throwIfAborted();
		return await writePrebuiltParity(directory, executable, expected, compiled);
	} finally {
		if (coverage) await rm(strayProfiles, { recursive: true, force: true });
	}
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
	const { env, buildDir, strayProfiles } = gpuiEnvironment(workspace, coverage);
	Object.assign(env, { KUBECOVE_E2E_ARTIFACTS: artifacts, KUBECOVE_PARITY_FIXTURE: fixture.path, KUBECOVE_PARITY_SCENARIO_IDS: ids });
	const manifest = process.env.KUBECOVE_PARITY_MANIFEST;
	const prebuilt = manifest ? await verifyPrebuiltParity(manifest, await parityBuild(coverage)) : undefined;
	const executable = prebuilt?.executable ?? await buildParity(workspace, env, artifacts, signal);
	if (prebuilt) console.log(`Running verified GPUI parity executable from ${manifest}`);
	mark(prebuilt ? "gpuiVerify" : "gpuiBuild");
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
	const claims = new Map(await Promise.all(contract.scenarios.map(async ({ id }, index) => {
		const owner = await readFile(join(artifacts, "gpui-claims", String(index)), "utf8").catch(() => undefined);
		return [id, owner] as const;
	})));
	const results = new Map<string, unknown[]>();
	const problems: string[] = [];
	for (const [index, run] of runs.entries()) {
		const before = problems.length;
		let reportedFailure = false;
		try {
			const report = JSON.parse(await readFile(join(artifacts, `gpui-${index}.json`), "utf8"));
			for (const value of driverReportRows("gpui", report)) {
				let id: string;
				let result: ReturnType<typeof object>;
				try {
					result = object(value);
					id = string(result.id);
				} catch (error) {
					problems.push(`Invalid GPUI result from shard ${index}/${count}: ${String(error)}`);
					continue;
				}
				if (!claims.has(id)) {
					problems.push(`Unexpected or duplicate GPUI result from shard ${index}/${count}: ${id}`);
					continue;
				}
				const claimed = claims.get(id) === String(index);
				if (!claimed) {
					problems.push(`Unverified GPUI claim for ${id} in shard ${index}/${count}`);
					// Keep the ID so a duplicate in another shard still invalidates coverage.
					result.status = "unverified";
				}
				const previous = results.get(id);
				if (previous) {
					problems.push(`Unexpected or duplicate GPUI result from shard ${index}/${count}: ${id}`);
					previous.push(result);
					continue;
				}
				results.set(id, [result]);
				if (result.status === "failed") reportedFailure = true;
				else if (claimed && result.status !== "passed") problems.push(`Invalid GPUI result from shard ${index}/${count}: ${id}`);
			}
		} catch (error) { problems.push(`GPUI shard ${index}/${count} did not produce a valid report: ${String(error)}`); }
		// The native harness writes the report, then fails its test if a scenario failed.
		const scenarioExit = run.code === LIBTEST_FAILURE && reportedFailure && problems.length === before;
		if (run.signaled || (run.code !== 0 && !scenarioExit)) {
			problems.push(`GPUI shard ${index}/${count} exited ${run.code}`);
		}
	}
	const missing = [...claims.keys()].filter((id) => !results.has(id));
	if (missing.length) problems.push(`Missing GPUI results: ${missing.join(", ")}`);
	const scenarios = contract.scenarios.flatMap(({ id }) => results.get(id) ?? []);
	await writeFile(join(artifacts, "gpui.json"), JSON.stringify({ driver: "gpui", scenarios }, null, 2));
	if (coverage) {
		const compiled = prebuilt?.compiled ?? await compiledFiles(buildDir, workspace, resolve("."));
		await recordGpuiCoverage(artifacts, executable, compiled, contract.scenarios.map((scenario) => scenario.id), shards, signal);
		await rm(strayProfiles, { recursive: true, force: true });
		mark("gpuiCoverage");
	}
	return problems.length ? problems.join("; ") : undefined;
}
