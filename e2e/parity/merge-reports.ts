import { readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { type Contract, loadContract, validateReports } from "./contract";
import { object, string } from "./contract-values";

export type Driver = "web" | "gpui";
type Report = { driver: Driver; scenarios: unknown[] };

export function parseDriver<Value>(value: Value): Driver {
	if (value === "web") return "web";
	if (value === "gpui") return "gpui";
	throw new Error(`Invalid parity driver: ${String(value)}`);
}

/** Requires exactly one passing result for every selected scenario from this driver. */
export function validateDriverReport<Report>(contract: Contract, driver: Driver, report: Report): void {
	const input = object(report);
	if (input.driver !== driver) throw new Error(`Expected ${driver} report`);
	if (!Array.isArray(input.scenarios)) throw new Error(`Missing scenarios: ${driver}`);
	const expected = new Set(contract.scenarios.map(({ id }) => id));
	for (const value of input.scenarios) {
		const result = object(value);
		const id = string(result.id);
		if (!expected.delete(id) || result.status !== "passed") throw new Error(`Invalid ${driver} result: ${id}`);
	}
	if (expected.size) throw new Error(`Missing ${driver} results: ${[...expected].join(", ")}`);
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

/**
 * A shard uploads its artifact as `<name>-attempt-<n>`. "Re-run failed jobs" keeps the passed
 * shards' artifacts from the earlier attempt and adds new ones for the shards it ran again, so
 * the newest attempt of each shard is its result, whether it passed or failed.
 */
export function newestAttempts(names: string[]): string[] {
	const newest = new Map<string, { name: string; attempt: number }>();
	for (const name of names) {
		const match = /^(.*)-attempt-(\d+)$/.exec(name);
		const [shard, attempt] = match ? [string(match[1]), Number(match[2])] : [name, 0];
		if (attempt >= (newest.get(shard)?.attempt ?? 0)) newest.set(shard, { name, attempt });
	}
	return [...newest.values()].map(({ name }) => name);
}

/** Reads downloaded artifacts. Each run must have finished and passed its declared driver. */
export async function collectReports(root: string): Promise<unknown[]> {
	const reports: unknown[] = [];
	async function visit(directory: string): Promise<void> {
		const entries = await readdir(directory, { withFileTypes: true });
		if (entries.some((entry) => entry.name === "scenarios.txt")) {
			const result = object(JSON.parse(await readFile(join(directory, "result.json"), "utf8")));
			if (result.status !== "passed") throw new Error(`Parity run did not pass: ${directory}`);
			const driver = parseDriver(result.driver);
			const ids = (await readFile(join(directory, "scenarios.txt"), "utf8")).split("\n").filter(Boolean);
			const report: unknown = JSON.parse(await readFile(join(directory, `${driver}.json`), "utf8"));
			validateDriverReport({ scenarios: ids.map((id) => ({ id, title: id, steps: [] })) }, driver, report);
			reports.push(report);
			return;
		}
		for (const entry of entries) if (entry.isDirectory()) await visit(join(directory, entry.name));
	}
	const artifacts = (await readdir(root, { withFileTypes: true })).filter((entry) => entry.isDirectory()).map(({ name }) => name);
	for (const artifact of newestAttempts(artifacts)) await visit(join(root, artifact));
	if (!reports.length) throw new Error(`No parity reports found: ${root}`);
	return reports;
}

if (import.meta.main) {
	const root = string(process.argv[2]);
	const contract = await loadContract();
	const reports = mergeReports(contract, await collectReports(root));
	await writeFile(join(root, "merged-reports.json"), JSON.stringify(reports, null, 2));
	console.log(`Both drivers passed all ${contract.scenarios.length} scenarios; merged reports validated.`);
}
