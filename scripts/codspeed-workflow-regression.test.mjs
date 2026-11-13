import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { benchmarkFiles, benchmarkGroups, benchmarkSelection } from "./codspeed-benchmark-suites.mjs";
import { frontendMatrix } from "./codspeed-selection.mjs";

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
    const result = bash(`git() { if [[ "$1" == "diff" ]]; then if [[ "$GIT_FAILURE" == true ]]; then return 1; fi; printf '%s' "$CHANGED_PATHS" | tr '\\n' '\\0'; fi; }\nnode() { if [[ "$NODE_FAILURE" == true ]]; then return 1; fi; if [[ "$1" == scripts/codspeed-detect.mjs && -n "\${NODE_OUTPUT+set}" ]]; then printf '%s' "$NODE_OUTPUT" >> "$GITHUB_OUTPUT"; return 0; fi; command node "$@"; }\n${runBlock(job("changes"))}`, {
      BASE_SHA: "a".repeat(40), HEAD_SHA: "b".repeat(40), EVENT_NAME: "pull_request",
      DEPENDENCIES_RESULT: "success", SELECTION_CHECK_RESULT: "success",
      GIT_FAILURE: "false", NODE_FAILURE: "false", RELEASE_PR: "false",
      CHANGED_PATHS: paths.join("\n"), GITHUB_OUTPUT: output, RUNNER_TEMP: root,
      GITHUB_STEP_SUMMARY: join(root, "summary"), ...overrides,
    });
    assert.equal(result.status, 0, result.stderr);
    const outputs = Object.fromEntries(readFileSync(output, "utf8").trim().split("\n").map((line) => [line.slice(0, line.indexOf("=")), line.slice(line.indexOf("=") + 1)]));
    const matrix = JSON.parse(outputs.frontend_matrix);
    assert.equal(matrix.include.length > 0, outputs.frontend === "true");
    if (overrides.NODE_OUTPUT === undefined) {
      const summary = readFileSync(join(root, "summary"), "utf8");
      assert.match(summary, /Rust shards/);
      for (const file of benchmarkFiles()) assert.ok(summary.includes(file) || summary.includes("Running every frontend file") || summary.includes("Labeled same-repository release PR"), file);
    }
    return { frontend: outputs.frontend, rust: outputs.rust };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

for (const [path, frontend, rust] of [
  ["README.md", false, false],
  ["src/App.svelte", false, false],
  ["src/lib/yamlFormat.ts", true, false],
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
  [".github/actions/codspeed-run/action.yml", true, true],
  [".github/workflows/ci.yml", false, false],
  [".github/actions/unrelated/action.yml", false, false],
  [".github/dependabot.yml", false, false],
  ["fixtures/resource.yaml", false, true],
  ["shared/fixtures/resource.json", false, true],
  ["shared/fixtures/stream.txt", false, true],
  ["scripts/codspeed-benchmark-suites.mjs", true, false],
  ["scripts/codspeed-import-closure.mjs", true, false],
  ["scripts/codspeed-selection.mjs", true, true],
  ["scripts/codspeed-detect.mjs", true, true],
  // Failure mode: a change to a script the Rust shards run merges without those shards running.
  ["scripts/ci-rust-cache-key.mjs", false, true],
  ["scripts/ci-mbx-cargo.mjs", false, true],
  // Frontend fixtures follow import closures. Rust retains its conservative fixture rule.
  ["e2e/parity/web.ts", false, false],
  ["e2e/parity/scenarios/argo-gaps.json", false, false],
  ["tests/release.test.ts", false, false],
  ["e2e/fixtures/chart/values.yaml", false, true],
]) {
  test(`selects affected benchmarks for ${path}`, () => {
    assert.deepEqual(detect([path]), { frontend: String(frontend), rust: String(rust) });
  });
}

test("combines frontend and Rust changes, including removed paths", () => {
  assert.deepEqual(detect(["src/lib/yamlFormat.ts", "src-tauri/src/removed.rs"]), { frontend: "true", rust: "true" });
});

for (const overrides of [{ EVENT_NAME: "push" }, { EVENT_NAME: "workflow_dispatch" }, { BASE_SHA: "" }, { BASE_SHA: "0".repeat(40) }, { DEPENDENCIES_RESULT: "failure" }, { SELECTION_CHECK_RESULT: "failure" }, { GIT_FAILURE: "true" }, { NODE_FAILURE: "true" }]) {
  test(`runs both suites without a usable comparison: ${JSON.stringify(overrides)}`, () => {
    assert.deepEqual(detect([], overrides), { frontend: "true", rust: "true" });
  });
}

