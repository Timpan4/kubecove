import { readFile } from "node:fs/promises";
import { join } from "node:path";
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
	// A missing or incomplete driver report leaves every unreported selected scenario eligible.
	const passed = new Set<string>();
	if (results !== undefined) {
		// Both drivers write { scenarios: { id, status }[] }. A truncated report contributes no passes.
		try {
			const report = JSON.parse(results) as { scenarios: { id: string; status: string }[] };
			for (const { id, status } of report.scenarios) if (status === "passed") passed.add(id);
		} catch {
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
	const previous = result === undefined ? undefined : (JSON.parse(result) as { driver?: string }).driver;
	const [web, gpui] = await Promise.all([
		failedScenarios(run, "web", previous === "gpui" ? "" : legacySelection),
		failedScenarios(run, "gpui", previous === "web" ? "" : legacySelection),
	]);
	return { web, gpui };
}
