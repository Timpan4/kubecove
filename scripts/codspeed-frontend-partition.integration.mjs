// Collects the real benchmark declarations with Vitest, so it needs installed dependencies
// and Node. `bun run test:bench-groups` runs it; the dependency-free workflow assertions
// stay in codspeed-workflow-regression.test.mjs.
import assert from "node:assert/strict";
import { relative } from "node:path";
import { test } from "node:test";
import { createVitest } from "vitest/node";
import { benchmarkFiles, benchmarkSelection } from "./codspeed-benchmark-suites.mjs";
import { frontendMatrix } from "./codspeed-selection.mjs";

// The selections the workflow's frontend jobs receive for these files, read the way a job reads them.
const jobSelections = (files) => frontendMatrix(files).include.map(({ selection }) => benchmarkSelection({ KUBECOVE_BENCHMARK_SELECTION: selection }));

async function collectBenchmarks() {
	const vitest = await createVitest({ watch: false, reporters: [], benchmarkOnly: true });
	try {
		const { testModules, unhandledErrors } = await vitest.collect([], { staticParse: false });
		assert.deepEqual(unhandledErrors, []);
		return testModules.flatMap((module) => {
			assert.deepEqual(module.errors(), []);
			return [...module.children.allTests()].map((item) => ({
				file: relative(process.cwd(), module.moduleId).replaceAll("\\", "/"),
				name: item.fullName,
			}));
		});
	} finally {
		await vitest.close();
	}
}

function assertBenchmarkPartition(benchmarks, selections) {
	const matches = selections.map((selection) => benchmarks.filter((item) =>
		selection.include.includes(item.file) && (!selection.testNamePattern || item.name.match(selection.testNamePattern)),
	));
	for (const [index, items] of matches.entries()) assert.ok(items.length, `group ${index} matches nothing`);
	for (const item of benchmarks) {
		const count = matches.filter((items) => items.includes(item)).length;
		assert.equal(count, 1, `${item.file} > ${item.name} matched ${count} groups`);
	}
}

test("frontend group jobs form a complete, disjoint partition of actual benchmarks", async (t) => {
	const files = benchmarkFiles();
	const selections = jobSelections(files);
	const count = selections.length;
	assert.ok(count > 1, "a full run starts one job or none");
	const benchmarks = await collectBenchmarks();
	assert.deepEqual([...new Set(benchmarks.map((item) => item.file))].sort(), files);
	assertBenchmarkPartition(benchmarks, selections);
	// A pull request runs a subset of the files. Its jobs must still run each benchmark of
	// those files once: a split file alone, both split files, and each with unsplit files.
	const split = ["benchmarks/resource-table.bench.ts", "benchmarks/topology.bench.ts"];
	const unsplit = files.filter((file) => !split.includes(file));
	for (const subset of [[split[0]], [split[1]], split, [unsplit[0]], [split[1], unsplit[0]], [...split, ...unsplit.slice(0, 5)], unsplit]) {
		await t.test(`partial selection of ${subset.length} files: ${subset[0]}`, () => {
			for (const file of subset) assert.ok(files.includes(file), file);
			assertBenchmarkPartition(benchmarks.filter((item) => subset.includes(item.file)), jobSelections(subset));
		});
	}
	const target = benchmarks.find((item) => item.file === "benchmarks/topology.bench.ts");
	assert.ok(target);
	const exact = target.name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
	await t.test("rejects a benchmark that runs in no group", () => {
		const missing = selections.map((selection) => ({ ...selection, testNamePattern: `^(?!${exact}$)(?:${selection.testNamePattern ?? ".*"})` }));
		assert.throws(() => assertBenchmarkPartition(benchmarks, missing), /matched 0 groups/);
	});
	await t.test("rejects a benchmark that runs in two groups", () => {
		const owner = selections.findIndex((selection) => selection.include.includes(target.file) && target.name.match(selection.testNamePattern ?? ".*"));
		const duplicate = selections.map((selection, index) => index !== (owner + 1) % count ? selection : ({
			...selection, include: [...new Set([...selection.include, target.file])],
			testNamePattern: `(?:${selection.testNamePattern ?? ".*"})|^${exact}$`,
		}));
		assert.throws(() => assertBenchmarkPartition(benchmarks, duplicate), /matched 2 groups/);
	});
	await t.test("rejects a group whose name filter matches nothing", () => {
		assert.throws(() => assertBenchmarkPartition(benchmarks, selections.map((selection, index) => index ? selection : ({ ...selection, testNamePattern: "(?!)" }))), /group 0 matches nothing/);
	});
	await t.test("rejects a new benchmark in a split file that no filter covers", () => {
		const added = { ...target, name: `${target.name.slice(0, target.name.lastIndexOf(" > "))} > newly added benchmark` };
		assert.throws(() => assertBenchmarkPartition([...benchmarks, added], selections), /newly added benchmark matched 0 groups/);
	});
});
