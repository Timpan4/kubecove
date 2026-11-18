import { createHash } from "node:crypto";
import { readdir, readFile, stat } from "node:fs/promises";
import { join } from "node:path";
import { isString, object, string } from "./contract-values";
import type { Driver } from "./merge-reports";

export const DRIVER_SELECTION_FORMAT = "driver-files-v1";
export const driverSelectionFile = { web: "web-scenarios.txt", gpui: "gpui-scenarios.txt" };

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
		const selected = await recordedSelections(path, result, hashes, { skipUnrecorded: true });
		if (requested ? selected[requested].size : selected.web.size || selected.gpui.size) return path;
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

async function recordedSelections(
	run: string,
	metadata: ReturnType<typeof object>,
	hashes: Record<Driver, string> | undefined,
	{ skipUnrecorded = false } = {},
): Promise<Record<Driver, Set<string>>> {
	const [web, gpui, shared] = await Promise.all([
		readRunFile(run, driverSelectionFile.web),
		readRunFile(run, driverSelectionFile.gpui),
		hashes ? undefined : readRunFile(run, "scenarios.txt"),
	]);
	const modern = metadata.selectionFormat !== undefined || hashes || web !== undefined || gpui !== undefined;
	if (skipUnrecorded && !hashes && !web && !gpui && !shared) return { web: new Set(), gpui: new Set() };
	if (!modern) {
		if (shared === undefined) throw new Error(`Missing scenarios.txt in ${run}. Restore the recorded selection before rerunning.`);
		const ids = scenarioIds(shared);
		if (metadata.driver === "both") return { web: ids, gpui: ids };
		if (metadata.driver === "web") return { web: ids, gpui: new Set() };
		if (metadata.driver === "gpui") return { web: new Set(), gpui: ids };
		throw new Error(`Legacy driver metadata is missing or invalid in ${run}. Restore result.json before rerunning.`);
	}
	if (web === undefined || gpui === undefined) {
		const missing = [web === undefined ? "web" : "", gpui === undefined ? "GPUI" : ""].filter(Boolean);
		const label = missing.length === 1 ? "selection" : "selections";
		throw new Error(`Missing ${missing.join(" and ")} ${label} in ${run}. Restore its driver selection files before rerunning.`);
	}
	if (!hashes) {
		throw new Error(`Missing driver selection hashes in ${join(run, "result.json")}. Restore result.json before rerunning.`);
	}
	if (hashes.web !== selectionHash(web) || hashes.gpui !== selectionHash(gpui)) {
		throw new Error(`Selection files in ${run} do not match result.json. Restore the recorded driver selections before rerunning.`);
	}
	return { web: scenarioIds(web), gpui: scenarioIds(gpui) };
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
export async function rerunSelections(run: string, { coverage = false } = {}): Promise<Record<Driver, Set<string>>> {
	let metadata: ReturnType<typeof object> = {};
	try {
		const result = await readRunFile(run, "result.json");
		if (result !== undefined) metadata = object(JSON.parse(result));
	} catch {
		// Selection validation reports what must be restored.
	}
	const hashes = selectionHashes(run, metadata);
	const recorded = await recordedSelections(run, metadata, hashes);
	if (coverage && metadata.coverageMapFailed === true) return recorded;
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
	if (metadata.status === "failed" && !web.size && !gpui.size) return recorded;
	return { web, gpui };
}
