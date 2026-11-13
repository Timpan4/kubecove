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
    'globalThis.fetch = async (url, options) => {',
    '  assert.equal(url, `https://api.github.com/repos/Timpan4/kubecove/actions/runs/123/attempts/${process.env.SOURCE_ATTEMPT ?? 1}/jobs`);',
    '  assert.equal(options.headers.Authorization, "Bearer fixture-token");',
    '  if (process.env.SOURCE_RESULT === "error") throw new Error("unavailable");',
    '  const job = { name: process.env.SOURCE_NAME ?? "CI gpui-test", status: process.env.SOURCE_STATUS ?? "completed", conclusion: process.env.SOURCE_RESULT ?? "success" };',
    '  const jobs = process.env.SOURCE_DUPLICATE ? [job, job] : [job];',
    '  const total = Number(process.env.SOURCE_TOTAL ?? jobs.length);',
    '  return { ok: !process.env.SOURCE_DENIED, json: async () => ({ total_count: total, jobs: process.env.SOURCE_MALFORMED ? null : jobs }) };',
    '};',
  ].join("\n"));
  const steps = Object.fromEntries(["setup-rust", "rust-cache", "rust-pass", "pass-cache", "decide-pass", "setup-mbx", "activate-cargo", "test-gpui"].map((id) => [id, { outcome: "success", conclusion: "success" }]));
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
    assert.equal(hit.log, `gpui-test: ${first.key} records https://github.com/Timpan4/kubecove/actions/runs/123/attempts/1; skipping build and test work.\n`);
    for (const overrides of [
      { PASS_CACHE_MATCH: "" }, { PASS_CACHE_MATCH: first.prefix }, { PASS_CACHE_MATCH: rerun.key },
      { PASS_EVENT: "push" }, { PASS_EVENT: "workflow_dispatch" },
      { PASS_KEY: key.replace(/b{64}$/, "c".repeat(64)) }, { PASS_TASK: "gpui-clippy" },
      { ImageVersion: "new-image" }, { ImageVersion: "" }, { RUNNER_ARCH: "ARM64" },
      ...["failure", "cancelled", "skipped", "", "error"].map((SOURCE_RESULT) => ({ SOURCE_RESULT })),
      // A refused request can still carry a body, and a second page can hold a failed job of the same name.
      { SOURCE_DENIED: "true" }, { SOURCE_TOTAL: "2" }, { SOURCE_TOTAL: "31" },
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
    const skipped = { ...steps, "decide-pass": { ...steps["decide-pass"], outputs: { skip: "true" } }, ...Object.fromEntries(["setup-mbx", "activate-cargo", "test-gpui"].map((id) => [id, { outcome: "skipped", conclusion: "skipped" }])) };
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
  ["bun.lock", ["frontend", "nix"]],
  ["src-tauri/src/lib.rs", ["rust"]],
  ["src-tauri/crates/models/src/lib.rs", ["rust"]],
  ["src-tauri/crates/kube/src/commands/discovery.rs", ["rust"]],
  ["src-tauri/crates/inspection/src/commands/resources/kubeconform.rs", ["rust"]],
  ["src-tauri/crates/live/src/commands/streams.rs", ["rust"]],
  ["src-tauri/crates/gitops/src/commands/argo/connected.rs", ["rust"]],
  ["src-tauri/crates/host/src/commands/usage.rs", ["rust"]],
  ["src-tauri/crates/inspection/Cargo.toml", ["nix", "rust"]],
  ["src-tauri/capabilities/default.yaml", ["rust"]],
  ["src-tauri/tauri.conf.json", ["frontend", "rust"]],
  ["src-tauri/Cargo.lock", ["nix", "rust"]],
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
  assert.deepEqual((await detect([], { data: { files: [{ filename: "src-tauri/src/removed.rs", status: "removed" }] } })).output, flags("rust"));
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
  frontend: ["frontend-e2e", "server-backend", "server"],
  rust: ["rust", "gpui-clippy", "gpui-test", "server-backend", "server"],
  nix: [],
};
test("change detection selects every required task without duplicate jobs", async () => {
  for (const selected of [[], ["docs"], ["frontend"], ["rust"], ["nix"], domains]) {
    const paths = selected.map((scope) => ({ docs: "README.md", frontend: "src/App.svelte", rust: "src-tauri/src/lib.rs", nix: "flake.nix" })[scope]);
    const { matrix } = await detect(paths);
    assert.deepEqual(matrix.include.map(({ task }) => task).sort(), [...new Set(selected.flatMap((scope) => tasksByScope[scope]))].sort());
    for (const task of matrix.include) {
      if (task.workspace) assert.ok(["src-tauri", "src-tauri/server", "src-tauri/gpui"].includes(task.workspace));
    }
  }
  // A limit below the task count starts the last tasks only after the first ones finish.
  assert.doesNotMatch(job("checks"), /max-parallel/);
  assert.match(job("checks"), /fail-fast: false/);
  assert.ok(job("checks").includes("matrix: ${{ fromJSON(needs.changes.outputs.matrix) }}"));
  assert.match(job("nix"), /needs:\n {6}- changes\n {6}- checks/);
  assert.match(job("check"), /needs:\n {6}- changes\n {6}- frontend\n {6}- checks\n {6}- nix/);
  assert.deepEqual([...workflow.slice(workflow.indexOf("\njobs:\n")).matchAll(/^ {2}([\w-]+):$/gm)].map((match) => match[1]), ["changes", "frontend", "checks", "nix", "check"]);
});
function step(name) {
  const source = job("checks");
  const start = source.search(new RegExp(`^ {6}- name: ${name}\\n`, "m"));
  assert.notEqual(start, -1, `Missing step: ${name}`);
  const end = source.indexOf("\n      - name: ", start + 1);
  return source.slice(start, end < 0 ? undefined : end);
}
test("Rust pass reuse skips expensive steps but always runs advisory audits and verifies completion", async () => {
  const { matrix } = await detect([".github/workflows/ci.yml"]);
  const lookup = step("Restore Rust task pass");
  const save = step("Save Rust task pass");
  const pin = /uses: actions\/cache@([a-f0-9]{40})/.exec(step("Cache Bun dependencies"))[1];
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
    const normalized = expression.replaceAll("steps.decide-pass", "steps.decidePass").replaceAll("steps.verify-pass", "steps.verifyPass").replaceAll("steps.pass-cache", "steps.passCache");
    return new Function("matrix", "steps", "github", "always", "success", `return Boolean(${normalized})`)(
      task, { decidePass: { outputs: skip === undefined ? {} : { skip: String(skip) } }, verifyPass: { outcome: successful ? "success" : "failure" }, passCache: { outputs: { key: "passed-key" } } },
      { event_name: event }, () => true, () => successful,
    );
  };
  const builds = ["Install Tauri Linux dependencies", "Install GPUI build dependencies", "Setup Mr Boxington", "Activate Cargo caching", "Setup Bun", "Cache Bun dependencies", "Install frontend dependencies", "Test Rust", "Lint Rust", "Lint GPUI", "Test GPUI", "Verify API-only server", "Build browser UI", "Verify GUI-free dependency tree", "Test GUI-free backend crates", "Test headless server", "Lint headless server", "Build headless server", "Verify HTTP and WebSocket lifecycle", "Upload headless verification report"];
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
  const shared = { docs: ["Setup Bun"], "frontend-e2e": ["Setup Bun", "Cache Bun dependencies", "Install frontend dependencies"] };
  for (const task of matrix.include.filter(({ workspace }) => !workspace)) {
    assert.ok(shared[task.task], task.task);
    for (const name of shared[task.task]) assert.equal(condition(name, task, undefined), true, `${task.task}: ${name}`);
    assert.equal(condition("Decide Rust task reuse", task, undefined), false);
  }
  assert.ok(job("checks").indexOf("- name: Key the Rust cache by its inputs") < job("checks").indexOf("- name: Restore Rust task pass"));
  assert.ok(job("checks").indexOf("- name: Decide Rust task reuse") < job("checks").indexOf("- name: Install Tauri Linux dependencies"));
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
        const expression = /^ {8}if: \$\{\{ (.+) \}\}$/m.exec(source)[1].replaceAll("steps.decide-pass", "steps.decidePass").replaceAll("steps.pass-cache", "steps.passCache");
        const selected = new Function("matrix", "steps", "github", "always", `return Boolean(${expression})`)(task, { decidePass: { outputs: { skip: "false" } }, passCache: { outputs: { key: "passed-key" } } }, { event_name: "push" }, () => true);
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
  const shared = ["tests/fixtures/kubernetes/pod-running.json", "package.json", "bun.lock", "scripts/ci-rust-pass.mjs", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock"];
  const expected = {
    rust: shared,
    "gpui-clippy": [...shared, "src-tauri/gpui/Cargo.lock", "src/lib/settings-integration-workspace.json"],
    "gpui-test": [...shared, "src-tauri/gpui/Cargo.lock", "src/lib/settings-integration-workspace.json"],
    "server-backend": [...shared, "src-tauri/server/Cargo.toml", "src-tauri/server/Cargo.lock"],
    server: [...shared, "src-tauri/server/Cargo.lock", "src/main-svelte.ts", "public/kubecove-logo.svg", "vite.config.ts", "tsconfig.json", "index.html", "scripts/tsrs.ts", "scripts/dev-interactions-plugin.ts", "scripts/frontend-bundle-report.ts", "e2e/headless-smoke.ts", "e2e/harness/port-forward-fixture.ts", "deploy/grafana/kubecove-server.json"],
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
  const desktop = "src-tauri :!src-tauri/gpui :!src-tauri/server";
  const gpui = "src-tauri :!src-tauri/server src/lib/*-workspace.json";
  const server = "src-tauri :!src-tauri/gpui";
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
    const result = spawnSync("bash", ["-c", `git ls-files -z -- .github/workflows/ci.yml ${task.passInputs ?? task.inputs}`], { cwd: new URL("../", import.meta.url), encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    const githubFiles = result.stdout.split("\0").filter((path) => path.startsWith(".github/"));
    assert.ok(githubFiles.includes(".github/workflows/ci.yml"), task.task);
    for (const path of githubFiles) {
      assert.ok(path === ".github/workflows/ci.yml" || uses.some((used) => path === used || path.startsWith(`${used}/`)), `${task.task} includes unrelated ${path}`);
    }
    for (const path of uses) assert.ok(githubFiles.some((file) => file === path || file.startsWith(`${path}/`)), `${task.task} misses ${path}`);
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
  const commands = {
    frontend: ["node --test scripts/ci-workflow-regression.test.mjs", "node --test scripts/release-workflow-regression.test.mjs", ...["audit", "docs:check", "typecheck", "typecheck:bench", "svelte:check", "lint", "lint:anti-slop", "test:bench-groups", "test:svelte"].map((name) => `bun run ${name}`), "bun test"],
    "frontend-e2e": ["bun run e2e:fast"],
    rust: ["mbx test --workspace --manifest-path src-tauri/Cargo.toml", "mbx clippy --workspace --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings"],
    "gpui-clippy": ["mbx clippy --manifest-path src-tauri/gpui/Cargo.toml --locked --all-targets -- -D warnings"],
    "gpui-test": ["mbx test --manifest-path src-tauri/gpui/Cargo.toml --locked --lib --test native"],
    "server-backend": ["cargo tree --manifest-path src-tauri/server/Cargo.toml --locked --edges normal,build --prefix none > server-dependencies.txt", "mbx test --locked --workspace --no-default-features --manifest-path src-tauri/Cargo.toml"],
    server: [
      "mbx test --manifest-path src-tauri/server/Cargo.toml --locked", "mbx clippy --manifest-path src-tauri/server/Cargo.toml --all-targets --locked -- -D warnings",
      "mbx test --manifest-path src-tauri/server/Cargo.toml --locked --no-default-features", "mbx clippy --manifest-path src-tauri/server/Cargo.toml --locked --no-default-features --all-targets -- -D warnings",
      "bun run build", "mbx build --manifest-path src-tauri/server/Cargo.toml --locked", "bun e2e/headless-smoke.ts",
    ],
  };
  for (const [task, expected] of Object.entries(commands)) {
    const source = job(task === "frontend" ? "frontend" : "checks");
    const steps = source.split(/(?=^      - name: )/m);
    for (const command of expected) {
      const found = steps.find((source) => source.split("\n").some((line) => line.trim() === `run: ${command}` || line.trim() === command));
      assert.ok(found, `Missing command: ${command}`);
      assert.equal(source.split("\n").filter((line) => line.trim() === `run: ${command}` || line.trim() === command).length, 1, `Duplicate command: ${command}`);
      assert.ok(task === "frontend" || found.includes(`matrix.task == '${task}'`), `Wrong task for ${command}`);
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
  assert.doesNotMatch(job("changes"), /actions\/checkout|git diff|fetch-depth/);
  assert.match(job("changes"), /actions\/github-script@[a-f0-9]{40}/);
  assert.ok(step("Install frontend dependencies").includes("matrix.task == 'frontend-e2e' || matrix.task == 'server'"));
  assert.ok(job("frontend").indexOf("- name: Test CI orchestration") < job("frontend").indexOf("- name: Install frontend dependencies"));
  assert.ok(step("Setup Rust").includes("matrix.workspace"));
  assert.ok(step("Check documentation").includes("matrix.task == 'docs'"));
  for (const name of ["Install Tauri Linux dependencies", "Install GPUI build dependencies"]) {
    assert.match(step(name), /if ! pkg-config --exists/);
  }
  assert.ok(step("Install Tauri Linux dependencies").includes("matrix.task == 'rust'"));
  assert.ok(step("Install GPUI build dependencies").includes("matrix.task == 'gpui-clippy' || matrix.task == 'gpui-test'"));
});
function gate(overrides = {}) {
  const env = { ...process.env, CHANGES_RESULT: "success", RELEASE_PR: "false", CHECKS_RESULT: "success", FRONTEND_RESULT: "success" };
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
test("pull requests skip Nix; only same-repository labeled release PRs skip the matrix", () => {
  assert.ok(job("nix").includes("github.event_name != 'pull_request' && needs.changes.outputs.nix == 'true'"));
  assert.ok(job("check").includes("NIX_REQUIRED: ${{ github.event_name != 'pull_request' && needs.changes.outputs.nix == 'true' }}"));
  assert.equal(gate({ NIX_REQUIRED: "false", NIX_RESULT: "skipped" }), 0);
  const release = { RELEASE_PR: "true", FRONTEND_RESULT: "skipped", CHECKS_RESULT: "skipped", NIX_REQUIRED: "false", NIX_RESULT: "skipped" };
  assert.equal(gate(release), 0);
  for (const name of ["FRONTEND_RESULT", "CHECKS_RESULT", "NIX_RESULT"]) {
    for (const result of ["failure", "cancelled", "success"]) assert.notEqual(gate({ ...release, [name]: result }), 0);
  }
  for (const condition of ["github.event.pull_request.head.repo.full_name != github.repository", "!startsWith(github.head_ref, 'release/app-v')", "!contains(github.event.pull_request.labels.*.name, 'release')"]) {
    assert.ok(job("checks").includes(condition));
    assert.ok(job("frontend").includes(condition));
  }
});
test("Rust verification selects all workspace members despite root-only default-members", () => {
  const { scripts } = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
  const release = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");
  const hook = readFileSync(new URL("../.githooks/pre-commit-user", import.meta.url), "utf8");
  const commands = [
    ...["rust:test", "rust:check", "rust:lint"].map((name) => scripts[name]),
    ...[job("checks"), release].flatMap((source) => [...source.matchAll(/run: ((?:cargo|mbx) (?:test|clippy) [^\n]*--manifest-path src-tauri\/Cargo.toml[^\n]*)/g)].map((match) => match[1])),
    ...[...hook.matchAll(/^\s*(cargo check [^\n]*--manifest-path src-tauri\/Cargo.toml[^\n]*)/gm)].map((match) => match[1]),
  ];
  // The GUI-free backend command was already required in the former server job.
  assert.equal(commands.length, 9);
  for (const command of commands) {
    const args = command.trim().split(/\s+/);
    const cargoArgs = args.slice(0, args.indexOf("--") === -1 ? args.length : args.indexOf("--"));
    assert.ok(cargoArgs.includes("--workspace"), command);
    assert.equal(cargoArgs[cargoArgs.indexOf("--manifest-path") + 1], "src-tauri/Cargo.toml", command);
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
  assert.deepEqual(audits, [["src-tauri", ["RUSTSEC-2024-0429"]], ["src-tauri/gpui", []], ["src-tauri/server", ["RUSTSEC-2023-0071", "RUSTSEC-2026-0387"]]]);
  assert.match(step("Audit server Rust dependencies"), /public-signature verification/);
});
test("server packaging follows the relocated kubeconform source without changing its standalone build", () => {
  const packaging = readFileSync(new URL("../.github/workflows/server-packaging.yml", import.meta.url), "utf8");
  assert.match(packaging, /^      - src-tauri\/crates\/inspection\/src\/commands\/resources\/kubeconform\.rs\r?$/m);
  assert.doesNotMatch(packaging, /src-tauri\/src\/commands\/resources\/kubeconform\.rs/);
  assert.match(packaging, /cargo build --locked --release --manifest-path src-tauri\/server\/Cargo.toml --target "\$TARGET"/);
  assert.doesNotMatch(packaging, /--workspace|--all-features/);
  const dockerIgnore = readFileSync(new URL("../.dockerignore", import.meta.url), "utf8");
  assert.match(dockerIgnore, /^!src-tauri\/crates\/\r?$/m, "Docker must reopen the crate directory");
  assert.match(dockerIgnore, /^!src-tauri\/crates\/\*\*\r?$/m, "Docker must include crate sources and manifests");
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
    rust: "src-tauri", "gpui-clippy": "src-tauri/gpui", "gpui-test": "src-tauri/gpui",
    "server-backend": "src-tauri", server: "src-tauri/server",
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
  const run = (overrides, pathspecs = ["workflow.yml", "src-tauri", ":!src-tauri/gpui"]) => spawnSync(
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
    write("workflow.yml", "one\n"); write("src-tauri/src/lib.rs", "one\n"); write("src-tauri/gpui/src/main.rs", "one\n"); write("README.md", "one\n");
    // The real checkout always tracks the two scripts the key includes, so the fixture does too.
    write("scripts/ci-rust-cache-key.mjs", "one\n"); write("scripts/ci-mbx-cargo.mjs", "one\n");
    const first = key();
    assert.deepEqual(key(), first);
    // The cleanup finds the caches it may delete by this key shape.
    assert.equal(cacheName(first.key), "ci-rust");
    write("src-tauri/gpui/src/main.rs", "two\n"); write("README.md", "two\n");
    assert.deepEqual(key(), first);
    write("src-tauri/src/lib.rs", "two\n");
    const source = key();
    assert.notEqual(source.key, first.key);
    assert.equal(source.prefix, first.prefix);
    write("workflow.yml", "two\n");
    assert.notEqual(key().key, source.key);
    assert.deepEqual(key({ ImageVersion: "2" }), key());
    assert.notEqual(key({ RUSTC_VERSION: "2" }).prefix, first.prefix);
    assert.notEqual(run({}, ["missing"]).status, 0);
    assert.notEqual(run({}, ["workflow.yml", "src-tauri", "missing"]).status, 0);
    assert.equal(run({}, ["workflow.yml", "src-tauri", ":!missing"]).status, 0);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
test("whitespace check rejects bad diffs even without an available base", () => {
  const root = mkdtempSync(join(tmpdir(), "kubecove-ci-whitespace-"));
  const env = { ...process.env, GIT_CONFIG_NOSYSTEM: "1", GIT_CONFIG_GLOBAL: "/dev/null", GIT_AUTHOR_NAME: "Test", GIT_AUTHOR_EMAIL: "test@example.invalid", GIT_COMMITTER_NAME: "Test", GIT_COMMITTER_EMAIL: "test@example.invalid" };
  const git = (...args) => {
    const result = spawnSync("git", args, { cwd: root, env, encoding: "utf8", timeout: 2000 });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  };
  try {
    git("init", "-q");
    writeFileSync(join(root, "README.md"), "clean\n");
    git("add", "README.md"); git("commit", "-qm", "clean");
    const clean = git("rev-parse", "HEAD");
    writeFileSync(join(root, "README.md"), "bad trailing whitespace  \n");
    git("add", "README.md"); git("commit", "-qm", "bad");
    const bad = git("rev-parse", "HEAD");
    const script = block(step("Check diff whitespace"), "run", 8);
    for (const from of [clean, "", "0".repeat(40), "d".repeat(40)]) {
      for (const [to, succeeds] of [[clean, true], [bad, false]]) {
        const result = spawnSync("bash", ["-e", "-o", "pipefail", "-c", script], { cwd: root, env: { ...env, BASE_SHA: from, HEAD_SHA: to }, encoding: "utf8", timeout: 2000 });
        assert.equal(result.status === 0, succeeds, `base=${from}, head=${to}: ${result.stderr}`);
      }
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});
// Failure modes: a matrix that misses a PARITY_SUITE_SHARD index silently drops that share of
// the parity suite; a job that runs on the other event runs the suite twice or never; a
// scenario that the shards exclude but no separate job runs never runs; a shard that fails
// without uploading its artifacts leaves nothing to inspect.
test("parity jobs run the full suite on pull requests and on main, and keep the artifacts", () => {
  const read = (name) => readFileSync(new URL(`../.github/workflows/${name}`, import.meta.url), "utf8").replaceAll("\r\n", "\n");
  const parity = read("parity.yml");
  const suite = read("parity-suite.yml");
  assert.deepEqual([...parity.slice(parity.indexOf("\njobs:\n")).matchAll(/^ {2}([\w-]+):$/gm)].map((match) => match[1]), ["pull-request-web", "pull-request-spike", "pull-request-gpui", "pull-request-result", "main", "main-spike", "main-build-cache", "coverage-map"]);
  // The stable check fails on any shard or spike failure, cancellation or unexpected skip.
  const result = job("pull-request-result", parity);
  assert.match(result, /^ {4}name: Parity result$/m);
  assert.match(result, /^ {4}needs:\n {6}- pull-request-web\n {6}- pull-request-gpui\n {6}- pull-request-spike$/m);
  assert.ok(result.includes("if: ${{ always() && github.event_name == 'pull_request' }}"));
  const verify = (env) => spawnSync("bash", ["-e", "-o", "pipefail", "-c", block(result, "run", 8)], { env: { ...process.env, RELEASE_PR: "false", WEB_RESULT: "success", GPUI_RESULT: "success", SPIKE_RESULT: "success", ...env }, encoding: "utf8", timeout: 2000 }).status;
  assert.equal(verify({}), 0);
  for (const outcome of ["failure", "cancelled", "skipped", ""]) {
    assert.notEqual(verify({ WEB_RESULT: outcome }), 0);
    assert.notEqual(verify({ GPUI_RESULT: outcome }), 0);
    assert.notEqual(verify({ SPIKE_RESULT: outcome }), 0);
  }
  assert.equal(verify({ WEB_RESULT: "skipped", GPUI_RESULT: "skipped", SPIKE_RESULT: "skipped", RELEASE_PR: "true" }), 0);
  assert.notEqual(verify({ WEB_RESULT: "success", GPUI_RESULT: "skipped", SPIKE_RESULT: "skipped", RELEASE_PR: "true" }), 0);
  // Each sharded job leaves out exactly the scenarios its spike job runs, and that ID exists.
  const ids = readdirSync(new URL("../e2e/parity/scenarios/", import.meta.url)).filter((name) => name.endsWith(".json"))
    .flatMap((name) => JSON.parse(readFileSync(new URL(`../e2e/parity/scenarios/${name}`, import.meta.url), "utf8")).scenarios.map((scenario) => scenario.id));
  for (const [name, spike] of [["pull-request-web", "pull-request-spike"], ["pull-request-gpui", "pull-request-spike"], ["main", "main-spike"]]) {
    const text = job(name, parity);
    const matrices = [...text.matchAll(/^ {8}shard: \[([\d, ]+)\]$/gm)].map((match) => match[1].split(",").map(Number));
    const count = Number(/^ {6}shards: (\d+)$/m.exec(text)?.[1]);
    assert.equal(matrices.length, 1, name);
    assert.ok(count > 0, name);
    assert.deepEqual(matrices[0], Array.from({ length: count }, (_, index) => index), name);
    assert.match(text, /^ {4}uses: \.\/\.github\/workflows\/parity-suite\.yml$/m, name);
    assert.match(text, /^ {6}shard: \$\{\{ matrix\.shard \}\}$/m, name);
    assert.match(text, /^ {6}fail-fast: false$/m, name);
    assert.doesNotMatch(text, /^ {6}prefix:/m, name);
    const excluded = /^ {6}exclude: (\S+)$/m.exec(text)?.[1];
    const spikeText = job(spike, parity);
    assert.match(spikeText, /^ {6}shard: 0\n {6}shards: 1$/m, spike);
    assert.equal(/^ {6}prefix: (\S+)$/m.exec(spikeText)?.[1], excluded, spike);
    assert.equal(/if: (.+)$/m.exec(spikeText)?.[1], /if: (.+)$/m.exec(text)?.[1], `${spike} runs on the same events as ${name}`);
    assert.equal(/^ {6}coverage: (\w+)$/m.exec(spikeText)?.[1], /^ {6}coverage: (\w+)$/m.exec(text)?.[1], spike);
    assert.deepEqual(ids.filter((id) => id.startsWith(excluded)), ["settings-diagnostics.topology-spike-harness"]);
  }
  const pullRequest = job("pull-request-web", parity);
  assert.ok(pullRequest.includes("if: ${{ github.event_name == 'pull_request' && ("), "pull request job runs only on pull requests");
  for (const condition of ["github.event.pull_request.head.repo.full_name != github.repository", "!startsWith(github.head_ref, 'release/app-v')", "!contains(github.event.pull_request.labels.*.name, 'release')"]) {
    assert.ok(pullRequest.includes(condition), condition);
  }
  assert.ok(job("main", parity).includes("if: ${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}"));
  assert.match(suite, /^ {10}PARITY_SUITE_SHARD: \$\{\{ inputs\.shard \}\}\/\$\{\{ inputs\.shards \}\}$/m);
  assert.match(suite, /^ {10}PARITY_PREFIX: \$\{\{ inputs\.prefix \}\}$/m);
  assert.match(suite, /^ {10}PARITY_EXCLUDE: \$\{\{ inputs\.exclude \}\}$/m);
  assert.match(suite, /^ {10}PARITY_STEP_SCREENSHOTS: "0"$/m);
  // Pull requests run the full suite: change selection fell back to it on every recent PR.
  assert.doesNotMatch(suite, /^ +PARITY_CHANGED:|parity-coverage-map-|fetch-depth/m);
  const run = suite.slice(suite.indexOf("- name: Run parity suite"), suite.indexOf("- name: Upload parity artifacts"));
  const command = block(run, "run", 8);
  const runners = (cpus, driver) => spawnSync("bash", ["-c", `nproc() { echo ${cpus}; }; ${command.replace("bun run e2e:parity", "printenv PARITY_JOBS PARITY_WEB_JOBS")}`], { env: { ...process.env, PARITY_DRIVER: driver }, encoding: "utf8" }).stdout.trim().replaceAll("\n", " ");
  for (const driver of ["web", "gpui"]) assert.equal(runners(4, driver), "4 4");
  assert.equal(runners(4, ""), "1 3");
  assert.equal(runners(1, ""), "1 1");
  assert.match(suite, /^ {10}PARITY_DRIVER: \$\{\{ inputs\.driver \}\}$/m);
  // Every attempt's artifacts: "Re-run failed jobs" leaves the passed shards' reports in the
  // earlier attempt, and merge-reports.ts keeps the newest attempt of each shard.
  assert.match(result, /^ {10}pattern: parity-pull_request-\*$/m);
  assert.match(result, /run: bun e2e\/parity\/merge-reports\.ts parity-reports/);
  for (const name of ["Checkout", "Setup Bun", "Download driver reports", "Validate merged driver reports"]) {
    assert.ok(result.includes(`- name: ${name}\n        if: \${{ needs.pull-request-web.result == 'success' }}`));
  }
  assert.match(job("pull-request-spike", parity), /^ {8}driver: \[web, gpui\]$/m);
  assert.match(job("pull-request-web", parity), /^ {6}driver: web$/m);
  assert.match(job("pull-request-gpui", parity), /^ {6}driver: gpui$/m);
  // A shard count that differs from its matrix leaves scenarios out or runs a job twice.
  for (const [name, count] of [["pull-request-web", 4], ["pull-request-gpui", 3]]) {
    assert.equal(Number(/^ {6}shards: (\d+)$/m.exec(job(name, parity))?.[1]), count);
    assert.match(job(name, parity), new RegExp(`^ {8}shard: \\[${Array.from({ length: count }, (_, index) => index).join(", ")}\\]$`, "m"));
  }
  for (const name of ["Install GPUI build dependencies", "Setup Rust", "Key the Rust cache by its inputs", "Cache Rust builds", "Activate Cargo caching"]) {
    const start = suite.indexOf(`- name: ${name}\n`);
    const end = suite.indexOf("\n      - name: ", start + 1);
    assert.match(suite.slice(start, end), /if: \$\{\{ inputs\.driver != 'web' \}\}/);
  }
  const chromium = suite.slice(suite.indexOf("- name: Install Chromium"), suite.indexOf("- name: Build parity test"));
  assert.match(chromium, /if: \$\{\{ !inputs\.build-only && inputs\.driver != 'gpui' \}\}/);
  assert.match(suite, /sudo apt-get install -y libfontconfig-dev libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev pkg-config/);
  assert.match(suite, /run: bunx playwright-core install --with-deps --no-shell chromium/);
  const upload = suite.slice(suite.indexOf("- name: Upload parity artifacts"));
  assert.match(upload, /^ {8}if: \$\{\{ always\(\) && !inputs\.build-only \}\}$/m);
  assert.match(upload, /^ {10}path: e2e\/artifacts\/parity-\*$/m);
  assert.match(upload, /^ {10}name: .*\$\{\{ inputs\.shard \}\}.*\$\{\{ inputs\.shards \}\}.*\$\{\{ github\.run_attempt \}\}$/m);
});

// Failure modes: a main run without PARITY_COVERAGE records no map; the map leaves out the
// spike job's scenario; a path or artifact name that differs between the jobs leaves local
// PARITY_CHANGED runs without a map; the instrumented build saved under the pull request cache
// key makes every pull request rebuild from scratch.
test("main records the parity coverage map from every parity job", () => {
  const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8").replaceAll("\r\n", "\n");
  const parity = read(".github/workflows/parity.yml");
  const suite = read(".github/workflows/parity-suite.yml");
  const expression = (text, key) => {
    const match = new RegExp(`^ +${key}: \\$\\{\\{ (.+) \\}\\}$`, "m").exec(text);
    assert.ok(match, `Missing ${key} expression`);
    return match[1];
  };
  const step = (name) => {
    const start = suite.indexOf(`- name: ${name}\n`);
    assert.notEqual(start, -1, `Missing step: ${name}`);
    const end = suite.indexOf("\n      - name: ", start + 1);
    return suite.slice(start, end < 0 ? undefined : end);
  };
  // GitHub expressions here use only operators that JavaScript evaluates the same way.
  const evaluate = (source, inputs, event) => {
    const github = { event_name: event, ref: event === "pull_request" ? "refs/pull/7/merge" : "refs/heads/main" };
    return new Function("inputs", "github", "always", "cancelled", `return (${source.replaceAll("build-only", "buildOnly").replaceAll(/'([^']*)'/g, "\"$1\"")});`)(inputs, github, () => true, () => false);
  };
  const modes = {
    pullRequest: [{ coverage: false, buildOnly: false }, "pull_request"],
    main: [{ coverage: true, buildOnly: false }, "push"],
    buildCache: [{ coverage: false, buildOnly: true }, "push"],
  };
  const run = step("Run parity suite");
  const results = Object.fromEntries(Object.entries(modes).map(([mode, [inputs, event]]) => [mode, {
    coverage: evaluate(expression(run, "PARITY_COVERAGE"), inputs, event),
    cacheKey: `parity${evaluate(/run: node scripts\/ci-rust-cache-key\.mjs parity\$\{\{ (.+) \}\} \.github\/workflows\/parity-suite\.yml src-tauri ':!src-tauri\/server' 'src\/lib\/\*-workspace\.json'$/m.exec(step("Key the Rust cache by its inputs"))?.[1] ?? "'missing'", inputs, event)}`,
    runs: evaluate(expression(run, "if"), inputs, event),
  }]));
  assert.deepEqual(results.pullRequest, { coverage: "0", cacheKey: "parity", runs: true });
  assert.deepEqual(results.main, { coverage: "1", cacheKey: "parity-coverage", runs: true });
  assert.equal(results.buildCache.runs, false);
  assert.equal(results.buildCache.cacheKey, "parity");
  assert.match(step("Build parity test"), /cargo test --manifest-path src-tauri\/gpui\/Cargo\.toml --test parity --no-run/);
  // A cached target directory does not survive a fresh checkout (ci.yml has the reason), and
  // one pull request job saving is enough: every GPUI job builds the same test binary.
  const cache = step("Cache Rust builds");
  assert.match(cache, /uses: jdx\/mr-boxington-action@[a-f0-9]{40} # v1\n/);
  for (const line of ['version: "1.30.0"', "working-directory: src-tauri/gpui", "cache-key: ${{ steps.rust-cache.outputs.key }}", "restore-keys: ${{ steps.rust-cache.outputs.prefix }}", "github-cache-mode: objects", "isolate-objects-cache: true", "clean-target-before-export: true"]) {
    assert.ok(cache.includes(`\n          ${line}\n`) || cache.endsWith(`\n          ${line}`), `Missing cache setting: ${line}`);
  }
  // Failure mode: the main shards and the main spike job build the same instrumented binary
  // under one cache key, and a second saver fails its job when GitHub has reserved the key
  // for the first. Exactly one of those jobs may save.
  // GitHub's `cache-mode` on the calling job decides it. A step cannot: the runner sets
  // ACTIONS_CACHE_MODE from the job's cache-mode and ignores a step's value (run 43407667697).
  const readOnly = (name) => /^ {4}cache-mode: read$/m.test(job(name, parity));
  assert.deepEqual(["main", "main-spike", "main-build-cache"].map(readOnly), [true, false, false]);
  // Each saver writes its own cache: the spike job the instrumented build, the build cache
  // job the uninstrumented one. A second `coverage: true` saver would bring the conflict back
  // and leave pull requests without a cache.
  assert.match(job("main-spike", parity), /^ {6}coverage: true$/m);
  assert.match(job("main-build-cache", parity), /^ {6}build-only: true$/m);
  assert.doesNotMatch(job("main-build-cache", parity), /coverage:/);
  // Pull request jobs never save, and with the same access they exercise the main shards' path.
  assert.deepEqual(["pull-request-web", "pull-request-gpui", "pull-request-spike"].map(readOnly), [true, true, true]);
  // Those four jobs are the only place that sets the mode. One at the top of either workflow
  // would take the two savers' access as well.
  assert.equal(parity.match(/^ *cache-mode:/gm)?.length, 4);
  assert.doesNotMatch(suite, /^ *cache-mode:/m);
  assert.doesNotMatch(`${suite}${parity}`, /ACTIONS_CACHE_MODE: |save-cache|^ +cache-mode: (?!read$)/m);
  assert.match(step("Activate Cargo caching"), /run: node scripts\/ci-mbx-cargo\.mjs/);
  assert.doesNotMatch(suite, /swatinem\/rust-cache/i);
  assert.match(step("Install LLVM tools"), /if: \$\{\{ inputs\.coverage \}\}\n {8}run: rustup component add llvm-tools/);
  // The jobs that call the suite on main pass the inputs these modes assume.
  assert.match(job("main", parity), /^ {6}coverage: true$/m);
  assert.doesNotMatch(job("pull-request-web", parity), /coverage:|build-only:/);
  const buildCache = job("main-build-cache", parity);
  assert.match(buildCache, /^ {6}build-only: true$/m);
  assert.ok(buildCache.includes("if: ${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}"));
  // One map path from the shard and spike uploads through the merge to the uploaded artifact.
  const merge = job("coverage-map", parity);
  const mapPath = /^export const DEFAULT_MAP = "(.+)";$/m.exec(read("e2e/parity/changed-scenarios.ts"))?.[1];
  assert.equal(mapPath, "e2e/artifacts/parity-coverage-map.json");
  assert.match(merge, /^ {4}needs:\n {6}- main\n {6}- main-spike$/m);
  assert.ok(merge.includes("if: ${{ !cancelled() && github.event_name == 'push' && github.ref == 'refs/heads/main' }}"));
  const shardArtifact = /name: (parity-coverage-shard-)\$\{\{/.exec(step("Upload shard coverage map"))?.[1];
  assert.ok(shardArtifact && merge.includes(`pattern: ${shardArtifact}*`), "merge downloads the shard maps the suite uploads");
  assert.match(step("Upload shard coverage map"), /path: e2e\/artifacts\/parity-\*\/coverage-map\.json/);
  assert.match(step("Upload shard coverage map"), /\$\{\{ inputs\.shard \}\}-of-\$\{\{ inputs\.shards \}\}/);
  assert.ok(merge.includes(`run: bun e2e/parity/coverage-map.ts merge ${mapPath} shard-maps/*/*/coverage-map.json`));
  const uploaded = merge.slice(merge.indexOf("- name: Upload coverage map"));
  assert.ok(uploaded.includes(`path: ${mapPath}`) && uploaded.includes("name: parity-coverage-map\n"));
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
