import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { cacheName, supersededCaches } from "./ci-prune-rust-caches.mjs";

const workflow = readFileSync(process.env.CI_WORKFLOW_PATH ?? new URL("../.github/workflows/ci.yml", import.meta.url), "utf8").replaceAll("\r\n", "\n");
const base = "a".repeat(40);
const head = "b".repeat(40);
const domains = ["docs", "frontend", "nix", "rust"];
const flags = (...selected) => Object.fromEntries(domains.map((key) => [key, String(selected.includes(key))]));
const all = flags(...domains);

// Missing or stale records, a record restored under another run's key, malformed
// proof, a different runner, non-PR reuse and a source job that failed in its post
// hooks must all run verification. The pass after such a failure must be reusable.
test("Rust pass records require exact inputs, runner identity and a pull request", () => {
  const root = mkdtempSync(join(tmpdir(), "kubecove-ci-pass-"));
  const key = `mbx-ci-gpui-test-pass-${"a".repeat(64)}-${"b".repeat(64)}`;
  const output = join(root, "output");
  const env = {
    ...process.env, PASS_TASK: "gpui-test", PASS_KEY: key, PASS_CACHE_MATCH: "", PASS_EVENT: "pull_request",
    RUNNER_OS: "Linux", RUNNER_ARCH: "X64", ImageOS: "ubuntu24", ImageVersion: "20261109.1.0",
    GITHUB_SERVER_URL: "https://github.com", GITHUB_REPOSITORY: "Timpan4/kubecove", GITHUB_RUN_ID: "123", GITHUB_RUN_ATTEMPT: "1", GITHUB_OUTPUT: output,
    GITHUB_API_URL: "https://api.github.com", GITHUB_TOKEN: "fixture-token",
  };
  const preload = join(root, "source-run.mjs");
  writeFileSync(preload, [
    'import assert from "node:assert/strict";',
    'import { writeFileSync } from "node:fs";',
    'if (process.env.SOURCE_ABORT) AbortSignal.timeout = (milliseconds) => {',
    '  writeFileSync(process.env.TIMEOUT_PROBE, String(milliseconds));',
    '  const controller = new AbortController();',
    '  queueMicrotask(() => controller.abort(new DOMException("Fixture deadline", "TimeoutError")));',
    '  return controller.signal;',
    '};',
    'globalThis.fetch = async (url, options) => {',
    '  assert.equal(url, `https://api.github.com/repos/Timpan4/kubecove/actions/runs/123/attempts/${process.env.SOURCE_ATTEMPT ?? 1}/jobs?per_page=100`);',
    '  assert.equal(options.headers.Authorization, "Bearer fixture-token");',
    '  if (process.env.SOURCE_RESULT === "error") throw new Error("unavailable");',
    '  const job = { name: process.env.SOURCE_NAME ?? "CI gpui-test", status: process.env.SOURCE_STATUS ?? "completed", conclusion: process.env.SOURCE_RESULT ?? "success" };',
    '  const unrelated = Array.from({ length: Number(process.env.SOURCE_OTHER_JOBS ?? 0) }, (_, i) => ({ name: "Other job " + i }));',
    '  const jobs = [...unrelated, job, ...(process.env.SOURCE_DUPLICATE ? [job] : [])];',
    '  const total = Number(process.env.SOURCE_TOTAL ?? jobs.length);',
    '  const aborted = () => new Promise((resolve, reject) => {',
    '    if (options.signal.aborted) reject(options.signal.reason);',
    '    else options.signal.addEventListener("abort", () => reject(options.signal.reason), { once: true });',
    '  });',
    '  if (process.env.SOURCE_ABORT === "headers" && options.signal) return aborted();',
    '  return { ok: !process.env.SOURCE_DENIED, json: process.env.SOURCE_ABORT === "body" && options.signal ? aborted : async () => ({ total_count: total, jobs: process.env.SOURCE_MALFORMED ? null : jobs }) };',
    '};',
  ].join("\n"));
  const gpuiSteps = ["setup-gpui-native", "setup-gpui-profile", "export-gpui-fixture", "test-gpui"];
  const steps = Object.fromEntries(["setup-rust", "rust-cache", "rust-pass", "pass-cache", "decide-pass", "setup-mbx", "activate-cargo", ...gpuiSteps].map((id) => [id, { outcome: "success", conclusion: "success" }]));
  steps["decide-pass"].outputs = { skip: "false" };
  const run = (mode, overrides = {}) => spawnSync(process.execPath, ["--import", preload, fileURLToPath(new URL("./ci-rust-pass.mjs", import.meta.url)), mode], {
    cwd: root, env: { ...env, ...overrides }, encoding: "utf8",
  });
  const decide = (overrides = {}) => {
    writeFileSync(output, "");
    const result = run("decide", overrides);
    assert.equal(result.status, 0, result.stderr);
    const decision = readFileSync(output, "utf8").trim();
    assert.ok(decision === "skip=true" || decision === "skip=false", decision);
    return { skip: decision === "skip=true", log: result.stdout };
  };
  const cache = (overrides = {}) => {
    writeFileSync(output, "");
    assert.equal(run("cache-key", overrides).status, 0);
    return Object.fromEntries(readFileSync(output, "utf8").trim().split("\n").map((line) => line.split("=")));
  };
  try {
    const first = cache();
    const rerun = cache({ GITHUB_RUN_ATTEMPT: "2" });
    env.PASS_CACHE_MATCH = first.key;
    assert.equal(decide().skip, false, "a missing record runs");
    const recorded = run("verify", { PASS_SKIP: "false", PASS_CACHE_MATCH: "", PASS_STEPS: JSON.stringify(steps) });
    assert.equal(recorded.status, 0, recorded.stderr);
    const path = join(root, ".ci-rust-pass/record.json");
    const record = readFileSync(path, "utf8");
    const hit = decide();
    assert.equal(hit.skip, true);
    assert.equal(decide({ SOURCE_OTHER_JOBS: "30" }).skip, true, "a complete page can prove a job past the default first 30");
    assert.equal(hit.log, `gpui-test: ${first.key} records https://github.com/Timpan4/kubecove/actions/runs/123/attempts/1; skipping build and test work.\n`);
    for (const phase of ["headers", "body"]) {
      const probe = join(root, "timeout-probe");
      writeFileSync(probe, "");
      assert.equal(decide({ SOURCE_ABORT: phase, TIMEOUT_PROBE: probe }).skip, false, `aborted ${phase} must run checks`);
      assert.equal(readFileSync(probe, "utf8"), "10000", "the owner selected ten seconds");
    }
    for (const overrides of [
      { PASS_CACHE_MATCH: "" }, { PASS_CACHE_MATCH: first.prefix }, { PASS_CACHE_MATCH: rerun.key },
      { PASS_EVENT: "push" }, { PASS_EVENT: "workflow_dispatch" },
      { PASS_KEY: key.replace(/b{64}$/, "c".repeat(64)) }, { PASS_TASK: "gpui-clippy" },
      { ImageVersion: "new-image" }, { ImageVersion: "" }, { RUNNER_ARCH: "ARM64" },
      ...["failure", "cancelled", "skipped", "", "error"].map((SOURCE_RESULT) => ({ SOURCE_RESULT })),
      // A refused request can still carry a body, and a second page can hold a failed job of the same name.
      { SOURCE_DENIED: "true" }, { SOURCE_TOTAL: "2" }, { SOURCE_TOTAL: "101" },
      { SOURCE_NAME: "another job" },
      { SOURCE_STATUS: "in_progress" }, { SOURCE_DUPLICATE: "true" }, { SOURCE_MALFORMED: "true" }, { GITHUB_TOKEN: "" },
    ]) assert.equal(decide(overrides).skip, false, JSON.stringify(overrides));
    const forged = [
      record.replace("https://github.com/", "https://attacker.invalid/"),
      // Same length as the repository, so only the prefix comparison can reject it.
      record.replace("Timpan4/kubecove", "Timpan4/kubecovf"),
      record.replace('"task":"gpui-test"', '"task":"gpui-clippy"'),
      JSON.stringify({ ...JSON.parse(record), key: key.replace(/b{64}$/, "c".repeat(64)) }),
      ...["RUNNER_OS", "RUNNER_ARCH", "ImageOS", "ImageVersion"].map((field) => JSON.stringify({
        ...JSON.parse(record), environment: { ...JSON.parse(record).environment, [field]: "other" },
      })),
    ];
    for (const content of forged) assert.notEqual(content, record);
    for (const content of ["not JSON", "null", "{}", "[]", JSON.stringify({ ...JSON.parse(record), run: [JSON.parse(record).run] }), ...forged]) {
      writeFileSync(path, content);
      assert.equal(decide().skip, false, content);
    }
    writeFileSync(path, record);
    const skipped = { ...steps, "decide-pass": { ...steps["decide-pass"], outputs: { skip: "true" } }, ...Object.fromEntries(["setup-mbx", "activate-cargo", ...gpuiSteps].map((id) => [id, { outcome: "skipped", conclusion: "skipped" }])) };
    assert.equal(run("verify", { PASS_SKIP: "true", PASS_STEPS: JSON.stringify(skipped) }).status, 0);
    assert.equal(run("verify", { PASS_SKIP: "true", PASS_STEPS: JSON.stringify(skipped), SOURCE_RESULT: "error" }).status, 0, "completion consumes the successful decision without another API request");
    assert.notEqual(run("verify", { PASS_SKIP: "true", PASS_STEPS: JSON.stringify({ ...skipped, "decide-pass": steps["decide-pass"] }) }).status, 0, "the decision output must justify the skip");
    assert.notEqual(run("verify", { PASS_SKIP: "true", PASS_CACHE_MATCH: "", PASS_STEPS: JSON.stringify(skipped) }).status, 0, "no proof rejects an unexpected skip");
    for (const outcome of ["failure", "cancelled", "skipped", "", undefined]) {
      const result = run("verify", { PASS_SKIP: "false", PASS_CACHE_MATCH: "", PASS_STEPS: JSON.stringify({ ...steps, "test-gpui": { outcome, conclusion: outcome } }) });
      assert.notEqual(result.status, 0, `unverified ${outcome} must fail`);
      assert.equal(readFileSync(path, "utf8"), record, "a failure cannot overwrite a pass");
    }
    // A step outside the required list must not have failed or been cancelled.
    // The record lookup continues on error, so its conclusion stays success.
    const withStep = (step) => run("verify", { PASS_SKIP: "false", PASS_CACHE_MATCH: "", PASS_STEPS: JSON.stringify({ ...steps, "restore-pass": step }) }).status;
    assert.equal(withStep({ outcome: "failure", conclusion: "success" }), 0);
    for (const conclusion of ["failure", "cancelled"]) assert.notEqual(withStep({ outcome: conclusion, conclusion }), 0, conclusion);
    // The source job failed in a post hook after it saved its record. The rerun
    // verifies again and saves under its own key, which later lookups restore.
    assert.equal(first.prefix, rerun.prefix);
    assert.ok(first.key !== rerun.key && rerun.key.startsWith(rerun.prefix));
    assert.equal(decide({ SOURCE_RESULT: "failure" }).skip, false);
    assert.equal(run("verify", { GITHUB_RUN_ATTEMPT: "2", PASS_SKIP: "false", PASS_STEPS: JSON.stringify(steps) }).status, 0);
    assert.equal(decide({ PASS_CACHE_MATCH: rerun.key, SOURCE_ATTEMPT: "2" }).skip, true);
    assert.equal(decide({ SOURCE_ATTEMPT: "2" }).skip, false, "a record must name the run in its restored key");

    // GitHub leaves skipped background steps out of the steps context entirely.
    const server = { PASS_TASK: "server", PASS_KEY: key.replace("gpui-test", "server") };
    const serverIds = ["setup-mbx", "activate-cargo", "api-only", "browser-build", "test-server", "lint-server", "build-server", "headless-report"];
    const background = ["server-command-contract", "http-lifecycle"];
    const base = Object.fromEntries(["setup-rust", "rust-cache", "rust-pass", "pass-cache", "decide-pass"].map((id) => [id, { outcome: "success", conclusion: "success" }]));
    const ran = { ...base, "decide-pass": { ...base["decide-pass"], outputs: { skip: "false" } }, ...Object.fromEntries([...serverIds, ...background].map((id) => [id, { outcome: "success", conclusion: "success" }])) };
    const serverKey = cache(server).key;
    assert.equal(run("verify", { ...server, PASS_SKIP: "false", PASS_STEPS: JSON.stringify(ran) }).status, 0);
    for (const id of background) {
      const { [id]: _, ...missing } = ran;
      assert.notEqual(run("verify", { ...server, PASS_SKIP: "false", PASS_STEPS: JSON.stringify(missing) }).status, 0, `a run that skipped ${id} must fail`);
    }
    const reused = { ...base, "decide-pass": { ...base["decide-pass"], outputs: { skip: "true" } }, ...Object.fromEntries(serverIds.map((id) => [id, { outcome: "skipped", conclusion: "skipped" }])) };
    const reuse = { ...server, PASS_SKIP: "true", PASS_CACHE_MATCH: serverKey };
    const reusedResult = run("verify", { ...reuse, PASS_STEPS: JSON.stringify(reused) });
    assert.equal(reusedResult.status, 0, reusedResult.stderr);
    const reusedSkipped = { ...reused, ...Object.fromEntries(background.map((id) => [id, { outcome: "skipped", conclusion: "skipped" }])) };
    assert.equal(run("verify", { ...reuse, PASS_STEPS: JSON.stringify(reusedSkipped) }).status, 0);
    assert.notEqual(run("verify", { ...reuse, PASS_STEPS: JSON.stringify({ ...reusedSkipped, "http-lifecycle": { outcome: "success", conclusion: "success" } }) }).status, 0, "a reused pass cannot run a background step");
  } finally { rmSync(root, { recursive: true, force: true }); }
});

