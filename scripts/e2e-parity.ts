import { copyFile, mkdir, readdir, readFile, stat, writeFile } from "node:fs/promises";
import { availableParallelism } from "node:os";
import { join, resolve } from "node:path";
import { changedScenarios, DEFAULT_MAP } from "../e2e/parity/changed-scenarios";
import { type Contract, loadContract, requireViewCoverage, validateReports } from "../e2e/parity/contract";
import { writeRunMap } from "../e2e/parity/coverage-map";
import { runGpui } from "../e2e/parity/gpui-run";
import { parseDriver, validateDriverReport } from "../e2e/parity/merge-reports";
import { VIEWS } from "../e2e/parity/visual/views";
import { runWeb } from "../e2e/parity/web";

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
 * - PARITY_RERUN, resolved to `rerun`: only the scenarios either driver did not pass there.
 */
async function selectScenarios(scenarios: Scenario[], rerun: string | undefined): Promise<Scenario[]> {
	const list = (name: string) => (process.env[name] ?? "").split(",").map((prefix) => prefix.trim()).filter(Boolean);
	const prefixes = list("PARITY_PREFIX");
	const excluded = list("PARITY_EXCLUDE");
	let selected = prefixes.length ? scenarios.filter((scenario) => prefixes.some((prefix) => scenario.id.startsWith(prefix))) : scenarios;
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
	if (rerun) {
		const [web, gpui] = await Promise.all([passedIn(rerun, "web"), passedIn(rerun, "gpui")]);
		// Only the scenarios that run selected; a prefixed run never ran the others.
		const earlier = new Set((await readFile(join(rerun, "scenarios.txt"), "utf8")).split("\n").filter(Boolean));
		selected = selected.filter((scenario) => earlier.has(scenario.id) && (!web.has(scenario.id) || !gpui.has(scenario.id)));
		console.log(`Rerunning ${selected.length} scenarios not passed in ${rerun}`);
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

async function passedIn(run: string, driver: string): Promise<Set<string>> {
	try {
		// SAFETY: runWeb and runGpui write each driver report as { driver, scenarios }. A report of another shape throws in the filter, and the catch returns no passes.
		const report = JSON.parse(await readFile(join(run, `${driver}.json`), "utf8")) as { scenarios: { id: string; status: string }[] };
		return new Set(report.scenarios.filter((result) => result.status === "passed").map((result) => result.id));
	} catch { return new Set(); }
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
const fail = (message: string) => { failure = failure ? `${failure}; ${message}` : message; };
/** Runs the requested drivers on the selected scenarios and records failures through `fail`. */
async function runDrivers(contract: Contract) {
	// The selection is what PARITY_RERUN reads back, and the GPUI processes run exactly these IDs.
	await writeFile(join(artifacts, "scenarios.txt"), contract.scenarios.map((scenario) => scenario.id).join("\n"));
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
	if (driver !== "web") {
		const exported = Bun.spawn(["bun", "e2e/parity/visual/export-fixture.ts", fixture], { stdout: "inherit", stderr: "inherit" });
		const exportExit = exported.exited.then((code) => { mark("fixture"); return code; });
		const cancelExport = () => exported.kill();
		controller.signal.addEventListener("abort", cancelExport, { once: true });
		runs.push(runGpui(contract, artifacts, { path: fixture, exit: exportExit }, jobs, controller.signal, mark, coverage)
			.then((problem) => { if (problem) fail(problem); }, (error) => fail(`GPUI driver: ${String(error)}`))
			.finally(async () => { await exportExit; controller.signal.removeEventListener("abort", cancelExport); }));
	}
	if (driver !== "gpui") runs.push(runWeb(contract, artifacts, controller.signal, webJobs).catch((error) => fail(`Web driver: ${String(error)}`)).finally(() => mark("web")));
	await Promise.all(runs);
	controller.signal.throwIfAborted();
	const drivers = driver ? [driver] : ["web", "gpui"] as const;
	const reports = await Promise.all(drivers.map(async (name) => JSON.parse(await readFile(join(artifacts, `${name}.json`), "utf8"))));
	for (const [index, name] of drivers.entries()) {
		try { validateDriverReport(contract, name, reports[index]); }
		catch (error) { fail(String(error)); }
	}
	if (!driver && !failure) validateReports(contract, reports);
	if (coverage) {
		// Scenarios that failed stay out of the map, so selection always runs them.
		const map = await writeRunMap(artifacts, join(artifacts, "coverage-map.json"));
		mark("coverageMap");
		console.log(`Coverage map: ${map.scenarios.length} scenarios, ${Object.keys(map.files).length} files`);
		const narrowed = ["PARITY_PREFIX", "PARITY_EXCLUDE", "PARITY_CHANGED", "PARITY_RERUN", "PARITY_SUITE_SHARD"].some((name) => process.env[name]);
		if (!narrowed) await copyFile(join(artifacts, "coverage-map.json"), resolve(DEFAULT_MAP));
	}
}

try {
	const contract = await loadContract();
	requireViewCoverage(contract, VIEWS);
	contract.scenarios = await selectScenarios(contract.scenarios, rerun);
	if (contract.scenarios.length) await runDrivers(contract);
	else if (rerun) console.log("Nothing to rerun: both drivers passed every selected scenario.");
	// A change no scenario can observe, or a CI shard with no share of a small selection.
	else if (process.env.PARITY_CHANGED) console.log("No parity scenarios selected by the changes.");
	else throw new Error("No parity scenarios selected");
} catch (error) { fail(String(error)); }
finally {
	process.removeListener("SIGINT", interrupt); process.removeListener("SIGTERM", interrupt);
	mark("total");
	await writeFile(join(artifacts, "result.json"), JSON.stringify({ level: "UI integration with mocks", driver: driver ?? "both", status: failure ? "failed" : "passed", error: failure, phases }, null, 2));
	console.log(`Parity artifacts: ${artifacts}`);
}
if (failure) { console.error(failure); process.exitCode = 1; }
