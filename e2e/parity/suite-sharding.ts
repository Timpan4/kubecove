import { readFile } from "node:fs/promises";

/** A duration map in milliseconds, or an existing web.json/gpui.json driver report. */
export function parseTimings(value: unknown): Map<string, number> {
	if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Expected a parity timing map or driver report");
	const durations = new Map<string, number>();
	const add = (id: unknown, duration: unknown) => {
		if (typeof id !== "string" || !id) throw new Error("Expected a scenario ID in parity timings");
		if (typeof duration !== "number" || !Number.isFinite(duration) || duration < 0) throw new Error(`Invalid duration for ${id}`);
		if (durations.has(id)) throw new Error(`Duplicate timing for ${id}`);
		durations.set(id, duration);
	};
	if ("scenarios" in value && Array.isArray(value.scenarios)) {
		for (const row of value.scenarios) {
			if (!row || typeof row !== "object" || typeof row.id !== "string" || !row.id) throw new Error("Expected a scenario ID in parity timings");
			// Some legacy or failed report rows have no duration.
			if (row.durationMs !== undefined) add(row.id, row.durationMs);
		}
	} else {
		for (const [id, duration] of Object.entries(value)) add(id, duration);
	}
	return durations;
}

export async function loadTimings(path: string | undefined): Promise<Map<string, number>> {
	if (!path) return new Map();
	try {
		return parseTimings(JSON.parse(await readFile(path, "utf8")));
	} catch (error) {
		if (error instanceof Error && "code" in error && error.code === "ENOENT") {
			console.warn(`No parity timings at ${path}. Sharding with equal weights.`);
			return new Map();
		}
		throw new Error(`Could not read parity timings from ${path}`, { cause: error });
	}
}

export function verifyShardAssignments(selected: readonly string[], assignments: readonly (readonly string[])[]): void {
	const expected = new Set<string>();
	for (const id of selected) {
		if (expected.has(id)) throw new Error(`Duplicate selected scenario: ${id}`);
		expected.add(id);
	}
	const seen = new Set<string>();
	for (const shard of assignments) {
		for (const id of shard) {
			if (!expected.has(id)) throw new Error(`Unexpected assigned scenario: ${id}`);
			if (seen.has(id)) throw new Error(`Duplicate assigned scenario: ${id}`);
			seen.add(id);
		}
	}
	if (seen.size !== selected.length) {
		throw new Error(`Missing assigned scenario: ${selected.filter((id) => !seen.has(id)).join(", ")}`);
	}
}

export function assignSuiteShards(selected: readonly string[], count: number, timings: ReadonlyMap<string, number> = new Map()): string[][] {
	if (!Number.isSafeInteger(count) || count < 1) throw new Error("Parity shard count must be a positive integer");
	const measured = selected.flatMap((id) => {
		const duration = timings.get(id);
		return duration === undefined ? [] : [duration];
	}).sort((left, right) => left - right);
	// Estimate unmeasured work from the median recorded duration, or use equal weights.
	const fallback = measured[Math.floor(measured.length / 2)] ?? 1;
	const ordered = selected.map((id, index) => ({ index, duration: timings.get(id) ?? fallback }))
		.sort((left, right) => right.duration - left.duration || left.index - right.index);
	const shards = Array.from({ length: count }, () => ({ positions: [] as number[], duration: 0 }));
	for (const { index, duration } of ordered) {
		const lightest = shards.reduce((best, shard) =>
			shard.duration < best.duration || (shard.duration === best.duration && shard.positions.length < best.positions.length) ? shard : best,
		);
		lightest.positions.push(index);
		lightest.duration += duration;
	}
	const assignments = shards.map(({ positions }) => positions.sort((left, right) => left - right).map((index) => selected[index]));
	verifyShardAssignments(selected, assignments);
	return assignments;
}
