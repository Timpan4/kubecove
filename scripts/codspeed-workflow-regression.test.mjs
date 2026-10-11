import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { benchmarkFiles, benchmarkSelection } from "./codspeed-benchmark-suites.mjs";

const workflow = readFileSync(new URL("../.github/workflows/codspeed.yml", import.meta.url), "utf8").replaceAll("\r\n", "\n");

function job(name) {
  const lines = workflow.split("\n");
  const start = lines.indexOf(`  ${name}:`);
  assert.notEqual(start, -1, `Missing job: ${name}`);
  let end = start + 1;
  while (end < lines.length && !/^ {2}[\w-]+:$/.test(lines[end])) end++;
  return lines.slice(start, end).join("\n");
}

function runBlock(text) {
  const marker = "        run: |\n";
  const start = text.indexOf(marker);
  assert.notEqual(start, -1, "Missing run block");
  const result = [];
  for (const line of text.slice(start + marker.length).split("\n")) {
    if (line && !line.startsWith("          ")) break;
    result.push(line.slice(10));
  }
  return result.join("\n");
}

function bash(script, env) {
  const result = spawnSync("bash", ["-e", "-o", "pipefail", "-c", script], {
    env: { ...process.env, ...env }, encoding: "utf8",
  });
  assert.ifError(result.error);
  return result;
}

function detect(paths, overrides = {}) {
  const root = mkdtempSync(join(tmpdir(), "kubecove-codspeed-"));
  const output = join(root, "output").replaceAll("\\", "/");
  try {
    const result = bash(`git() { if [[ "$1" == "diff" ]]; then printf '%s\\n' "$CHANGED_PATHS"; fi; }\n${runBlock(job("changes"))}`, {
      BASE_SHA: "a".repeat(40), HEAD_SHA: "b".repeat(40), EVENT_NAME: "pull_request",
      CHANGED_PATHS: paths.join("\n"), GITHUB_OUTPUT: output, ...overrides,
    });
    assert.equal(result.status, 0, result.stderr);
    return Object.fromEntries(readFileSync(output, "utf8").trim().split("\n").map((line) => line.split("=")));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

for (const [path, frontend, rust] of [
  ["README.md", false, false],
  ["src/App.svelte", true, false],
  ["src/lib/resource-table.ts", true, false],
  ["benchmarks/topology.bench.ts", true, false],
  ["package.json", true, false],
  ["bun.lock", true, false],
  ["vitest.config.ts", true, false],
  ["tsconfig.json", true, false],
  ["src-tauri/src/lib.rs", false, true],
  ["src-tauri/Cargo.toml", false, true],
  ["src-tauri/Cargo.lock", false, true],
  ["src-tauri/tauri.conf.json", false, true],
  [".cargo/config.toml", false, true],
  ["rust-toolchain.toml", false, true],
  [".github/workflows/codspeed.yml", true, true],
  ["fixtures/resource.yaml", true, true],
  ["shared/fixtures/resource.json", true, true],
  ["shared/fixtures/stream.txt", true, true],
  ["scripts/codspeed-benchmark-suites.mjs", true, false],
]) {
  test(`selects affected benchmarks for ${path}`, () => {
    assert.deepEqual(detect([path]), { frontend: String(frontend), rust: String(rust) });
  });
}

test("combines frontend and Rust changes, including removed paths", () => {
  assert.deepEqual(detect(["src/removed.ts", "src-tauri/src/removed.rs"]), { frontend: "true", rust: "true" });
});

for (const overrides of [{ EVENT_NAME: "workflow_dispatch" }, { BASE_SHA: "" }, { BASE_SHA: "0".repeat(40) }]) {
  test(`runs both suites without a usable comparison: ${JSON.stringify(overrides)}`, () => {
    assert.deepEqual(detect([], overrides), { frontend: "true", rust: "true" });
  });
}

function gate(overrides = {}) {
  return bash(runBlock(job("check")), {
    CHANGES_RESULT: "success", RELEASE_PR: "false",
    FRONTEND_REQUIRED: "true", FRONTEND_RESULT: "success",
    RUST_REQUIRED: "true", RUST_RESULT: "success", ...overrides,
  }).status;
}

test("the stable gate requires successful detection and all selected jobs", () => {
  assert.match(job("check"), /name: CodSpeed Performance Analysis/);
  assert.match(job("check"), /if: always\(\)/);
  assert.equal(gate(), 0);
  for (const result of ["failure", "cancelled", "skipped", ""]) {
    assert.notEqual(gate({ CHANGES_RESULT: result }), 0);
  }
});

for (const domain of ["FRONTEND", "RUST"]) {
  test(`the gate rejects incomplete ${domain} jobs and accepts only deliberate skips`, () => {
    for (const result of ["failure", "cancelled", "skipped", ""]) {
      assert.notEqual(gate({ [`${domain}_RESULT`]: result }), 0);
    }
    assert.equal(gate({ [`${domain}_REQUIRED`]: "false", [`${domain}_RESULT`]: "skipped" }), 0);
    assert.notEqual(gate({ [`${domain}_REQUIRED`]: "false" }), 0);
    assert.notEqual(gate({ [`${domain}_REQUIRED`]: "", [`${domain}_RESULT`]: "skipped" }), 0);
  });
}

test("release PRs still require both benchmark jobs to be skipped", () => {
  const release = { RELEASE_PR: "true", FRONTEND_RESULT: "skipped", RUST_RESULT: "skipped" };
  assert.equal(gate(release), 0);
  assert.notEqual(gate({ ...release, FRONTEND_RESULT: "success" }), 0);
  assert.notEqual(gate({ ...release, RUST_RESULT: "success" }), 0);
  assert.notEqual(gate({ ...release, RELEASE_PR: "false" }), 0);
});

test("frontend file jobs form a complete, disjoint partition of actual benchmark files", () => {
	const files = benchmarkFiles();
	assert.ok(files.length);
	const selected = files.flatMap(
		(file) => benchmarkSelection({ KUBECOVE_BENCHMARK_FILE: file }).include,
	);
	assert.deepEqual(selected, files);
	assert.equal(new Set(selected).size, selected.length);
	assert.deepEqual(benchmarkSelection({}).include, files);
	const legacy = ["topology", "other"]
		.flatMap(
			(suite) =>
				benchmarkSelection({ KUBECOVE_BENCHMARK_SUITE: suite }).include,
		)
		.sort();
	assert.deepEqual(legacy, files);
	assert.match(
		job("frontend"),
		/fromJSON\(needs\.changes\.outputs\.frontend_suites\)/,
	);
	assert.match(
		job("frontend"),
		/KUBECOVE_BENCHMARK_FILE: \$\{\{ matrix\.file \}\}/,
	);
});

test("invalid or conflicting frontend selectors fail instead of running every benchmark", () => {
	assert.throws(
		() =>
			benchmarkSelection({
				KUBECOVE_BENCHMARK_FILE: "benchmarks/missing.bench.ts",
			}),
		/Unknown benchmark file/,
	);
	assert.throws(
		() => benchmarkSelection({ KUBECOVE_BENCHMARK_SUITE: "typo" }),
		/Unknown benchmark suite/,
	);
	assert.throws(
		() =>
			benchmarkSelection({
				KUBECOVE_BENCHMARK_FILE: benchmarkFiles()[0],
				KUBECOVE_BENCHMARK_SUITE: "other",
			}),
		/not both/,
	);
});
