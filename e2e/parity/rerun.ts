import { createHash } from "node:crypto";
import { readdir, readFile, stat } from "node:fs/promises";
import { join } from "node:path";
import { isString, object, string } from "./contract-values";
import type { Driver } from "./merge-reports";

export const DRIVER_SELECTION_FORMAT = "driver-files-v1";
export const driverSelectionFile = { web: "web-scenarios.txt", gpui: "gpui-scenarios.txt" };

class MissingLegacyDriver extends Error {}

function isMissingFile<Value>(error: Value): boolean {
	return error instanceof Error && "code" in error && error.code === "ENOENT";
}

export function selectionHash(contents: string): string {
	return createHash("sha256").update(contents).digest("hex");
}

function selectionHashes(run: string, metadata: ReturnType<typeof object>): Record<Driver, string> | undefined {
	if (metadata.selectionHashes === undefined) return undefined;
	try {
		const { web, gpui } = object(metadata.selectionHashes);
		const sha256 = /^[0-9a-f]{64}$/;
		if (isString(web) && isString(gpui) && sha256.test(web) && sha256.test(gpui)) return { web, gpui };
	} catch {
		// Report which artifact needs restoring for malformed manifests as well.
	}
	throw new Error(`Invalid driver selection hashes in ${join(run, "result.json")}. Restore result.json before rerunning.`);
}

/** Finds the newest finished, non-interrupted run that recorded work. */
export async function lastRerunSource(root: string, requested?: Driver): Promise<string> {
	const names = await readdir(root).catch((error) => {
		if (isMissingFile(error)) return [];
		throw error;
	});
	const runs = await Promise.all(names.filter((name) => name.startsWith("parity-")).map(async (name) => {
		const path = join(root, name);
		const resultPath = join(path, "result.json");
		try {
			const result = object(JSON.parse(await readFile(resultPath, "utf8")));
			if (result.interrupted === true) return undefined;
			if (result.interrupted === undefined && isString(result.error) && result.error.includes("Interrupted")) return undefined;
			return { path, result, time: (await stat(resultPath)).mtimeMs };
		} catch { return undefined; }
	}));
	const newestFirst = runs.flatMap((run) => run ? [run] : []).sort((left, right) => right.time - left.time);
	const emptyHash = selectionHash("");
	for (const { path, result } of newestFirst) {
		const hashes = selectionHashes(path, result);
		if (hashes) {
			const hasWork = requested ? hashes[requested] !== emptyHash : hashes.web !== emptyHash || hashes.gpui !== emptyHash;
			if (!hasWork) continue;
		}
		try {
			const selected = await recordedSelections(path, result, hashes, requested);
			if (requested ? selected[requested].size : selected.web.size || selected.gpui.size) return path;
		} catch (error) {
			if (hashes || error instanceof MissingLegacyDriver) throw error;
		}
	}
	throw new Error("PARITY_RERUN=last found no finished parity run");
}

async function readRunFile(run: string, name: string): Promise<string | undefined> {
	try {
		return await readFile(join(run, name), "utf8");
	} catch (error) {
		if (isMissingFile(error)) return undefined;
		throw error;
	}
}

function scenarioIds(contents: string): Set<string> {
	return new Set(contents.split(/\r?\n/).filter(Boolean));
}

