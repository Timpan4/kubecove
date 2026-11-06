import { mkdir, readdir, readFile, stat, writeFile } from "node:fs/promises";
import { availableParallelism } from "node:os";
import { join, resolve } from "node:path";
import { loadContract, requireViewCoverage, validateReports } from "../e2e/parity/contract";
import { runGpui } from "../e2e/parity/gpui-run";
import { VIEWS } from "../e2e/parity/visual/views";
import { runWeb } from "../e2e/parity/web";

// Selection, all optional:
// - PARITY_PREFIX: comma-separated scenario ID prefixes.
// - PARITY_RERUN: an earlier artifacts directory, or "last", to run only the scenarios
//   that either driver did not pass there.
// - PARITY_JOBS: parallel web pages and GPUI processes each. The default splits the CPUs
//   between the two drivers. Measured on 16 CPUs (2026-11-06): 16 per driver finished no
//   sooner than 8 per driver and timed out more web scenarios.

/**
 * The newest earlier run that finished, by its result.json. A run that was interrupted or is
 * still going has none, and its empty reports would select every scenario.
 */
async function lastRun(): Promise<string> {
	const root = resolve("e2e/artifacts");
	const runs = await Promise.all((await readdir(root)).filter((name) => name.startsWith("parity-")).map(async (name) => {
		try { return { path: join(root, name), time: (await stat(join(root, name, "result.json"))).mtimeMs }; } catch { return undefined; }
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
const controller = new AbortController();
const interrupt = () => controller.abort(new Error("Interrupted"));
process.once("SIGINT", interrupt);
process.once("SIGTERM", interrupt);
let failure: string | undefined;
const fail = (message: string) => { failure = failure ? `${failure}; ${message}` : message; };
try {
	const contract = await loadContract();
	requireViewCoverage(contract, VIEWS);
	const prefixes = (process.env.PARITY_PREFIX ?? "").split(",").map((prefix) => prefix.trim()).filter(Boolean);
	if (prefixes.length) contract.scenarios = contract.scenarios.filter((scenario) => prefixes.some((prefix) => scenario.id.startsWith(prefix)));
	if (rerun) {
		const [web, gpui] = await Promise.all([passedIn(rerun, "web"), passedIn(rerun, "gpui")]);
		contract.scenarios = contract.scenarios.filter((scenario) => !web.has(scenario.id) || !gpui.has(scenario.id));
		console.log(`Rerunning ${contract.scenarios.length} scenarios not passed in ${rerun}`);
	}
	if (!contract.scenarios.length) throw new Error("No parity scenarios selected");
	const jobs = Number(process.env.PARITY_JOBS ?? Math.max(1, Math.floor(availableParallelism() / 2)));
	if (!Number.isInteger(jobs) || jobs < 1) throw new Error(`PARITY_JOBS must be a positive integer: ${process.env.PARITY_JOBS}`);
	// The GPUI driver renders the same browser-mock responses the web driver reads.
	const fixture = join(artifacts, "fixture.json");
	const exported = Bun.spawn(["bun", "e2e/parity/visual/export-fixture.ts", fixture], { stdout: "inherit", stderr: "inherit" });
	if (await exported.exited !== 0) throw new Error("Fixture export failed");
	// The drivers share only the exported fixture, so they run at the same time.
	await Promise.all([
		runWeb(contract, artifacts, controller.signal, jobs).catch((error: unknown) => fail(`Web driver: ${String(error)}`)),
		runGpui(contract, artifacts, fixture, jobs, controller.signal).then((problem) => { if (problem) fail(problem); }, (error: unknown) => fail(`GPUI driver: ${String(error)}`)),
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
	await writeFile(join(artifacts, "result.json"), JSON.stringify({ level: "UI integration with mocks", status: failure ? "failed" : "passed", error: failure }, null, 2));
	console.log(`Parity artifacts: ${artifacts}`);
}
if (failure) { console.error(failure); process.exitCode = 1; }
