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

async function failedScenarios(run: string, driver: Driver, legacySelection: string): Promise<Set<string>> {
	const selected = await readRunFile(run, `${driver}-scenarios.txt`) ?? legacySelection;
	const results = await readRunFile(run, `${driver}.json`);
	const passed = new Set<string>();
	if (results !== undefined) {
		try {
			const report = object(JSON.parse(results));
			if (Array.isArray(report.scenarios)) {
				for (const value of report.scenarios) {
					const { id, status } = object(value);
					if (status === "passed") passed.add(string(id));
				}
			}
		} catch {
			// A truncated or malformed report leaves its selected scenarios eligible.
			passed.clear();
		}
	}
	return new Set(selected.split("\n").filter((id) => id && !passed.has(id)));
}

/** Reads the recorded selection for each driver, including reports from older runners. */
export async function rerunSelections(run: string): Promise<Record<Driver, Set<string>>> {
	const [legacySelection, result] = await Promise.all([
		readFile(join(run, "scenarios.txt"), "utf8"),
		readRunFile(run, "result.json"),
	]);
	// Older runners recorded a shared selection and which driver ran.
	const previous = result === undefined ? undefined : object(JSON.parse(result)).driver;
	const [web, gpui] = await Promise.all([
		failedScenarios(run, "web", previous === "gpui" ? "" : legacySelection),
		failedScenarios(run, "gpui", previous === "web" ? "" : legacySelection),
	]);
	return { web, gpui };
}
