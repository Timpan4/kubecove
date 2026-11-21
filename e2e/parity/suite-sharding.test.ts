import { expect, test } from "bun:test";
import { assignSuiteShards, parseTimings, verifyShardAssignments } from "./suite-sharding";

test("balances slow scenarios across shards and keeps contract order within each shard", () => {
	const ids = ["a", "b", "c", "d", "e", "f"];
	const timings = parseTimings({ scenarios: [
		{ id: "a", durationMs: 9 }, { id: "c", durationMs: 8 },
		{ id: "e", durationMs: 7 }, { id: "b", durationMs: 6 },
		{ id: "d", durationMs: 5 }, { id: "f", durationMs: 4 },
	] });
	const shards = assignSuiteShards(ids, 2, timings);
	expect(shards).toEqual([["a", "b", "d"], ["c", "e", "f"]]);
	expect(shards.map((shard) => shard.reduce((sum, id) => sum + (timings.get(id) ?? 0), 0))).toEqual([20, 19]);
});

test("assigns missing and tied timings deterministically, independent of timing order", () => {
	const ids = ["c", "a", "d", "b"];
	expect(assignSuiteShards(ids, 2)).toEqual([["c", "d"], ["a", "b"]]);
	const partial = assignSuiteShards(ids, 2, parseTimings({ c: 2, a: 2 }));
	expect(partial).toEqual(assignSuiteShards(ids, 2, parseTimings({ a: 2, c: 2 })));
	expect(partial).toEqual([["c", "d"], ["a", "b"]]);
});

test.each([
	{ assigned: [["a"], ["a", "b"]], message: "Duplicate assigned scenario: a" },
	{ assigned: [["a"], []], message: "Missing assigned scenario: b" },
	{ assigned: [["a"], ["b", "extra"]], message: "Unexpected assigned scenario: extra" },
])("rejects invalid assignments: $message", ({ assigned, message }) => {
	expect(() => verifyShardAssignments(["a", "b"], assigned)).toThrow(message);
});

test("rejects duplicate scenario IDs in the selected inventory and malformed timings", () => {
	expect(() => assignSuiteShards(["a", "a"], 2)).toThrow("Duplicate selected scenario: a");
	expect(() => parseTimings({ scenarios: [{ id: "a", durationMs: 2 }, { id: "a", durationMs: 3 }] })).toThrow("Duplicate timing");
	expect(() => parseTimings({ a: -1 })).toThrow("Invalid duration");
});
