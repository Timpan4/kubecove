import { mkdir, readdir, readFile, stat, writeFile } from "node:fs/promises";
import { availableParallelism } from "node:os";
import { join, resolve } from "node:path";
import { type Contract, loadContract, requireViewCoverage, validateReports } from "../e2e/parity/contract";
import { runGpui } from "../e2e/parity/gpui-run";
import { VIEWS } from "../e2e/parity/visual/views";
import { runWeb } from "../e2e/parity/web";

// PARITY_JOBS: parallel web pages and GPUI processes each. The default splits the CPUs
// between the two drivers. Measured on 16 CPUs (2026-11-06): 16 per driver finished no
// sooner than 8 per driver and timed out more web scenarios. `selectScenarios` lists the
// selection variables.

type Scenario = Contract["scenarios"][number];

/**
 * The scenarios this run executes, in contract order. Each optional filter narrows the one
 * before it; a filter by changed files belongs here too.
 * - PARITY_PREFIX: comma-separated scenario ID prefixes.
 * - PARITY_RERUN, resolved to `rerun`: only the scenarios either driver did not pass there.
 * - PARITY_SUITE_SHARD=<index>/<count>: every `count`-th scenario from position `index`, so
 *   separate CI jobs split one suite.
 */
async function selectScenarios(scenarios: Scenario[], rerun: string | undefined): Promise<Scenario[]> {
	const prefixes = (process.env.PARITY_PREFIX ?? "").split(",").map((prefix) => prefix.trim()).filter(Boolean);
	let selected = prefixes.length ? scenarios.filter((scenario) => prefixes.some((prefix) => scenario.id.startsWith(prefix))) : scenarios;
	if (rerun) {
		const [web, gpui] = await Promise.all([passedIn(rerun, "web"), passedIn(rerun, "gpui")]);
		// Only the scenarios that run selected; a prefixed run never ran the others.
		const earlier = new Set((await readFile(join(rerun, "scenarios.txt"), "utf8")).split("\n").filter(Boolean));
		selected = selected.filter((scenario) => earlier.has(scenario.id) && (!web.has(scenario.id) || !gpui.has(scenario.id)));
		console.log(`Rerunning ${selected.length} scenarios not passed in ${rerun}`);
	}
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
		const report = JSON.parse(await readFile(join(run, `${driver}.json`), "utf8")) as { scenarios: { id: string; status: string }[] };
		return new Set(report.scenarios.filter((result) => result.status === "passed").map((result) => result.id));
	} catch { return new Set(); }
}

function notPassed(driver: string, ids: string[], report: { scenarios?: { id: string; status: string }[] }): string | undefined {
	const passed = new Set((report.scenarios ?? []).filter((result) => result.status === "passed").map((result) => result.id));
	const missing = ids.filter((id) => !passed.has(id));
	return missing.length ? `${driver} did not pass ${missing.length}: ${missing.join(", ")}` : undefined;
}

const rerun = process.env.PARITY_RERUN === "last" ? await lastRun() : process.env.PARITY_RERUN;
const artifacts = resolve("e2e/artifacts", `parity-${crypto.randomUUID()}`);
await mkdir(artifacts, { recursive: true });
await writeFile(join(artifacts, "web.json"), JSON.stringify({ driver: "web", scenarios: [] }));
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
try {
	const contract = await loadContract();
	requireViewCoverage(contract, VIEWS);
	contract.scenarios = await selectScenarios(contract.scenarios, rerun);
	if (!contract.scenarios.length) {
		if (rerun) {
			console.log("Nothing to rerun: both drivers passed every selected scenario.");
			process.exit(0);
		}
		throw new Error("No parity scenarios selected");
	}
	// The selection is what PARITY_RERUN reads back, and the GPUI processes run exactly these IDs.
	await writeFile(join(artifacts, "scenarios.txt"), contract.scenarios.map((scenario) => scenario.id).join("\n"));
	const jobs = Number(process.env.PARITY_JOBS ?? Math.max(1, Math.floor(availableParallelism() / 2)));
	if (!Number.isInteger(jobs) || jobs < 1) throw new Error(`PARITY_JOBS must be a positive integer: ${process.env.PARITY_JOBS}`);
	// The GPUI test processes render the same browser-mock responses the web driver reads.
	// Only they read the fixture, so the web driver and the GPUI build start during the export.
	const fixture = join(artifacts, "fixture.json");
	const exported = Bun.spawn(["bun", "e2e/parity/visual/export-fixture.ts", fixture], { stdout: "inherit", stderr: "inherit" });
	const exportExit = exported.exited.then((code) => { mark("fixture"); return code; });
	await Promise.all([
		runWeb(contract, artifacts, controller.signal, jobs).catch((error: unknown) => fail(`Web driver: ${String(error)}`)).finally(() => mark("web")),
		runGpui(contract, artifacts, { path: fixture, exit: exportExit }, jobs, controller.signal, mark).then((problem) => { if (problem) fail(problem); }, (error: unknown) => fail(`GPUI driver: ${String(error)}`)),
	]);
	controller.signal.throwIfAborted();
	const reports = await Promise.all(["web", "gpui"].map(async (driver) => JSON.parse(await readFile(join(artifacts, `${driver}.json`), "utf8"))));
	const ids = contract.scenarios.map((scenario) => scenario.id);
	for (const [index, driver] of ["web", "gpui"].entries()) {
		const problem = notPassed(driver, ids, reports[index]);
		if (problem) fail(problem);
	}
	if (!failure) validateReports(contract, reports);
} catch (error) { fail(String(error)); }
finally {
	process.removeListener("SIGINT", interrupt); process.removeListener("SIGTERM", interrupt);
	mark("total");
	await writeFile(join(artifacts, "result.json"), JSON.stringify({ level: "UI integration with mocks", status: failure ? "failed" : "passed", error: failure, phases }, null, 2));
	console.log(`Parity artifacts: ${artifacts}`);
}
if (failure) { console.error(failure); process.exitCode = 1; }