const jobs = (selection) => JSON.stringify({ include: [{ group: 0, selection }] });

// Failure mode: the selector exits 0 with missing, extra or contradictory outputs, and the
// benchmark jobs read them: an unparsable matrix, or a frontend lane switched off by accident.
for (const [name, output] of [
  ["nothing", ""],
  ["a matrix that is not JSON", "frontend=true\nrust=false\nfrontend_matrix=not-json\n"],
  ["a matrix without include", "frontend=false\nrust=false\nfrontend_matrix={}\n"],
  ["an empty matrix with the frontend lane on", 'frontend=true\nrust=false\nfrontend_matrix={"include":[]}\n'],
  ["jobs with the frontend lane off", 'frontend=false\nrust=false\nfrontend_matrix={"include":[{"group":0,"selection":""}]}\n'],
  ["a job without a selection", 'frontend=true\nrust=false\nfrontend_matrix={"include":[{"group":0}]}\n'],
  ["a flag that is not a boolean", 'frontend=false\nrust=maybe\nfrontend_matrix={"include":[]}\n'],
  ["a missing Rust flag", 'frontend=false\nfrontend_matrix={"include":[]}\n'],
  ["a fourth output", 'frontend=false\nrust=false\nfrontend_matrix={"include":[]}\nrust=true\n'],
  // Failure mode: the benchmark run rejects its job's selection, so the job fails where
  // the fallback would have run everything.
  ["a job selection that is not JSON", `frontend=true\nrust=false\nfrontend_matrix=${jobs("not-json")}\n`],
  ["a job selection without files", `frontend=true\nrust=false\nfrontend_matrix=${jobs("{}")}\n`],
  ["a job selection with no file", `frontend=true\nrust=false\nfrontend_matrix=${jobs('{"include":[]}')}\n`],
  ["a job selection naming an unknown file", `frontend=true\nrust=false\nfrontend_matrix=${jobs('{"include":["benchmarks/gone.bench.ts"]}')}\n`],
  ["a job selection with a name filter that is not text", `frontend=true\nrust=false\nfrontend_matrix=${jobs('{"include":["benchmarks/topology.bench.ts"],"testNamePattern":null}')}\n`],
  ["a job selection with an invalid regex", `frontend=true\nrust=false\nfrontend_matrix=${jobs('{"include":["benchmarks/topology.bench.ts"],"testNamePattern":"["}')}\n`],
  ["two matrix documents", 'frontend=false\nrust=false\nfrontend_matrix={"include":[{}]} {"include":[]}\n'],
]) {
  test(`runs both suites when the selector succeeds with ${name}`, () => {
    assert.deepEqual(detect([], { NODE_OUTPUT: output }), { frontend: "true", rust: "true" });
  });
}

// Failure mode: the check above rejects what the benchmark run accepts, and every pull
// request runs everything.
test("keeps a selection the benchmark run accepts", () => {
  const selection = '{"include":["benchmarks/topology.bench.ts"],"exclude":[],"testNamePattern":"F07"}';
  assert.deepEqual(benchmarkSelection({ KUBECOVE_BENCHMARK_SELECTION: selection }).include, ["benchmarks/topology.bench.ts"]);
  assert.deepEqual(detect([], { NODE_OUTPUT: `frontend=true\nrust=false\nfrontend_matrix=${jobs(selection)}\n` }), { frontend: "true", rust: "false" });
});

