import { copyFile, mkdir, readdir, readFile, rename, stat, writeFile } from "node:fs/promises";
import { availableParallelism } from "node:os";
import { join, resolve } from "node:path";
import { changedScenarios, DEFAULT_MAP } from "../e2e/parity/changed-scenarios";
import { type Contract, loadContract, requireActionCoverage, requireViewCoverage, validateReports } from "../e2e/parity/contract";
import { writeRunMap } from "../e2e/parity/coverage-map";
import { runGpui } from "../e2e/parity/gpui-run";
import { type Driver, parseDriver, validateDriverReport } from "../e2e/parity/merge-reports";
import { rerunSelections } from "../e2e/parity/rerun";
import { selectedContract } from "../e2e/parity/selected-contract";
import { VIEWS } from "../e2e/parity/visual/views";
import { holdFrameIsolation } from "./gpui-frame-process";

// PARITY_JOBS: parallel web pages and GPUI processes each. The default splits the CPUs
// between the two drivers. Measured on 16 CPUs (2026-11-06): 16 per driver finished no
// sooner than 8 per driver and timed out more web scenarios. PARITY_WEB_JOBS sets the web pages
// alone: a CI shard's web scenarios took about five times the CPU time of its GPUI scenarios
// (2026-11-12). `selectScenarios` lists the selection variables.

type Scenario = Contract["scenarios"][number];

/**
 * The scenarios this run executes, in contract order. Each optional filter narrows the one
 * before it.
 * - PARITY_PREFIX: comma-separated scenario ID prefixes.
 * - PARITY_EXCLUDE: comma-separated scenario ID prefixes to leave out, such as a scenario that
 *   CI runs in a job of its own.
 * - PARITY_CHANGED=<base ref>: the scenarios the changes since the merge base with that ref can
 *   affect, from the coverage map at PARITY_COVERAGE_MAP (e2e/parity/changed-scenarios.ts).
 * - PARITY_SUITE_SHARD=<index>/<count>: every `count`-th scenario from position `index`, so
 *   separate CI jobs split one suite.
 */
async function selectScenarios(scenarios: Scenario[]): Promise<Scenario[]> {
	const list = (name: string) => (process.env[name] ?? "").split(",").map((prefix) => prefix.trim()).filter(Boolean);
	const prefixes = list("PARITY_PREFIX");
	const excluded = list("PARITY_EXCLUDE");
	const exact = selectedContract({ scenarios }, process.env.PARITY_SCENARIOS).scenarios;
	let selected = prefixes.length ? exact.filter((scenario) => prefixes.some((prefix) => scenario.id.startsWith(prefix))) : exact;
	selected = selected.filter((scenario) => !excluded.some((prefix) => scenario.id.startsWith(prefix)));
	const base = process.env.PARITY_CHANGED;
	if (base) {
		const selection = await changedScenarios(base, selected.map((scenario) => scenario.id), process.env.PARITY_COVERAGE_MAP || DEFAULT_MAP);
		await writeFile(join(artifacts, "selection.json"), JSON.stringify({ base, all: selection.all, because: selection.because }, null, 2));
		console.log(selection.all ? `Changes since ${base}: full suite (${selection.all})` : `Changes since ${base} select ${selection.ids.size} of ${selected.length} scenarios`);
		selected = selected.filter((scenario) => selection.ids.has(scenario.id));
	}
	// Shard before the rerun filter, so a shard's rerun keeps the scenarios that shard ran.
	const shard = process.env.PARITY_SUITE_SHARD;
	if (shard) {
		const match = /^(\d+)\/(\d+)$/.exec(shard);
		const [index, count] = [Number(match?.[1]), Number(match?.[2])];
		if (!match || index >= count) throw new Error(`PARITY_SUITE_SHARD must be <index>/<count>: ${shard}`);
		selected = selected.filter((_, position) => position % count === index);
	}
	return selected;
}

/**
 * The newest earlier run that finished and recorded its selection. A run that is still going
 * has no result.json, and one that failed before selecting has no scenarios.txt.
 */
