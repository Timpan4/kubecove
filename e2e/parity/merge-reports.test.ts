import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import type { Contract } from "./contract";
import { collectReports, mergeReports, newestAttempts } from "./merge-reports";

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

// Failure modes of a partial re-run: a superseded failed attempt still counts, so the merged
// report fails or holds duplicates; an older pass hides a newer failure; attempt 10 sorts
// before attempt 9 as text; an artifact without an attempt suffix is dropped.
test("a re-run shard is judged by its newest attempt", async () => {
	expect(newestAttempts(["web-0-attempt-9", "web-0-attempt-10", "gpui-0-attempt-1", "plain"]).sort())
		.toEqual(["gpui-0-attempt-1", "plain", "web-0-attempt-10"]);
	await mkdir("e2e/artifacts", { recursive: true });
	const root = await mkdtemp("e2e/artifacts/parity-merge-test-");
	const shard = async (artifact: string, driver: string, status: string, ...ids: string[]) => {
		const run = join(root, artifact, "parity-run");
		await mkdir(run, { recursive: true });
		await writeFile(join(run, "scenarios.txt"), ids.join("\n"));
		await writeFile(join(run, "result.json"), JSON.stringify({ driver, status }));
		await writeFile(join(run, `${driver}.json`), JSON.stringify(status === "passed" ? report(driver, ...ids) : { driver, scenarios: [] }));
	};
	try {
		await shard("web-0-attempt-1", "web", "failed", "one");
		await shard("web-1-attempt-1", "web", "passed", "two");
		await shard("gpui-0-attempt-1", "gpui", "passed", "one", "two");
		await expect(collectReports(root)).rejects.toThrow();
		await shard("web-0-attempt-2", "web", "passed", "one");
		expect(mergeReports(contract, await collectReports(root)).map((item) => item.scenarios.length)).toEqual([2, 2]);
		await shard("web-1-attempt-3", "web", "failed", "two");
		await expect(collectReports(root)).rejects.toThrow();
		// A new plan consolidates the regular shards and drops the earlier layout.
		await shard("web-single-attempt-4", "web", "passed", "one", "two");
		await shard("gpui-single-attempt-4", "gpui", "passed", "one", "two");
		expect(mergeReports(contract, await collectReports(root, 4)).map((item) => item.scenarios.length)).toEqual([2, 2]);
	} finally { await rm(root, { recursive: true, force: true }); }
});
