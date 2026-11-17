import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { object, string } from "./contract-values";
import type { Driver } from "./merge-reports";

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

	const passed = new Set<string>();
	try {
		const report = object(JSON.parse(await readFile(join(run, `${driver}.json`), "utf8")));
		if (Array.isArray(report.scenarios)) {
			for (const value of report.scenarios) {
				const { id, status } = object(value);
				if (status === "passed") passed.add(string(id));
			}
		}
	} catch {
		// An unreadable or incomplete report cannot prove that any of its scenarios passed.
		return ids;
	}
	for (const id of passed) ids.delete(id);
	return ids;
}

/** Reads the recorded selection for each driver, including reports from older runners. */
export async function rerunSelections(run: string): Promise<Record<Driver, Set<string>>> {
	let [webSelection, gpuiSelection] = await Promise.all([
		readRunFile(run, "web-scenarios.txt"),
		readRunFile(run, "gpui-scenarios.txt"),
	]);
	if (webSelection === undefined || gpuiSelection === undefined) {
		const [legacySelection, result] = await Promise.all([
			readFile(join(run, "scenarios.txt"), "utf8"),
			readRunFile(run, "result.json"),
		]);
		// Older runners recorded a shared selection and which driver ran.
		const previous = result === undefined ? undefined : object(JSON.parse(result)).driver;
		webSelection ??= previous === "gpui" ? "" : legacySelection;
		gpuiSelection ??= previous === "web" ? "" : legacySelection;
	}
	const [web, gpui] = await Promise.all([
		failedScenarios(run, "web", webSelection),
		failedScenarios(run, "gpui", gpuiSelection),
	]);
	return { web, gpui };
}
