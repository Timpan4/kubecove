import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { benchmarkFiles, benchmarkGroups, benchmarkSelection } from "./codspeed-benchmark-suites.mjs";

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
  ["src-tauri/crates/models/src/lib.rs", false, true],
  ["src-tauri/crates/kube/src/commands/discovery.rs", false, true],
  ["src-tauri/crates/inspection/src/commands/resources/kubeconform.rs", false, true],
  ["src-tauri/crates/live/src/commands/streams.rs", false, true],
  ["src-tauri/crates/gitops/src/commands/argo/connected.rs", false, true],
  ["src-tauri/crates/host/src/commands/usage.rs", false, true],
  ["src-tauri/crates/inspection/Cargo.toml", false, true],
  ["src-tauri/Cargo.toml", false, true],
  ["src-tauri/Cargo.lock", false, true],
  ["src-tauri/tauri.conf.json", false, true],
  ["src-tauri/gpui/src/model.rs", false, false],
  ["src-tauri/gpui/Cargo.lock", false, false],
  ["src-tauri/server/src/main.rs", false, false],
  ["src-tauri/server/Cargo.toml", false, false],
  [".cargo/config.toml", false, true],
  ["rust-toolchain.toml", false, true],
  [".github/workflows/codspeed.yml", true, true],
  ["fixtures/resource.yaml", true, true],
  ["shared/fixtures/resource.json", true, true],
  ["shared/fixtures/stream.txt", true, true],
  ["scripts/codspeed-benchmark-suites.mjs", true, false],
  // No benchmark imports from the E2E or unit test trees; their fixtures still select both.
  ["e2e/parity/web.ts", false, false],
  ["e2e/parity/scenarios/argo-gaps.json", false, false],
  ["tests/release.test.ts", false, false],
  ["e2e/fixtures/chart/values.yaml", true, true],
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

// Failure modes: a gap between shard filters silently drops benchmarks from the report; an
// overlap runs a benchmark twice in one CodSpeed run, which CodSpeed rejects; a path without
// `::` (a benchmark at the bench crate root) matches no shard.
test("Rust benchmark shard filters split every benchmark path into exactly one shard", () => {
  const filters = [...job("rust").matchAll(/^ {12}filter: "(.+)"$/gm)].map((match) => new RegExp(match[1]));
  assert.equal(filters.length, 6);
  assert.match(job("rust"), /cargo codspeed run --bench backend_hot_paths '\$\{\{ matrix\.filter \}\}'/);
  const leading = [...Array.from({ length: 95 }, (_, index) => String.fromCharCode(32 + index)), "é", "Ω"].filter((character) => character !== ":");
  for (const character of leading) {
    for (const path of [`${character}bench`, `module::${character}bench`, `a::b::${character}bench::Type`]) {
      const last = path.split("::").at(-1);
      const shards = filters.filter((filter) => filter.test(path));
      assert.equal(shards.length, 1, `${path} matched ${shards.length} shards`);
      assert.ok(shards[0].test(`x::${last}`), `${path} must be judged by its last segment`);
    }
  }
  // Rust benchmarks gate the required check through one aggregated matrix result.
  assert.match(job("check"), /RUST_RESULT: \$\{\{ needs\.rust\.result \}\}/);
  assert.match(job("rust"), /cache-targets: false/);
  // The key follows the build inputs, so an unchanged build skips the export and upload.
  assert.ok(job("rust").includes("run: node scripts/ci-rust-cache-key.mjs codspeed-backend .github/workflows/codspeed.yml src-tauri ':!src-tauri/gpui' ':!src-tauri/server'"));
  assert.ok(job("rust").includes("cache-key: ${{ steps.rust-cache.outputs.key }}"));
  assert.ok(job("rust").includes("restore-keys: ${{ steps.rust-cache.outputs.prefix }}"));
  assert.ok(job("rust").indexOf("dtolnay/rust-toolchain@") < job("rust").indexOf("scripts/ci-rust-cache-key.mjs"));
  assert.ok(job("rust").indexOf("scripts/ci-rust-cache-key.mjs") < job("rust").indexOf("jdx/mr-boxington-action@"));
  assert.match(job("rust"), /github-cache-mode: objects/);
  // The cache is inactive unless the shim is on PATH, and one shard saving is enough: every shard builds the same binary.
  assert.match(job("rust"), /version: "1\.30\.0"/);
  assert.match(job("rust"), /isolate-objects-cache: true/);
  assert.match(job("rust"), /clean-target-before-export: true/);
  assert.match(job("rust"), /save-if: \$\{\{ github\.ref == 'refs\/heads\/main' && matrix\.shard == 'ac' \}\}/);
  assert.match(job("rust"), /^ {10}- shard: ac$/m);
  assert.ok(job("rust").indexOf("run: node scripts/ci-mbx-cargo.mjs") < job("rust").indexOf("cargo codspeed build"));
  // The benchmark build leaves out the desktop feature, so it needs no GUI packages and
  // links no GUI stack. `bench-support` must not turn the feature back on in any crate.
  assert.match(job("rust"), /^ {8}run: cargo codspeed build --no-default-features --features bench-support --bench backend_hot_paths --locked$/m);
  assert.doesNotMatch(job("rust"), /apt-get|webkit/);
  for (const manifest of ["", "crates/kube/", "crates/inspection/", "crates/live/", "crates/gitops/", "crates/host/"]) {
    const features = /^bench-support = \[[^\]]*\]/m.exec(readFileSync(new URL(`../src-tauri/${manifest}Cargo.toml`, import.meta.url), "utf8"))?.[0];
    assert.ok(features, `${manifest}Cargo.toml has no bench-support feature`);
    assert.doesNotMatch(features, /"desktop"/, `${manifest}Cargo.toml`);
  }
  assert.notEqual(job("rust").indexOf("run: node scripts/ci-mbx-cargo.mjs"), -1);
  assert.match(job("rust"), /cargo-codspeed-x86_64-unknown-linux-musl/);
  assert.match(job("rust"), /CARGO_CODSPEED_SHA256/);
});

