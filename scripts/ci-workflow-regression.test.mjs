import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";

const workflow = readFileSync(process.env.CI_WORKFLOW_PATH ?? new URL("../.github/workflows/ci.yml", import.meta.url), "utf8").replaceAll("\r\n", "\n");
const base = "a".repeat(40);
const head = "b".repeat(40);
const domains = ["docs", "frontend", "nix", "rust"];
const flags = (...selected) => Object.fromEntries(domains.map((key) => [key, String(selected.includes(key))]));
const all = flags(...domains);

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
  // The GPUI checks run exactly when the Rust checks do.
  const gpui = overrides.RUST_REQUIRED === "false" ? "skipped" : "success";
  const env = { ...process.env, CHANGES_RESULT: "success", RELEASE_PR: "false", SERVER_RESULT: "success", GPUI_RESULT: gpui };
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
// Failure mode: GPUI is a separate Cargo workspace, so a Rust change that breaks its lints or
// native tests passes every other job.
test("Rust changes require the GPUI checks, which lint and test the GPUI workspace", () => {
  for (const result of ["failure", "cancelled", "skipped", ""]) assert.notEqual(gate({ GPUI_RESULT: result }), 0);
  assert.equal(gate({ RUST_REQUIRED: "false", RUST_RESULT: "skipped", GPUI_RESULT: "skipped" }), 0);
  assert.notEqual(gate({ RUST_REQUIRED: "false", RUST_RESULT: "skipped", GPUI_RESULT: "success" }), 0);
  assert.match(job("gpui"), /needs\.changes\.outputs\.rust == 'true'/);
  assert.match(job("gpui"), /run: mbx clippy --manifest-path src-tauri\/gpui\/Cargo\.toml --locked --all-targets -- -D warnings/);
  assert.match(job("gpui"), /run: mbx test --manifest-path src-tauri\/gpui\/Cargo\.toml --locked --lib --test native/);
});
test("pull requests skip Nix without failing the required gate", () => {
  const condition = "github.event_name != 'pull_request' && needs.changes.outputs.nix == 'true'";
  assert.ok(job("nix").includes(`if: \${{ ${condition} }}`));
  assert.ok(job("check").includes(`NIX_REQUIRED: \${{ ${condition} }}`));
  assert.equal(gate({ NIX_REQUIRED: "false", NIX_RESULT: "skipped" }), 0);
});
test("release PRs skip Nix along with the other expensive checks", () => {
  const release = { RELEASE_PR: "true", DOCS_RESULT: "skipped", FRONTEND_RESULT: "skipped", GPUI_RESULT: "skipped", NIX_REQUIRED: "false", NIX_RESULT: "skipped", RUST_RESULT: "skipped", SERVER_RESULT: "skipped" };
  assert.equal(gate(release), 0);
  for (const result of ["failure", "cancelled", "success"]) assert.notEqual(gate({ ...release, NIX_RESULT: result }), 0);
  assert.notEqual(gate({ ...release, FRONTEND_RESULT: "success" }), 0);
  assert.notEqual(gate({ ...release, GPUI_RESULT: "success" }), 0);
  assert.notEqual(gate({ ...release, RELEASE_PR: "false" }), 0);
  for (const name of ["docs", "frontend", "gpui", "rust"]) {
    assert.ok(job(name).includes("github.event.pull_request.head.repo.full_name != github.repository"));
    assert.ok(job(name).includes("!startsWith(github.head_ref, 'release/app-v')"));
    assert.ok(job(name).includes("!contains(github.event.pull_request.labels.*.name, 'release')"));
  }
});
test("Rust verification selects all workspace members despite root-only default-members", () => {
  const { scripts } = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
  const release = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");
  const hook = readFileSync(new URL("../.githooks/pre-commit-user", import.meta.url), "utf8");
  const commands = [
    ...["rust:test", "rust:check", "rust:lint"].map((name) => scripts[name]),
    ...[job("rust"), release].flatMap((source) => [...source.matchAll(/run: ((?:cargo|mbx) (?:test|clippy) [^\n]+)/g)].map((match) => match[1])),
    ...[...hook.matchAll(/^\s*(cargo check [^\n]*--manifest-path src-tauri\/Cargo.toml[^\n]*)/gm)].map((match) => match[1]),
  ];
  assert.equal(commands.length, 8, "package, CI, release and hook verification commands must be covered");
  for (const command of commands) {
    const args = command.trim().split(/\s+/);
    const cargoArgs = args.slice(0, args.indexOf("--") === -1 ? args.length : args.indexOf("--"));
    assert.ok(cargoArgs.includes("--workspace"), command);
    assert.equal(cargoArgs[cargoArgs.indexOf("--manifest-path") + 1], "src-tauri/Cargo.toml", command);
  }
});
test("Rust dependency audits cover every checked-in lockfile with narrow exceptions", () => {
  const trackedFiles = spawnSync("git", ["ls-files", "-z"], {
    cwd: new URL("../", import.meta.url),
    encoding: "utf8",
  });
  assert.equal(trackedFiles.status, 0, trackedFiles.stderr);
  const lockfileDirectories = trackedFiles.stdout
    .split("\0")
    .filter((path) => path === "Cargo.lock" || path.endsWith("/Cargo.lock"))
    .map(dirname)
    .sort();

  const rustJob = job("rust");
  assert.match(
    rustJob,
    /run: node --test --test-name-pattern="Rust dependency audits cover every checked-in lockfile with narrow exceptions" scripts\/ci-workflow-regression\.test\.mjs/,
  );
  assert.doesNotMatch(rustJob, /uses: rustsec\/audit-check@/,"audit-check compiles cargo-audit and fails on check-run API errors");
  assert.match(rustJob, /uses: taiki-e\/install-action@[a-f0-9]{40} # v[\d.]+\n {8}with:\n {10}tool: cargo-audit@\d+\.\d+\.\d+\n/);
  const auditSteps = rustJob.split("\n      - name: ").filter((step) => step.includes("run: cargo-audit audit "));
  const audits = auditSteps
    .map((step) => {
      const command = /run: cargo-audit audit (--file \S+(?: --ignore \S+)*)\n/.exec(`${step}\n`)?.[1];
      assert.ok(command, `Audit step must name only its lockfile and ignored advisories:\n${step}`);
      const directory = dirname(/--file (\S+)/.exec(command)[1]);
      assert.match(command, /--file \S+\/Cargo\.lock/);
      const ignoredAdvisories = [...command.matchAll(/--ignore (\S+)/g)].map((match) => match[1]);
      return { directory, ignoredAdvisories };
    })
    .sort((left, right) => left.directory.localeCompare(right.directory));
  const auditedDirectories = audits.map(({ directory }) => directory);
  assert.deepEqual(auditedDirectories, lockfileDirectories);
  assert.deepEqual(
    audits.map(({ directory, ignoredAdvisories }) => [directory, ignoredAdvisories]),
    [
      ["src-tauri", ["RUSTSEC-2024-0429"]],
      ["src-tauri/gpui", []],
      ["src-tauri/server", ["RUSTSEC-2023-0071", "RUSTSEC-2026-0387"]],
    ],
  );

  const serverAudit = auditSteps.find((step) => step.includes("--file src-tauri/server/Cargo.lock"));
  assert.match(serverAudit, /public-signature verification/);
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
test("headless checks run without GUI packages and the gate requires their success", () => {
  assert.doesNotMatch(job("server"), /apt-get|cache-apt-pkgs|pkg-config|--all-features/);
  assert.match(job("server"), /mbx test --locked --workspace --no-default-features --manifest-path src-tauri\/Cargo.toml/);
  assert.match(job("server"), /cargo tree --manifest-path src-tauri\/server\/Cargo.toml --locked --edges normal,build/);
  assert.match(job("server"), /mbx test --manifest-path src-tauri\/server\/Cargo.toml --locked --no-default-features/);
  assert.match(job("server"), /mbx test --manifest-path src-tauri\/server\/Cargo.toml --locked/);
  for (const result of ["failure", "cancelled", "skipped", ""]) assert.notEqual(gate({ SERVER_RESULT: result }), 0);
  assert.equal(gate({ FRONTEND_REQUIRED: "false", FRONTEND_RESULT: "skipped", RUST_REQUIRED: "false", RUST_RESULT: "skipped", SERVER_RESULT: "skipped" }), 0);
});
// Missing workspace paths produce empty caches; shared keys can restore the wrong target tree.
// Running Cargo directly bypasses mbx; save opt-ins would expand the trusted write policy.
test("Mr Boxington covers CI workspaces with separate caches and default-branch-only writes", () => {
  const generations = new Set();
  for (const [name, directories] of [["rust", ["src-tauri"]], ["server", ["src-tauri", "src-tauri/server"]], ["gpui", ["src-tauri/gpui"]]]) {
    const source = job(name);
    assert.doesNotMatch(source, /swatinem\/rust-cache|save-on-(?:pull-request|protected-branch|workflow-dispatch)/i);
    assert.doesNotMatch(source, /(?:run: |^\s+)cargo (?:build|test|clippy)\b/m);
    const setups = [...source.matchAll(/uses: jdx\/mr-boxington-action@([a-f0-9]{40})[^\n]*\n {8}with:\n {10}version: "([^"]+)"\n {10}working-directory: ([^\n]+)\n {10}cache-generation: ([^\n]+)/g)];
    assert.deepEqual(setups.map((match) => match[3]), directories);
    assert.ok(source.indexOf("dtolnay/rust-toolchain@") < source.indexOf("jdx/mr-boxington-action@"));
    for (const setup of setups) {
      assert.match(setup[2], /^\d+\.\d+\.\d+$/);
      assert.ok(!generations.has(setup[4]), "each target tree needs its own restore namespace");
      generations.add(setup[4]);
    }
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
// Failure modes: a matrix that misses a PARITY_SUITE_SHARD index silently drops that share of
// the parity suite; a job that runs on the other event runs the suite twice or never; a
// scenario that the shards exclude but no separate job runs never runs; a shard that fails
// without uploading its artifacts leaves nothing to inspect.
test("parity jobs run the full suite on pull requests and on main, and keep the artifacts", () => {
  const read = (name) => readFileSync(new URL(`../.github/workflows/${name}`, import.meta.url), "utf8").replaceAll("\r\n", "\n");
  const parity = read("parity.yml");
  const suite = read("parity-suite.yml");
  assert.deepEqual([...parity.slice(parity.indexOf("\njobs:\n")).matchAll(/^ {2}([\w-]+):$/gm)].map((match) => match[1]), ["pull-request", "pull-request-spike", "pull-request-result", "main", "main-spike", "main-build-cache", "coverage-map"]);
  // The stable check fails on any shard or spike failure, cancellation or unexpected skip.
  const result = job("pull-request-result", parity);
  assert.match(result, /^ {4}name: Parity result$/m);
  assert.match(result, /^ {4}needs:\n {6}- pull-request\n {6}- pull-request-spike$/m);
  assert.ok(result.includes("if: ${{ always() && github.event_name == 'pull_request' }}"));
  const verify = (env) => spawnSync("bash", ["-e", "-o", "pipefail", "-c", block(result, "run", 8)], { env: { ...process.env, RELEASE_PR: "false", PARITY_RESULT: "success", SPIKE_RESULT: "success", ...env }, encoding: "utf8", timeout: 2000 }).status;
  assert.equal(verify({}), 0);
  for (const outcome of ["failure", "cancelled", "skipped", ""]) {
    assert.notEqual(verify({ PARITY_RESULT: outcome }), 0);
    assert.notEqual(verify({ SPIKE_RESULT: outcome }), 0);
  }
  assert.equal(verify({ PARITY_RESULT: "skipped", SPIKE_RESULT: "skipped", RELEASE_PR: "true" }), 0);
  assert.notEqual(verify({ PARITY_RESULT: "success", SPIKE_RESULT: "skipped", RELEASE_PR: "true" }), 0);
  // Each sharded job leaves out exactly the scenarios its spike job runs, and that ID exists.
  const ids = readdirSync(new URL("../e2e/parity/scenarios/", import.meta.url)).filter((name) => name.endsWith(".json"))
    .flatMap((name) => JSON.parse(readFileSync(new URL(`../e2e/parity/scenarios/${name}`, import.meta.url), "utf8")).scenarios.map((scenario) => scenario.id));
  for (const [name, spike] of [["pull-request", "pull-request-spike"], ["main", "main-spike"]]) {
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
  const pullRequest = job("pull-request", parity);
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
  // A block scalar: the command's ": " would end a plain YAML scalar and invalidate the file.
  const command = /^ {8}run: \|\n {10}(CARGO_BUILD_JOBS=.* bun run e2e:parity)$/m.exec(suite)?.[1];
  assert.ok(command, "Missing parity run command");
  // One GPUI process and web pages on the other CPUs, at least one each.
  const runners = (cpus) => spawnSync("bash", ["-c", `nproc() { echo ${cpus}; }; ${command.replace("bun run e2e:parity", "printenv PARITY_JOBS PARITY_WEB_JOBS")}`], { encoding: "utf8" }).stdout.trim().replaceAll("\n", " ");
  assert.equal(runners(4), "1 3");
  assert.equal(runners(1), "1 1");
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
    cacheKey: `kubecove-parity${evaluate(/shared-key: kubecove-parity\$\{\{ (.+) \}\}/.exec(step("Cache Rust builds"))?.[1] ?? "''", inputs, event)}`,
    runs: evaluate(expression(run, "if"), inputs, event),
  }]));
  assert.deepEqual(results.pullRequest, { coverage: "0", cacheKey: "kubecove-parity", runs: true });
  assert.deepEqual(results.main, { coverage: "1", cacheKey: "kubecove-parity-coverage", runs: true });
  assert.equal(results.buildCache.runs, false);
  assert.equal(results.buildCache.cacheKey, "kubecove-parity");
  assert.match(step("Build parity test"), /cargo test --manifest-path src-tauri\/gpui\/Cargo\.toml --test parity --no-run/);
  assert.match(step("Cache Rust builds"), /save-if: \$\{\{ github\.ref == 'refs\/heads\/main' && inputs\.shard == 0 \}\}/);
  assert.match(step("Install LLVM tools"), /if: \$\{\{ inputs\.coverage \}\}\n {8}run: rustup component add llvm-tools/);
  // The jobs that call the suite on main pass the inputs these modes assume.
  assert.match(job("main", parity), /^ {6}coverage: true$/m);
  assert.doesNotMatch(job("pull-request", parity), /coverage:|build-only:/);
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