// Extract the workflow's fixed-indentation blocks; execute its actual code.
function job(name, text = workflow) {
  const lines = text.split("\n");
  const start = lines.indexOf(`  ${name}:`);
  assert.notEqual(start, -1, `Missing job: ${name}`);
  let end = start + 1;
  while (end < lines.length && !/^ {2}[\w-]+:$/.test(lines[end])) end++;
  return lines.slice(start, end).join("\n");
}
function block(text, key, indent) {
  const marker = `${" ".repeat(indent)}${key}: |\n`;
  const start = text.indexOf(marker);
  assert.notEqual(start, -1, `Missing ${key} block`);
  const lines = text.slice(start + marker.length).split("\n");
  const result = [];
  for (const line of lines) {
    if (line && !line.startsWith(" ".repeat(indent + 2))) break;
    result.push(line.slice(indent + 2));
  }
  return result.join("\n");
}
async function detect(paths, options = {}) {
  const context = {
    repo: { owner: "Timpan4", repo: "kubecove" },
    eventName: "pull_request", sha: "c".repeat(40),
    payload: { pull_request: { base: { sha: base }, head: { sha: head } } },
    ...options.context,
  };
  const output = {};
  const warnings = [];
  const requests = [];
  const github = { request: async (route, params) => {
    requests.push({ route, params });
    if (Object.hasOwn(options, "error")) throw options.error;
    return { data: {
      base_commit: { sha: base }, merge_base_commit: { sha: base }, status: "ahead",
      files: paths.map((filename) => ({ filename, status: "modified" })),
      ...options.data,
    } };
  } };
  const core = { setOutput: (key, value) => { output[key] = value; }, warning: (message) => warnings.push(message) };
  const AsyncFunction = Object.getPrototypeOf(async () => {}).constructor;
  await new AsyncFunction("github", "context", "core", block(job("changes"), "script", 10))(github, context, core);
  const matrix = JSON.parse(output.matrix);
  delete output.matrix;
  return { output, matrix, warnings, requests };
}

