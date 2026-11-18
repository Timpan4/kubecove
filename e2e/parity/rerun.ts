import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { object, string } from "./contract-values";
import { type Driver, parseDriver } from "./merge-reports";

async function readRunFile(run: string, name: string): Promise<string | undefined> {
	try {
		return await readFile(join(run, name), "utf8");
	} catch (error) {
		if (error instanceof Error && "code" in error && error.code === "ENOENT") return undefined;
		throw error;
	}
}

async function failedScenarios(run: string, driver: Driver, selected: string): Promise<Set<string>> {
	const ids = new Set(selected.split("\n").filter(Boolean));
	if (!ids.size) return ids;

	const results = new Map<string, boolean>();
	try {
		const report = object(JSON.parse(await readFile(join(run, `${driver}.json`), "utf8")));
		if (report.driver !== driver || !Array.isArray(report.scenarios)) return ids;
		for (const value of report.scenarios) {
			const result = object(value);
			const id = string(result.id);
			if (!ids.has(id)) return ids;
			results.set(id, !results.has(id) && result.status === "passed");
		}
	} catch {
		// An unreadable or incomplete report cannot prove that any of its scenarios passed.
		return ids;
	}
	for (const [id, passed] of results) if (passed) ids.delete(id);
	return ids;
}

/** Reads the recorded selection for each driver, including reports from older runners. */
export async function rerunSelections(run: string): Promise<Record<Driver, Set<string>>> {
	let [webSelection, gpuiSelection] = await Promise.all([
		readRunFile(run, "web-scenarios.txt"),
		readRunFile(run, "gpui-scenarios.txt"),
	]);
	if (webSelection === undefined && gpuiSelection === undefined) {
		const legacySelection = await readFile(join(run, "scenarios.txt"), "utf8");
		let previous: Driver | undefined;
		try {
			const result = await readRunFile(run, "result.json");
			if (result !== undefined) {
				const recorded = object(JSON.parse(result)).driver;
				if (recorded !== "both") previous = parseDriver(recorded);
			}
		} catch {
			console.warn(`Could not read legacy driver metadata in ${run}. Considering both drivers for its shared selection.`);
		}
		webSelection = previous === "gpui" ? "" : legacySelection;
		gpuiSelection = previous === "web" ? "" : legacySelection;
	} else if (webSelection === undefined || gpuiSelection === undefined) {
		const missing = webSelection === undefined ? "web" : "GPUI";
		// The runner writes the shared list only after publishing both driver lists.
		if (await readRunFile(run, "scenarios.txt") !== undefined) {
			throw new Error(`Missing ${missing} selection in ${run}. Restore its driver selection before rerunning.`);
		}
		console.warn(`No ${missing} selection was recorded in ${run}.`);
	}
	const [web, gpui] = await Promise.all([
		failedScenarios(run, "web", webSelection ?? ""),
		failedScenarios(run, "gpui", gpuiSelection ?? ""),
	]);
	return { web, gpui };
}
