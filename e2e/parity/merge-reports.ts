import { readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { type Contract, loadContract, validateReports } from "./contract";
import { count, object, string } from "./contract-values";
import { selectedContract } from "./selected-contract";
import { newestAttempts } from "./shard-attempts";

export type Driver = "web" | "gpui";
type Report = { driver: Driver; scenarios: unknown[] };

export function parseDriver<Value>(value: Value): Driver {
	if (value === "web") return "web";
	if (value === "gpui") return "gpui";
	throw new Error(`Invalid parity driver: ${String(value)}`);
}

export function driverReportRows<Report>(driver: Driver, report: Report): unknown[] {
	const input = object(report);
	if (input.driver !== driver) throw new Error(`Expected ${driver} report`);
	if (!Array.isArray(input.scenarios)) throw new Error(`Missing scenarios: ${driver}`);
	return input.scenarios;
}

function scenarioResult(driver: Driver, id: string, result: ReturnType<typeof object>) {
	const status = result.status;
	if (status !== "passed" && status !== "failed") throw new Error(`Invalid ${driver} result: ${id}`);
	return { id, status };
}

export function driverScenarioResults<Report>(driver: Driver, report: Report) {
	return driverReportRows(driver, report).map((value) => {
		const result = object(value);
		return scenarioResult(driver, string(result.id), result);
	});
}

/** Preserves only unique, valid selected rows and reports validation failures. */
export function inspectDriverResults<Report>(contract: Contract, driver: Driver, report: Report) {
	const expected = new Set(contract.scenarios.map(({ id }) => id));
	const results = new Map<string, ReturnType<typeof scenarioResult>>();
	const problems: string[] = [];
	for (const value of driverReportRows(driver, report)) {
		try {
			const result = object(value);
			const id = string(result.id);
			if (!expected.delete(id)) {
				results.delete(id);
				throw new Error(`Invalid ${driver} result: ${id}`);
			}
			results.set(id, scenarioResult(driver, id, result));
		} catch (error) {
			problems.push(error instanceof Error ? error.message : String(error));
		}
	}
	if (expected.size) problems.push(`Missing ${driver} results: ${[...expected].join(", ")}`);
	return { results: [...results.values()], problems };
}

/** Requires exactly one passing result for every selected scenario from this driver. */
export function validateDriverReport<Report>(contract: Contract, driver: Driver, report: Report): void {
	const { results, problems } = inspectDriverResults(contract, driver, report);
	if (problems.length) throw new Error(problems.join("; "));
	const failed = results.find(({ status }) => status === "failed");
	if (failed) throw new Error(`Invalid ${driver} result: ${failed.id}`);
}

/** Concatenation preserves duplicate IDs, so validateReports rejects overlapping shards. */
export function mergeReports(contract: Contract, values: unknown[]): Report[] {
	const merged: Report[] = [{ driver: "web", scenarios: [] }, { driver: "gpui", scenarios: [] }];
	for (const value of values) {
		const report = object(value);
		const driver = parseDriver(report.driver);
		if (!Array.isArray(report.scenarios)) throw new Error(`Missing scenarios: ${driver}`);
		merged[driver === "web" ? 0 : 1].scenarios.push(...report.scenarios);
	}
	validateReports(contract, merged);
	return merged;
}

/** Reads downloaded artifacts. Each run must have finished and passed its declared driver. */
export async function collectReports(root: string, selectionAttempt = 0): Promise<unknown[]> {
	const reports: unknown[] = [];
	async function visit(directory: string): Promise<void> {
		const entries = await readdir(directory, { withFileTypes: true });
		if (entries.some((entry) => entry.name === "scenarios.txt")) {
			const result = object(JSON.parse(await readFile(join(directory, "result.json"), "utf8")));
			if (result.status !== "passed") throw new Error(`Parity run did not pass: ${directory}`);
			const drivers = result.driver === "both" ? (["web", "gpui"] as const) : [parseDriver(result.driver)];
			const ids = (await readFile(join(directory, "scenarios.txt"), "utf8")).split("\n").filter(Boolean);
			const selection = { scenarios: ids.map((id) => ({ id, title: id, steps: [] })) };
			for (const driver of drivers) {
				const report: unknown = JSON.parse(await readFile(join(directory, `${driver}.json`), "utf8"));
				validateDriverReport(selection, driver, report);
				reports.push(report);
			}
			return;
		}
		for (const entry of entries) if (entry.isDirectory()) await visit(join(directory, entry.name));
	}
	const artifacts = (await readdir(root, { withFileTypes: true })).filter((entry) => entry.isDirectory()).map(({ name }) => name);
	// Re-running the planner can change the selected IDs and shard count. Reports from
	// before that plan belong to a different selection. A failed-shard rerun keeps the
	// original plan's attempt and can still use its earlier successful shard reports.
	const current = artifacts.filter((name) => Number(/-attempt-(\d+)$/.exec(name)?.[1] ?? 0) >= selectionAttempt);
	for (const artifact of newestAttempts(current)) await visit(join(root, artifact));
	if (!reports.length) throw new Error(`No parity reports found: ${root}`);
	return reports;
}

if (import.meta.main) {
	const root = string(process.argv[2]);
	const contract = await loadContract();
	const selected = selectedContract(contract, process.env.PARITY_SCENARIOS);
	const selectionAttempt = process.env.PARITY_SELECTION_ATTEMPT ? count(Number(process.env.PARITY_SELECTION_ATTEMPT)) : 0;
	const reports = mergeReports(selected, selected.scenarios.length ? await collectReports(root, selectionAttempt) : []);
	if (selected.scenarios.length) await writeFile(join(root, "merged-reports.json"), JSON.stringify(reports, null, 2));
	const reason = process.env.PARITY_SELECTION_REASON ? string(JSON.parse(process.env.PARITY_SELECTION_REASON)) : "Full contract";
	const summary = selected.scenarios.length
		? `Both drivers passed ${selected.scenarios.length} of ${contract.scenarios.length} scenarios; merged reports validated. ${reason}.\n`
		: `No parity scenarios selected (0 of ${contract.scenarios.length}); no driver jobs were needed. ${reason}.\n`;
	console.log(summary);
	if (process.env.GITHUB_STEP_SUMMARY) await writeFile(process.env.GITHUB_STEP_SUMMARY, summary, { flag: "a" });
}
