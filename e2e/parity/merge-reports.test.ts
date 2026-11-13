import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import type { Contract } from "./contract";
import { collectReports, mergeReports } from "./merge-reports";

// Failure modes are recorded in the implementation report before these tests:
// missing report, duplicate scenario, one-driver-only scenario, invalid or failed result.
const contract: Contract = { scenarios: [
	{ id: "one", title: "One", steps: [] },
	{ id: "two", title: "Two", steps: [] },
] };
const report = (driver: string, ...ids: string[]) => ({ driver, scenarios: ids.map((id) => ({ id, status: "passed" })) });

test("merges different shard layouts and preserves the full contract check", () => {
	const reports = [report("web", "one"), report("web", "two"), report("gpui", "one", "two")];
	expect(mergeReports(contract, reports).map((item) => item.scenarios.length)).toEqual([2, 2]);
	for (const incomplete of [
		reports.slice(0, 2),
		[report("web", "one", "two"), report("gpui", "one")],
		[report("web", "one"), report("gpui", "one")],
	]) expect(() => mergeReports(contract, incomplete)).toThrow();
});

test("rejects duplicate, unexpected, failed and malformed results", () => {
	const valid = [report("web", "one", "two"), report("gpui", "one", "two")];
	for (const invalid of [
		[...valid, report("web", "one")],
		[report("web", "one", "one", "two"), valid[1]],
		[report("web", "one", "two", "extra"), valid[1]],
		[{ driver: "web", scenarios: [{ id: "one", status: "failed" }] }, valid[1]],
		[report("other", "one", "two"), valid[1]],
		[null, valid[1]],
	]) expect(() => mergeReports(contract, invalid)).toThrow();
});

test("collection fails when a selected run has no report or never passed", async () => {
	await mkdir("e2e/artifacts", { recursive: true });
	const root = await mkdtemp("e2e/artifacts/parity-merge-test-");
	try {
		const run = join(root, "artifact", "parity-run");
		await mkdir(run, { recursive: true });
		await writeFile(join(run, "scenarios.txt"), "one\ntwo");
		await writeFile(join(run, "result.json"), JSON.stringify({ driver: "web", status: "passed" }));
		await expect(collectReports(root)).rejects.toThrow();
		await writeFile(join(run, "web.json"), JSON.stringify(report("web", "one", "two")));
		expect(await collectReports(root)).toEqual([report("web", "one", "two")]);
		await writeFile(join(run, "result.json"), JSON.stringify({ driver: "web", status: "failed" }));
		await expect(collectReports(root)).rejects.toThrow();
	} finally { await rm(root, { recursive: true, force: true }); }
});
