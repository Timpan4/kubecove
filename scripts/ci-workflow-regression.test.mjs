import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const workflow = readFileSync(process.env.CI_WORKFLOW_PATH ?? new URL("../.github/workflows/ci.yml", import.meta.url), "utf8");
const base = "a".repeat(40);
const head = "b".repeat(40);
const domains = ["docs", "frontend", "nix", "rust"];
const flags = (...selected) => Object.fromEntries(domains.map((key) => [key, String(selected.includes(key))]));
const all = flags(...domains);

// Extract the workflow's fixed-indentation blocks; execute its actual code.
function job(name) {
  const lines = workflow.split("\n");
  const start = lines.indexOf(`  ${name}:`);
  assert.notEqual(start, -1, `Missing job: ${name}`);
  let end = start + 1;
  while (end < lines.length && !/^  [\w-]+:$/.test(lines[end])) end++;
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
      files: paths.map((path) => typeof path === "string" ? { filename: path, status: "modified" } : path),
      ...options.data,
    } };
  } };
  const core = { setOutput: (key, value) => { output[key] = value; }, warning: (message) => warnings.push(message) };
  const AsyncFunction = Object.getPrototypeOf(async () => {}).constructor;
  await new AsyncFunction("github", "context", "core", block(job("changes"), "script", 10))(github, context, core);
  return { output, warnings, requests };
}

for (const [path, selected] of [
  ["README.md", ["docs"]],
  ["docs/assets/wiki/screenshot.png", ["docs"]],
  ["src/App.svelte", ["frontend"]],
  ["src/fixtures/example.yaml", ["frontend"]],
  ["public/app-icon.svg", ["frontend"]],
  ["e2e/fixtures/resource.yaml", ["frontend"]],
  ["scripts/check-docs.ts", ["frontend"]],
  ["bun.lock", ["frontend"]],
  ["src-tauri/src/lib.rs", ["rust"]],
  ["src-tauri/capabilities/default.yaml", ["rust"]],
  ["src-tauri/tauri.conf.json", ["frontend", "rust"]],
  ["src-tauri/Cargo.lock", ["nix", "rust"]],
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
  const result = await detect([{ filename: "docs/example.md", previous_filename: "src/example.ts", status: "renamed" }]);
  assert.deepEqual(result.output, flags("docs", "frontend"));
});
test("removed files still select their checks", async () => {
  assert.deepEqual((await detect([{ filename: "src-tauri/src/removed.rs", status: "removed" }])).output, flags("rust"));
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
test("change gate has no checkout; docs install no application dependencies", () => {
  assert.doesNotMatch(job("changes"), /actions\/checkout|git diff|fetch-depth/);
  assert.match(job("changes"), /actions\/github-script@[a-f0-9]{40}/);
  assert.doesNotMatch(job("docs"), /bun install|actions\/cache/);
  assert.match(job("docs"), /bun run docs:check/);
  assert.match(job("frontend"), /node --test scripts\/ci-workflow-regression\.test\.mjs/);
  for (const command of ["audit", "docs:check", "typecheck", "svelte:check", "lint", "lint:anti-slop", "e2e:fast"]) {
    assert.ok(job("frontend").includes(`run: bun run ${command}\n`), command);
  }
  assert.match(job("frontend"), /run: bun test/);
});

function gate(overrides = {}) {
  const env = { ...process.env, CHANGES_RESULT: "success", RELEASE_PR: "false" };
  for (const key of domains) { env[`${key.toUpperCase()}_REQUIRED`] = "true"; env[`${key.toUpperCase()}_RESULT`] = "success"; }
  return spawnSync("bash", ["-e", "-o", "pipefail", "-c", block(job("check"), "run", 8)], {
    env: { ...env, ...overrides }, encoding: "utf8", timeout: 2000,
  }).status;
}
test("stable required check succeeds only after successful change detection", () => {
  assert.match(job("check"), /name: Typecheck and test/);
  assert.match(job("check"), /if: always\(\)/);
  assert.equal(gate(), 0);
  for (const result of ["failure", "cancelled", "skipped"]) assert.notEqual(gate({ CHANGES_RESULT: result }), 0);
});
for (const domain of domains) {
  test(`required gate rejects failed, cancelled, skipped or unknown ${domain} results`, () => {
    const key = domain.toUpperCase();
    for (const result of ["failure", "cancelled", "skipped", ""]) assert.notEqual(gate({ [`${key}_RESULT`]: result }), 0);
    assert.equal(gate({ [`${key}_REQUIRED`]: "false", [`${key}_RESULT`]: "skipped" }), 0);
    assert.notEqual(gate({ [`${key}_REQUIRED`]: "false" }), 0);
    assert.notEqual(gate({ [`${key}_REQUIRED`]: "", [`${key}_RESULT`]: "skipped" }), 0);
  });
}
test("existing release PR exception still requires Nix success", () => {
  const release = { RELEASE_PR: "true", DOCS_RESULT: "skipped", FRONTEND_RESULT: "skipped", RUST_RESULT: "skipped" };
  assert.equal(gate(release), 0);
  for (const result of ["failure", "cancelled", "skipped"]) assert.notEqual(gate({ ...release, NIX_RESULT: result }), 0);
  assert.notEqual(gate({ ...release, FRONTEND_RESULT: "success" }), 0);
  assert.notEqual(gate({ ...release, RELEASE_PR: "false" }), 0);
  for (const name of ["docs", "frontend", "rust"]) {
    assert.ok(job(name).includes("github.event.pull_request.head.repo.full_name != github.repository"));
    assert.ok(job(name).includes("!startsWith(github.head_ref, 'release/app-v')"));
    assert.ok(job(name).includes("!contains(github.event.pull_request.labels.*.name, 'release')"));
  }
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
    const script = block(job("docs"), "run", 8);
    for (const from of [clean, "", "0".repeat(40), "d".repeat(40)]) {
      for (const [to, succeeds] of [[clean, true], [bad, false]]) {
        const result = spawnSync("bash", ["-e", "-o", "pipefail", "-c", script], { cwd: root, env: { ...env, BASE_SHA: from, HEAD_SHA: to }, encoding: "utf8", timeout: 2000 });
        assert.equal(result.status === 0, succeeds, `base=${from}, head=${to}: ${result.stderr}`);
      }
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});
