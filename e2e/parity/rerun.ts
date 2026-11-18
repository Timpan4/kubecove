import { createHash } from "node:crypto";
import { readdir, readFile, stat } from "node:fs/promises";
import { join, posix } from "node:path";
import { DEFAULT_MAP } from "./changed-scenarios";
import { isString, object } from "./contract-values";
import { type Driver, driverScenarioResults } from "./merge-reports";

export const DRIVER_SELECTION_FORMAT = "driver-files-v1";
export const driverSelectionFile = { web: "web-scenarios.txt", gpui: "gpui-scenarios.txt" };
export type CoverageMapFailure = "generation" | "publication";

type DriverSelections = Record<Driver, Set<string>>;
type RetrySelection = DriverSelections & { coveragePublication?: "retry" | "unknown" };

function isMissingFile<Value>(error: Value): boolean {
	return error instanceof Error && "code" in error && error.code === "ENOENT";
}

export function selectionHash(contents: string): string {
	return createHash("sha256").update(contents).digest("hex");
}

function selectionHashes(run: string, metadata: ReturnType<typeof object>): Record<Driver, string> | undefined {
	if (metadata.selectionFormat !== undefined && metadata.selectionFormat !== DRIVER_SELECTION_FORMAT) {
		throw new Error(`Unsupported parity selection format in ${join(run, "result.json")}. Use a runner that supports this artifact.`);
	}
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
	const entries = await readdir(root, { withFileTypes: true }).catch((error) => {
		if (isMissingFile(error)) return [];
		throw error;
	});
	const directories = entries.filter((entry) => entry.isDirectory() && entry.name.startsWith("parity-"));
	const runs = await Promise.all(directories.map(async ({ name }) => {
		const path = join(root, name);
		const resultPath = join(path, "result.json");
		const file = await stat(resultPath).catch((error) => {
			if (!isMissingFile(error)) console.warn(`Skipping parity artifact ${resultPath}: ${String(error)}`);
			return undefined;
		});
		if (!file) return undefined;
		const time = file.mtimeMs;
		try {
			const result = object(JSON.parse(await readFile(resultPath, "utf8")));
			if (result.selectionRecorded === false) return undefined;
			if (result.interrupted === true) return undefined;
			if (result.interrupted === undefined && isString(result.error) && result.error.includes("Interrupted")) return undefined;
			return { path, result, time };
		} catch { return { path, result: undefined, time }; }
	}));
	const newestFirst = runs.flatMap((run) => run ? [run] : []).sort((left, right) => right.time - left.time);
	const emptyHash = selectionHash("");
	for (const { path, result } of newestFirst) {
		if (!result) {
			const files = await Promise.all(["scenarios.txt", ...Object.values(driverSelectionFile)].map(async (name) => {
				try { return (await stat(join(path, name))).size; }
				catch (error) {
					if (isMissingFile(error)) return 0;
					throw error;
				}
			}));
			if (files.some((size) => size > 0)) throw new Error(`Could not read ${join(path, "result.json")}. Restore result.json before rerunning.`);
			continue;
		}
		const hashes = selectionHashes(path, result);
		if (hashes) {
			const hasWork = requested ? hashes[requested] !== emptyHash : hashes.web !== emptyHash || hashes.gpui !== emptyHash;
			if (!hasWork) continue;
		}
		const selected = await recordedSelections(path, result, hashes, { skipUnrecorded: true, requested });
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
	{ skipUnrecorded = false, requested }: { skipUnrecorded?: boolean; requested?: Driver } = {},
): Promise<DriverSelections> {
	const [web, gpui] = await Promise.all([
		readRunFile(run, driverSelectionFile.web),
		readRunFile(run, driverSelectionFile.gpui),
	]);
	const modern = metadata.selectionFormat !== undefined || hashes || web !== undefined || gpui !== undefined;
	if (!modern) {
		const previous = metadata.driver;
		if (requested && (previous === "web" || previous === "gpui") && previous !== requested) {
			return { web: new Set(), gpui: new Set() };
		}
		const shared = await readRunFile(run, "scenarios.txt");
		if (skipUnrecorded && !shared) return { web: new Set(), gpui: new Set() };
		if (shared === undefined) throw new Error(`Missing scenarios.txt in ${run}. Restore the recorded selection before rerunning.`);
		const ids = scenarioIds(shared);
		if (previous === "both") return { web: ids, gpui: ids };
		if (previous === "web") return { web: ids, gpui: new Set() };
		if (previous === "gpui") return { web: new Set(), gpui: ids };
		throw new Error(`Legacy driver metadata is missing or invalid in ${run}. Restore result.json before rerunning.`);
	}
	if (skipUnrecorded && !hashes && !web && !gpui && !(await readRunFile(run, "scenarios.txt"))) {
		return { web: new Set(), gpui: new Set() };
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

type DriverFailure = { driverFailed: boolean; legacyFailedShards?: string[] };

async function failedScenarios(run: string, driver: Driver, ids: Set<string>, { driverFailed, legacyFailedShards = [] }: DriverFailure): Promise<Set<string>> {
	// Failures outside scenario execution can depend on the original selection.
	if (!ids.size || driverFailed) return ids;

	const results = new Map<string, boolean>();
	try {
		const report = JSON.parse(await readFile(join(run, `${driver}.json`), "utf8"));
		const scenarios = driverScenarioResults(driver, report);
		for (const { id, status } of scenarios) {
			const duplicate = results.has(id);
			if (!ids.has(id) || (legacyFailedShards.length && duplicate)) return ids;
			results.set(id, !duplicate && status === "passed");
		}
		if (legacyFailedShards.length) {
			if (results.size !== ids.size) return ids;
			const shards = await Promise.all(legacyFailedShards.map(async (index) => {
				const shard = JSON.parse(await readFile(join(run, `gpui-${index}.json`), "utf8"));
				return driverScenarioResults("gpui", shard);
			}));
			const seen = new Set<string>();
			for (const shard of shards) {
				if (!shard.some(({ status }) => status === "failed")) return ids;
				for (const { id, status } of shard) {
					if (seen.has(id) || results.get(id) !== (status === "passed")) return ids;
					seen.add(id);
				}
			}
		}
	} catch {
		// An unreadable or incomplete report cannot prove that any of its scenarios passed.
		return ids;
	}
	return new Set([...ids].filter((id) => !results.get(id)));
}

function isLegacyDefaultMapCopyError(error: string): boolean {
	const [, from, to] = error.match(/\bcopyfile '([^'\r\n]+)' -> '([^'\r\n]+)'/) ?? [];
	if (!from || !to) return false;
	const source = from.replaceAll("\\", "/");
	const destination = to.replaceAll("\\", "/");
	const run = posix.dirname(source);
	const root = posix.dirname(run);
	return posix.basename(source) === "coverage-map.json" &&
		posix.basename(run).startsWith("parity-") &&
		root.endsWith(`/${posix.dirname(DEFAULT_MAP)}`) &&
		destination === posix.join(root, posix.basename(DEFAULT_MAP));
}

function hasLegacyRunError(error: string): boolean {
	const scopedFailure = /^(?:(?:Web|GPUI) driver: |(?:web|gpui) report: |Error: (?:Invalid (?:web|gpui) result: |Missing (?:web|gpui) results: )|GPUI shard \d+\/\d+ (?:exited -?\d+|wrote no report))/;
	return error.split("; ").some((message) => message && !scopedFailure.test(message));
}

/** Reads the recorded selection for each driver, including reports from older runners. */
export async function rerunSelections(run: string, { coverage = false } = {}): Promise<RetrySelection> {
	let metadata: ReturnType<typeof object> = {};
	try {
		const result = await readRunFile(run, "result.json");
		if (result !== undefined) metadata = object(JSON.parse(result));
	} catch {
		// Selection validation reports what must be restored.
	}
	if (metadata.selectionRecorded === false) throw new Error(`Run ${run} did not finish recording its selection. Choose an earlier run to retry.`);
	const hashes = selectionHashes(run, metadata);
	const recorded = await recordedSelections(run, metadata, hashes);
	if (coverage) {
		if (metadata.coverageMapFailure === "publication") return { ...recorded, coveragePublication: "retry" };
		if (metadata.coverageMapFailure === "generation") {
			return metadata.coveragePublicationRequested === true ? { ...recorded, coveragePublication: "retry" } : recorded;
		}
		// Earlier runners could fail to publish coverage without recording a separate outcome.
		if (metadata.status === "failed" && metadata.coverageMapFailure === undefined && metadata.coverageMapFailed !== false) {
			const copyFailed = isString(metadata.error) && isLegacyDefaultMapCopyError(metadata.error);
			return { ...recorded, coveragePublication: copyFailed ? "retry" : "unknown" };
		}
	}
	if (metadata.runLevelFailed === true) return recorded;
	// A scenario report cannot establish that driver cleanup succeeded.
	if (metadata.status !== "passed" && metadata.status !== "failed") return recorded;
	const failedDrivers = metadata.failedDrivers;
	if (failedDrivers !== undefined && (!Array.isArray(failedDrivers) || failedDrivers.some((name) => name !== "web" && name !== "gpui"))) return recorded;
	const legacyError = metadata.status === "failed" && isString(metadata.error) ? metadata.error : "";
	if (metadata.runLevelFailed === undefined && hasLegacyRunError(legacyError)) return recorded;
	const driverFailed = (driver: Driver, legacyLabel: string) =>
		failedDrivers === undefined ? legacyError.includes(legacyLabel) : failedDrivers.includes(driver);
	const legacyGpuiExits = failedDrivers === undefined ? [...legacyError.matchAll(/\bGPUI shard (\d+)\/\d+ (?:exited (-?\d+)|wrote no report)/g)] : [];
	// Rust libtest exits 101 on an assertion failure. Only a complete report can confirm it.
	const gpuiInfrastructureFailure = legacyGpuiExits.some((exit) => exit[2] !== "101");
	const [web, gpui] = await Promise.all([
		failedScenarios(run, "web", recorded.web, { driverFailed: driverFailed("web", "Web driver:") }),
		failedScenarios(run, "gpui", recorded.gpui, {
			driverFailed: driverFailed("gpui", "GPUI driver:") || gpuiInfrastructureFailure,
			legacyFailedShards: legacyGpuiExits.flatMap((exit) => exit[1] ? [exit[1]] : []),
		}),
	]);
	if (metadata.status === "failed" && !web.size && !gpui.size) return recorded;
	return { web, gpui };
}
