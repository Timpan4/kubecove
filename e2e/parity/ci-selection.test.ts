import { expect, test } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { delimiter, join, resolve } from "node:path";
import { selectionPlan, SPIKE } from "./ci-selection";
import { type Contract, loadContract } from "./contract";
import { encodeMap, type ScenarioCoverage } from "./coverage-map";
import { type Driver, mergeReports } from "./merge-reports";
import { selectedContract } from "./selected-contract";

// Failure modes were written in the implementation report before implementation:
// transport/history/selection failure becomes empty; stale blind inputs are ignored;
// prefix collisions enlarge selection; skipped drivers or extra reports pass unnoticed.
const ids = ["one", "one.more", SPIKE];
const contract: Contract = { scenarios: ids.map((id) => ({ id, title: id, steps: [] })) };
const report = (driver: Driver, ...ids: string[]) => ({ driver, scenarios: ids.map((id) => ({ id, status: "passed" })) });
const planner = resolve("e2e/parity/ci-selection.ts");

async function run(cwd: string, args: string[], env = process.env) {
	const child = Bun.spawn(args, { cwd, env, stdout: "pipe", stderr: "pipe" });
	const [stdout, stderr] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text()]);
	if (await child.exited !== 0) throw new Error(`${args[0]} failed: ${stderr}`);
	return stdout.trim();
}

async function fixture(work: (root: string, base: string) => Promise<void>) {
	await mkdir("e2e/artifacts", { recursive: true });
	const root = resolve(await mkdtemp("e2e/artifacts/parity-selection-test-"));
	const git = (...args: string[]) => run(root, ["git", ...args]);
	try {
		await git("init", "--quiet");
		await mkdir(join(root, "src"));
		await writeFile(join(root, "src/feature.ts"), "export function first() {\n return 1;\n}\nexport function second() {\n return 2;\n}\n");
		await git("add", ".");
		await git("-c", "user.name=Parity test", "-c", "user.email=parity@example.invalid", "commit", "--quiet", "-m", "Fixture base");
		const base = await git("rev-parse", "HEAD");
		const records = ids.map((id, index): ScenarioCoverage => {
			const files: ScenarioCoverage["files"] = {};
			if (index < 2) files["src/feature.ts"] = { active: true, lines: [[index * 3 + 1, index * 3 + 3]] };
			return { id, files };
		});
		await writeFile(join(root, "map.json"), JSON.stringify(encodeMap(base, records, { code: {}, compiled: [] })));
		await work(root, base);
	} finally { await rm(root, { recursive: true, force: true }); }
}

async function plan(root: string, mapPath = "map.json", reject = false) {
	const code = `import { planPullRequest } from ${JSON.stringify(planner)};
	console.log(JSON.stringify(await planPullRequest(${JSON.stringify(ids)}, "HEAD", async () => {
		if (process.env.REJECT === "1") throw new Error("Artifact download refused");
		return process.env.MAP;
	})));`;
	return JSON.parse(await run(root, [process.execPath, "--eval", code], { ...process.env, MAP: mapPath, REJECT: reject ? "1" : "0" }));
}

test("execution and report validation share exact selected IDs, including an explicit empty selection", () => {
	const selected = selectedContract(contract, '["one"]');
	expect(selected.scenarios.map(({ id }) => id)).toEqual(["one"]);
	expect(mergeReports(selected, [report("web", "one"), report("gpui", "one")])).toHaveLength(2);
	for (const reports of [
		[report("web", "one")],
		[report("web", "one"), report("gpui")],
		[report("web", "one", "one.more"), report("gpui", "one")],
	]) expect(() => mergeReports(selected, reports)).toThrow();
	expect(mergeReports(selectedContract(contract, "[]"), [])).toEqual([report("web"), report("gpui")]);
	expect(selectedContract(contract, undefined)).toBe(contract);
	for (const invalid of ["null", "{}", '["unknown"]', '["one","one"]', "[1]", "["]) {
		expect(() => selectedContract(contract, invalid)).toThrow();
	}
});