// Failure modes: the upstream action downloads the runner installer from codspeed.io, which
// answered HTTP 429 or 504 in five runs in two weeks; a version that differs from the pinned
// digest fails every job; uploads without the matrix context do not merge into one report.
test("benchmark jobs install the CodSpeed runner from its pinned GitHub release", () => {
  const action = readFileSync(new URL("../.github/actions/codspeed-run/action.yml", import.meta.url), "utf8").replaceAll("\r\n", "\n");
  assert.doesNotMatch(workflow, /CodSpeedHQ\/action@|codspeed\.io/);
  assert.doesNotMatch(action.replaceAll(/^#.*$/gm, ""), /codspeed\.io/);
  for (const name of ["frontend", "rust"]) {
    assert.match(job(name), /^ {6}- name: Run benchmarks\n {8}uses: \.\/\.github\/actions\/codspeed-run\n/m);
    assert.ok(job(name).indexOf("actions/checkout@") < job(name).indexOf("./.github/actions/codspeed-run"));
    assert.match(job(name), /id-token: write/);
  }
  assert.match(action, /^ {4}default: "\d+\.\d+\.\d+"$/m);
  assert.match(action, /^ {4}default: [0-9a-f]{64}$/m);
  assert.ok(action.includes('"https://github.com/CodSpeedHQ/codspeed/releases/download/v${VERSION}/codspeed-runner-installer.sh"'));
  assert.ok(action.indexOf("sha256sum --check --status") < action.indexOf('bash "${installer}" --quiet'));
  assert.ok(action.includes('GH_MATRIX: "${{ toJson(matrix) }}"') && action.includes('GH_STRATEGY: "${{ toJson(strategy) }}"'));
  assert.ok(action.includes('codspeed run --mode=simulation --setup-cache-dir="${HOME}/.cache/codspeed-action" -- "${CODSPEED_INPUT_RUN}"'));
});

test("release PRs still require both benchmark jobs to be skipped", () => {
  const release = { RELEASE_PR: "true", FRONTEND_RESULT: "skipped", RUST_RESULT: "skipped" };
  assert.equal(gate(release), 0);
  assert.notEqual(gate({ ...release, FRONTEND_RESULT: "success" }), 0);
  assert.notEqual(gate({ ...release, RUST_RESULT: "success" }), 0);
  assert.notEqual(gate({ ...release, RELEASE_PR: "false" }), 0);
});

// This file runs without installed dependencies (the CodSpeed detection job) and under
// `bun test`. The check of the name filters against the benchmarks Vitest collects needs
// both, so it lives in codspeed-frontend-partition.integration.mjs.
test("frontend group jobs select every benchmark file within the measured projection", (t) => {
	const files = benchmarkFiles();
	assert.ok(files.length);
	const groups = /^ {8}group: \[([\d, ]+)\]$/m.exec(job("frontend"))?.[1].split(",").map(Number);
	const count = Number(/KUBECOVE_BENCHMARK_GROUP: \$\{\{ matrix\.group \}\}\/(\d+)$/m.exec(job("frontend"))?.[1]);
	assert.deepEqual(groups, Array.from({ length: count }, (_, index) => index));
	const selections = groups.map((group) => benchmarkSelection({ KUBECOVE_BENCHMARK_GROUP: `${group}/${count}` }));
	assert.deepEqual([...new Set(selections.flatMap((selection) => selection.include))].sort(), files);
	const ci = readFileSync(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8");
	const scripts = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8")).scripts;
	assert.equal(scripts["test:bench-groups"], "node --test scripts/codspeed-frontend-partition.integration.mjs");
	assert.match(ci, /^ {12}run: bun run test:bench-groups$/m);
	const durations = groups.map((group) => benchmarkGroups(files, count)[group].seconds);
	t.diagnostic(`Predicted Run benchmarks seconds by group: ${durations.map((seconds) => seconds.toFixed(3)).join(", ")}`);
	assert.equal(count, 8);
	assert.ok(durations.every((seconds) => seconds <= 150), `predicted seconds: ${durations}`);
	assert.match(job("frontend"), /43322953935/);
	for (const file of files) assert.deepEqual(benchmarkSelection({ KUBECOVE_BENCHMARK_FILE: file }).include, [file]);
	assert.deepEqual(benchmarkSelection({}).include, files);
	const legacy = ["topology", "other"]
		.flatMap(
			(suite) =>
				benchmarkSelection({ KUBECOVE_BENCHMARK_SUITE: suite }).include,
		)
		.sort();
	assert.deepEqual(legacy, files);
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
		/only one/,
	);
	for (const group of ["4/4", "1", "a/4", "0/0", "1/2/3"]) {
		assert.throws(() => benchmarkSelection({ KUBECOVE_BENCHMARK_GROUP: group }), /KUBECOVE_BENCHMARK_GROUP must be/);
	}
	assert.throws(
		() => benchmarkSelection({ KUBECOVE_BENCHMARK_GROUP: "0/4", KUBECOVE_BENCHMARK_FILE: benchmarkFiles()[0] }),
		/only one/,
	);
});
