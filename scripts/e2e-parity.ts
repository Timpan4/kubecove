import { copyFile, mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { availableParallelism } from "node:os";
import { join, resolve } from "node:path";
import { changedScenarios, DEFAULT_MAP } from "../e2e/parity/changed-scenarios";
import { type Contract, loadContract, requireActionCoverage, requireViewCoverage } from "../e2e/parity/contract";
import { writeRunMap } from "../e2e/parity/coverage-map";
import { exportGpuiParity, runGpui } from "../e2e/parity/gpui-run";
import { type Driver, inspectDriverResults, parseDriver } from "../e2e/parity/merge-reports";
import { type CoverageMapFailure, DRIVER_SELECTION_FORMAT, driverSelectionFile, lastRerunSource, rerunSelections, selectionHash } from "../e2e/parity/rerun";
import { selectedContract } from "../e2e/parity/selected-contract";
import { assignSuiteShards, loadTimings } from "../e2e/parity/suite-sharding";
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
 * - PARITY_SUITE_SHARD=<index>/<count>: duration-balanced assignment across CI jobs.
 *   Every shard in the suite must receive the same optional KUBECOVE_PARITY_TIMINGS_PATH.
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
		if (!match || !Number.isSafeInteger(index) || !Number.isSafeInteger(count) || index >= count) throw new Error(`PARITY_SUITE_SHARD must be <index>/<count>: ${shard}`);
		const timings = await loadTimings(process.env.KUBECOVE_PARITY_TIMINGS_PATH);
		const assignments = assignSuiteShards(selected.map(({ id }) => id), count, timings);
		const ids = new Set(assignments[index]);
		selected = selected.filter(({ id }) => ids.has(id));
	}
	return selected;
}

// PARITY_COVERAGE=1 records the coverage map (e2e/parity/README.md); normal runs leave it off.
const coverage = process.env.PARITY_COVERAGE === "1";
const buildDir = process.env.KUBECOVE_PARITY_BUILD_DIR;
const controller = new AbortController();
const interrupt = () => controller.abort(new Error("Interrupted"));
process.once("SIGINT", interrupt);
process.once("SIGTERM", interrupt);
if (buildDir) {
	if (process.env.KUBECOVE_PARITY_MANIFEST) throw new Error("KUBECOVE_PARITY_BUILD_DIR and KUBECOVE_PARITY_MANIFEST are mutually exclusive");
	console.log(`GPUI parity manifest: ${await exportGpuiParity(resolve(buildDir), coverage, controller.signal)}`);
	process.exit(0);
}
const driver = process.env.PARITY_DRIVER ? parseDriver(process.env.PARITY_DRIVER) : undefined;
const rerun = process.env.PARITY_RERUN === "last" ? await lastRerunSource(resolve("e2e/artifacts"), driver) : process.env.PARITY_RERUN;
if (driver === "web" && process.env.KUBECOVE_PARITY_MANIFEST) throw new Error("KUBECOVE_PARITY_MANIFEST requires the GPUI driver");
if (coverage && driver) throw new Error("PARITY_COVERAGE requires both drivers");
const artifacts = resolve("e2e/artifacts", `parity-${crypto.randomUUID()}`);
await mkdir(artifacts, { recursive: true });
await writeFile(join(artifacts, "vite.log"), "Web driver did not start.\n");
await writeFile(join(artifacts, "gpui.log"), "Native driver did not start.\n");
// Seconds from the start of the run to the end of each phase, recorded in result.json.
const started = performance.now();
const phases: Record<string, number> = {};
const mark = (phase: string) => { phases[phase] = Math.round(performance.now() - started) / 1000; };
let failure: string | undefined;
let runLevelFailed = false;
let resultDriver: Driver | "both" = driver ?? "both";
let selectionHashes: Record<Driver, string> | undefined;
let coverageMapFailure: CoverageMapFailure | null = null;
let coveragePublicationRequested = false;
const failedDrivers = new Set<Driver>();
const fail = (message: string) => { failure = failure ? `${failure}; ${message}` : message; };
const driverFailed = (name: Driver, message: string) => {
	failedDrivers.add(name);
	fail(message);
};
async function writeSelections(selection: Record<Driver, Contract>) {
	const contents = {
		web: selection.web.scenarios.map(({ id }) => id).join("\n"),
		gpui: selection.gpui.scenarios.map(({ id }) => id).join("\n"),
	};
	await Promise.all((["web", "gpui"] as const).map(async (name) => {
		const path = join(artifacts, driverSelectionFile[name]);
		await writeFile(`${path}.tmp`, contents[name]);
		await rename(`${path}.tmp`, path);
	}));
	const allIds = new Set([...selection.web.scenarios, ...selection.gpui.scenarios].map(({ id }) => id));
	// The report collector uses the combined list to find artifacts with work.
	if (allIds.size) await writeFile(join(artifacts, "scenarios.txt"), [...allIds].join("\n"));
	selectionHashes = { web: selectionHash(contents.web), gpui: selectionHash(contents.gpui) };
}