for (const [path, selected] of [
  ["README.md", ["docs"]],
  ["docs/assets/wiki/screenshot.png", ["docs"]],
  ["src/App.svelte", ["frontend"]],
  ["src/fixtures/example.yaml", ["frontend"]],
  ["public/app-icon.svg", ["frontend"]],
  ["e2e/fixtures/resource.yaml", ["frontend"]],
  ["scripts/check-docs.ts", ["frontend"]],
  ["scripts/ci-mbx-cargo.mjs", domains],
  ["scripts/ci-rust-cache-key.mjs", domains],
  ["scripts/check-rust-format.mjs", domains],
  ["bun.lock", ["frontend", "nix"]],
  ["rust/src/lib.rs", ["rust"]],
  ["rust/crates/models/src/lib.rs", ["rust"]],
  ["rust/crates/kube/src/commands/discovery.rs", ["rust"]],
  ["rust/crates/inspection/src/commands/resources/kubeconform.rs", ["rust"]],
  ["rust/crates/live/src/commands/streams.rs", ["rust"]],
  ["rust/crates/gitops/src/commands/argo/connected.rs", ["rust"]],
  ["rust/crates/host/src/commands/usage.rs", ["rust"]],
  ["rust/crates/inspection/Cargo.toml", ["nix", "rust"]],
  ["rust/gpui/updater.json", ["frontend", "rust"]],
  ["rust/Cargo.lock", ["nix", "rust"]],
  ["tools/new-cli/Cargo.lock", ["nix", "rust"]],
  ["Cargo.toml", ["nix", "rust"]],
  [".cargo/config.toml", ["rust"]],
  ["rust-toolchain.toml", ["rust"]],
  ["flake.lock", ["nix"]],
  ["flake.nix", ["nix"]],
  [".github/workflows/ci.yml", domains],
  [".github/actions/setup/action.yml", domains],
  ["unknown-build-input.conf", domains],
]) {
  test(`classifies ${path}`, async () => assert.deepEqual((await detect([path])).output, flags(...selected)));
}
test("combines scopes and both sides of a cross-domain rename", async () => {
  const result = await detect([], { data: { files: [{ filename: "docs/example.md", previous_filename: "src/example.ts", status: "renamed" }] } });
  assert.deepEqual(result.output, flags("docs", "frontend"));
});
test("removed files still select their checks", async () => {
  assert.deepEqual((await detect([], { data: { files: [{ filename: "rust/src/removed.rs", status: "removed" }] } })).output, flags("rust"));
});
test("filenames are data, including newlines and shell metacharacters", async () => {
  assert.deepEqual((await detect(["public/odd\n$(echo bad).svg"])).output, flags("frontend"));
});
test("a verified empty diff skips domain jobs", async () => assert.deepEqual((await detect([])).output, flags()));
test("requests immutable event SHAs, not the changing PR or merge ref", async () => {
  const result = await detect(["README.md"]);
  assert.equal(result.requests.length, 1);
  assert.deepEqual(result.requests[0], {
    route: "GET /repos/{owner}/{repo}/compare/{basehead}",
    params: { owner: "Timpan4", repo: "kubecove", basehead: `${base}...${head}`, per_page: 1, page: 1, request: { timeout: 15000 } },
  });
  assert.deepEqual(result.warnings, []);
});
test("fork PRs use their immutable head SHA without write permissions", async () => {
  const result = await detect(["README.md"], { context: { payload: { pull_request: {
    base: { sha: base }, head: { sha: head, repo: { full_name: "contributor/kubecove" } },
  } } } });
  assert.deepEqual(result.output, flags("docs"));
  assert.equal(result.requests[0].params.basehead, `${base}...${head}`);
});
test("PR comparisons may use an older merge base", async () => {
  assert.deepEqual((await detect(["README.md"], { data: { status: "diverged", merge_base_commit: { sha: "d".repeat(40) } } })).output, flags("docs"));
});
for (const [name, options] of [
  ["file-list limit", { data: { files: Array.from({ length: 300 }, (_, i) => ({ filename: `docs/${i}.md` })) } }],
  ["missing file list", { data: { files: undefined } }],
  ["missing base metadata", { data: { base_commit: undefined } }],
  ["wrong base SHA", { data: { base_commit: { sha: head } } }],
  ["unverified merge base", { data: { merge_base_commit: {} } }],
  ["unexpected comparison status", { data: { status: "behind" } }],
  ["inconsistent identical comparison", { data: { status: "identical" } }],
  ["missing filename", { data: { files: [{}] } }],
  ["missing rename source", { data: { files: [{ filename: "docs/x.md", status: "renamed" }] } }],
  ["invalid rename source", { data: { files: [{ filename: "docs/x.md", previous_filename: 42 }] } }],
  ["API permission denial", { error: new Error("403") }],
  ["unavailable commit", { error: new Error("404") }],
  ["API rate limit", { error: new Error("429") }],
  ["API timeout", { error: new Error("timeout") }],
  ["non-Error rejection", { error: null }],
  ["unsupported event", { context: { eventName: "workflow_dispatch" } }],
  ["missing push base", { context: { eventName: "push", sha: head, payload: {} } }],
  ["initial push", { context: { eventName: "push", sha: head, payload: { before: "0".repeat(40) } } }],
  ["deleted head", { context: { eventName: "push", sha: "0".repeat(40), payload: { before: base } } }],
  ["force push", { context: { eventName: "push", sha: head, payload: { before: base, forced: true } } }],
  ["divergent push", { context: { eventName: "push", sha: head, payload: { before: base } }, data: { status: "diverged", merge_base_commit: { sha: "d".repeat(40) } } }],
]) {
  test(`runs every domain on ${name}`, async () => {
    const result = await detect(["README.md"], options);
    assert.deepEqual(result.output, all);
    assert.equal(result.warnings.length, 1);
  });
}
test("fast-forward pushes use before and after SHAs", async () => {
  const result = await detect(["src/App.svelte"], { context: { eventName: "push", sha: head, payload: { before: base } } });
  assert.deepEqual(result.output, flags("frontend"));
  assert.equal(result.requests[0].params.basehead, `${base}...${head}`);
});
// Failure modes: a task omitted from the selected matrix drops its check; fail-fast drops
// independent checks; a task per command pays one setup and one build cache per command and
// takes a runner slot from the other workflows under GitHub's limit of 20 concurrent jobs.
const tasksByScope = {
  docs: ["docs"],
  frontend: ["frontend-e2e", "gpui-test", "server-backend", "server"],
  rust: ["rust", "gpui-clippy", "gpui-test", "server-backend", "server"],
  nix: [],
};
test("change detection selects every required task without duplicate jobs", async () => {
  for (const selected of [[], ["docs"], ["frontend"], ["rust"], ["nix"], domains]) {
    const paths = selected.map((scope) => ({ docs: "README.md", frontend: "src/App.svelte", rust: "rust/src/lib.rs", nix: "flake.nix" })[scope]);
    const { matrix } = await detect(paths);
    assert.deepEqual(matrix.include.map(({ task }) => task).sort(), [...new Set(selected.flatMap((scope) => tasksByScope[scope]))].sort());
    for (const task of matrix.include) {
      if (task.workspace) assert.ok(["rust", "rust/server", "rust/gpui"].includes(task.workspace));
    }
  }
  // text_fit reads the fixture exported from the browser mocks, so changes to either run it.
  for (const path of ["e2e/parity/visual/export-fixture.ts", "src/lib/tauri-dev-mocks.ts"]) {
    const { matrix } = await detect([path]);
    assert.ok(matrix.include.some(({ task }) => task === "gpui-test"), path);
  }
  // A limit below the task count starts the last tasks only after the first ones finish.
  assert.doesNotMatch(job("checks"), /max-parallel/);
  assert.match(job("checks"), /fail-fast: false/);
  assert.ok(job("checks").includes("matrix: ${{ fromJSON(needs.changes.outputs.matrix) }}"));
  assert.match(job("nix"), /needs:\n {6}- changes\n {6}- checks/);
  assert.match(job("check"), /needs:\n {6}- changes\n {6}- frontend\n {6}- checks\n {6}- nix/);
  assert.deepEqual([...workflow.slice(workflow.indexOf("\njobs:\n")).matchAll(/^ {2}([\w-]+):$/gm)].map((match) => match[1]), ["changes", "frontend", "checks", "nix", "check"]);
});
function step(name, jobName = "checks") {
  const source = job(jobName);
  const start = source.search(new RegExp(`^ {6}- name: ${name}\\n`, "m"));
  assert.notEqual(start, -1, `Missing step: ${name}`);
  const end = source.indexOf("\n      - name: ", start + 1);
  return source.slice(start, end < 0 ? undefined : end);
}
// Recognize only the condition forms these steps support. New syntax needs an
// explicit truth table here rather than execution as JavaScript.
function rustCondition(expression, task, skip, event = "push", successful = true) {
  let selected;
  if (expression === "matrix.workspace" || expression === "always() && matrix.workspace") {
    selected = Boolean(task.workspace);
  } else if (expression === "matrix.workspace && steps.pass-cache.outputs.key != '' && github.event_name == 'pull_request'") {
    selected = Boolean(task.workspace) && event === "pull_request";
  } else if (expression === "success() && matrix.workspace && steps.pass-cache.outputs.key != '' && steps.decide-pass.outputs.skip != 'true' && steps.verify-pass.outcome == 'success'") {
    selected = Boolean(task.workspace) && skip !== "true" && successful;
  } else {
    const single = /^matrix\.task == '([\w-]+)'$/.exec(expression);
    const group = /^\((?:always\(\) && )?(matrix\.workspace|matrix\.task == '[\w-]+'(?: \|\| matrix\.task == '[\w-]+')*)\) && steps\.decide-pass\.outputs\.skip != 'true'$/.exec(expression);
    if (single) selected = task.task === single[1];
    else if (group) {
      const matches = group[1] === "matrix.workspace" ? Boolean(task.workspace) : [...group[1].matchAll(/matrix\.task == '([\w-]+)'/g)].some((match) => match[1] === task.task);
      selected = matches && skip !== "true";
    } else throw new Error(`Unsupported Rust condition: ${expression}`);
  }
  // GitHub supplies an implicit success() guard unless a status function overrides it.
  return Boolean(selected && (expression.includes("always()") || successful));
}
test("Rust pass reuse skips expensive steps but always runs advisory audits and verifies completion", async () => {
  const source = readFileSync(new URL(import.meta.url), "utf8");
  const rustTests = source.slice(source.indexOf('test("Rust pass reuse skips'), source.lastIndexOf('test("Rust task keys cover'));
  assert.doesNotMatch(rustTests, /\bnew\s+Function\s*\(/, "Rust condition checks must not execute expression text");
  const { matrix } = await detect([".github/workflows/ci.yml"]);
  const lookup = step("Restore Rust task pass");
  const save = step("Save Rust task pass");
  const pin = /uses: actions\/cache\/restore@([a-f0-9]{40})/.exec(step("Restore Rust task pass"))[1];
  assert.ok(lookup.includes(`uses: actions/cache/restore@${pin}`));
  assert.ok(save.includes(`uses: actions/cache/save@${pin}`));
  assert.match(lookup, /continue-on-error: true/);
  for (const source of [lookup, save]) {
    assert.ok(source.includes("key: ${{ steps.pass-cache.outputs.key }}"));
    assert.ok(source.includes("path: .ci-rust-pass/record.json"));
    assert.doesNotMatch(source, /lookup-only:/);
  }
  assert.ok(lookup.includes("restore-keys: ${{ steps.pass-cache.outputs.prefix }}\n"));
  assert.doesNotMatch(save, /restore-keys:/);
  for (const name of ["Decide Rust task reuse", "Verify Rust task completion"]) {
    assert.ok(step(name).includes("PASS_CACHE_MATCH: ${{ steps.restore-pass.outputs.cache-matched-key }}\n"), name);
  }
  // Without always(), a failed or cancelled step skips the check that fails the job.
  assert.ok(step("Verify Rust task completion").includes("if: ${{ always() && matrix.workspace }}\n"));
  const condition = (name, task, skip, event = "pull_request", successful = true) => {
    const expression = /if: \$\{\{ (.+) \}\}/.exec(step(name))[1];
    return rustCondition(expression, task, skip === undefined ? "" : String(skip), event, successful);
  };
  const builds = ["Install GPUI build dependencies", "Set up the GPUI CI profile", "Export the GPUI text-fit fixture", "Setup Mr Boxington", "Activate Cargo caching", "Setup Bun", "Install frontend dependencies", "Test Rust", "Lint Rust", "Lint GPUI", "Test GPUI", "Verify API-only server", "Build browser UI", "Verify GUI-free dependency tree", "Test GUI-free backend crates", "Test headless server", "Lint headless server", "Build headless server", "Verify server command wrappers", "Verify HTTP and WebSocket lifecycle", "Upload headless verification report"];
  for (const task of matrix.include.filter(({ workspace }) => workspace)) {
    assert.equal(condition("Restore Rust task pass", task, false), true);
    for (const event of ["push", "workflow_dispatch"]) assert.equal(condition("Restore Rust task pass", task, false, event), false);
    assert.equal(condition("Verify Rust task completion", task, true), true);
    assert.equal(condition("Save Rust task pass", task, true), false);
    assert.equal(condition("Save Rust task pass", task, false), true);
    assert.equal(condition("Save Rust task pass", task, false, "pull_request", false), false);
    for (const name of builds) assert.equal(condition(name, task, true), false, `${task.task}: ${name} on a hit`);
    assert.equal(condition("Setup Mr Boxington", task, false), true);
    for (const name of ["Verify Rust audit coverage", "Install cargo-audit", "Audit desktop Rust dependencies", "Audit GPUI Rust dependencies", "Audit server Rust dependencies"]) {
      assert.equal(condition(name, task, true), task.task === "rust", name);
    }
  }
  // Tasks without a Rust workspace never decide, and their shared setup must still run.
  const shared = { docs: ["Setup Bun"], "frontend-e2e": ["Setup Bun", "Install frontend dependencies"] };
  for (const task of matrix.include.filter(({ workspace }) => !workspace)) {
    assert.ok(shared[task.task], task.task);
    for (const name of shared[task.task]) assert.equal(condition(name, task, undefined), true, `${task.task}: ${name}`);
    assert.equal(condition("Decide Rust task reuse", task, undefined), false);
  }
  assert.ok(job("checks").indexOf("- name: Key the Rust cache by its inputs") < job("checks").indexOf("- name: Restore Rust task pass"));
  assert.ok(job("checks").indexOf("- name: Decide Rust task reuse") < job("checks").indexOf("- name: Install GPUI build dependencies"));
  assert.ok(job("checks").indexOf("- name: Verify Rust task completion") > job("checks").indexOf("- name: Upload headless verification report"));
  assert.ok(step("Verify Rust task completion").includes("PASS_STEPS: ${{ toJSON(steps) }}"));
});
test("every Rust task rejects a missing, failed or cancelled verification step before recording a pass", async () => {
  const { matrix } = await detect([".github/workflows/ci.yml"]);
  const root = mkdtempSync(join(tmpdir(), "kubecove-ci-completion-"));
  const helper = fileURLToPath(new URL("./ci-rust-pass.mjs", import.meta.url));
  const definitions = [...job("checks").matchAll(/^      - name: ([^\n]+)\n([\s\S]*?)(?=^      - name: |$(?![\s\S]))/gm)]
    .map(([, name, source]) => ({ name, source, id: /^ {8}id: (.+)$/m.exec(source)?.[1] })).filter(({ id }) => id && id !== "verify-pass");
  try {
    for (const task of matrix.include.filter(({ workspace }) => workspace)) {
      const steps = {};
      for (const { source, id } of definitions) {
        const expression = /^ {8}if: \$\{\{ (.+) \}\}$/m.exec(source)[1];
        const selected = rustCondition(expression, task, "false");
        steps[id] = { outcome: selected ? "success" : "skipped", conclusion: selected ? "success" : "skipped" };
        if (id === "decide-pass") steps[id].outputs = { skip: "false" };
      }
      const run = (outcomes) => spawnSync(process.execPath, [helper, "verify"], { cwd: root, encoding: "utf8", env: {
        ...process.env, PASS_TASK: task.task, PASS_KEY: `mbx-ci-${task.task}-pass-${"a".repeat(64)}-${"b".repeat(64)}`,
        PASS_EVENT: "push", PASS_CACHE_MATCH: "", PASS_SKIP: "false", PASS_STEPS: JSON.stringify(outcomes),
        GITHUB_SERVER_URL: "https://github.com", GITHUB_REPOSITORY: "Timpan4/kubecove", GITHUB_RUN_ID: "123", GITHUB_RUN_ATTEMPT: "1",
      } });
      const successful = run(steps);
      assert.equal(successful.status, 0, `${task.task}: ${successful.stderr}`);
      for (const [id, { outcome }] of Object.entries(steps)) {
        if (outcome !== "success") continue;
        for (const result of ["failure", "cancelled", "skipped"]) {
          assert.notEqual(run({ ...steps, [id]: { outcome: result, conclusion: result } }).status, 0, `${task.task}: ${id} ${result}`);
        }
        const rest = Object.fromEntries(Object.entries(steps).filter(([other]) => other !== id));
        assert.notEqual(run(rest).status, 0, `${task.task}: ${id} missing`);
      }
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});
test("Rust task keys cover the audited inputs outside each workspace", async () => {
  const { matrix } = await detect([".github/workflows/ci.yml"]);
  const shared = ["tests/fixtures/kubernetes/pod-running.json", "package.json", "bun.lock", "scripts/ci-rust-pass.mjs", "rust/Cargo.toml", "rust/Cargo.lock"];
  const expected = {
    rust: shared,
    "gpui-clippy": [...shared, "rust/gpui/Cargo.lock", "src/lib/settings-integration-workspace.json"],
    // text_fit reads the fonts under rust/gpui and the fixture exported from the browser mocks.
    "gpui-test": [...shared, "rust/gpui/Cargo.lock", "src/lib/settings-integration-workspace.json", "rust/gpui/assets/fonts/inter-400.ttf", "e2e/parity/visual/export-fixture.ts", "e2e/fixtures/exec-credential-plugin.mjs", "src/lib/tauri-dev-mocks.ts", "src/features/resources/constants.ts"],
    "server-backend": [...shared, "rust/server/Cargo.toml", "rust/server/Cargo.lock"],
    server: [...shared, "rust/server/Cargo.lock", "src/main-svelte.ts", "public/kubecove-logo.svg", "vite.config.ts", "tsconfig.json", "index.html", "scripts/tsrs.ts", "scripts/dev-interactions-plugin.ts", "scripts/frontend-bundle-report.ts", "e2e/headless-smoke.ts", "e2e/harness/server-browser.ts", "e2e/harness/server-process.ts", "e2e/harness/server-session.ts", ".agents/skills/verify-kubecove/scripts/processes.ts", "tests/server-command-contract.test.ts", "e2e/harness/port-forward-fixture.ts", "deploy/grafana/kubecove-server.json"],
  };
  for (const task of matrix.include.filter(({ workspace }) => workspace)) {
    // Exercise the exact shell pathspec arguments supplied to the key writer.
    const result = spawnSync("bash", ["-c", `git ls-files -z -- ${task.passInputs}`], { cwd: new URL("../", import.meta.url), encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    const files = new Set(result.stdout.split("\0"));
    for (const path of expected[task.task]) assert.ok(files.has(path), `${task.task} misses ${path}`);
  }
  // The repository has no root Cargo configuration or toolchain file today, so a
  // scratch repository shows that the pass inputs would cover one.
  const root = mkdtempSync(join(tmpdir(), "kubecove-ci-pass-inputs-"));
  try {
    const future = [".cargo/config.toml", ".config/nextest.toml", "rust-toolchain.toml", "rust-toolchain"];
    for (const path of future) {
      mkdirSync(dirname(join(root, path)), { recursive: true });
      writeFileSync(join(root, path), "");
    }
    assert.equal(spawnSync("git", ["init", "-q"], { cwd: root }).status, 0);
    assert.equal(spawnSync("git", ["add", "."], { cwd: root }).status, 0);
    for (const task of matrix.include.filter(({ workspace }) => workspace)) {
      const result = spawnSync("bash", ["-c", `git ls-files -z -- ${task.passInputs}`], { cwd: root, encoding: "utf8" });
      assert.equal(result.status, 0, result.stderr);
      assert.deepEqual(result.stdout.split("\0").filter(Boolean).sort(), [...future].sort(), task.task);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
  assert.equal((await detect(["tests/fixtures/kubernetes/pod-running.json"])).output.rust, "true");
  assert.equal((await detect(["src/lib/settings-integration-workspace.json"])).output.rust, "true");
});
// Result inputs must not refresh compiler bundles, unrelated workflows must not
// invalidate passes, and images must not compete for an immutable pass entry.
test("compiler bundles keep the original inputs while pass keys use the full audit", async () => {
  const { matrix } = await detect([".github/workflows/ci.yml"]);
  const desktop = "rust :!rust/gpui :!rust/server";
  const gpui = "rust :!rust/server src/lib/*-workspace.json .github/actions/setup-linux-native-deps .github/actions/setup-gpui-ci-profile";
  const server = "rust :!rust/gpui";
  assert.deepEqual(Object.fromEntries(matrix.include.filter(({ workspace }) => workspace).map(({ task, inputs }) => [task, inputs])), {
    rust: desktop, "gpui-clippy": gpui, "gpui-test": gpui, "server-backend": desktop, server,
  });
  assert.ok(step("Key the Rust cache by its inputs").includes("ci-${{ matrix.task }} .github/workflows/ci.yml ${{ matrix.inputs }}"));
  assert.ok(step("Key the Rust pass by its inputs").includes("ci-${{ matrix.task }}-pass .github/workflows/ci.yml ${{ matrix.passInputs }}"));
  for (const task of matrix.include.filter(({ workspace }) => workspace)) {
    assert.ok(task.passInputs && task.passInputs !== task.inputs, task.task);
  }
});
test("pass inputs retain CI and local actions without unrelated workflows", async () => {
  const { matrix } = await detect([".github/workflows/ci.yml"]);
  const uses = [...workflow.matchAll(/uses: (\.\/\.github\/\S+)/g)].map(([, path]) => path.slice(2));
  for (const task of matrix.include.filter(({ workspace }) => workspace)) {
    const taskActions = uses.filter((path) => ![".github/actions/setup-linux-native-deps", ".github/actions/setup-gpui-ci-profile"].includes(path) || task.task.startsWith("gpui-"));
    const result = spawnSync("bash", ["-c", `git ls-files -z -- .github/workflows/ci.yml ${task.passInputs ?? task.inputs}`], { cwd: new URL("../", import.meta.url), encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    const githubFiles = result.stdout.split("\0").filter((path) => path.startsWith(".github/"));
    assert.ok(githubFiles.includes(".github/workflows/ci.yml"), task.task);
    for (const path of githubFiles) {
      assert.ok(path === ".github/workflows/ci.yml" || taskActions.some((used) => path === used || path.startsWith(`${used}/`)), `${task.task} includes unrelated ${path}`);
    }
    for (const path of taskActions) assert.ok(githubFiles.some((file) => file === path || file.startsWith(`${path}/`)), `${task.task} misses ${path}`);
  }
});
test("pass cache keys partition both hosted images and never match compiler-cache cleanup", () => {
  const root = mkdtempSync(join(tmpdir(), "kubecove-ci-pass-image-"));
  const output = join(root, "output");
  const env = {
    ...process.env, PASS_TASK: "gpui-test", PASS_KEY: `mbx-ci-gpui-test-pass-${"a".repeat(64)}-${"b".repeat(64)}`, RUNNER_OS: "Linux", RUNNER_ARCH: "X64", ImageOS: "ubuntu24", ImageVersion: "20261028.624", GITHUB_OUTPUT: output,
    GITHUB_SERVER_URL: "https://github.com", GITHUB_REPOSITORY: "Timpan4/kubecove", GITHUB_RUN_ID: "123", GITHUB_RUN_ATTEMPT: "1",
  };
  const key = (overrides = {}) => {
    writeFileSync(output, "");
    const result = spawnSync(process.execPath, [fileURLToPath(new URL("./ci-rust-pass.mjs", import.meta.url)), "cache-key"], { cwd: root, encoding: "utf8", env: { ...env, ...overrides } });
    assert.equal(result.status, 0, result.stderr);
    const [saved, prefix] = readFileSync(output, "utf8").trim().split("\n");
    assert.equal(prefix, `prefix=${saved.slice(4).replace(/\d+-\d+$/, "")}`);
    return saved.replace(/^key=/, "");
  };
  try {
    const first = key();
    const second = key({ ImageVersion: "20261103.629" });
    assert.ok(first.startsWith(`passed-${env.PASS_KEY}-`));
    assert.notEqual(first, second);
    assert.notEqual(first, key({ ImageOS: "another-image-os" }));
    assert.equal(first, key());
    assert.equal(key({ ImageVersion: "" }), "", "missing identity cannot seed an image-independent pass");
    assert.ok(first.endsWith("-123-1"));
    for (const overrides of [{ GITHUB_RUN_ID: "" }, { GITHUB_RUN_ATTEMPT: "" }, { GITHUB_RUN_ID: "1/2" }]) assert.equal(key(overrides), "", "a pass needs the run that saved it");
    for (const saved of [first, second]) assert.equal(cacheName(saved), undefined);
    assert.deepEqual(supersededCaches([first, second].map((saved, id) => ({ id, key: saved, ref: "refs/heads/main", created_at: `2026-11-13T10:00:0${id}Z` }))), []);
    for (const source of [step("Restore Rust task pass"), step("Save Rust task pass")]) {
      assert.ok(source.includes("key: ${{ steps.pass-cache.outputs.key }}"));
      assert.doesNotMatch(source, /steps\.rust-cache\.outputs/);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});
test("each existing check remains in its selected task", () => {
  const checks = job("checks");
  for (const [name, command] of [
    ["Verify server command wrappers", "bun test tests/server-command-contract.test.ts"],
    ["Verify HTTP and WebSocket lifecycle", "bun e2e/headless-smoke.ts --browser"],
  ]) {
    const background = step(name);
    assert.match(background, /background: true/);
    assert.ok(background.includes(`run: ${command}\n`));
    assert.ok(checks.indexOf(step("Build headless server")) < checks.indexOf(background));
    assert.ok(checks.indexOf(background) < checks.indexOf(step("Test headless server")));
  }
  const backgroundIds = checks.split(/\n {6}- /).map((block) => `\n${" ".repeat(8)}${block}\n`).filter((block) => /\n {8}background: true\n/.test(block)).map((block) => block.match(/\n {8}id: ([\w-]+)\n/)?.[1]);
  const helper = readFileSync(fileURLToPath(new URL("./ci-rust-pass.mjs", import.meta.url)), "utf8");
  assert.deepEqual(JSON.parse(helper.match(/const backgroundSteps = new Set\((\[[^\]]*\])\)/)[1]).sort(), backgroundIds.sort(), "the pass helper must know every background step");
  assert.ok(checks.indexOf(step("Lint headless server")) < checks.indexOf("      - wait-all:"));
  assert.ok(checks.indexOf("      - wait-all:") < checks.indexOf(step("Upload headless verification report")));
  const commands = {
    "frontend-e2e": ["bun run e2e:fast"],
    rust: ["node scripts/check-rust-format.mjs", "mbx test --workspace --manifest-path rust/Cargo.toml --features desktop-release", "mbx clippy --workspace --manifest-path rust/Cargo.toml --all-targets --all-features -- -D warnings"],
    "gpui-clippy": ["mbx clippy --manifest-path rust/gpui/Cargo.toml --locked --all-targets -- -D warnings"],
    "gpui-test": [
      "bun e2e/parity/visual/export-fixture.ts e2e/artifacts/text-fit/fixture.json",
      "if grep -F 'text_fit: skipped' e2e/artifacts/text-fit/report.txt; then exit 1; fi",
    ],
    "server-backend": ["cargo tree --manifest-path rust/server/Cargo.toml --locked --edges normal,build --prefix none > server-dependencies.txt", "mbx test --locked --workspace --no-default-features --manifest-path rust/Cargo.toml"],
    server: [
      "mbx test --manifest-path rust/server/Cargo.toml --locked", "mbx clippy --manifest-path rust/server/Cargo.toml --all-targets --locked -- -D warnings",
      "mbx test --manifest-path rust/server/Cargo.toml --locked --no-default-features", "mbx clippy --manifest-path rust/server/Cargo.toml --locked --no-default-features --all-targets -- -D warnings",
      "bun run build", "mbx build --manifest-path rust/server/Cargo.toml --locked", "bun test tests/server-command-contract.test.ts", "bun e2e/headless-smoke.ts --browser",
    ],
  };
  for (const [task, expected] of Object.entries(commands)) {
    const steps = checks.split(/(?=^      - name: )/m);
    for (const command of expected) {
      const found = steps.find((source) => source.split("\n").some((line) => line.trim() === `run: ${command}` || line.trim() === command));
      assert.ok(found, `Missing command: ${command}`);
      assert.equal(checks.split("\n").filter((line) => line.trim() === `run: ${command}` || line.trim() === command).length, 1, `Duplicate command: ${command}`);
      assert.ok(found.includes(`matrix.task == '${task}'`), `Wrong task for ${command}`);
    }
  }
  // A missing task condition runs frontend commands in Rust-only jobs without Bun.
  for (const match of job("checks").matchAll(/^ {6,10}- name: ([^\n]+)\n([\s\S]*?)(?=^ {6,10}- name:|$(?![\s\S]))/gm)) {
    if (match[1] !== "Checkout") assert.match(match[2], /^ +if: \$\{\{ .+ \}\}$/m, match[1]);
  }
  assert.ok(step("Build browser UI").includes("KUBECOVE_PUBLIC_RUNTIME: web"));
  assert.ok(step("Build browser UI").includes("matrix.task == 'server'"));
  assert.ok(job("checks").indexOf("- name: Build browser UI") < job("checks").indexOf("- name: Test headless server"));
  // The API-only server must build in a checkout that has no built browser UI.
  assert.ok(job("checks").indexOf("- name: Verify API-only server") < job("checks").indexOf("- name: Build browser UI"));
  const report = step("Upload headless verification report");
  assert.ok(report.includes("always() && matrix.task == 'server'"));
  assert.match(report, /if-no-files-found: error/);
  assert.match(report, /path: e2e\/artifacts\/headless\/\*\*/);
  assert.match(step("Verify GUI-free dependency tree"), /if grep -E '\^\(tauri /);
});
test("lightweight tasks avoid Rust and package setup; CI regressions precede installs", () => {
  assert.doesNotMatch(workflow, /\.bun\/install\/cache/);
  assert.doesNotMatch(job("changes"), /actions\/checkout|git diff|fetch-depth/);
  assert.match(job("changes"), /actions\/github-script@[a-f0-9]{40}/);
  assert.ok(step("Install frontend dependencies").includes("matrix.task == 'frontend-e2e' || matrix.task == 'server'"));
  const frontend = job("frontend");
  assert.ok(frontend.indexOf(step("Test CI orchestration", "frontend")) < frontend.indexOf(step("Install frontend dependencies", "frontend")));
  assert.ok(step("Setup Rust").includes("matrix.workspace"));
  assert.ok(step("Check documentation").includes("matrix.task == 'docs'"));
  assert.match(step("Install GPUI build dependencies"), /uses: \.\/\.github\/actions\/setup-linux-native-deps/);
  assert.ok(step("Install GPUI build dependencies").includes("matrix.task == 'gpui-clippy' || matrix.task == 'gpui-test'"));
});
test("frontend CI and the local check use the same verification commands", () => {
  const { scripts } = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
  const frontend = job("frontend");
  for (const command of ["bun run test:ci", "bun run check:frontend"]) {
    assert.match(frontend, new RegExp(`^ +run: ${command}$`, "m"));
    assert.ok(scripts.check.split(" && ").includes(command), `The local check must run ${command}`);
  }
  const parallel = frontend.indexOf("\n      - parallel:\n");
  assert.ok(parallel > frontend.indexOf(step("Install frontend dependencies", "frontend")));
  const independent = frontend.slice(parallel);
  assert.match(independent, /^ {12}run: bun run check:frontend$/m);
  assert.match(independent, /^ {12}run: bun run audit$/m);
  assert.doesNotMatch(independent, /^ +(if|continue-on-error):/m);

  const [preflight, ...compilation] = scripts["check:frontend"].split(" && ");
  const tasks = preflight.split(" ");
  assert.deepEqual(tasks.slice(0, 3), ["bun", "run", "--parallel"]);
  for (const check of ["docs:check", "lint", "lint:anti-slop", "typecheck", "typecheck:bench", "svelte:check", "test:release", "test:bench-groups"]) {
    assert.ok(tasks.includes(check), `The shared frontend check must run ${check}`);
  }
  assert.deepEqual(compilation, ["bun test", "bun run test:svelte", "bun run build:bundle"]);
});
test("native dependencies are installed only if GPUI cannot link or Playwright cannot launch Chromium", () => {
  const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8").replaceAll("\r\n", "\n");
  const native = block(read("../.github/actions/setup-linux-native-deps/action.yml"), "run", 6);
  const suite = read("../.github/workflows/parity-suite.yml");
  const chromium = block(suite.slice(suite.indexOf("- name: Install Chromium")), "run", 8);
  const run = (probe, commands, phase) => spawnSync("bash", ["-e", "-o", "pipefail", "-c", commands + probe], {
    env: { ...process.env, PHASE: phase }, encoding: "utf8",
  });
  for (const phase of ["ready", "missing", "broken"]) {
    const gpui = run(native, [
      'pkg-config() { printf "%s\n" "-lfontconfig -lxcb -lxkbcommon -lxkbcommon-x11"; }',
      'cc() { cat >/dev/null; echo link; [[ "$PHASE" == ready || ( "$PHASE" == missing && "$installed" == true ) ]]; }',
      'sudo() { echo "$*"; installed=true; }',
      'installed=false',
      '',
    ].join("\n"), phase);
    const web = run(chromium, [
      'bunx() { echo "$*"; if [[ "$2" == install-deps ]]; then installed=true; fi; }',
      'node() { echo launch; [[ "$PHASE" == ready || ( "$PHASE" == missing && "$installed" == true ) ]]; }',
      'installed=false',
      '',
    ].join("\n"), phase);
    assert.equal(gpui.status, phase === "broken" ? 1 : 0, gpui.stderr);
    assert.equal(web.status, phase === "broken" ? 1 : 0, web.stderr);
    assert.equal(gpui.stdout.trim(), phase === "ready" ? "link" : [
      "link", "apt-get update", "apt-get install -y libfontconfig-dev libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev pkg-config", "link",
    ].join("\n"));
    assert.equal(web.stdout.trim(), [
      "playwright-core install --no-shell chromium", "launch",
      ...(phase === "ready" ? [] : ["playwright-core install-deps chromium", "launch"]),
    ].join("\n"));
  }
});
test("GPUI tests keep all three phases in one mbx export and reject a skipped text-fit test", () => {
  const gpui = step("Test GPUI");
  const outputDir = "e2e/artifacts/text-fit";
  const root = mkdtempSync(join(tmpdir(), "kubecove-ci-gpui-"));
  const summary = join(root, "summary");
  const mock = [
    'mbx() {',
    '  printf "mbx %s, export group %s, fixture %s\n" "$*" "$MBX_CACHE_EXPORT_GROUP" "${KUBECOVE_PARITY_FIXTURE:-none}";',
    '  echo "mbx[cache]: object cache: 2 hits, 1 misses";',
    '  if [[ "$*" == *text_fit && "$MODE" == skipped ]]; then echo "text_fit: skipped"; fi;',
    '  if [[ "$*" == *text_fit && "$MODE" == failed ]]; then return 7; fi;',
    '}',
    '',
  ].join("\n");
  try {
    mkdirSync(join(root, outputDir), { recursive: true });
    assert.equal((job("checks").match(/uses: jdx\/mr-boxington-action@/g) ?? []).length, 1);
    assert.match(gpui, /CARGO_TARGET_DIR: .+\/rust\/gpui\/target/);
    assert.match(gpui, /CARGO_BUILD_BUILD_DIR: .+\/rust\/gpui\/target/);
    for (const mode of ["ready", "skipped", "failed"]) {
      writeFileSync(summary, "");
      const result = spawnSync("bash", ["-e", "-o", "pipefail", "-c", mock + block(gpui, "run", 8)], {
        cwd: root, encoding: "utf8",
        env: { ...process.env, GITHUB_STEP_SUMMARY: summary, RUNNER_TEMP: root, MBX_CACHE_EXPORT_GROUP: "shared", MODE: mode },
      });
      assert.equal(result.status, { ready: 0, skipped: 1, failed: 7 }[mode], result.stderr);
      const common = "mbx test --manifest-path rust/gpui/Cargo.toml --locked";
      assert.ok(result.stdout.includes(`${common} --lib --test native, export group shared, fixture none`));
      assert.match(result.stdout, /--test text_fit, export group shared, fixture .+\/e2e\/artifacts\/text-fit\/fixture\.json/);
      assert.equal(result.stdout.includes(`${common} --features e2e --test native -- automation:: e2e_interlock::, export group shared, fixture none`), mode === "ready");
      const report = readFileSync(summary, "utf8");
      assert.equal((report.match(/2 hits, 1 misses/g) ?? []).length, mode === "ready" ? 3 : 2);
      assert.match(report, /\*\*Text fit\*\* \(\d+s, exit [07]\)/);
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});

test("CI installs the per-package GPUI profile in the checkout without replacing Cargo settings", () => {
  const actionPath = new URL("../.github/actions/setup-gpui-ci-profile/", import.meta.url);
  const action = readFileSync(new URL("action.yml", actionPath), "utf8").replaceAll("\r\n", "\n");
  const config = readFileSync(new URL("config.toml", actionPath), "utf8").replaceAll("\r\n", "\n");
  const root = mkdtempSync(join(tmpdir(), "kubecove-ci-gpui-profile-"));
  const run = () => spawnSync("bash", ["-e", "-c", block(action, "run", 6)], {
    cwd: root, env: { ...process.env, GITHUB_ACTION_PATH: fileURLToPath(actionPath) }, encoding: "utf8",
  });
  try {
    assert.match(step("Set up the GPUI CI profile"), /matrix.task == 'gpui-clippy' \|\| matrix.task == 'gpui-test'/);
    assert.match(config, /^\[profile\.dev\.package\.kubecove\]\ndebug = "line-tables-only"\n$/);
    const result = run();
    assert.equal(result.status, 0, result.stderr);
    const path = join(root, ".cargo", "config.toml");
    assert.equal(readFileSync(path, "utf8"), config);
    writeFileSync(path, "[build]\n");
    assert.notEqual(run().status, 0);
    assert.equal(readFileSync(path, "utf8"), "[build]\n");
  } finally { rmSync(root, { recursive: true, force: true }); }
});
function gate(overrides = {}) {
  const env = { ...process.env, CHANGES_RESULT: "success", CHECKS_RESULT: "success", FRONTEND_RESULT: "success" };
  for (const key of domains) { env[`${key.toUpperCase()}_REQUIRED`] = "true"; }
  env.NIX_RESULT = "success";
  return spawnSync("bash", ["-e", "-o", "pipefail", "-c", block(job("check"), "run", 8)], {
    env: { ...env, ...overrides }, encoding: "utf8", timeout: 2000,
  }).status;
}
test("stable required check rejects any failed, cancelled, skipped or unknown matrix result", () => {
  assert.match(job("check"), /name: Typecheck and test/);
  assert.match(job("check"), /if: always\(\)/);
  assert.equal(gate(), 0);
  for (const name of ["CHANGES_RESULT", "FRONTEND_RESULT", "CHECKS_RESULT", "NIX_RESULT"]) {
    for (const result of ["failure", "cancelled", "skipped", ""]) assert.notEqual(gate({ [name]: result }), 0);
  }
  for (const name of ["DOCS_REQUIRED", "FRONTEND_REQUIRED", "RUST_REQUIRED", "NIX_REQUIRED"]) {
    assert.notEqual(gate({ [name]: "" }), 0);
  }
  const skipped = { DOCS_REQUIRED: "false", FRONTEND_REQUIRED: "false", RUST_REQUIRED: "false", NIX_REQUIRED: "false", CHECKS_RESULT: "skipped", FRONTEND_RESULT: "skipped", NIX_RESULT: "skipped" };
  assert.equal(gate(skipped), 0);
  assert.notEqual(gate({ ...skipped, CHECKS_RESULT: "success" }), 0);
  for (const name of ["DOCS_REQUIRED", "FRONTEND_REQUIRED", "RUST_REQUIRED"]) {
    assert.equal(gate({ ...skipped, [name]: "true", CHECKS_RESULT: "success", FRONTEND_RESULT: name === "FRONTEND_REQUIRED" ? "success" : "skipped" }), 0);
    assert.notEqual(gate({ ...skipped, [name]: "true" }), 0);
  }
});
test("release PRs use normal CI selection and skip Nix like other PRs", () => {
  assert.ok(job("nix").includes("github.event_name != 'pull_request' && needs.changes.outputs.nix == 'true'"));
  assert.ok(job("check").includes("NIX_REQUIRED: ${{ github.event_name != 'pull_request' && needs.changes.outputs.nix == 'true' }}"));
  const release = { RELEASE_PR: "true", NIX_REQUIRED: "false", NIX_RESULT: "skipped" };
  assert.equal(gate(release), 0);
  assert.notEqual(gate({ ...release, FRONTEND_RESULT: "skipped" }), 0);
  assert.notEqual(gate({ ...release, CHECKS_RESULT: "skipped" }), 0);
});
test("Rust verification selects all workspace members despite root-only default-members", () => {
  const { scripts } = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
  const hook = readFileSync(new URL("../.githooks/pre-commit-user", import.meta.url), "utf8");
  const commands = [
    ...["rust:test", "rust:check", "rust:lint"].map((name) => scripts[name]),
    ...[...job("checks").matchAll(/run: ((?:cargo|mbx) (?:test|clippy) [^\n]*--manifest-path rust\/Cargo.toml[^\n]*)/g)].map((match) => match[1]),
    ...[...hook.matchAll(/^\s*(cargo check [^\n]*--manifest-path rust\/Cargo.toml[^\n]*)/gm)].map((match) => match[1]),
  ];
  // Three package scripts, three CI checks (including the API-only server lane), and the hook.
  assert.equal(commands.length, 7);
  for (const command of commands) {
    const args = command.trim().split(/\s+/);
    const cargoArgs = args.slice(0, args.indexOf("--") === -1 ? args.length : args.indexOf("--"));
    assert.ok(cargoArgs.includes("--workspace"), command);
    assert.equal(cargoArgs[cargoArgs.indexOf("--manifest-path") + 1], "rust/Cargo.toml", command);
  }
});
test("Rust dependency audits cover every checked-in lockfile with narrow exceptions", () => {
  const trackedFiles = spawnSync("git", ["ls-files", "-z"], { cwd: new URL("../", import.meta.url), encoding: "utf8" });
  assert.equal(trackedFiles.status, 0, trackedFiles.stderr);
  const lockfileDirectories = trackedFiles.stdout.split("\0").filter((path) => path === "Cargo.lock" || path.endsWith("/Cargo.lock")).map(dirname).sort();
  assert.ok(step("Verify Rust audit coverage").includes('node --test --test-name-pattern="Rust dependency audits cover every checked-in lockfile with narrow exceptions" scripts/ci-workflow-regression.test.mjs'));
  assert.doesNotMatch(job("checks"), /uses: rustsec\/audit-check@/);
  assert.match(step("Install cargo-audit"), /uses: taiki-e\/install-action@[a-f0-9]{40}/);
  assert.match(step("Install cargo-audit"), /tool: cargo-audit@0\.22\.2/);
  const audits = job("checks").split("\n      - name: ").filter((source) => source.includes("run: cargo-audit audit ")).map((source) => {
    assert.ok(source.includes("matrix.task == 'rust'"));
    const command = /run: cargo-audit audit (--file \S+(?: --ignore \S+)*)\n/.exec(`${source}\n`)?.[1];
    assert.ok(command, source);
    return [dirname(/--file (\S+)/.exec(command)[1]), [...command.matchAll(/--ignore (\S+)/g)].map((match) => match[1])];
  }).sort(([left], [right]) => left.localeCompare(right));
  assert.deepEqual(audits.map(([directory]) => directory), lockfileDirectories);
  assert.deepEqual(audits, [["rust", []], ["rust/gpui", []], ["rust/server", ["RUSTSEC-2023-0071", "RUSTSEC-2026-0387"]]]);
  assert.match(step("Audit server Rust dependencies"), /public-signature verification/);
});
test("server packaging follows the relocated kubeconform source without changing its standalone build", () => {
  const packaging = readFileSync(new URL("../.github/workflows/server-packaging.yml", import.meta.url), "utf8");
  assert.match(packaging, /^      - rust\/crates\/inspection\/src\/commands\/resources\/kubeconform\.rs\r?$/m);
  assert.doesNotMatch(packaging, /rust\/src\/commands\/resources\/kubeconform\.rs/);
  assert.match(packaging, /cargo build --locked --release --manifest-path rust\/server\/Cargo.toml --target "\$TARGET"/);
  assert.doesNotMatch(packaging, /--workspace|--all-features/);
  const dockerIgnore = readFileSync(new URL("../.dockerignore", import.meta.url), "utf8");
  assert.match(dockerIgnore, /^!rust\/crates\/\r?$/m, "Docker must reopen the crate directory");
  assert.match(dockerIgnore, /^!rust\/crates\/\*\*\r?$/m, "Docker must include crate sources and manifests");
  assert.match(dockerIgnore, /^\*\*\/target\r?$/m, "Rust build caches must stay out of the context");
});
// Failure mode: vite.config.ts imports a scripts/ module that the container context or the Nix
// frontend fileset excludes, so that build cannot load its config. PR CI skips the container build.
test("container and Nix build sources include every scripts module vite.config.ts imports", () => {
  const viteConfig = readFileSync(new URL("../vite.config.ts", import.meta.url), "utf8");
  const dockerIgnore = readFileSync(new URL("../.dockerignore", import.meta.url), "utf8");
  const nixFrontend = readFileSync(new URL("../nix/frontend.nix", import.meta.url), "utf8");
  const imported = [...viteConfig.matchAll(/from "\.\/(scripts\/[^"]+)"/g)].map((match) => match[1]);
  assert.ok(imported.length > 0, "vite.config.ts should import its build plugins from scripts/");
  for (const path of imported) {
    const escaped = path.replaceAll(".", "\\.");
    assert.match(dockerIgnore, new RegExp(`^!${escaped}\\r?$`, "m"), `.dockerignore must reopen ${path}`);
    assert.match(nixFrontend, new RegExp(`^\\s+\\.\\./${escaped}\\r?$`, "m"), `nix/frontend.nix must include ${path}`);
  }
});
// Cargo 1.100 unit state sits inside build/, which the action's target scanner skips.
// Objects transport must export each phase independently and allow same-repository PR warming.
test("Mr Boxington uses isolated object bundles for all Rust workspaces", async () => {
  const setup = step("Setup Mr Boxington");
  assert.match(setup, /uses: jdx\/mr-boxington-action@[a-f0-9]{40}/);
  assert.match(setup, /version: "1\.30\.0"/);
  assert.ok(setup.includes("working-directory: ${{ matrix.workspace }}"));
  assert.match(setup, /github-cache-mode: objects/);
  assert.match(setup, /isolate-objects-cache: true/);
  assert.match(setup, /clean-target-before-export: true/);
  // Pull requests restore main's cache and never save: one saving push stored about 7 GB
  // of the 10 GB quota, which evicts the caches every other pull request restores.
  for (const file of ["ci.yml", "parity-suite.yml", "codspeed.yml"]) {
    assert.doesNotMatch(readFileSync(new URL(`../.github/workflows/${file}`, import.meta.url), "utf8"), /save-on-pull-request/, file);
  }
  // Each task keys its own cache by its inputs; the action's generated key is unique per
  // run, so every run would export and upload again.
  assert.ok(setup.includes("cache-key: ${{ steps.rust-cache.outputs.key }}"));
  assert.ok(setup.includes("restore-keys: ${{ steps.rust-cache.outputs.prefix }}"));
  assert.doesNotMatch(setup, /cache-generation/);
  assert.ok(step("Key the Rust cache by its inputs").includes("run: node scripts/ci-rust-cache-key.mjs ci-${{ matrix.task }} .github/workflows/ci.yml ${{ matrix.inputs }}"));
  assert.ok(job("checks").indexOf("- name: Key the Rust cache by its inputs") < job("checks").indexOf("- name: Setup Mr Boxington"));
  assert.ok(job("checks").indexOf("- name: Setup Rust") < job("checks").indexOf("- name: Key the Rust cache by its inputs"));
  assert.match(job("checks"), /^ {6}CARGO_PROFILE_DEV_DEBUG: line-tables-only$/m);
  const { matrix } = await detect([".github/workflows/ci.yml"]);
  const workspaces = Object.fromEntries(matrix.include.filter(({ workspace }) => workspace).map(({ task, workspace, inputs }) => [task, [workspace, inputs]]));
  // The GPUI and server crates are separate workspaces that both build the desktop crates.
  assert.deepEqual(Object.fromEntries(Object.entries(workspaces).map(([task, [workspace]]) => [task, workspace])), {
    rust: "rust", "gpui-clippy": "rust/gpui", "gpui-test": "rust/gpui",
    "server-backend": "rust", server: "rust/server",
  });
  assert.equal((job("checks").match(/uses: jdx\/mr-boxington-action@/g) ?? []).length, 1);
  assert.doesNotMatch(job("checks"), /swatinem\/rust-cache|save-on-(?:protected-branch|workflow-dispatch)/i);
  assert.doesNotMatch(job("checks"), /(?:run: |^\s+)cargo (?:build|test|clippy)\b/m);
  assert.ok(job("checks").indexOf("dtolnay/rust-toolchain@") < job("checks").indexOf("jdx/mr-boxington-action@"));
  assert.match(step("Activate Cargo caching"), /node scripts\/ci-mbx-cargo\.mjs/);
});
// Failure modes: an input change that keeps the key leaves the new compiler output unsaved
// for good; a change outside the inputs that moves the key uploads the same cache again,
// and the runner image is such a change: two images run side by side during a rollout, so
// a key that follows the image stores every cache twice and overruns the cache quota; a
// new compiler that keeps the key never refreshes the cache; a compiler change that keeps
// the prefix downloads a cache the build cannot use; a mistyped pathspec keys the cache on
// nothing.
test("the Rust cache key follows the build inputs and the compiler, not the runner image", () => {
  const root = mkdtempSync(join(tmpdir(), "kubecove-ci-cache-key-"));
  const { GITHUB_OUTPUT: _, ...inherited } = process.env;
  const env = { ...inherited, GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: "/dev/null", PATH: `${join(root, "bin")}${delimiter}${process.env.PATH}`, RUSTC_VERSION: "1", ImageVersion: "1" };
  const write = (path, text) => {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    writeFileSync(join(root, path), text);
    assert.equal(spawnSync("git", ["add", "--", path], { cwd: root, env }).status, 0);
  };
  const run = (overrides, pathspecs = ["workflow.yml", "rust", ":!rust/gpui"]) => spawnSync(
    process.execPath, [fileURLToPath(new URL("./ci-rust-cache-key.mjs", import.meta.url)), "ci-rust", ...pathspecs],
    { cwd: root, env: { ...env, ...overrides }, encoding: "utf8" },
  );
  const key = (overrides = {}) => {
    const result = run(overrides);
    assert.equal(result.status, 0, result.stderr);
    const output = Object.fromEntries(result.stdout.trim().split("\n").map((line) => line.split("=")));
    assert.ok(output.prefix.startsWith("mbx-ci-rust-") && output.key.startsWith(output.prefix) && output.key.length > output.prefix.length);
    return output;
  };
  try {
    assert.equal(spawnSync("git", ["init", "-q"], { cwd: root, env }).status, 0);
    // The script asks the compiler on PATH for its identity.
    mkdirSync(join(root, "bin"));
    writeFileSync(join(root, "bin", "rustc"), "#!/bin/sh\necho \"rustc $RUSTC_VERSION\"\n");
    chmodSync(join(root, "bin", "rustc"), 0o755);
    write("workflow.yml", "one\n"); write("rust/src/lib.rs", "one\n"); write("rust/gpui/src/main.rs", "one\n"); write("README.md", "one\n");
    // The real checkout always tracks the two scripts the key includes, so the fixture does too.
    write("scripts/ci-rust-cache-key.mjs", "one\n"); write("scripts/ci-mbx-cargo.mjs", "one\n");
    const first = key();
    assert.deepEqual(key(), first);
    // The cleanup finds the caches it may delete by this key shape.
    assert.equal(cacheName(first.key), "ci-rust");
    write("rust/gpui/src/main.rs", "two\n"); write("README.md", "two\n");
    assert.deepEqual(key(), first);
    write("rust/src/lib.rs", "two\n");
    const source = key();
    assert.notEqual(source.key, first.key);
    assert.equal(source.prefix, first.prefix);
    write("workflow.yml", "two\n");
    assert.notEqual(key().key, source.key);
    assert.deepEqual(key({ ImageVersion: "2" }), key());
    assert.notEqual(key({ RUSTC_VERSION: "2" }).prefix, first.prefix);
    assert.notEqual(run({}, ["missing"]).status, 0);
    assert.notEqual(run({}, ["workflow.yml", "rust", "missing"]).status, 0);
    assert.equal(run({}, ["workflow.yml", "rust", ":!missing"]).status, 0);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
test("the required gate checks code-only diffs, including shallow pull request merges", async () => {
  const { output } = await detect(["src/App.css"]);
  assert.equal(output.docs, "false");
  const check = step("Check diff whitespace", "check");
  assert.doesNotMatch(check, /^ {8}if:/m);
  const root = mkdtempSync(join(tmpdir(), "kubecove-ci-whitespace-"));
  const env = { ...process.env, GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: "/dev/null", GIT_AUTHOR_NAME: "Test", GIT_AUTHOR_EMAIL: "test@example.invalid", GIT_COMMITTER_NAME: "Test", GIT_COMMITTER_EMAIL: "test@example.invalid" };
  const git = (...args) => {
    const result = spawnSync("git", args, { cwd: root, env, encoding: "utf8", timeout: 2000 });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  };
  try {
    git("init", "-q");
    mkdirSync(join(root, "src"));
    writeFileSync(join(root, "src/App.css"), "body {}\n");
    git("add", "src/App.css"); git("commit", "-qm", "clean");
    const clean = git("rev-parse", "HEAD");
    writeFileSync(join(root, "src/App.css"), "body {}\n\n");
    git("add", "src/App.css"); git("commit", "-qm", "bad");
    const bad = git("rev-parse", "HEAD");
    const script = block(check, "run", 8);
    const verify = (from, to, cwd = root) => spawnSync("bash", ["-e", "-o", "pipefail", "-c", script], { cwd, env: { ...env, BASE_SHA: from, HEAD_SHA: to }, encoding: "utf8", timeout: 2000 });
    for (const from of [clean, "", "0".repeat(40), "d".repeat(40)]) {
      for (const [to, succeeds] of [[clean, true], [bad, false]]) {
        const result = verify(from, to);
        assert.equal(result.status === 0, succeeds, `base=${from}, head=${to}: ${result.stderr}`);
      }
    }
    writeFileSync(join(root, "README.md"), "Clean last commit.\n");
    git("add", "README.md"); git("commit", "-qm", "clean tip with earlier whitespace");
    const tip = git("rev-parse", "HEAD");
    const missingMain = verify("", tip);
    assert.equal(missingMain.status, 2, missingMain.stdout);
    assert.match(missingMain.stdout, /src\/App\.css:2: new blank line at EOF/);

    const unrelated = git("commit-tree", `${clean}^{tree}`, "-m", "unrelated history");
    git("update-ref", "refs/remotes/origin/main", unrelated);
    const unrelatedMain = verify("", tip);
    assert.equal(unrelatedMain.status, 2, unrelatedMain.stdout);
    assert.match(unrelatedMain.stdout, /src\/App\.css:2: new blank line at EOF/);

    git("update-ref", "refs/remotes/origin/main", bad);
    const fromMain = verify("", tip);
    assert.equal(fromMain.status, 0, fromMain.stdout);
    const dispatched = verify("", bad);
    assert.equal(dispatched.status, 2, dispatched.stdout);
    assert.match(dispatched.stdout, /src\/App\.css:2: new blank line at EOF/);
    const rootCommit = git("commit-tree", `${bad}^{tree}`, "-m", "root with whitespace");
    git("update-ref", "refs/remotes/origin/main", rootCommit);
    const dispatchedRoot = verify("", rootCommit);
    assert.equal(dispatchedRoot.status, 2, dispatchedRoot.stdout);
    assert.match(dispatchedRoot.stdout, /src\/App\.css:2: new blank line at EOF/);

    const merge = git("commit-tree", `${bad}^{tree}`, "-p", clean, "-p", bad, "-m", "pull request merge");
    git("checkout", "--quiet", "--detach", merge);
    git("clone", "--quiet", "--no-local", "--depth", "2", root, "checkout");
    assert.equal(git("-C", "checkout", "rev-parse", "--is-shallow-repository"), "true");
    const result = verify("HEAD^1", merge, join(root, "checkout"));
    assert.equal(result.status, 2);
    assert.match(result.stdout, /src\/App\.css:2: new blank line at EOF/);
    assert.doesNotMatch(result.stdout, /No available base commit/);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
// Failure modes: a matrix that misses a PARITY_SUITE_SHARD index silently drops that share of
// the parity suite; a job that runs on the other event runs the suite twice or never; a
// scenario that the shards exclude but no separate job runs never runs; a shard that fails
// without uploading its artifacts leaves nothing to inspect.
test("parity jobs select pull request scenarios and preserve full main coverage", () => {
  const read = (name) => readFileSync(new URL(`../.github/workflows/${name}`, import.meta.url), "utf8").replaceAll("\r\n", "\n");
  const parity = read("parity.yml");
  const suite = read("parity-suite.yml");
  assert.deepEqual([...parity.slice(parity.indexOf("\njobs:\n")).matchAll(/^ {2}([\w-]+):$/gm)].map((match) => match[1]), ["timings", "pull-request-selection", "pull-request-web", "pull-request-gpui-build", "pull-request-web-spike", "pull-request-gpui-spike", "pull-request-gpui", "pull-request-result", "main-gpui-build", "main", "main-spike", "main-build-cache", "publish-timings", "coverage-map"]);
  const selection = job("pull-request-selection", parity);
  assert.match(selection, /^ {6}contents: read\n {6}actions: read$/m);
  assert.match(selection, /^ {10}fetch-depth: 0$/m);
  assert.match(selection, /^ {10}persist-credentials: false$/m);
  // A stalled planner must end and leave empty outputs, which run the full suite.
  // The limits are the ones "Detect CI changes" and "Parity result" already use.
  assert.match(selection, /^ {8}id: select\n {8}timeout-minutes: 3\n {8}continue-on-error: true$/m);
  // So must a stalled checkout, setup or upload, which the step limit does not cover.
  assert.match(selection, /^ {4}timeout-minutes: 5$/m);
  assert.match(selection, /run: bun e2e\/parity\/ci-selection\.ts/);
  assert.match(selection, /^ {6}attempt: \$\{\{ github.run_attempt \}\}$/m);
  assert.doesNotMatch(selection, /cargo |playwright|bun install/);
  assert.match(selection, /^ {4}if: \$\{\{ github.event_name == 'pull_request' \}\}$/m);
  const result = job("pull-request-result", parity);
  assert.match(result, /^ {4}name: Parity result$/m);
  assert.match(result, /^ {4}needs:\n {6}- pull-request-selection\n {6}- timings\n {6}- pull-request-gpui-build\n {6}- pull-request-web\n {6}- pull-request-gpui\n {6}- pull-request-web-spike\n {6}- pull-request-gpui-spike$/m);
  assert.ok(result.includes("if: ${{ always() && github.event_name == 'pull_request' }}"));
  // Execute the real shell gate for the supported job graphs and every unexpected outcome.
  const jobs = ["WEB_RESULT", "GPUI_RESULT", "WEB_SPIKE_RESULT", "GPUI_SPIKE_RESULT", "GPUI_BUILD_RESULT", "TIMINGS_RESULT"];
  const verify = (env) => spawnSync("bash", ["-e", "-o", "pipefail", "-c", block(result, "run", 8)], { env: { ...process.env, REGULAR_SELECTED: "", SPIKE_SELECTED: "", ...Object.fromEntries(jobs.map((key) => [key, "success"])), ...env }, encoding: "utf8" }).status;
  for (const [regular, spike] of [["", ""], ["true", "true"], ["true", "false"], ["false", "true"], ["false", "false"]]) {
    const regularResult = regular === "false" ? "skipped" : "success";
    const spikeResult = spike === "false" ? "skipped" : "success";
    const buildResult = regularResult === "skipped" && spikeResult === "skipped" ? "skipped" : "success";
    const good = { REGULAR_SELECTED: regular, SPIKE_SELECTED: spike, WEB_RESULT: regularResult, GPUI_RESULT: regularResult, WEB_SPIKE_RESULT: spikeResult, GPUI_SPIKE_RESULT: spikeResult, GPUI_BUILD_RESULT: buildResult, TIMINGS_RESULT: "success" };
    assert.equal(verify(good), 0);
    for (const key of jobs) {
      if (key === "TIMINGS_RESULT" && buildResult === "skipped") continue;
      for (const outcome of ["failure", "cancelled", "skipped", "success", ""]) {
        if (outcome !== good[key]) assert.notEqual(verify({ ...good, [key]: outcome }), 0);
      }
    }
  }
  assert.notEqual(verify({ WEB_RESULT: "skipped", GPUI_RESULT: "skipped", WEB_SPIKE_RESULT: "skipped", GPUI_SPIKE_RESULT: "skipped", RELEASE_PR: "true" }), 0);
  // Match the supported Actions condition forms, then exercise their string/status semantics.
  const drivers = {
    "pull-request-web": { selections: ["regular"], needsTimings: true },
    "pull-request-gpui": { selections: ["regular"], needsTimings: true, needsBuild: true },
    "pull-request-web-spike": { selections: ["spike"], needsTimings: true },
    "pull-request-gpui-spike": { selections: ["spike"], needsTimings: true, needsBuild: true },
    "pull-request-gpui-build": { selections: ["regular", "spike"] },
  };
  const selectionJob = "needs.pull-request-selection";
  const enabled = (name, outputs, result = "success", event = "pull_request", build = "success", timings = "success", cancelled = false) => {
    const { selections, needsTimings, needsBuild } = drivers[name];
    const expected = [
      "!cancelled()", "github.event_name == 'pull_request'",
      ...(needsTimings ? ["needs.timings.result == 'success'"] : []),
      ...(needsBuild ? ["needs.pull-request-gpui-build.result == 'success'"] : []),
      `${selectionJob}.result != 'skipped'`,
      `(${[`${selectionJob}.result != 'success'`, ...selections.map((key) => `${selectionJob}.outputs.${key} != 'false'`)].join(" || ")})`,
    ].join(" && ");
    assert.equal(/^ {4}if: \$\{\{ (.+) \}\}$/m.exec(job(name, parity))?.[1], expected);
    return !cancelled && event === "pull_request" && (!needsTimings || timings === "success") &&
      (!needsBuild || build === "success") && result !== "skipped" &&
      (result !== "success" || selections.some((key) => (outputs[key] ?? "") !== "false"));
  };
  for (const [regular, spike, expected] of [
    ["true", "false", [true, true, false, false, true]],
    ["false", "true", [false, false, true, true, true]],
    ["false", "false", [false, false, false, false, false]],
    ["true", "true", [true, true, true, true, true]],
    ["", "", [true, true, true, true, true]],
  ]) assert.deepEqual(Object.keys(drivers).map((name) => enabled(name, { regular, spike })), expected);
  for (const [result, event, build, timings, cancelled, expected] of [
    ["success", "pull_request", "failure", "success", false, [true, false, true, false, true]],
    ["success", "pull_request", "success", "failure", false, [false, false, false, false, true]],
    ["failure", "pull_request", "success", "success", false, [true, true, true, true, true]],
    ["skipped", "pull_request", "success", "success", false, [false, false, false, false, false]],
    ["success", "pull_request", "success", "success", true, [false, false, false, false, false]],
    ["success", "push", "success", "success", false, [false, false, false, false, false]],
  ]) assert.deepEqual(Object.keys(drivers).map((name) => enabled(name, {}, result, event, build, timings, cancelled)), expected);

  const value = (text, key, outputs, result = "success") => {
    const driver = /^ {6}driver: (web|gpui)$/m.exec(text)?.[1];
    const full = driver === "web" ? [0, 1, 2, 3] : [0, 1, 2];
    const readers = {
      scenarios: ["ids", ""], PARITY_SCENARIOS: ["ids", ""],
      REGULAR_SELECTED: ["regular", ""], SPIKE_SELECTED: ["spike", ""],
      PARITY_SELECTION_REASON: ["reason", '"Selection planner failed; full suite"'],
      matrix: [`${driver}-matrix`, JSON.stringify({ shard: full })],
      shards: [`${driver}-shards`, String(full.length)],
    };
    assert.ok(Object.hasOwn(readers, key), `Unsupported parity reader: ${key}`);
    const [output, fallback] = readers[key];
    const json = key === "matrix" || key === "shards";
    const guarded = `${selectionJob}.result == 'success' && ${selectionJob}.outputs.${output} || '${fallback}'`;
    assert.equal(new RegExp(`^ +${key}: \\$\\{\\{ (.+) \\}\\}$`, "m").exec(text)?.[1], json ? `fromJSON(${guarded})` : guarded);
    const selected = (result === "success" ? outputs[output] : "") || fallback;
    return json ? JSON.parse(selected) : selected;
  };
  // Failure mode: a planner job that fails after writing its outputs, such as an empty
  // selection, must not decide anything. Every reader falls back to the full suite.
  const empty = { ids: "[]", regular: "false", spike: "false", "web-shards": "0", "gpui-shards": "0", "web-matrix": '{"shard":[0]}', "gpui-matrix": '{"shard":[0]}', reason: '"Nothing"' };
  // Failure mode: the selection step fails after it wrote its outputs, and continue-on-error
  // keeps the job successful. The job must publish nothing, which every reader takes as full suite.
  const published = (key, outcome) => {
    const expression = new RegExp(`^ {6}${key}: \\$\\{\\{ (.+) \\}\\}$`, "m").exec(selection)?.[1];
    assert.equal(expression, `steps.select.outcome == 'success' && steps.select.outputs.${key} || ''`);
    return outcome === "success" ? empty[key] : "";
  };
  const unpublished = Object.fromEntries(Object.keys(empty).map((key) => [key, published(key, "failure")]));
  for (const key of Object.keys(empty)) {
    assert.equal(published(key, "success"), empty[key]);
    for (const outcome of ["failure", "cancelled", "skipped"]) assert.equal(published(key, outcome), "", `${key} after a ${outcome} step`);
  }
  for (const name of ["pull-request-web", "pull-request-gpui", "pull-request-web-spike", "pull-request-gpui-spike"]) {
    assert.equal(enabled(name, unpublished), true, "a failed selection step runs the full suite");
    assert.equal(value(job(name, parity), "scenarios", unpublished), "");
    assert.equal(enabled(name, {}, "failure"), true, "planner failure runs full suite");
    for (const outcome of ["failure", "cancelled"]) assert.equal(enabled(name, empty, outcome), true, `a ${outcome} planner's outputs are ignored`);
    assert.equal(enabled(name, {}, "skipped"), false, "unexpected planner skip is rejected by the gate");
    assert.equal(enabled(name, {}, "success", "pull_request", "success", "failure"), false);
    assert.equal(enabled(name, {}, "success", "push"), false, "main graph stays separate");
    assert.equal(value(job(name, parity), "scenarios", { ids: '["one"]' }), '["one"]');
    assert.equal(value(job(name, parity), "scenarios", empty), "[]");
    assert.equal(value(job(name, parity), "scenarios", {}), "");
    assert.equal(value(job(name, parity), "scenarios", empty, "failure"), "");
  }
  for (const [name, driver, count] of [["pull-request-web", "web", 4], ["pull-request-gpui", "gpui", 3]]) {
    const text = job(name, parity);
    const full = { shard: Array.from({ length: count }, (_, index) => index) };
    const partial = { [`${driver}-matrix`]: '{"shard":[0,1]}', [`${driver}-shards`]: "2" };
    assert.deepEqual([value(text, "matrix", partial), value(text, "shards", partial)], [{ shard: [0, 1] }, 2]);
    for (const [outputs, outcome] of [[{}, "success"], [{}, "failure"], [empty, "failure"], [partial, "cancelled"]]) {
      assert.deepEqual([value(text, "matrix", outputs, outcome), value(text, "shards", outputs, outcome)], [full, count]);
    }
    assert.match(text, new RegExp(`^ {6}driver: ${driver}$`, "m"));
    assert.match(text, /^ {6}exclude: settings-diagnostics.topology-spike-harness$/m);
  }
  // The gate and the report check read the same guarded values, so a failed planner with an
  // empty selection and skipped drivers fails the gate.
  const guarded = (outputs, outcome) => ({ REGULAR_SELECTED: value(result, "REGULAR_SELECTED", outputs, outcome), SPIKE_SELECTED: value(result, "SPIKE_SELECTED", outputs, outcome) });
  assert.deepEqual(guarded(empty, "success"), { REGULAR_SELECTED: "false", SPIKE_SELECTED: "false" });
  assert.deepEqual(guarded(empty, "failure"), { REGULAR_SELECTED: "", SPIKE_SELECTED: "" });
  const skipped = Object.fromEntries(jobs.filter((key) => key !== "TIMINGS_RESULT").map((key) => [key, "skipped"]));
  assert.equal(verify({ ...guarded(empty, "success"), ...skipped }), 0);
  assert.notEqual(verify({ ...guarded(empty, "failure"), ...skipped }), 0);
  assert.equal(value(result, "PARITY_SCENARIOS", empty), "[]");
  assert.equal(value(result, "PARITY_SCENARIOS", empty, "failure"), "");
  assert.equal(value(result, "PARITY_SELECTION_REASON", empty), '"Nothing"');
  assert.equal(value(result, "PARITY_SELECTION_REASON", empty, "failure"), '"Selection planner failed; full suite"');
  // Failure mode: the report check stops running, and job results alone pass the gate.
  const validate = result.slice(result.indexOf("      - name: Validate merged driver reports\n"));
  assert.match(validate, /^ {6}- name: Validate merged driver reports\n {8}env:$/m);
  assert.match(validate, /^ {8}run: bun e2e\/parity\/merge-reports\.ts parity-reports$/m);
  assert.match(selection, /^ {6}- name: Upload selection\n {8}continue-on-error: true$/m);
  for (const driver of ["web", "gpui"]) {
    const spike = job(`pull-request-${driver}-spike`, parity);
    assert.match(spike, new RegExp(`^ {6}driver: ${driver}$`, "m"));
    assert.match(spike, /^ {6}prefix: settings-diagnostics.topology-spike-harness$/m);
  }
  const main = job("main", parity);
  assert.match(main, /^ {8}shard: \[0, 1, 2, 3\]$/m);
  assert.match(main, /^ {6}shards: 4$/m);
  assert.match(job("main-spike", parity), /^ {6}shard: 0\n {6}shards: 1$/m);
  assert.ok(main.includes("if: ${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}"));
  assert.match(suite, /^ {10}PARITY_SUITE_SHARD: \$\{\{ inputs.shard \}\}\/\$\{\{ inputs.shards \}\}$/m);
  assert.match(suite, /^ {10}PARITY_SCENARIOS: \$\{\{ inputs.scenarios \}\}$/m);
  assert.match(suite, /^ {10}PARITY_STEP_SCREENSHOTS: "0"$/m);
  const run = suite.slice(suite.indexOf("- name: Run parity suite"), suite.indexOf("- name: Upload parity artifacts"));
  const command = block(run, "run", 8);
  const runners = (cpus, driver) => spawnSync("bash", ["-e", "-c", `unset PARITY_JOBS PARITY_WEB_JOBS PARITY_GPUI_COVERAGE_JOBS; nproc() { echo ${cpus}; }; bun() { echo "\${PARITY_JOBS:-default} \${PARITY_WEB_JOBS:-default} \${PARITY_GPUI_COVERAGE_JOBS:-default}"; }; ${command}`], { env: { ...process.env, PARITY_DRIVER: driver, KUBECOVE_PARITY_TIMINGS_PATH: fileURLToPath(import.meta.url) }, encoding: "utf8" }).stdout.trim();
  for (const driver of ["web", "gpui"]) assert.equal(runners(4, driver), "4 4 default");
  assert.equal(runners(4, ""), "default default default");
  assert.match(result, /^ {10}pattern: parity-pull_request-\*$/m);
  assert.match(result, /PARITY_SELECTION_ATTEMPT: \$\{\{ needs.pull-request-selection.outputs.attempt \}\}/);
  assert.match(result, /run: bun e2e\/parity\/merge-reports\.ts parity-reports/);
  assert.ok(result.includes(`if: \${{ needs.pull-request-web.result == 'success' || needs.pull-request-web-spike.result == 'success' }}`));
  const chromium = suite.slice(suite.indexOf("- name: Install Chromium"), suite.indexOf("- name: Seed the uninstrumented GPUI cache"));
  assert.match(chromium, /if: \$\{\{ inputs.build == '' && inputs.driver != 'gpui' \}\}/);
  assert.match(suite, /uses: \.\/\.github\/actions\/setup-linux-native-deps/);
  assert.doesNotMatch(suite, /\.bun\/install\/cache/);
  assert.match(suite, /bun install --frozen-lockfile/);
  const upload = suite.slice(suite.indexOf("- name: Upload parity artifacts"));
  assert.match(upload, /^ {8}if: \$\{\{ always\(\) && inputs.build == '' \}\}$/m);
  assert.match(upload, /^ {10}path: e2e\/artifacts\/parity-\*$/m);
  assert.match(upload, /^ {10}name: .*\$\{\{ inputs.shard \}\}.*\$\{\{ inputs.shards \}\}.*\$\{\{ github.run_attempt \}\}$/m);
  // A partial suite and the spike both use shard 0 of 1. Their uploads must not collide.
  const artifactTemplate = /^ {10}name: (.+)$/m.exec(upload)[1];
  const artifactName = (inputs, event = "pull_request") => {
    const fields = {
      "github.event_name": event, "github.run_attempt": 1,
      "inputs.shard": inputs.shard, "inputs.shards": inputs.shards,
      "inputs.driver || 'both'": inputs.driver || "both",
      "inputs.driver && inputs.prefix && '-spike' || ''": inputs.driver && inputs.prefix ? "-spike" : "",
    };
    return artifactTemplate.replace(/\$\{\{ (.*?) \}\}/g, (_, expression) => {
      assert.ok(Object.hasOwn(fields, expression), `Unsupported parity artifact expression: ${expression}`);
      return fields[expression];
    });
  };
  const partialArtifacts = ["web", "gpui"].flatMap((driver) => ["", "settings-diagnostics.topology-spike-harness"].map((prefix) => artifactName({ driver, prefix, shard: 0, shards: 1 })));
  assert.equal(new Set(partialArtifacts).size, 4);
  assert.equal(artifactName({ driver: "", prefix: "", shard: 0, shards: 4 }, "push"), "parity-push-both-shard-0-of-4-attempt-1");
  assert.equal(artifactName({ driver: "", prefix: "settings-diagnostics.topology-spike-harness", shard: 0, shards: 1 }, "push"), "parity-push-both-shard-0-of-1-attempt-1");
});

test("parity builds each GPUI binary once and runs shards from the matching artifact", () => {
  const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8").replaceAll("\r\n", "\n");
  const parity = read(".github/workflows/parity.yml");
  const suite = read(".github/workflows/parity-suite.yml");
  const step = (name) => {
    const start = suite.indexOf(`- name: ${name}\n`);
    assert.notEqual(start, -1, `Missing step: ${name}`);
    const end = suite.indexOf("\n      - name: ", start + 1);
    return suite.slice(start, end < 0 ? undefined : end);
  };

  const prBuild = job("pull-request-gpui-build", parity);
  const mainBuild = job("main-gpui-build", parity);
  const seed = job("main-build-cache", parity);
  for (const build of [prBuild, mainBuild, seed]) assert.match(build, /uses: \.\/\.github\/workflows\/parity-suite\.yml/);
  assert.match(prBuild, /^ {4}cache-mode: read$/m);
  assert.match(prBuild, /^ {6}build: artifact$/m);
  assert.doesNotMatch(prBuild, /coverage:|matrix:/);
  assert.match(mainBuild, /^ {6}build: artifact\n {6}coverage: true$/m);
  assert.match(seed, /^ {6}build: cache$/m);
  assert.doesNotMatch(`${mainBuild}${seed}`, /cache-mode:|matrix:/);
  assert.doesNotMatch(seed, /coverage:/);
  for (const build of [mainBuild, seed]) assert.ok(build.includes(`if: \${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}`));

  for (const [build, coverage, consumers] of [
    ["pull-request-gpui-build", false, ["pull-request-gpui", "pull-request-gpui-spike"]],
    ["main-gpui-build", true, ["main", "main-spike"]],
  ]) {
    for (const consumer of consumers) {
      const text = job(consumer, parity);
      assert.ok(/^ {4}needs: \[([^\]]*)\]$/m.exec(text)?.[1].split(", ").includes(build));
      for (const output of ["gpui-artifact", "rust-version"]) {
        assert.ok(text.includes(`${output}: \${{ needs.${build}.outputs.${output} }}`));
        assert.ok(suite.includes(`${output}:\n        value: \${{ jobs.suite.outputs.${output} }}`));
      }
      assert.equal(/^ {6}coverage: true$/m.test(text), coverage);
      assert.doesNotMatch(text, /^ {6}build:/m);
    }
  }

  for (const name of ["Set up the GPUI CI profile", "Setup Rust for the GPUI build", "Key the Rust cache by its inputs", "Cache Rust builds", "Activate Cargo caching"]) {
    assert.ok(step(name).includes(`if: \${{ inputs.build != '' }}`));
  }
  assert.match(step("Set up the GPUI CI profile"), /uses: \.\/\.github\/actions\/setup-gpui-ci-profile/);
  assert.match(step("Install GPUI Linux dependencies"), /if: \$\{\{ inputs.driver != 'web' \}\}\n {8}uses: \.\/\.github\/actions\/setup-linux-native-deps/);
  assert.match(step("Key the Rust cache by its inputs"), /parity\$\{\{ inputs.coverage && '-coverage' \|\| '' \}\} \.github\/workflows\/parity-suite\.yml/);
  assert.match(step("Cache Rust builds"), /uses: jdx\/mr-boxington-action@[a-f0-9]{40} # v1\n/);
  for (const line of ['version: "1.30.0"', "working-directory: rust/gpui", `cache-key: \${{ steps.rust-cache.outputs.key }}`, `restore-keys: \${{ steps.rust-cache.outputs.prefix }}`, "github-cache-mode: objects", "isolate-objects-cache: true", "clean-target-before-export: true"]) {
    assert.ok(step("Cache Rust builds").includes(`\n          ${line}\n`), `Missing cache setting: ${line}`);
  }
  assert.match(step("Activate Cargo caching"), /run: node scripts\/ci-mbx-cargo\.mjs/);
  assert.doesNotMatch(suite, /swatinem\/rust-cache|^ *cache-mode:|ACTIONS_CACHE_MODE: /im);
  const seedStep = step("Seed the uninstrumented GPUI cache");
  assert.ok(seedStep.includes(`if: \${{ inputs.build == 'cache' }}`));
  assert.match(seedStep, /cargo test --manifest-path rust\/gpui\/Cargo\.toml --test parity --no-run --message-format=json-render-diagnostics$/m);

  const build = step("Export the GPUI parity binary");
  const run = step("Run parity suite");
  assert.ok(build.includes(`if: \${{ inputs.build == 'artifact' }}`));
  assert.ok(run.includes(`if: \${{ inputs.build == '' }}`));
  for (const entry of [build, run]) assert.ok(entry.includes(`PARITY_COVERAGE: \${{ inputs.coverage && '1' || '0' }}`));
  assert.ok(build.includes(`KUBECOVE_PARITY_BUILD_DIR: \${{ runner.temp }}/gpui-parity`));
  assert.match(build, /bun run e2e:parity/);
  assert.match(suite, /gpui-artifact: \$\{\{ steps.binary.outputs.artifact-id \}\}/);
  assert.match(suite, /rust-version: \$\{\{ steps.export.outputs.rust-version \}\}/);
  assert.ok(step("Upload the GPUI binary").includes(`name: parity-gpui-binary-attempt-\${{ github.run_attempt }}`));
  assert.ok(step("Upload the GPUI binary").includes(`\${{ runner.temp }}/gpui-parity/manifest.json\n            \${{ runner.temp }}/gpui-parity/parity`));
  for (const name of ["Require inputs for GPUI replay", "Setup the GPUI build's Rust release", "Download the GPUI binary"]) {
    assert.ok(step(name).includes(`if: \${{ inputs.build == '' && inputs.driver != 'web' }}`));
  }
  assert.ok(step("Setup the GPUI build's Rust release").includes(`toolchain: \${{ inputs.rust-version }}`));
  assert.ok(step("Setup the GPUI build's Rust release").includes(`components: \${{ inputs.coverage && 'llvm-tools' || '' }}`));
  assert.ok(step("Download the GPUI binary").includes(`artifact-ids: \${{ inputs.gpui-artifact }}`));
  assert.doesNotMatch(step("Download the GPUI binary"), /run-id:|github-token:/);
  assert.ok(run.includes(`KUBECOVE_PARITY_MANIFEST: \${{ inputs.driver != 'web' && format('{0}/gpui-parity/manifest.json', runner.temp) || '' }}`));
  assert.ok(run.includes(`RUSTUP_TOOLCHAIN: \${{ inputs.rust-version }}`));
  assert.doesNotMatch(run, /cargo |CARGO_BUILD_JOBS/);
  assert.doesNotMatch(step("Checkout"), /^ +path:/m);

  const merge = job("coverage-map", parity);
  const mapPath = /^export const DEFAULT_MAP = "(.+)";$/m.exec(read("e2e/parity/changed-scenarios.ts"))?.[1];
  assert.equal(mapPath, "e2e/artifacts/parity-coverage-map.json");
  assert.match(merge, /^ {4}needs:\n {6}- main\n {6}- main-spike$/m);
  assert.ok(merge.includes(`if: \${{ !cancelled() && github.event_name == 'push' && github.ref == 'refs/heads/main' }}`));
  assert.doesNotMatch(merge, /main(-spike)?\.result|merge-reports\.ts|parity-scenario-timings/);
  const map = step("Upload shard coverage map");
  assert.match(map, /path: e2e\/artifacts\/parity-\*\/coverage-map\.json/);
  const shardArtifact = /name: (parity-coverage-shard-)\$\{\{/.exec(map)?.[1];
  assert.ok(shardArtifact && merge.includes(`pattern: ${shardArtifact}*`), "merge downloads the shard maps the suite uploads");
  assert.match(map, /\$\{\{ inputs\.shard \}\}-of-\$\{\{ inputs\.shards \}\}/);
  assert.match(map, /-attempt-\$\{\{ github\.run_attempt \}\}/);
  assert.ok(merge.includes(`run: bun e2e/parity/coverage-map.ts merge ${mapPath} shard-maps/*/*/coverage-map.json`));
  const uploaded = merge.slice(merge.indexOf("- name: Upload coverage map"));
  assert.ok(uploaded.includes(`path: ${mapPath}`) && uploaded.includes("name: parity-coverage-map\n"));
  assert.match(uploaded, /^ {10}overwrite: true$/m);
});

test("parity shares one immutable timing snapshot across shard families and reruns", () => {
  const read = (name) => readFileSync(new URL(`../.github/workflows/${name}`, import.meta.url), "utf8").replaceAll("\r\n", "\n");
  const parity = read("parity.yml");
  const suite = read("parity-suite.yml");
  const timings = job("timings", parity);
  for (const name of ["pull-request-web", "pull-request-gpui", "pull-request-web-spike", "pull-request-gpui-spike", "main", "main-spike"]) {
    assert.ok(/^ {4}needs: \[([^\]]*)\]$/m.exec(job(name, parity))?.[1].split(", ").includes("timings"));
  }
  assert.match(timings, /name: parity-timing-snapshot\n/);
  assert.doesNotMatch(timings, /overwrite:/);
  assert.match(suite, /name: parity-timing-snapshot\n/);
  assert.ok(suite.includes(`KUBECOVE_PARITY_TIMINGS_PATH: \${{ runner.temp }}/parity-timings/\${{ inputs.driver || 'web' }}.json`));
  assert.match(suite, /test -s "\$KUBECOVE_PARITY_TIMINGS_PATH"/);
  const publish = job("publish-timings", parity);
  assert.match(publish, /^ {4}needs:\n {6}- main\n {6}- main-spike$/m);
  // The default success() guard requires both dependencies and the validation step to pass.
  assert.ok(publish.includes(`if: \${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}`));
  assert.match(publish, /pattern: parity-push-\*/);
  assert.match(publish, /run: \|\n {10}bun e2e\/parity\/merge-reports\.ts parity-reports\n/);
  assert.doesNotMatch(publish, /continue-on-error:/);
  assert.match(publish, /^ {6}- name: Upload scenario timings for future runs\n {8}uses:.*\n {8}with:\n {10}name: parity-scenario-timings$/m);

  const command = block(timings, "run", 8);
  const stub = [
    "gh() {",
    '  case "$1 $2" in',
    '    "api repos/fake/repo/actions/runs/42/artifacts?name=parity-timing-snapshot") [[ "$CASE" == reuse ]] && echo 91; return 0 ;;',
    '    "api --paginate") [[ "$CASE" == lost ]] && echo 92; return 0 ;;',
    '    "run list") echo 21 ;;',
    '    "run download")',
    '      [[ "$3" == 21 && "$7" == parity-scenario-timings ]] || return 2',
    '      [[ "$CASE" != missing ]] || return 1',
    '      printf \'{"one":17}\\n\' > parity-timings/web.json',
    '      printf \'%s\\n\' "$GPUI_TIMINGS" > parity-timings/gpui.json ;;',
    "    *) return 2 ;;",
    "  esac",
    "}",
  ].join("\n");
  for (const state of ["valid", "missing", "malformed", "reuse", "lost", "retry"]) {
    const cwd = mkdtempSync(join(tmpdir(), "kubecove-parity-timings-"));
    try {
      const run = spawnSync("bash", ["-e", "-o", "pipefail", "-c", `${stub}\n${command}`], {
        cwd, encoding: "utf8", env: { ...process.env, CASE: state, GITHUB_RUN_ATTEMPT: ["reuse", "lost", "retry"].includes(state) ? "2" : "1", GITHUB_REPOSITORY: "fake/repo", GITHUB_RUN_ID: "42", GITHUB_OUTPUT: "output", GPUI_TIMINGS: state === "malformed" ? '{"one":"slow"}' : '{"one":4}' },
      });
      if (state === "lost") {
        assert.equal(run.status, 1, run.stderr);
        assert.match(run.stderr, /Cannot replace a missing timing snapshot/);
        assert.throws(() => readFileSync(join(cwd, "output")), /ENOENT/);
        continue;
      }
      assert.equal(run.status, 0, run.stderr);
      if (state === "reuse") {
        assert.match(run.stdout, /Reusing this run's timing snapshot/);
        assert.throws(() => readFileSync(join(cwd, "output")), /ENOENT/);
      } else {
        assert.equal(readFileSync(join(cwd, "output"), "utf8"), "upload=true\n");
        for (const [driver, duration] of [["web", 17], ["gpui", 4]]) {
          assert.deepEqual(JSON.parse(readFileSync(join(cwd, "parity-timings", `${driver}.json`), "utf8")), ["valid", "retry"].includes(state) ? { one: duration } : {});
        }
      }
    } finally { rmSync(cwd, { recursive: true, force: true }); }
  }
});

test("Linux E2E sets up the Bun executable without archiving downloaded packages", () => {
  const e2e = readFileSync(new URL("../.github/workflows/e2e.yml", import.meta.url), "utf8").replaceAll("\r\n", "\n");
  for (const name of ["real-primary", "real-matrix"]) {
    const linux = job(name, e2e);
    assert.match(linux, /uses: oven-sh\/setup-bun@/);
    assert.match(linux, /bun install --frozen-lockfile/);
    assert.doesNotMatch(linux, /\.bun\/install\/cache/);
  }
});
// Ways the cleanup could go wrong: it deletes the cache in use, it takes a longer name such
// as ci-server-backend for a version of ci-server, it deletes a cache of another ref or of
// another tool, it depends on the order GitHub lists caches in, or it deletes nothing.
test("cache cleanup deletes only main's Rust build caches that a newer one of the same name replaced", () => {
  const [a, b, c] = ["a", "b", "c"].map((digit) => digit.repeat(64));
  const cache = (id, key, created_at, ref = "refs/heads/main") => ({ id, key, ref, created_at });
  const caches = [
    cache(1, `mbx-ci-server-${a}-${a}`, "2026-11-13T10:00:00Z"),
    cache(2, `mbx-ci-server-backend-${a}-${a}`, "2026-11-13T09:00:00Z"),
    // A later save replaces an earlier one, also when the compiler digest changed.
    cache(3, `mbx-ci-server-${b}-${c}`, "2026-11-13T11:00:00Z"),
    // The newer cache is listed first. Compared as text, "00Z" would sort after "00.5Z".
    cache(4, `mbx-parity-${a}-${b}`, "2026-11-13T12:00:00.5Z"),
    cache(5, `mbx-parity-${a}-${a}`, "2026-11-13T12:00:00Z"),
    cache(6, `mbx-parity-coverage-${a}-${a}`, "2026-11-13T08:00:00Z"),
    cache(7, `mbx-ci-server-${a}-${b}`, "2026-11-13T07:00:00Z", "refs/heads/release/app-v1.0.0"),
    cache(8, `mbx-ci-server-${a}-${b}`, "2026-11-13T07:00:00Z", "refs/pull/1/merge"),
    cache(9, `Linux-bun-${a}`, "2026-11-13T07:00:00Z"),
    cache(10, `Linux-bun-${b}`, "2026-11-13T07:30:00Z"),
    cache(11, `mbx-parity-${a}`, "2026-11-13T07:00:00Z"),
    // Text around the key shape makes another tool's key. Read as parity caches, these would replace cache 4.
    cache(12, `other-mbx-parity-${a}-${c}`, "2026-11-13T13:00:00Z"),
    cache(13, `mbx-parity-${a}-${c}-part`, "2026-11-13T13:00:00Z"),
    // Saved in the same millisecond: which one is newer is unknown, so neither is deleted.
    cache(14, `mbx-e2e-${a}-${a}`, "2026-11-13T12:00:00.500001Z"),
    cache(15, `mbx-e2e-${a}-${b}`, "2026-11-13T12:00:00.500999Z"),
    // A later save replaces both of two that share a millisecond, whichever is listed first.
    cache(16, `mbx-ci-rust-${a}-${a}`, "2026-11-13T12:00:00.500001Z"),
    cache(17, `mbx-ci-rust-${a}-${b}`, "2026-11-13T12:00:00.500999Z"),
    cache(18, `mbx-ci-rust-${a}-${c}`, "2026-11-13T12:00:01Z"),
  ];
  const deleted = (list) => supersededCaches(list).map(({ id }) => id).sort((first, second) => first - second);
  assert.deepEqual(deleted(caches), [1, 5, 16, 17]);
  assert.deepEqual(deleted(caches.toReversed()), [1, 5, 16, 17]);
});
test("cache cleanup deletes only with --delete, passes over a cache that is already gone, and fails on a refusal", () => {
  const root = mkdtempSync(join(tmpdir(), "kubecove-ci-cache-prune-"));
  const log = join(root, "deleted");
  const digest = "a".repeat(64);
  const caches = ["2026-11-13T10:00:00Z", "2026-11-13T11:00:00Z", "2026-11-13T12:00:00Z"].map((created_at, index) => (
    { id: index + 1, key: `mbx-parity-${digest}-${String(index).repeat(64)}`, ref: "refs/heads/main", created_at, size_in_bytes: 1 }
  ));
  const run = (args, refusal = "") => spawnSync(process.execPath, [fileURLToPath(new URL("./ci-prune-rust-caches.mjs", import.meta.url)), ...args], {
    env: { ...process.env, PATH: `${join(root, "bin")}${delimiter}${process.env.PATH}`, GH_LOG: log, GH_LIST: join(root, "list.json"), GH_REFUSAL: refusal },
    encoding: "utf8",
  });
  try {
    mkdirSync(join(root, "bin"));
    writeFileSync(join(root, "list.json"), JSON.stringify([{ actions_caches: caches }]));
    // Stands in for `gh api`: cache 1 is gone when its deletion arrives, and GH_REFUSAL refuses cache 2.
    writeFileSync(join(root, "bin", "gh"), [
      "#!/bin/sh",
      'case "$*" in',
      '  *DELETE*/caches/1) echo "gh: Not Found (HTTP 404)" >&2; exit 1 ;;',
      '  *DELETE*/caches/2) if [ -n "$GH_REFUSAL" ]; then echo "gh: $GH_REFUSAL" >&2; exit 1; fi; echo "$*" >> "$GH_LOG" ;;',
      '  *DELETE*) echo "$*" >> "$GH_LOG" ;;',
      '  *) cat "$GH_LIST" ;;',
      "esac",
      "",
    ].join("\n"));
    chmodSync(join(root, "bin", "gh"), 0o755);
    const listed = run([]);
    assert.equal(listed.status, 0, listed.stderr);
    assert.match(listed.stdout, /Would delete mbx-parity-.+\nWould delete mbx-parity-/);
    assert.throws(() => readFileSync(log), /ENOENT/);
    assert.equal(run(["--delete"]).status, 0);
    assert.match(readFileSync(log, "utf8"), /^[^\n]+\/caches\/2\n$/);
    assert.notEqual(run(["--delete"], "Resource not accessible by integration (HTTP 403)").status, 0);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
test("cache cleanup runs after workflows that exist", () => {
  const directory = new URL("../.github/workflows/", import.meta.url);
  const read = (file) => readFileSync(new URL(file, directory), "utf8").replaceAll("\r\n", "\n");
  const names = readdirSync(directory).map((file) => /^name: (.+)$/m.exec(read(file))?.[1]);
  const followed = /^ {4}workflows: \[(.+)\]$/m.exec(read("rust-cache-cleanup.yml"))?.[1].split(", ");
  // A renamed workflow would stop starting the cleanup without any failure.
  assert.ok(followed?.length, "the cleanup lists the workflows it follows on one line");
  for (const name of followed) assert.ok(names.includes(name), `no workflow is named ${name}`);
});
