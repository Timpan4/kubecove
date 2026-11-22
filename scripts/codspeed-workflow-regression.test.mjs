import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
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
      GIT_FAILURE: "false", NODE_FAILURE: "false",
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
      for (const file of benchmarkFiles()) assert.ok(summary.includes(file) || summary.includes("Running every frontend file"), file);
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
  ["src-tauri/crates/models/src/command_contract.rs", false, true],
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
  ["scripts/codspeed-rust-artifact.mjs", true, true],
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
    const result = spawnSync(process.execPath, ["scripts/codspeed-detect.mjs", join(root, "missing")], { cwd: new URL("../", import.meta.url), env: { ...process.env, EVENT_NAME: "pull_request", GITHUB_OUTPUT: output, GITHUB_STEP_SUMMARY: "", SELECTION_FALLBACK: "" }, encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(readFileSync(output, "utf8"), 'frontend=true\nrust=true\nfrontend_matrix={"include":[{"group":0,"selection":""}]}\n');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

// Failure mode: a selector setup step that fails the detection job skips every benchmark job
// and fails the gate, where the fallback would have run everything.
test("a failed selector setup step still reaches the detection step", () => {
  const changes = job("changes");
  for (const name of ["Setup Bun", "Install selector dependencies", "Test benchmark orchestration"]) {
    const start = changes.indexOf(`      - name: ${name}\n`);
    assert.notEqual(start, -1, name);
    assert.match(changes.slice(start, changes.indexOf("\n      - name: ", start + 1)), /^ {8}continue-on-error: true$/m, name);
  }
  assert.doesNotMatch(workflow, /\.bun\/install\/cache/);
  assert.match(changes, /bun install --frozen-lockfile/);
  assert.match(changes.slice(changes.indexOf("      - name: Detect frontend and Rust benchmark changes\n")), /^ {8}if: \$\{\{ !cancelled\(\) \}\}$/m);
});

test("release metadata cannot skip benchmarks selected by the diff", () => {
  const paths = ["package.json", "src-tauri/Cargo.toml", "CHANGELOG.md"];
  assert.deepEqual(detect(paths, { RELEASE_PR: "true" }), { frontend: "true", rust: "true" });
});

function gate(overrides = {}) {
  const root = mkdtempSync(join(tmpdir(), "codspeed-gate-"));
  try {
    return bash(runBlock(job("check")), {
      CHANGES_RESULT: "success",
      FRONTEND_REQUIRED: "true", FRONTEND_RESULT: "success",
      RUST_REQUIRED: "true", RUST_BUILD_RESULT: "success", RUST_RESULT: "success", RUST_SAVER_RESULT: "success",
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
  assert.equal(gate({ FRONTEND_REQUIRED: "false", FRONTEND_RESULT: "skipped", RUST_REQUIRED: "false", RUST_BUILD_RESULT: "skipped", RUST_RESULT: "skipped", RUST_SAVER_RESULT: "skipped" }), 0);
  for (const result of ["failure", "cancelled", "skipped", ""]) {
    assert.notEqual(gate({ CHANGES_RESULT: result }), 0);
  }
});

// The shared build and both groups of Rust shards have to pass.
for (const [domain, results] of [["FRONTEND", ["FRONTEND_RESULT"]], ["RUST", ["RUST_BUILD_RESULT", "RUST_RESULT", "RUST_SAVER_RESULT"]]]) {
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
  const leading = [...Array.from({ length: 95 }, (_, index) => String.fromCharCode(32 + index)), "é", "Ω"].filter((character) => character !== ":");
  for (const character of leading) {
    for (const path of [`${character}bench`, `module::${character}bench`, `a::b::${character}bench::Type`]) {
      const last = path.split("::").at(-1);
      const shards = filters.filter((filter) => filter.test(path));
      assert.equal(shards.length, 1, `${path} matched ${shards.length} shards`);
      assert.ok(shards[0].test(`x::${last}`), `${path} must be judged by its last segment`);
    }
  }
  assert.match(job("rust"), /CODSPEED_CARGO_WORKSPACE_ROOT="\$PWD" exec \.\/target\/codspeed\/analysis\/kubecove\/backend_hot_paths '\$\{\{ matrix\.filter \}\}'/);
  assert.deepEqual([...job("rust").matchAll(/^ {10}- shard: (\w+)$/gm)].map(([, shard]) => shard), ["dl", "es", "hr", "jn", "other"]);
  assert.match(job("rust-saver"), /^ {10}- shard: ac$/m);
  assert.match(job("rust"), /^ {4}steps: &rust-steps$/m);
  assert.match(job("rust-saver"), /^ {4}steps: \*rust-steps$/m);
  for (const name of ["rust", "rust-saver"]) assert.match(job(name), /^ {4}needs: \[changes, rust-build\]$/m);
});

test("one Rust job builds on the existing runner and each shard verifies its artifact before simulation", () => {
  const check = job("check");
  assert.match(check, /^ {6}- rust-build\n {6}- rust\n {6}- rust-saver$/m);
  assert.match(check, /RUST_BUILD_RESULT: \$\{\{ needs\.rust-build\.result \}\}/);
  assert.match(check, /RUST_RESULT: \$\{\{ needs\.rust\.result \}\}/);
  assert.match(check, /RUST_SAVER_RESULT: \$\{\{ needs\.rust-saver\.result \}\}/);
  const build = job("rust-build");
  const shards = job("rust");
  for (const name of ["rust-build", "rust", "rust-saver"]) assert.match(job(name), /^ {4}runs-on: ubuntu-latest$/m);
  assert.match(build, /^ {4}timeout-minutes: 30$/m);
  const artifactScript = readFileSync(new URL("./codspeed-rust-artifact.mjs", import.meta.url), "utf8");
  assert.match(artifactScript, /\["--measurement-mode", "simulation",/);
  assert.doesNotMatch(shards, /rust-toolchain|rust-cache|mr-boxington|ci-mbx-cargo|cargo codspeed build|cargo-codspeed-x86/);
  assert.match(build, /cache-targets: false/);
  assert.match(build, /save-if: \$\{\{ github\.ref == 'refs\/heads\/main' \}\}/);
  assert.match(build, /github-cache-mode: objects/);
  assert.match(build, /isolate-objects-cache: true/);
  assert.match(build, /clean-target-before-export: true/);
  assert.match(build, /version: "1\.30\.0"/);
  assert.match(build, /cache-key: \$\{\{ steps\.rust-cache\.outputs\.key \}\}/);
  assert.match(build, /restore-keys: \$\{\{ steps\.rust-cache\.outputs\.prefix \}\}/);
  assert.match(build, /CARGO_CODSPEED_SHA256: df4b2618fda2aa19f8b92e8d745548a49d9ce20785472eeb584fdc2c99624395/);
  assert.match(build, /download\/v5\.0\.1\/cargo-codspeed-x86_64-unknown-linux-musl/);
  assert.doesNotMatch(build, /id-token: write|apt-get|webkit/);
  assert.match(build, /codspeed-backend \.github\/workflows\/codspeed\.yml scripts\/codspeed-rust-artifact\.mjs src-tauri/);
  assert.match(build, /artifact-id: \$\{\{ steps\.upload\.outputs\.artifact-id \}\}/);
  assert.match(build, /if-no-files-found: error/);
  assert.match(shards, /artifact-ids: \$\{\{ needs\.rust-build\.outputs\.artifact-id \}\}/);
  assert.match(shards, /digest-mismatch: error/);
  assert.match(shards, /EXPECTED_MANIFEST_SHA256: \$\{\{ needs\.rust-build\.outputs\.manifest-sha256 \}\}/);
  assert.match(shards, /EXPECTED_TOOLCHAIN: \$\{\{ needs\.rust-build\.outputs\.toolchain \}\}/);
  const inOrder = (source, steps) => {
    for (const [index, name] of steps.entries()) {
      const position = source.indexOf(name);
      assert.notEqual(position, -1, name);
      if (index > 0) assert.ok(position > source.indexOf(steps[index - 1]), name);
    }
  };
  inOrder(build, ["dtolnay/rust-toolchain@", "node scripts/ci-rust-cache-key.mjs", "jdx/mr-boxington-action@", "node scripts/ci-mbx-cargo.mjs", "node scripts/codspeed-rust-artifact.mjs build", "actions/upload-artifact@"]);
  inOrder(shards, ["Require the build artifact identity", "actions/download-artifact@", "node scripts/codspeed-rust-artifact.mjs install", "./.github/actions/codspeed-run"]);
  for (const manifest of ["", "crates/kube/", "crates/inspection/", "crates/live/", "crates/gitops/", "crates/host/"]) {
    const features = /^bench-support = \[[^\]]*\]/m.exec(readFileSync(new URL(`../src-tauri/${manifest}Cargo.toml`, import.meta.url), "utf8"))?.[0];
    assert.ok(features, `${manifest}Cargo.toml has no bench-support feature`);
    assert.doesNotMatch(features, /"desktop"/, `${manifest}Cargo.toml`);
  }
});

// Failure modes: the upstream action downloads the runner installer from codspeed.io, which
// answered HTTP 429 or 504 in five runs in two weeks; a version that differs from the pinned
// digest fails every job; uploads without the matrix context do not merge into one report.
test("Rust shards reject missing or tampered artifacts before installing the executable", () => {
  if (process.platform !== "linux" || process.arch !== "x64") return;
  const root = mkdtempSync(join(tmpdir(), "kubecove-codspeed-artifact-"));
  const artifact = join(root, "artifact");
  const benchmark = "backend_hot_paths";
  const installed = join(root, "src-tauri", "target", "codspeed", "analysis", "kubecove", benchmark);
  const lock = "test lockfile\n";
  const executable = "test executable\n";
  const toolchain = `1.100.0/${"b".repeat(40)}`;
  const digest = (value) => createHash("sha256").update(value).digest("hex");
  const git = (...args) => {
    const result = spawnSync("git", args, { cwd: root, encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  };
  try {
    mkdirSync(artifact);
    mkdirSync(join(root, "src-tauri"));
    writeFileSync(join(root, "src-tauri", "Cargo.lock"), lock);
    git("init", "-q");
    git("add", "src-tauri/Cargo.lock");
    git("-c", "user.name=Test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false", "commit", "-qm", "fixture");
    const commit = git("rev-parse", "HEAD");
    const metadata = {
      commit, toolchain, target: "x86_64-unknown-linux-gnu",
      buildArgs: ["--measurement-mode", "simulation", "--profile", "bench", "--no-default-features", "--features", "bench-support", "--bench", benchmark, "--locked"],
      cargoLockSha256: digest(lock), executableSha256: digest(executable),
    };
    const run = ({ overrides, invalidDigest, omitManifest, binary = executable } = {}) => {
      const manifest = JSON.stringify({ ...metadata, ...overrides });
      if (!omitManifest) writeFileSync(join(artifact, "manifest.json"), manifest);
      writeFileSync(join(artifact, benchmark), binary);
      return spawnSync(process.execPath, [fileURLToPath(new URL("./codspeed-rust-artifact.mjs", import.meta.url)), "install", artifact], {
        cwd: root, encoding: "utf8",
        env: { ...process.env, GITHUB_SHA: commit, EXPECTED_TOOLCHAIN: toolchain, EXPECTED_MANIFEST_SHA256: invalidDigest ? digest("wrong") : digest(manifest) },
      });
    };
    for (const [input, reason] of [
      [{ omitManifest: true }, /Unexpected benchmark artifact files/],
      [{ invalidDigest: true }, /manifest digest does not match/],
      [{ overrides: { commit: "a".repeat(40) } }, /build identity does not match/],
      [{ overrides: { toolchain: `1.99.1/${"b".repeat(40)}` } }, /build identity does not match/],
      [{ overrides: { target: "aarch64-unknown-linux-gnu" } }, /build identity does not match/],
      [{ overrides: { buildArgs: ["--features", "desktop"] } }, /build identity does not match/],
      [{ overrides: { cargoLockSha256: digest("wrong") } }, /build identity does not match/],
      [{ binary: "tampered" }, /executable digest does not match/],
    ]) {
      const result = run(input);
      assert.notEqual(result.status, 0, result.stdout);
      assert.match(result.stderr, reason);
      assert.equal(existsSync(installed), false);
    }
    const result = run();
    assert.equal(result.status, 0, result.stderr);
    assert.equal(readFileSync(installed, "utf8"), executable);
    assert.equal(statSync(installed).mode & 0o777, 0o755);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

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