/** Runs the requested drivers on the selected scenarios and records failures through `fail`. */
async function runDrivers(selection: Record<Driver, Contract>, executed: Driver[], publishCoverageMap: boolean) {
	const positive = (name: string, fallback: number) => {
		const value = Number(process.env[name] ?? fallback);
		if (!Number.isInteger(value) || value < 1) throw new Error(`${name} must be a positive integer: ${process.env[name]}`);
		return value;
	};
	const jobs = positive("PARITY_JOBS", Math.max(1, Math.floor(availableParallelism() / 2)));
	const webJobs = executed.includes("web") ? positive("PARITY_WEB_JOBS", jobs) : jobs;
	// The GPUI test processes render the same browser-mock responses the web driver reads.
	// Only they read the fixture, so the web driver and the GPUI build start during the export.
	const fixture = join(artifacts, "fixture.json");
	const runs: Promise<unknown>[] = [];
	if (executed.includes("gpui")) {
		const exported = Bun.spawn(["bun", "e2e/parity/visual/export-fixture.ts", fixture], { stdout: "inherit", stderr: "inherit" });
		const exportExit = exported.exited.then((code) => { mark("fixture"); return code; });
		const cancelExport = () => exported.kill();
		controller.signal.addEventListener("abort", cancelExport, { once: true });
		runs.push(runGpui(selection.gpui, artifacts, { path: fixture, exit: exportExit }, jobs, controller.signal, mark, coverage)
			.then((problem) => { if (problem) driverFailed("gpui", problem); }, (error) => driverFailed("gpui", `GPUI driver: ${String(error)}`))
			.finally(async () => { await exportExit; controller.signal.removeEventListener("abort", cancelExport); }));
	}
	if (executed.includes("web")) {
		runs.push(import("../e2e/parity/web")
			.then(({ runWeb }) => runWeb(selection.web, artifacts, controller.signal, webJobs))
			.catch((error) => driverFailed("web", `Web driver: ${String(error)}`))
			.finally(() => mark("web")));
	}
	await Promise.all(runs);
	controller.signal.throwIfAborted();
	const reports = await Promise.all(executed.map(async (name) => {
		try {
			const report = JSON.parse(await readFile(join(artifacts, `${name}.json`), "utf8"));
			return { name, ...inspectDriverResults(selection[name], name, report) };
		} catch (error) {
			return { name, results: [], problems: [error instanceof Error ? error.message : String(error)] };
		}
	}));
	const passed = { web: new Set<string>(), gpui: new Set<string>() };
	for (const { name, results, problems } of reports) {
		if (problems.length) driverFailed(name, `${name} report: Error: ${problems.join("; ")}`);
		const failed = results.find(({ status }) => status === "failed");
		if (failed) fail(`${name} report: Error: Invalid ${name} result: ${failed.id}`);
		for (const { id, status } of results) if (status === "passed") passed[name].add(id);
	}
	if (!coverage) return;
	try {
		// Each coverage row must have a unique passing result from both drivers.
		const bothPassed = new Set([...passed.web].filter((id) => passed.gpui.has(id)));
		const path = join(artifacts, "coverage-map.json");
		const map = await writeRunMap(artifacts, path, bothPassed);
		mark("coverageMap");
		console.log(`Coverage map: ${map.scenarios.length} scenarios, ${Object.keys(map.files).length} files`);
		const covered = new Set(map.scenarios);
		const missing = [...bothPassed].filter((id) => !covered.has(id));
		if (missing.length) throw new Error(`Missing coverage for passing scenarios: ${missing.join(", ")}`);
		if (publishCoverageMap) {
			coverageMapFailure = "publication";
			if (failedDrivers.size) return;
			const temporary = join(artifacts, "default-map.tmp");
			await copyFile(path, temporary);
			controller.signal.throwIfAborted();
			await rename(temporary, resolve(DEFAULT_MAP));
		}
		coverageMapFailure = null;
	} catch (error) {
		fail(`Coverage map ${coverageMapFailure}: ${String(error)}`);
	}
}