// Failure mode: the selector's own error handling selects nothing instead of everything.
test("the selector runs everything when it cannot read the changed paths", () => {
  const root = mkdtempSync(join(tmpdir(), "kubecove-codspeed-detect-"));
  try {
    const output = join(root, "output");
    const result = spawnSync(process.execPath, ["scripts/codspeed-detect.mjs", join(root, "missing")], { cwd: new URL("../", import.meta.url), env: { ...process.env, EVENT_NAME: "pull_request", GITHUB_OUTPUT: output, GITHUB_STEP_SUMMARY: "", RELEASE_PR: "false", SELECTION_FALLBACK: "" }, encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(readFileSync(output, "utf8"), 'frontend=true\nrust=true\nfrontend_matrix={"include":[{"group":0,"selection":""}]}\n');
    const release = spawnSync(process.execPath, ["scripts/codspeed-detect.mjs", join(root, "missing")], { cwd: new URL("../", import.meta.url), env: { ...process.env, EVENT_NAME: "pull_request", GITHUB_OUTPUT: join(root, "release-output"), GITHUB_STEP_SUMMARY: "", RELEASE_PR: "true", SELECTION_FALLBACK: "" }, encoding: "utf8" });
    assert.equal(release.status, 0, release.stderr);
    assert.equal(readFileSync(join(root, "release-output"), "utf8"), 'frontend=false\nrust=false\nfrontend_matrix={"include":[]}\n');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

// Failure mode: a selector setup step that fails the detection job skips every benchmark job
// and fails the gate, where the fallback would have run everything.
test("a failed selector setup step still reaches the detection step", () => {
  const changes = job("changes");
  for (const name of ["Setup Bun", "Cache Bun dependencies", "Install selector dependencies", "Test benchmark orchestration"]) {
    const start = changes.indexOf(`      - name: ${name}\n`);
    assert.notEqual(start, -1, name);
    assert.match(changes.slice(start, changes.indexOf("\n      - name: ", start + 1)), /^ {8}continue-on-error: true$/m, name);
  }
  assert.ok(changes.indexOf("      - name: Cache Bun dependencies\n") < changes.indexOf("      - name: Install selector dependencies\n"));
  assert.match(changes.slice(changes.indexOf("      - name: Detect frontend and Rust benchmark changes\n")), /^ {8}if: \$\{\{ !cancelled\(\) \}\}$/m);
});

// Failure mode: a release pull request whose selector setup failed reports that every
// benchmark runs, while the benchmark jobs skip a release pull request.
for (const overrides of [{}, { DEPENDENCIES_RESULT: "failure" }, { SELECTION_CHECK_RESULT: "failure" }, { GIT_FAILURE: "true" }, { NODE_FAILURE: "true" }]) {
  test(`a release pull request selects nothing: ${JSON.stringify(overrides)}`, () => {
    assert.deepEqual(detect(["bun.lock"], { RELEASE_PR: "true", ...overrides }), { frontend: "false", rust: "false" });
  });
}

function gate(overrides = {}) {
  const root = mkdtempSync(join(tmpdir(), "codspeed-gate-"));
  try {
    return bash(runBlock(job("check")), {
      CHANGES_RESULT: "success", RELEASE_PR: "false",
      FRONTEND_REQUIRED: "true", FRONTEND_RESULT: "success",
      RUST_REQUIRED: "true", RUST_RESULT: "success", RUST_SAVER_RESULT: "success",
      GITHUB_STEP_SUMMARY: join(root, "summary"), ...overrides,
    }).status;
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("the stable gate requires successful detection and all selected jobs", () => {
  assert.match(job("check"), /name: CodSpeed Performance Analysis/);
  assert.match(job("check"), /if: always\(\)/);
  assert.equal(gate(), 0);
  assert.equal(gate({ FRONTEND_REQUIRED: "false", FRONTEND_RESULT: "skipped", RUST_REQUIRED: "false", RUST_RESULT: "skipped", RUST_SAVER_RESULT: "skipped" }), 0);
  for (const result of ["failure", "cancelled", "skipped", ""]) {
    assert.notEqual(gate({ CHANGES_RESULT: result }), 0);
  }
});

// The Rust shards are two jobs (the saving shard and the read-only ones), and each must pass.
for (const [domain, results] of [["FRONTEND", ["FRONTEND_RESULT"]], ["RUST", ["RUST_RESULT", "RUST_SAVER_RESULT"]]]) {
  test(`the gate rejects incomplete ${domain} jobs and accepts only deliberate skips`, () => {
    const skipped = Object.fromEntries(results.map((name) => [name, "skipped"]));
    for (const name of results) {
      for (const result of ["failure", "cancelled", "skipped", ""]) {
        assert.notEqual(gate({ [name]: result }), 0, `${name}=${result}`);
      }
      assert.notEqual(gate({ [`${domain}_REQUIRED`]: "false", ...skipped, [name]: "success" }), 0, `${name} ran`);
    }
    assert.equal(gate({ [`${domain}_REQUIRED`]: "false", ...skipped }), 0);
    assert.notEqual(gate({ [`${domain}_REQUIRED`]: "", ...skipped }), 0);
  });
}

// Failure modes: a gap between shard filters silently drops benchmarks from the report; an
// overlap runs a benchmark twice in one CodSpeed run, which CodSpeed rejects; a path without
// `::` (a benchmark at the bench crate root) matches no shard.
test("Rust benchmark shard filters split every benchmark path into exactly one shard", () => {
  const filters = [...`${job("rust")}\n${job("rust-saver")}`.matchAll(/^ {12}filter: "(.+)"$/gm)].map((match) => new RegExp(match[1]));
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
  // Rust benchmarks gate the required check through the results of both shard jobs.
  assert.match(job("check"), /RUST_RESULT: \$\{\{ needs\.rust\.result \}\}/);
  assert.match(job("check"), /RUST_SAVER_RESULT: \$\{\{ needs\.rust-saver\.result \}\}/);
  assert.match(job("check"), /^ {6}- rust\n {6}- rust-saver$/m);
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
  // Failure mode: the shards share one cache key, and a second saver fails its job when
  // GitHub has reserved the key for the first. Exactly one shard may save. GitHub's
  // `cache-mode` makes the others read-only, and it takes one literal per job, so the
  // saving shard is a job of its own with the same steps and condition. A step-level
  // ACTIONS_CACHE_MODE does nothing: the runner overrides it (run 43407667697).
  const shards = (name) => [...job(name).matchAll(/^ {10}- shard: (\w+)$/gm)].map(([, shard]) => shard);
  assert.deepEqual(shards("rust-saver"), ["ac"]);
  assert.deepEqual(shards("rust"), ["dl", "es", "hr", "jn", "other"]);
  assert.match(job("rust"), /^ {4}cache-mode: read$/m);
  assert.doesNotMatch(job("rust-saver"), /cache-mode/);
  // One at the top of the workflow would take the saver's access as well.
  assert.equal(workflow.match(/^ *cache-mode:/gm)?.length, 1);
  assert.doesNotMatch(workflow, /ACTIONS_CACHE_MODE: /);
  assert.match(job("rust"), /^ {4}steps: &rust-steps$/m);
  assert.match(job("rust-saver"), /^ {4}steps: \*rust-steps$/m);
  for (const key of ["name", "if", "runs-on"]) {
    const value = (name) => new RegExp(`^ {4}${key}: (.+)$`, "m").exec(job(name))?.[1];
    assert.ok(value("rust"), `Missing ${key}`);
    assert.equal(value("rust-saver"), value("rust"), `the saver's ${key} differs`);
  }
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
  const release = { RELEASE_PR: "true", FRONTEND_RESULT: "skipped", RUST_RESULT: "skipped", RUST_SAVER_RESULT: "skipped" };
  assert.equal(gate(release), 0);
  assert.notEqual(gate({ ...release, FRONTEND_RESULT: "success" }), 0);
  assert.notEqual(gate({ ...release, RUST_RESULT: "success" }), 0);
  assert.notEqual(gate({ ...release, RUST_SAVER_RESULT: "success" }), 0);
  assert.notEqual(gate({ ...release, RELEASE_PR: "false" }), 0);
});

// Vitest's collected name-filter contract is checked separately by
// codspeed-frontend-partition.integration.mjs. No benchmarks execute here.
test("frontend group jobs select every benchmark file within the measured projection", (t) => {
	const files = benchmarkFiles();
	assert.ok(files.length);
	const matrix = frontendMatrix(files).include;
	const count = matrix.length;
	const groups = matrix.map(({ group }) => group);
	assert.match(job("frontend"), /matrix: \$\{\{ fromJSON\(needs.changes.outputs.frontend_matrix\) \}\}/);
	assert.match(job("frontend"), /KUBECOVE_BENCHMARK_SELECTION: \$\{\{ matrix.selection \}\}/);
	const selections = matrix.map(({ selection }) => benchmarkSelection({ KUBECOVE_BENCHMARK_SELECTION: selection }));
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
	for (const selection of ["null", "{}", '{"include":[]}', '{"include":["missing"]}', JSON.stringify({ include: [benchmarkFiles()[0]], testNamePattern: 1 })]) {
		assert.throws(() => benchmarkSelection({ KUBECOVE_BENCHMARK_SELECTION: selection }));
	}
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
