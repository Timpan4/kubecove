import { readdir, readFile, stat } from "node:fs/promises";
import { join } from "node:path";
import { isString, object, string } from "./contract-values";
import { type Driver, parseDriver } from "./merge-reports";

export const DRIVER_SELECTION_FORMAT = "driver-files-v1";

/** Finds the newest finished, non-interrupted run that recorded work. */
export async function lastRerunSource(root: string): Promise<string> {
	const names = await readdir(root);
	const runs = await Promise.all(names.filter((name) => name.startsWith("parity-")).map(async (name) => {
		const path = join(root, name);
		const resultPath = join(path, "result.json");
		try {
			const result = object(JSON.parse(await readFile(resultPath, "utf8")));
			if (isString(result.error) && result.error.includes("Interrupted")) return undefined;
			if (result.selectionFormat === DRIVER_SELECTION_FORMAT) {
				const web = await stat(join(path, "web-scenarios.txt"));
				const gpui = await stat(join(path, "gpui-scenarios.txt"));
				if (!web.isFile() || !gpui.isFile() || (!web.size && !gpui.size)) return undefined;
			} else if (!(await stat(join(path, "scenarios.txt"))).isFile()) return undefined;
			return { path, time: (await stat(resultPath)).mtimeMs };
		} catch { return undefined; }
	}));
	const newest = runs.flatMap((run) => run ? [run] : []).sort((left, right) => right.time - left.time)[0];
	if (!newest) throw new Error("PARITY_RERUN=last found no finished parity run");
	return newest.path;
}

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
		let previous: Driver | undefined;
		let hasDriverSelections = false;
		try {
			const result = await readRunFile(run, "result.json");
			if (result !== undefined) {
				const metadata = object(JSON.parse(result));
				hasDriverSelections = metadata.selectionFormat === DRIVER_SELECTION_FORMAT;
				if (!hasDriverSelections && metadata.driver !== "both") previous = parseDriver(metadata.driver);
			}
		} catch {
			console.warn(`Could not read legacy driver metadata in ${run}. Considering both drivers for its shared selection.`);
		}
		if (hasDriverSelections) throw new Error(`Missing web and GPUI selections in ${run}. Restore its driver selections before rerunning.`);
		const legacySelection = await readFile(join(run, "scenarios.txt"), "utf8");
		webSelection = previous === "gpui" ? "" : legacySelection;
		gpuiSelection = previous === "web" ? "" : legacySelection;
	} else if (webSelection === undefined || gpuiSelection === undefined) {
		const missing = webSelection === undefined ? "web" : "GPUI";
		throw new Error(`Missing ${missing} selection in ${run}. Restore its driver selection before rerunning.`);
	}
	const [web, gpui] = await Promise.all([
		failedScenarios(run, "web", webSelection),
		failedScenarios(run, "gpui", gpuiSelection),
	]);
	return { web, gpui };
}
