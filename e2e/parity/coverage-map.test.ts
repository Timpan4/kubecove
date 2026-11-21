import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { encodeMap, mergeMaps } from "./coverage-map";

// A retry can omit a scenario that passed earlier and replace the rows it did record.
test("merges the newest numeric attempt of each coverage shard", async () => {
	await mkdir("e2e/artifacts", { recursive: true });
	const root = await mkdtemp("e2e/artifacts/coverage-merge-test-");
	const commit = "a".repeat(40);
	const shard = async (name: string, ...ids: string[]) => {
		const input = join(root, `parity-coverage-shard-${name}`, "parity-run", "coverage-map.json");
		await mkdir(dirname(input), { recursive: true });
		const records = ids.map((id) => ({ id, files: {} }));
		const map = encodeMap(commit, records, { code: {}, compiled: [] });
		await writeFile(input, JSON.stringify(map));
		return input;
	};
	try {
		const newest = await shard("0-of-2-attempt-10", "newest");
		const superseded = await shard("0-of-2-attempt-9", "stale");
		const unchanged = await shard("1-of-2-attempt-9", "unchanged");
		const out = join(root, "merged.json");
		const map = await mergeMaps(out, [newest, superseded, unchanged]);
		expect(map.scenarios.toSorted()).toEqual(["newest", "unchanged"]);
	} finally {
		await rm(root, { recursive: true, force: true });
	}
});