try {
	const contract = await loadContract();
	requireViewCoverage(contract, VIEWS);
	requireActionCoverage(contract);
	const scenarios = await selectScenarios(contract.scenarios);
	const failed = rerun ? await rerunSelections(rerun, { coverage }) : undefined;
	if (failed?.coveragePublication === "unknown") {
		console.warn("Publication intent is unknown for this legacy run. To refresh the default coverage map, run unfiltered with PARITY_COVERAGE=1 and PARITY_RERUN unset.");
	}
	const coverageIds = coverage && failed ? new Set([...failed.web, ...failed.gpui]) : undefined;
	const selectedFor = (name: Driver): Contract => {
		if (driver && driver !== name) return { scenarios: [] };
		if (!failed) return { scenarios };
		const ids = coverageIds ?? failed[name];
		return { scenarios: scenarios.filter(({ id }) => ids.has(id)) };
	};
	const selection = { web: selectedFor("web"), gpui: selectedFor("gpui") };
	const executed = (["web", "gpui"] as const).filter((name) => selection[name].scenarios.length);
	const narrowed = ["PARITY_PREFIX", "PARITY_SCENARIOS", "PARITY_EXCLUDE", "PARITY_CHANGED", "PARITY_SUITE_SHARD"].some((name) => process.env[name]);
	const fullSelection = executed.length === 2 && executed.every((name) => selection[name].scenarios.length === contract.scenarios.length);
	const retryPublication = failed?.coveragePublication === "retry";
	if (retryPublication && !narrowed && !fullSelection) {
		throw new Error("The recorded selection does not cover the current suite. Run PARITY_COVERAGE=1 without PARITY_RERUN to publish the default map.");
	}
	coveragePublicationRequested = coverage && !narrowed && fullSelection && (!rerun || retryPublication);
	if (!executed.length && !rerun && !process.env.PARITY_CHANGED) throw new Error("No parity scenarios selected");
	if (executed.length === 1) resultDriver = executed[0];
	await writeSelections(selection);
	if (coverage && executed.length) coverageMapFailure = "generation";
	if (executed.includes("web")) await writeFile(join(artifacts, "web.json"), JSON.stringify({ driver: "web", scenarios: [] }));
	if (rerun) console.log(`Rerunning ${selection.web.scenarios.length} web and ${selection.gpui.scenarios.length} GPUI scenarios from ${rerun}`);
	if (executed.length) {
		// Frame measurements wait for this run, and this run waits for one in progress.
		const isolation = await holdFrameIsolation("shared", controller.signal);
		try {
			controller.signal.throwIfAborted();
			await runDrivers(selection, executed, coveragePublicationRequested);
		}
		finally { if (isolation.state === "held") await isolation.release(); }
	} else if (rerun) console.log("Nothing to rerun: no earlier failures match this selection.");
	// A change no scenario can observe, or a CI shard with no share of a small selection.
	else console.log("No parity scenarios selected by the changes.");
} catch (error) {
	runLevelFailed = true;
	fail(String(error));
}
finally {
	process.removeListener("SIGINT", interrupt); process.removeListener("SIGTERM", interrupt);
	const interrupted = controller.signal.aborted;
	if (interrupted && !failure) fail("Interrupted");
	mark("total");
	const resultPath = join(artifacts, "result.json");
	await writeFile(`${resultPath}.tmp`, JSON.stringify({
		level: "UI integration with mocks",
		driver: resultDriver,
		selectionFormat: DRIVER_SELECTION_FORMAT,
		selectionRecorded: selectionHashes !== undefined,
		selectionHashes,
		failedDrivers: [...failedDrivers],
		runLevelFailed,
		coverageMapFailure,
		coveragePublicationRequested,
		interrupted,
		status: failure ? "failed" : "passed",
		error: failure,
		phases,
	}, null, 2));
	await rename(`${resultPath}.tmp`, resultPath);
	console.log(`Parity artifacts: ${artifacts}`);
}
if (failure) { console.error(failure); process.exitCode = 1; }
