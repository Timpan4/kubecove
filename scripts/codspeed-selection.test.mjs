import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { selectBenchmarks, frontendMatrix } from "./codspeed-selection.mjs";
import { benchmarkFiles, benchmarkGroups, benchmarkSelection } from "./codspeed-benchmark-suites.mjs";

// The fixture drives the public selector with real source and Vite resolution.
// Failure modes are recorded in the implementation report before this test.
test("import closures select dependants and conservatively retain unknown dependencies", async () => {
  const root = mkdtempSync(join(tmpdir(), "codspeed-selection-"));
  const files = ["a", "b", "dynamic", "glob", "missing", "syntax", "svelte", "io"].map((name) => `benchmarks/${name}.bench.ts`);
  const sources = {
    "vitest.config.ts": `export default { resolve: { alias: { "@": ${JSON.stringify(join(root, "src"))} } } };`,
    "benchmarks/a.bench.ts": 'import "@/barrel"; import "./fixture";',
    "benchmarks/b.bench.ts": 'import("@/literal"); import "../fixtures/shared";',
    "fixtures/shared.ts": 'export const shared = 1;',
    "benchmarks/fixture.ts": 'export const fixture = 1;',
    "src/barrel.ts": 'export * from "./cycle";',
    "src/cycle.ts": 'import "./barrel"; export * from "./leaf";',
    "src/leaf.ts": 'export const value = 1;',
    "src/literal.ts": 'export const value = 2;',
    "benchmarks/dynamic.bench.ts": 'import(path);',
    "benchmarks/glob.bench.ts": 'import.meta.glob("../src/*.ts");',
    "benchmarks/missing.bench.ts": 'import "@/removed";',
    "benchmarks/syntax.bench.ts": 'export const = ;',
    "benchmarks/svelte.bench.ts": 'import "@/Component.svelte";',
    "src/Component.svelte": '<script>import "./leaf";</script>',
    "benchmarks/io.bench.ts": 'import { readFileSync } from "node:fs";',
  };
  try {
    for (const [path, source] of Object.entries(sources)) {
      mkdirSync(join(root, path, ".."), { recursive: true });
      writeFileSync(join(root, path), source);
    }
    const always = files.slice(2);
    const select = async (paths) => (await selectBenchmarks(paths, { root, files })).selected;
    assert.deepEqual(await select(["src/leaf.ts"]), [files[0], ...always]);
    assert.deepEqual(await select(["src/literal.ts"]), [files[1], ...always]);
    assert.deepEqual(await select(["benchmarks/fixture.ts"]), [files[0], ...always]);
    assert.deepEqual(await select(["fixtures/shared.ts"]), [files[1], ...always]);
    assert.deepEqual(await select(["e2e/fixtures/unrelated.yaml"]), always);
    assert.deepEqual(await select(["e2e/scenario.ts"]), always);
    writeFileSync(join(root, "src/cycle.ts"), 'export * from "./introduced";');
    writeFileSync(join(root, "src/introduced.ts"), 'export const added = 3;');
    assert.deepEqual(await select(["src/introduced.ts"]), [files[0], ...always]);
    rmSync(join(root, "src/introduced.ts"));
    assert.deepEqual(await select(["src/introduced.ts"]), [files[0], ...always]);
    for (const path of ["bun.lock", "package.json", "vitest.config.ts", "vite.config.ts", "svelte.config.js", "tsconfig.benchmarks.json", "benchmarks/benchmark.ts", ".github/workflows/codspeed.yml", ".github/actions/codspeed-run/action.yml"]) {
      assert.deepEqual(await select([path]), files, path);
    }
    const baseline = await selectBenchmarks([], { root, files, fullReason: "main baseline" });
    assert.deepEqual(baseline.selected, files);
    const irrelevant = await selectBenchmarks([".github/workflows/ci.yml"], { root, files: [] });
    assert.equal(irrelevant.rust, false);
    for (const path of ["src-tauri/src/lib.rs", "scripts/ci-rust-cache-key.mjs", "scripts/ci-mbx-cargo.mjs", ".github/actions/codspeed-run/action.yml"]) {
      assert.equal((await selectBenchmarks([path], { root, files: [] })).rust, true, path);
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

// Failure modes: Vitest runs setup files the benchmark never imports, and a deleted module
// can leave its importers resolving to another file, so no current closure names the change.
test("setup files and deleted modules that another file replaces select their benchmarks", async () => {
  const root = mkdtempSync(join(tmpdir(), "codspeed-selection-"));
  const files = ["benchmarks/a.bench.ts", "benchmarks/b.bench.ts"];
  const sources = {
    "vitest.config.ts": 'export default { test: { setupFiles: ["./setup.ts"], globalSetup: "./global.ts" } };',
    "setup.ts": 'import "./setup-helper";',
    "setup-helper.ts": "export const helper = 1;",
    "global.ts": 'import "./global-helper";',
    "global-helper.ts": "export const helper = 2;",
    "benchmarks/a.bench.ts": 'import "../src/value";',
    "benchmarks/b.bench.ts": 'import "../src/other";',
    "src/value.ts": "export const value = 1;",
    "src/value/index.ts": "export const value = 2;",
    "src/other.ts": "export const other = 1;",
  };
  try {
    for (const [path, source] of Object.entries(sources)) {
      mkdirSync(join(root, path, ".."), { recursive: true });
      writeFileSync(join(root, path), source);
    }
    const select = async (paths) => (await selectBenchmarks(paths, { root, files })).selected;
    assert.deepEqual(await select(["src/other.ts"]), [files[1]]);
    assert.deepEqual(await select(["src/value.ts"]), [files[0]]);
    for (const path of ["setup.ts", "setup-helper.ts", "global.ts", "global-helper.ts"]) assert.deepEqual(await select([path]), files, path);
    // An unrelated deletion selects nothing; a deletion with a replacement selects everything.
    assert.deepEqual(await select(["src/gone.ts", "docs/gone.json"]), []);
    rmSync(join(root, "src/value.ts"));
    const replaced = await selectBenchmarks(["src/value.ts"], { root, files });
    assert.deepEqual(replaced.selected, files);
    assert.match(replaced.decisions[0].reason, /deleted module that another file now resolves for: src\/value\.ts/);
    assert.equal(replaced.rust, false);
    rmSync(join(root, "src/value/index.ts"));
    writeFileSync(join(root, "src/value.js"), "export const value = 3;");
    assert.deepEqual(await select(["src/value/index.ts"]), files);
    rmSync(join(root, "setup.ts"));
    assert.deepEqual(await select(["src/other.ts"]), files);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("selected files use the fewest groups meeting the measured full-run duration", () => {
  const files = benchmarkFiles();
  const ceiling = Math.max(...benchmarkGroups(files, 8).map((group) => group.seconds));
  assert.deepEqual(frontendMatrix([]), { include: [] });
  for (const selected of [["benchmarks/detail-metadata.bench.ts"], ["benchmarks/topology.bench.ts"], files]) {
    const matrix = frontendMatrix(selected).include;
    assert.ok(matrix.length <= 8);
    const selections = matrix.map(({ selection }) => benchmarkSelection({ KUBECOVE_BENCHMARK_SELECTION: selection }));
    assert.deepEqual([...new Set(selections.flatMap((entry) => entry.include))].sort(), [...selected].sort());
    assert.ok(benchmarkGroups(selected, matrix.length).every((group) => group.seconds <= ceiling));
    if (matrix.length > 1) assert.ok(benchmarkGroups(selected, matrix.length - 1).some((group) => group.seconds > ceiling));
  }
  assert.equal(frontendMatrix(["benchmarks/detail-metadata.bench.ts"]).include.length, 1);
  assert.equal(frontendMatrix(files).include.length, 8);
  // New benchmarks have no measured duration. Run all selected files unfiltered.
  const unknown = frontendMatrix(["benchmarks/new.bench.ts"]).include;
  assert.deepEqual(JSON.parse(unknown[0].selection).include, ["benchmarks/new.bench.ts"]);
  assert.equal(JSON.parse(unknown[0].selection).testNamePattern, undefined);
});

test("repository diffs report the selected files and matrix size", async (t) => {
  for (const path of ["src/App.svelte", "src/lib/yamlFormat.ts", "bun.lock", "e2e/parity/web.ts", ".github/workflows/codspeed.yml"]) {
    const result = await selectBenchmarks([path]);
    t.diagnostic(JSON.stringify({ path, selected: result.selected, jobs: frontendMatrix(result.selected).include.length, rust: result.rust }));
  }
});