async function lastRun(): Promise<string> {
	const root = resolve("e2e/artifacts");
	const runs = await Promise.all((await readdir(root)).filter((name) => name.startsWith("parity-")).map(async (name) => {
		try {
			await stat(join(root, name, "scenarios.txt"));
			// An interrupted run never reached most scenarios, so its results say nothing about them.
			// SAFETY: the finally block below writes result.json with an optional string `error`, which is all this reads.
			const result = JSON.parse(await readFile(join(root, name, "result.json"), "utf8")) as { error?: string };
			if (result.error?.includes("Interrupted")) return undefined;
			return { path: join(root, name), time: (await stat(join(root, name, "result.json"))).mtimeMs };
		} catch { return undefined; }
	}));
	const newest = runs.flatMap((run) => run ? [run] : []).sort((left, right) => right.time - left.time)[0];
	if (!newest) throw new Error("PARITY_RERUN=last found no finished parity run");
	return newest.path;
}

const driver = process.env.PARITY_DRIVER ? parseDriver(process.env.PARITY_DRIVER) : undefined;
const rerun = process.env.PARITY_RERUN === "last" ? await lastRun() : process.env.PARITY_RERUN;
// PARITY_COVERAGE=1 records the coverage map (e2e/parity/README.md); normal runs leave it off.
const coverage = process.env.PARITY_COVERAGE === "1";
if (coverage && driver) throw new Error("PARITY_COVERAGE requires both drivers");
const artifacts = resolve("e2e/artifacts", `parity-${crypto.randomUUID()}`);
await mkdir(artifacts, { recursive: true });
if (driver !== "gpui") await writeFile(join(artifacts, "web.json"), JSON.stringify({ driver: "web", scenarios: [] }));
await writeFile(join(artifacts, "vite.log"), "Web driver did not start.\n");
await writeFile(join(artifacts, "gpui.log"), "Native driver did not start.\n");
// Seconds from the start of the run to the end of each phase, recorded in result.json.
const started = performance.now();
const phases: Record<string, number> = {};
const mark = (phase: string) => { phases[phase] = Math.round(performance.now() - started) / 1000; };
const controller = new AbortController();
const interrupt = () => controller.abort(new Error("Interrupted"));
process.once("SIGINT", interrupt);
process.once("SIGTERM", interrupt);
let failure: string | undefined;
let resultDriver: Driver | "both" = driver ?? "both";
const fail = (message: string) => { failure = failure ? `${failure}; ${message}` : message; };
/** Runs the requested drivers on the selected scenarios and records failures through `fail`. */
async function runDrivers(selection: Record<Driver, Contract>) {
	const executed = (["web", "gpui"] as const).filter((name) => selection[name].scenarios.length);
	resultDriver = executed.length === 1 ? executed[0] : "both";
	const allIds = new Set(executed.flatMap((name) => selection[name].scenarios.map((scenario) => scenario.id)));
	await Promise.all((["web", "gpui"] as const).map(async (name) => {
		const path = join(artifacts, `${name}-scenarios.txt`);
		await writeFile(`${path}.tmp`, selection[name].scenarios.map((scenario) => scenario.id).join("\n"));
		await rename(`${path}.tmp`, path);
	}));
	// Publish the combined list for older readers after both driver selections are complete.
	await writeFile(join(artifacts, "scenarios.txt"), [...allIds].join("\n"));
	const positive = (name: string, fallback: number) => {
		const value = Number(process.env[name] ?? fallback);
		if (!Number.isInteger(value) || value < 1) throw new Error(`${name} must be a positive integer: ${process.env[name]}`);
		return value;
	};
	const jobs = positive("PARITY_JOBS", Math.max(1, Math.floor(availableParallelism() / 2)));
	const webJobs = positive("PARITY_WEB_JOBS", jobs);
	// The GPUI test processes render the same browser-mock responses the web driver reads.
	// Only they read the fixture, so the web driver and the GPUI build start during the export.
	const fixture = join(artifacts, "fixture.json");
	const runs: Promise<unknown>[] = [];
	if (selection.gpui.scenarios.length) {
		const exported = Bun.spawn(["bun", "e2e/parity/visual/export-fixture.ts", fixture], { stdout: "inherit", stderr: "inherit" });
		const exportExit = exported.exited.then((code) => { mark("fixture"); return code; });
		const cancelExport = () => exported.kill();
		controller.signal.addEventListener("abort", cancelExport, { once: true });
		runs.push(runGpui(selection.gpui, artifacts, { path: fixture, exit: exportExit }, jobs, controller.signal, mark, coverage)
			.then((problem) => { if (problem) fail(problem); }, (error) => fail(`GPUI driver: ${String(error)}`))
			.finally(async () => { await exportExit; controller.signal.removeEventListener("abort", cancelExport); }));
	}
	if (selection.web.scenarios.length) {
		runs.push(import("../e2e/parity/web")
			.then(({ runWeb }) => runWeb(selection.web, artifacts, controller.signal, webJobs))
			.catch((error) => fail(`Web driver: ${String(error)}`))
			.finally(() => mark("web")));
	}
	await Promise.all(runs);
	controller.signal.throwIfAborted();
	const reports = await Promise.all(executed.map(async (name) => JSON.parse(await readFile(join(artifacts, `${name}.json`), "utf8"))));
	for (const [index, name] of executed.entries()) {
		try { validateDriverReport(selection[name], name, reports[index]); }
		catch (error) { fail(String(error)); }
	}
	if (executed.length === 2 && (!rerun || coverage) && !failure) validateReports(selection.web, reports);
	if (coverage) {
		// Scenarios that failed stay out of the map, so selection always runs them.
		const map = await writeRunMap(artifacts, join(artifacts, "coverage-map.json"));
		mark("coverageMap");
		console.log(`Coverage map: ${map.scenarios.length} scenarios, ${Object.keys(map.files).length} files`);
		const narrowed = ["PARITY_PREFIX", "PARITY_SCENARIOS", "PARITY_EXCLUDE", "PARITY_CHANGED", "PARITY_RERUN", "PARITY_SUITE_SHARD"].some((name) => process.env[name]);
		if (!narrowed) await copyFile(join(artifacts, "coverage-map.json"), resolve(DEFAULT_MAP));
	}
}