async function recordedSelections(run: string, metadata: ReturnType<typeof object>, hashes: Record<Driver, string> | undefined, requested?: Driver): Promise<Record<Driver, Set<string>>> {
	const [web, gpui] = await Promise.all([
		readRunFile(run, driverSelectionFile.web),
		readRunFile(run, driverSelectionFile.gpui),
	]);
	if (web === undefined && gpui === undefined && !hashes && metadata.selectionFormat !== DRIVER_SELECTION_FORMAT) {
		const ids = scenarioIds(await readFile(join(run, "scenarios.txt"), "utf8"));
		if (!ids.size) return { web: ids, gpui: ids };
		const previous = metadata.driver;
		if (previous === "both") return { web: ids, gpui: ids };
		const selected = previous === "web" || previous === "gpui" ? previous : requested;
		if (selected === "web") return { web: ids, gpui: new Set() };
		if (selected === "gpui") return { web: new Set(), gpui: ids };
		throw new MissingLegacyDriver(`Legacy driver metadata is missing or invalid in ${run}. Restore result.json or set PARITY_DRIVER.`);
	}
	if (web === undefined || gpui === undefined) {
		const missing = [web === undefined ? "web" : "", gpui === undefined ? "GPUI" : ""].filter(Boolean);
		const label = missing.length === 1 ? "selection" : "selections";
		throw new Error(`Missing ${missing.join(" and ")} ${label} in ${run}. Restore its driver selection files before rerunning.`);
	}
	const selected = { web: scenarioIds(web), gpui: scenarioIds(gpui) };
	if (hashes) {
		if (hashes.web !== selectionHash(web) || hashes.gpui !== selectionHash(gpui)) {
			throw new Error(`Selection files in ${run} do not match result.json. Restore the recorded driver selections before rerunning.`);
		}
		return selected;
	}
	// Earlier results recorded the effective mode and sometimes a shared list.
	if (selected.web.size || selected.gpui.size) {
		const executed = (["web", "gpui"] as const).filter((name) => selected[name].size);
		const recorded = executed.length === 1 ? executed[0] : "both";
		if ((metadata.driver === "web" || metadata.driver === "gpui" || metadata.driver === "both") && metadata.driver !== recorded) {
			throw new Error(`Selection files in ${run} do not match the recorded driver. Restore the recorded driver selections before rerunning.`);
		}
	}
	const shared = await readRunFile(run, "scenarios.txt");
	if (shared !== undefined) {
		const recorded = scenarioIds(shared);
		const union = new Set([...selected.web, ...selected.gpui]);
		if (recorded.size !== union.size || [...recorded].some((id) => !union.has(id))) {
			throw new Error(`Driver selections in ${run} do not match scenarios.txt. Restore the recorded driver selections before rerunning.`);
		}
	}
	return selected;
}

async function failedScenarios(run: string, driver: Driver, ids: Set<string>, driverFailed: boolean): Promise<Set<string>> {
	// Failures outside scenario execution can depend on the original selection.
	if (!ids.size || driverFailed) return ids;

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
	return new Set([...ids].filter((id) => !results.get(id)));
}

/** Reads the recorded selection for each driver, including reports from older runners. */
export async function rerunSelections(run: string, requested?: Driver): Promise<Record<Driver, Set<string>>> {
	let metadata: ReturnType<typeof object> = {};
	try {
		const result = await readRunFile(run, "result.json");
		if (result !== undefined) metadata = object(JSON.parse(result));
	} catch {
		// Newer runs can still be retried if both selection files are intact.
	}
	const hashes = selectionHashes(run, metadata);
	const recorded = await recordedSelections(run, metadata, hashes, requested);
	// A scenario report cannot establish that driver cleanup succeeded.
	if (metadata.status !== "passed" && metadata.status !== "failed") return recorded;
	const failedDrivers = metadata.failedDrivers;
	if (failedDrivers !== undefined && (!Array.isArray(failedDrivers) || failedDrivers.some((name) => name !== "web" && name !== "gpui"))) return recorded;
	const legacyError = metadata.status === "failed" && isString(metadata.error) ? metadata.error : "";
	const driverFailed = (driver: Driver, legacyLabel: string) =>
		failedDrivers === undefined ? legacyError.includes(legacyLabel) : failedDrivers.includes(driver);
	const [web, gpui] = await Promise.all([
		failedScenarios(run, "web", recorded.web, driverFailed("web", "Web driver:")),
		failedScenarios(run, "gpui", recorded.gpui, driverFailed("gpui", "GPUI driver:")),
	]);
	return { web, gpui };
}