test("job counts follow empty, partial, spike-only and full contracts", () => {
	// Twelve regular scenarios: a full-suite web shard carries three and a GPUI shard four.
	const many = [...Array.from({ length: 12 }, (_, index) => `many.${index}`), SPIKE];
	const cases = [
		{ selected: [], regular: false, spike: false, web: 0, gpui: 0 },
		{ selected: ["one"], regular: true, spike: false, web: 1, gpui: 1 },
		{ selected: [SPIKE], regular: false, spike: true, web: 0, gpui: 0 },
		{ selected: ["one", SPIKE], regular: true, spike: true, web: 1, gpui: 1 },
		{ selected: ids, regular: true, spike: true, web: 4, gpui: 3 },
		{ all: many, selected: many.slice(0, 3), regular: true, spike: false, web: 1, gpui: 1 },
		{ all: many, selected: many.slice(0, 7), regular: true, spike: false, web: 3, gpui: 2 },
		{ all: many, selected: many.slice(0, 12), regular: true, spike: false, web: 4, gpui: 3 },
	];
	for (const item of cases) {
		const plan = selectionPlan(item.all ?? ids, { ids: new Set(item.selected), because: {} });
		expect([plan.regular, plan.spike, plan.webShards, plan.gpuiShards]).toEqual([item.regular, item.spike, item.web, item.gpui]);
		if (item.regular) {
			expect(plan.webMatrix.shard).toEqual(Array.from({ length: item.web }, (_, index) => index));
			expect(plan.gpuiMatrix.shard).toEqual(Array.from({ length: item.gpui }, (_, index) => index));
		}
	}
});

test("real git diffs select covered code, nothing for docs, and full suite on every unavailable selection", async () => {
	await fixture(async (root, base) => {
		const commit = async () => {
			await run(root, ["git", "add", "src", "docs.md"]);
			await run(root, ["git", "-c", "user.name=Parity test", "-c", "user.email=parity@example.invalid", "commit", "--quiet", "-m", "Fixture edit"]);
		};
		await writeFile(join(root, "docs.md"), "Docs only\n");
		await commit();
		expect((await plan(root)).ids).toEqual([]);
		await writeFile(join(root, "src/feature.ts"), "export function first() {\n return 3;\n}\nexport function second() {\n return 2;\n}\n");
		await commit();
		expect((await plan(root)).ids).toEqual(["one"]);
		for (const [path, reject] of [["missing.json", false], ["map.json", true]] as const) {
			const result = await plan(root, path, reject);
			expect(result.ids).toEqual(ids);
			expect(result.fallback).toContain("Selection unavailable");
		}
		await writeFile(join(root, "bad-map.json"), "{}");
		expect((await plan(root, "bad-map.json")).ids).toEqual(ids);
		const map = JSON.parse(await readFile(join(root, "map.json"), "utf8"));
		// A commit this clone lacks, then one it has that the tested head does not contain.
		const tree = await run(root, ["git", "rev-parse", "HEAD^{tree}"]);
		const aside = await run(root, ["git", "-c", "user.name=Parity test", "-c", "user.email=parity@example.invalid", "commit-tree", tree, "-p", base, "-m", "Not in the tested head"]);
		for (const commit of ["0".repeat(40), aside]) {
			await writeFile(join(root, "bad-map.json"), JSON.stringify({ ...map, commit }));
			const result = await plan(root, "bad-map.json");
			expect(result.ids).toEqual(ids);
			expect(result.fallback).toContain("Selection unavailable");
		}
	});
});

test("main changes the map already recorded select nothing for a branch that is behind it", async () => {
	await fixture(async (root, base) => {
		const git = (...args: string[]) => run(root, ["git", "-c", "user.name=Parity test", "-c", "user.email=parity@example.invalid", ...args]);
		// Main changes second() after the branch point, and its map records that commit.
		await writeFile(join(root, "src/feature.ts"), "export function first() {\n return 1;\n}\nexport function second() {\n return 4;\n}\n");
		await git("commit", "--quiet", "-am", "Main edit");
		const main = await git("rev-parse", "HEAD");
		const map = JSON.parse(await readFile(join(root, "map.json"), "utf8"));
		await writeFile(join(root, "map.json"), JSON.stringify({ ...map, commit: main }));
		// The branch left before that commit and changes only docs; the tested head merges both.
		await git("checkout", "--quiet", "-b", "branch", base);
		await writeFile(join(root, "docs.md"), "Docs only\n");
		await git("add", "docs.md");
		await git("commit", "--quiet", "-m", "Branch edit");
		await git("merge", "--quiet", "--no-edit", main);
		expect((await plan(root)).ids).toEqual([]);
	});
});

