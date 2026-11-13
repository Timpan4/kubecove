// Collects the real benchmark declarations with Vitest, so it needs installed dependencies
// and Node. `bun run test:bench-groups` runs it; the dependency-free workflow assertions
// stay in codspeed-workflow-regression.test.mjs.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { relative } from "node:path";
import { test } from "node:test";
import { createVitest } from "vitest/node";
import { benchmarkFiles, benchmarkSelection } from "./codspeed-benchmark-suites.mjs";

const workflow = readFileSync(new URL("../.github/workflows/codspeed.yml", import.meta.url), "utf8");

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
	const count = Number(/KUBECOVE_BENCHMARK_GROUP: \$\{\{ matrix\.group \}\}\/(\d+)\r?$/m.exec(workflow)?.[1]);
	assert.ok(count > 0, "the workflow sets no benchmark group count");
	const selections = Array.from({ length: count }, (_, group) => benchmarkSelection({ KUBECOVE_BENCHMARK_GROUP: `${group}/${count}` }));
	const benchmarks = await collectBenchmarks();
	assert.deepEqual([...new Set(benchmarks.map((item) => item.file))].sort(), files);
	assertBenchmarkPartition(benchmarks, selections);
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