try {
	const contract = await loadContract();
	requireViewCoverage(contract, VIEWS);
	requireActionCoverage(contract);
	const scenarios = await selectScenarios(contract.scenarios);
	const failed = rerun ? await rerunSelections(rerun) : undefined;
	const selectedFor = (name: Driver): Contract => ({
		scenarios: driver && driver !== name ? [] : scenarios.filter(({ id }) =>
			!failed || (coverage ? failed.web.has(id) || failed.gpui.has(id) : failed[name].has(id))),
	});
	const selection = { web: selectedFor("web"), gpui: selectedFor("gpui") };
	if (rerun) console.log(`Rerunning ${selection.web.scenarios.length} web and ${selection.gpui.scenarios.length} GPUI scenarios from ${rerun}`);
	if (selection.web.scenarios.length || selection.gpui.scenarios.length) {
		// Frame measurements wait for this run, and this run waits for one in progress.
		const isolation = await holdFrameIsolation("shared", controller.signal);
		try {
			controller.signal.throwIfAborted();
			await runDrivers(selection);
		}
		finally { if (isolation.state === "held") await isolation.release(); }
	} else if (rerun) console.log("Nothing to rerun: no earlier failures match this selection.");
	// A change no scenario can observe, or a CI shard with no share of a small selection.
	else if (process.env.PARITY_CHANGED) console.log("No parity scenarios selected by the changes.");
	else throw new Error("No parity scenarios selected");
} catch (error) { fail(String(error)); }
finally {
	process.removeListener("SIGINT", interrupt); process.removeListener("SIGTERM", interrupt);
	mark("total");
	await writeFile(join(artifacts, "result.json"), JSON.stringify({ level: "UI integration with mocks", driver: resultDriver, status: failure ? "failed" : "passed", error: failure, phases }, null, 2));
	console.log(`Parity artifacts: ${artifacts}`);
}
if (failure) { console.error(failure); process.exitCode = 1; }