test("a stale map includes intervening source changes and falls back for blind inputs, mocks and exporter", async () => {
	for (const path of ["src/feature.ts", "e2e/parity/visual/export-fixture.ts", "e2e/parity/scenarios/new.json", "src/lib/tauri-dev-mocks.ts", "src/lib/mcp-dev-mocks.ts", ".github/workflows/parity.yml"]) {
		await fixture(async (root) => {
			await mkdir(join(root, path, ".."), { recursive: true });
			await writeFile(join(root, path), path === "src/feature.ts" ? "export function first() {\n return 3;\n}\nexport function second() {\n return 2;\n}\n" : path.endsWith("new.json") ? JSON.stringify({ scenarios: [{ id: "one" }] }) : "Changed input\n");
			await run(root, ["git", "add", path]);
			await run(root, ["git", "-c", "user.name=Parity test", "-c", "user.email=parity@example.invalid", "commit", "--quiet", "-m", "Intervening main edit"]);
			const result = await plan(root);
			if (path === "src/feature.ts" || path.endsWith("new.json")) expect(result.ids).toEqual(["one"]);
			else {
				expect(result.ids).toEqual(ids);
				expect(result.fallback).toContain(path);
			}
		});
	}
});

test("an empty selected result succeeds without report artifacts and states its coverage", async () => {
	const root = resolve(".");
	const output = await run(root, [process.execPath, "e2e/parity/merge-reports.ts", "does-not-exist"], { ...process.env, PARITY_SCENARIOS: "[]", PARITY_SELECTION_REASON: '"Docs only"', GITHUB_STEP_SUMMARY: "" });
	expect(output).toContain(`No parity scenarios selected (0 of ${(await loadContract()).scenarios.length})`);
	expect(output).toContain("Docs only");
});

test("main artifact acquisition uses read-only gh commands and every transport failure selects the full contract", async () => {
	await fixture(async (root, base) => {
		await mkdir(join(root, "bin"));
		await writeFile(join(root, "bin/gh"), `#!/usr/bin/env bun
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
const args = process.argv.slice(2);
if (args[0] !== "run") process.exit(1);
if (args[1] === "list") {
  for (const pair of [["--branch", "main"], ["--event", "push"], ["--status", "success"], ["--limit", "1"]]) {
    if (args[args.indexOf(pair[0]) + 1] !== pair[1]) process.exit(1);
  }
  if (process.env.MODE === "list-failure") process.exit(1);
  console.log(JSON.stringify(process.env.MODE === "no-main" ? [] : [{ databaseId: 1, headSha: process.env.MODE === "fetch-failure" ? "0".repeat(40) : process.env.BASE }]));
} else if (args[1] === "download") {
  if (process.env.MODE === "download-failure") process.exit(1);
  if (args[args.indexOf("--name") + 1] !== "parity-coverage-map") process.exit(1);
  const directory = args[args.indexOf("--dir") + 1];
  await mkdir(directory, { recursive: true });
  const target = join(directory, "parity-coverage-map.json");
  if (process.env.MODE === "missing-artifact") process.exit(0);
  await copyFile("map.json", target);
  if (["fetch-failure", "wrong-commit"].includes(process.env.MODE)) {
    const map = JSON.parse(await readFile(target, "utf8"));
    map.commit = "0".repeat(40);
    await writeFile(target, JSON.stringify(map));
  }
} else process.exit(1);
`, { mode: 0o755 });
		const code = `import { planPullRequest } from ${JSON.stringify(planner)};
		console.log(JSON.stringify(await planPullRequest(${JSON.stringify(ids)}, "HEAD")));`;
		// Missing artifact runs first, before a preceding successful download could leave a file.
		for (const mode of ["missing-artifact", "no-main", "list-failure", "download-failure", "fetch-failure", "wrong-commit", "success"]) {
			const output = await run(root, [process.execPath, "--eval", code], { ...process.env, PATH: `${join(root, "bin")}${delimiter}${process.env.PATH}`, BASE: base, MODE: mode, GITHUB_REPOSITORY: "owner/repo" });
			const result = JSON.parse(output.split("\n").at(-1) ?? "");
			expect(result.ids).toEqual(mode === "success" ? [] : ids);
			if (mode !== "success") expect(result.fallback).toContain("Selection unavailable");
		}
	});
});
