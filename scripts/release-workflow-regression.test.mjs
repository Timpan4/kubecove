import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { posix, win32 } from "node:path";
import { test } from "node:test";
import { runInNewContext } from "node:vm";
import { parse } from "yaml";
import { buildGpuiManifest, gpuiUpdaterAssets, verifyUpdaterSignature } from "./gpui-release-manifest.mjs";

const workflow = parse(readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8"));
const packaging = parse(readFileSync(new URL("../.github/workflows/server-packaging.yml", import.meta.url), "utf8"));
const gpuiRelease = parse(readFileSync(new URL("../.github/workflows/gpui-release.yml", import.meta.url), "utf8"));
const targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"];

// The nested builders need an executable shim on PATH and unchanged artifact locations.
test("Cargo activation exports the platform shim only after successful setup", () => {
  const code = readFileSync(new URL("./ci-mbx-cargo.mjs", import.meta.url), "utf8").replace(/^import .*;\n/gm, "");
  for (const platform of ["linux", "darwin", "win32"]) {
    const paths = platform === "win32" ? win32 : posix;
    const home = platform === "win32" ? "C:\\Users\\runner" : "/home/runner";
    const env = { GITHUB_PATH: "path-file", GITHUB_ENV: "env-file", LOCALAPPDATA: paths.join(home, "AppData", "Local"), XDG_DATA_HOME: "/custom/data" };
    let data;
    if (platform === "win32") {
      data = env.LOCALAPPDATA;
    } else if (platform === "darwin") {
      data = paths.join(home, "Library", "Application Support");
    } else {
      data = env.XDG_DATA_HOME;
    }
    const shim = paths.join(data, "mbx", "bin");
    const writes = [];
    let installed = false;
    const context = {
      process: { platform, env }, join: paths.join, homedir: () => home, constants: { X_OK: 1 },
      execFileSync: (command, args) => {
        assert.equal(command, "mbx");
        assert.equal(JSON.stringify(args), JSON.stringify(["setup", "--yes"]));
        installed = true;
      },
      accessSync: (path) => { assert.ok(installed); assert.equal(path, paths.join(shim, platform === "win32" ? "cargo.exe" : "cargo")); },
      appendFileSync: (...args) => writes.push(args),
    };
    runInNewContext(code, context);
    assert.deepEqual(writes, [["path-file", `${shim}\n`], ["env-file", "MBX_TARGET_VIEWS=0\n"]]);
    writes.length = 0;
    assert.throws(() => runInNewContext(code, { ...context, execFileSync: () => { throw new Error("setup failed"); } }), /setup failed/);
    assert.deepEqual(writes, []);
    assert.throws(() => runInNewContext(code, { ...context, accessSync: () => { throw new Error("missing shim"); } }), /missing shim/);
    assert.deepEqual(writes, []);
  }
});

function job(name, source = workflow) {
  assert.ok(source.jobs[name], `Missing job: ${name}`);
  return source.jobs[name];
}

function step(job, name) {
  const found = job.steps.find((step) => step.name === name);
  assert.ok(found, `Missing step: ${name}`);
  return found;
}

test("release preparation commits desktop and server version metadata", () => {
  const preparation = parse(readFileSync(new URL("../.github/workflows/prepare-release.yml", import.meta.url), "utf8"));
  const commands = step(job("prepare", preparation), "Commit release branch").run;
  const staged = commands.match(/^\s*git add (.+)$/m)[1].split(/\s+/);
  for (const path of ["package.json", "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml",
    "src-tauri/Cargo.lock", "src-tauri/server/Cargo.toml", "src-tauri/server/Cargo.lock",
    "src-tauri/gpui/Cargo.toml", "src-tauri/gpui/Cargo.lock", "CHANGELOG.md"]) {
    assert.ok(staged.includes(path), `Release commit omits ${path}`);
  }
});

test("release preparation does not use manual dispatch for required PR checks", () => {
  const preparation = parse(readFileSync(new URL("../.github/workflows/prepare-release.yml", import.meta.url), "utf8"));
  for (const { run = "" } of job("prepare", preparation).steps) {
    assert.doesNotMatch(run, /gh workflow run (ci|codspeed)\.yml/,
      "workflow_dispatch jobs cannot satisfy PR ruleset checks");
  }
  for (const name of ["ci", "codspeed"]) {
    const checks = parse(readFileSync(new URL(`../.github/workflows/${name}.yml`, import.meta.url), "utf8"));
    assert.ok(Object.hasOwn(checks.on, "pull_request"), `${name} must accept eligible PR events`);
    const activities = checks.on.pull_request?.types;
    for (const activity of ["opened", "synchronize"]) {
      assert.ok(activities === undefined || activities.includes(activity), `${name} must accept PR ${activity}`);
    }
  }
});

test("release creation and synchronization use the approved token with verified API commits", () => {
  const preparation = parse(readFileSync(new URL("../.github/workflows/prepare-release.yml", import.meta.url), "utf8"));
  const prepare = job("prepare", preparation);
  for (const name of ["Verify release automation token", "Commit release branch", "Open release PR"]) {
    assert.match(step(prepare, name).env.GH_TOKEN, /^\$\{\{ secrets\.NIX_LOCK_REFRESH_TOKEN \}\}$/);
  }
  assert.deepEqual(preparation.permissions, { contents: "read" });
  assert.match(step(prepare, "Commit release branch").run, /node scripts\/commit-release-pr\.mjs/);
});

test("release token preflight stops before writes when missing or rejected", () => {
  const preparation = parse(readFileSync(new URL("../.github/workflows/prepare-release.yml", import.meta.url), "utf8"));
  const prepare = job("prepare", preparation);
  const preflight = step(prepare, "Verify release automation token");
  assert.ok(prepare.steps.indexOf(preflight) < prepare.steps.indexOf(step(prepare, "Commit release branch")));
  for (const [token, rejected] of [["", false], ["fixture-auth", true], ["fixture-auth", false]]) {
    const result = spawnSync("bash", ["-c", `
      gh() {
        printf 'API probe %s\\n' "$*"
        if [ "$REJECTED" = true ]; then return 1; fi
      }
      ${preflight.run}
    `], { encoding: "utf8", env: { ...process.env, GH_TOKEN: token, GH_REPO: "owner/repo", REJECTED: String(rejected) } });
    assert.equal(result.status, token && !rejected ? 0 : 1, result.stderr);
    assert.equal(result.stdout.includes("API probe"), Boolean(token));
    assert.ok(!result.stdout.includes("fixture-auth"));
  }
});

test("release preparation creates verified commits without rewriting the branch", () => {
  const preparation = parse(readFileSync(new URL("../.github/workflows/prepare-release.yml", import.meta.url), "utf8"));
  const commands = step(job("prepare", preparation), "Commit release branch").run;
  assert.match(commands, /node scripts\/commit-release-pr\.mjs/);
  assert.doesNotMatch(commands, /git commit|git push|git checkout -B/);
  const script = readFileSync(new URL("./commit-release-pr.mjs", import.meta.url), "utf8")
    .replace(/^import .+;\r?\n/gm, "");
  const base = "a".repeat(40);
  const existing = "b".repeat(40);
  for (const parent of [null, existing]) {
    for (const valid of [true, false]) {
      const calls = [];
      let payload;
      const execute = () => runInNewContext(script, {
        Buffer,
        console: { log() {} },
        process: { env: { GH_REPO: "owner/repo", BRANCH_NAME: "release/app-v1.2.3", RELEASE_TITLE: "release: KubeCove v1.2.3" } },
        readFileSync: (path) => Buffer.from(`${path} contents`),
        spawnSync: (command, args, options) => {
          calls.push([command, ...args]);
          let stdout;
          if (command === "git") stdout = args[0] === "rev-parse" ? base : "package.json\0src-tauri/server/Cargo.toml\0";
          else if (args[1] === "graphql") {
            payload = JSON.parse(options.input);
            stdout = JSON.stringify({ data: { createCommitOnBranch: { commit: { oid: "c".repeat(40), signature: { isValid: valid } } } } });
          } else if (args.includes("--paginate")) stdout = JSON.stringify([{ commits: [{ sha: existing, commit: { verification: { verified: true } } }] }]);
          else stdout = args[1].includes("matching-refs") ? JSON.stringify(parent ? [{ ref: "refs/heads/release/app-v1.2.3", object: { sha: parent } }] : []) : "{}";
          return { status: 0, stdout, stderr: "" };
        },
      });
      if (valid) assert.doesNotThrow(execute);
      else assert.throws(execute, /verified signature/);
      assert.equal(payload.variables.input.expectedHeadOid, parent ?? base);
      assert.deepEqual(payload.variables.input.branch, {
        repositoryNameWithOwner: "owner/repo",
        branchName: "release/app-v1.2.3",
      });
      assert.deepEqual(payload.variables.input.fileChanges.additions.map(({ path, contents }) => [path, Buffer.from(contents, "base64").toString()]),
        [["package.json", "package.json contents"], ["src-tauri/server/Cargo.toml", "src-tauri/server/Cargo.toml contents"]]);
      assert.equal(calls.filter((call) => call.includes("POST")).length, parent ? 0 : 1);
      assert.ok(calls.every((call) => !call.includes("push") && !call.includes("update-ref")));
    }
  }
});

test("release reruns reject unverified history before advancing an existing branch", () => {
  const script = readFileSync(new URL("./commit-release-pr.mjs", import.meta.url), "utf8")
    .replace(/^import .+;\r?\n/gm, "");
  const calls = [];
  assert.throws(() => runInNewContext(script, {
    Buffer,
    console: { log() {} },
    process: { env: { GH_REPO: "owner/repo", BRANCH_NAME: "release/app-v1.2.3", RELEASE_TITLE: "release: KubeCove v1.2.3" } },
    readFileSync: () => Buffer.from("contents"),
    spawnSync: (command, args) => {
      calls.push([command, ...args]);
      let stdout;
      if (command === "git") stdout = args[0] === "rev-parse" ? "a".repeat(40) : "package.json\0";
      else if (args.includes("--paginate")) stdout = JSON.stringify([
        { commits: [{ sha: "b".repeat(40), commit: { verification: { verified: true } } }] },
        { commits: [{ sha: "c".repeat(40), commit: { verification: { verified: false } } }] },
      ]);
      else if (args[1].includes("matching-refs")) stdout = JSON.stringify([{ ref: "refs/heads/release/app-v1.2.3", object: { sha: "b".repeat(40) } }]);
      else stdout = JSON.stringify({ data: { createCommitOnBranch: { commit: { oid: "d".repeat(40), signature: { isValid: true } } } } });
      return { status: 0, stdout, stderr: "" };
    },
  }), /unverified commit.*before rerunning/);
  assert.ok(calls.every((call) => !call.includes("graphql") && !call.includes("POST") && !call.includes("DELETE")));
});

test("parallel frontend checks retain coverage and finish before compilation-heavy tests", () => {
  const ci = parse(readFileSync(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8"));
  const common = [
    "bun run docs:check",
    "node --test scripts/release-workflow-regression.test.mjs",
    "bun run typecheck",
    "bun run lint",
  ];
  for (const [source, commands] of [
    [ci, [...common, "bun run audit", "bun run typecheck:bench", "bun run svelte:check", "bun run lint:anti-slop"]],
    [workflow, common],
  ]) {
    const frontend = job("frontend", source);
    const installed = frontend.steps.indexOf(step(frontend, "Install frontend dependencies"));
    const groups = frontend.steps.filter(({ parallel }) => parallel);
    assert.equal(groups.length, 1);
    const group = groups[0];
    const checked = frontend.steps.indexOf(group);
    assert.ok(checked > installed, "parallel checks need installed dependencies");
    assert.deepEqual(Object.keys(group), ["parallel"], "group failures must propagate");
    assert.deepEqual(group.parallel.map(({ run }) => run).sort(), [...commands].sort());
    for (const child of group.parallel) {
      assert.deepEqual(Object.keys(child).sort(), ["name", "run"], "each check must run and fail normally");
    }
    const tested = frontend.steps.indexOf(step(frontend, "Test frontend"));
    assert.ok(tested > checked, "Svelte compilation tests need the runner after parallel checks finish");
    assert.equal(frontend.steps[tested].run, "bun test");
    if (source === ci) {
      assert.ok(frontend.steps.indexOf(step(frontend, "Run fast end-to-end suite")) > tested,
        "E2E must wait for the frontend tests");
    }
  }
});

test("both container variants and architectures are verified before registry publication", () => {
  const build = job("container-build", packaging);
  assert.deepEqual(build.strategy.matrix.include.map(({ arch, variant }) => `${arch}/${variant}`).sort(),
    ["amd64/full", "amd64/slim", "arm64/full", "arm64/slim"]);
  const commands = step(build, "Build and verify container").run;
  assert.match(commands, /docker build[^\n]+--target "\$VARIANT"/);
  assert.match(commands, /scripts\/container-smoke\.sh/);
  assert.ok(commands.indexOf("scripts/container-smoke.sh") < commands.indexOf("docker save"));
  assert.doesNotMatch(commands, /docker push|docker login/);
  const shared = job("server-packaging");
  assert.equal(shared.uses, "./.github/workflows/server-packaging.yml");
  assert.equal(shared.with["release-tag"], "${{ github.ref_name }}");
  const verify = job("verify");
  assert.ok(verify.needs.includes("server-packaging"));
  assert.equal(step(verify, "Verify required jobs").env.SERVER_PACKAGING_RESULT, "${{ needs.server-packaging.result }}");
  assert.match(step(verify, "Verify required jobs").run, /test "\$\{SERVER_PACKAGING_RESULT\}" = "success"/);
  const publish = job("publish");
  assert.equal(publish.permissions.packages, "write");
  assert.ok(publish.needs.includes("server-packaging"));
  const publishCommands = step(publish, "Publish container images").run;
  assert.match(publishCommands, /docker load/);
  assert.match(publishCommands, /--password-stdin/);
  assert.match(publishCommands, /for variant in full slim/);
  assert.match(publishCommands, /for arch in amd64 arm64/);
  assert.match(publishCommands, /suffix="-slim"/);
  assert.match(publishCommands, /docker buildx imagetools create/);
  assert.doesNotMatch(publishCommands, /docker build --/);
  assert.ok(publish.steps.indexOf(step(publish, "Publish container images")) >
    publish.steps.indexOf(step(publish, "Verify final updater platform coverage")));
});

test("native builds include Linux arm64 and both variants without a desktop runtime", () => {
  const build = job("server-build", packaging);
  assert.deepEqual(build.strategy.matrix.include.map(({ target }) => target).sort(), [...targets].sort());
  const web = step(build, "Build embedded web assets");
  assert.equal(web.env.KUBECOVE_PUBLIC_RUNTIME, "web");
  assert.equal(web.run, "bun --bun vite build");
  const native = step(build, "Build and package both native variants");
  assert.ok(build.steps.indexOf(web) < build.steps.indexOf(native));
  assert.match(native.run, /for variant in full slim/);
  assert.match(native.run, /cargo build --locked --release --manifest-path src-tauri\/server\/Cargo\.toml --target/);
  assert.match(native.run, /--no-default-features/);
  assert.match(native.run, /scripts\/package-server\.sh/);
  assert.equal(step(build, "Upload server archive for release assembly").with.path, "kubecove-server_*");
  assert.doesNotMatch(JSON.stringify(build), /cache-apt-pkgs|apt-get install|webkit2gtk|appindicator|libsecret/);
});

test("native shell builds both variants with a nonempty command array", () => {
  const commands = step(job("server-build", packaging), "Build and package both native variants").run;
  // Replace external build/package tools with recordings; execute the real workflow loop.
  const result = spawnSync("bash", ["-euc", `
    cargo() { printf 'cargo %s\\n' "$*"; }
    bash() { printf 'package %s\\n' "$*"; }
    TARGET=aarch64-unknown-linux-gnu
    TAG_NAME=app-v1.2.3
    ${commands}
  `], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.stdout.trim().split("\n"), [
    "cargo build --locked --release --manifest-path src-tauri/server/Cargo.toml --target aarch64-unknown-linux-gnu",
    "package scripts/package-server.sh aarch64-unknown-linux-gnu full 1.2.3",
    "cargo build --locked --release --manifest-path src-tauri/server/Cargo.toml --target aarch64-unknown-linux-gnu --no-default-features",
    "package scripts/package-server.sh aarch64-unknown-linux-gnu slim 1.2.3",
  ]);
  // Bash 3.2 treats empty arrays as unset under nounset; the base command must never be empty.
  assert.match(commands, /build=\(cargo build/);
});

test("server build reuses pinned actions already used by release jobs", () => {
  const build = job("server-build", packaging);
  const existing = job("frontend");
  for (const action of ["actions/checkout", "oven-sh/setup-bun", "actions/cache"]) {
    const pinned = existing.steps.find(({ uses }) => uses?.startsWith(`${action}@`))?.uses;
    assert.match(pinned, /@[a-f0-9]{40}$/);
    assert.ok(build.steps.some(({ uses }) => uses === pinned), `${action} pin differs`);
  }
  for (const action of ["dtolnay/rust-toolchain", "jdx/mr-boxington-action"]) {
    assert.match(build.steps.find(({ uses }) => uses?.startsWith(`${action}@`))?.uses, /@[a-f0-9]{40}$/);
  }
});

// Release binaries must not inherit shared compiler outputs. E2E must wrap nested Cargo.
test("release artifacts use local mbx while E2E restores both workspace caches", () => {
  const e2e = parse(readFileSync(new URL("../.github/workflows/e2e.yml", import.meta.url), "utf8"));
  for (const build of [job("build"), job("server-build", packaging), job("bundle", gpuiRelease)]) {
    const setup = build.steps.find(({ uses }) => uses?.startsWith("jdx/mr-boxington-action@"));
    assert.equal(setup?.with.backend, "local");
    assert.doesNotMatch(JSON.stringify(build), /swatinem\/rust-cache/);
    const activation = step(build, "Activate Cargo caching");
    assert.equal(activation.run, "node scripts/ci-mbx-cargo.mjs");
    const consumer = build.steps.find(({ uses, run }) => uses?.startsWith("tauri-apps/tauri-action@")
      || run?.includes("for variant in full slim") || run?.includes("cargo build --locked"));
    assert.ok(build.steps.indexOf(activation) < build.steps.indexOf(consumer));
  }
  for (const check of [job("e2e"), ...Object.values(e2e.jobs)]) {
    const setups = check.steps.filter(({ uses }) => uses?.startsWith("jdx/mr-boxington-action@"));
    assert.deepEqual(setups.map(({ with: inputs }) => inputs["working-directory"]), ["src-tauri", "src-tauri/server"]);
    assert.deepEqual(setups.map(({ with: inputs }) => inputs["cache-generation"]), ["kubecove-e2e", "kubecove-server-e2e"]);
    const activation = step(check, "Activate Cargo caching");
    assert.equal(activation.run, "node scripts/ci-mbx-cargo.mjs");
    const consumer = check.steps.find(({ run }) => run?.includes("bun run e2e:"));
    assert.ok(check.steps.indexOf(activation) < check.steps.indexOf(consumer));
    assert.doesNotMatch(JSON.stringify(check), /swatinem\/rust-cache/);
  }
});

test("publishing can read build artifacts and waits for all release gates", () => {
  const publish = job("publish");
  assert.equal(publish.permissions.actions, "read");
  assert.equal(publish.permissions.contents, "write");
  for (const need of ["prepare", "verify", "build", "server-packaging", "gpui"]) {
    assert.ok(publish.needs.includes(need), need);
  }
  assert.match(step(publish, "Download release build artifacts").run, /gh run download/);
  assert.match(step(publish, "Upload headless server release assets").run, /gh release upload[^\n]+kubecove-server-\*\/kubecove-server_\*/);
  // The artifact name the GPUI workflow uploads is the one publish sends to the release.
  const signed = job("sign", gpuiRelease);
  assert.equal(step(signed, "Upload GPUI release assets").with.name, "kubecove-gpui-release");
  assert.match(step(publish, "Upload GPUI release assets").run, /gh release upload "\$TAG_NAME" run-assets\/kubecove-gpui-release\/\* --clobber/);
  assert.ok(publish.steps.indexOf(step(publish, "Upload GPUI release assets")) <
    publish.steps.indexOf(step(publish, "Verify final updater platform coverage")));
});

// A dry run starts from a branch, so any job that reads `github.ref_name` as a release tag must not run.
test("a GPUI dry run starts only the GPUI job and can never write a release", () => {
  const input = workflow.on.workflow_dispatch.inputs["gpui-dry-run"];
  assert.deepEqual({ type: input.type, default: input.default }, { type: "boolean", default: false });
  const others = Object.entries(workflow.jobs).filter(([name]) => name !== "gpui");
  assert.ok(others.length > 0);
  for (const [name, { if: condition = "" }] of others) {
    assert.match(condition, /^\$\{\{ (always\(\) && )?!inputs\.gpui-dry-run \}\}$/, `${name} must skip a dry run`);
  }
  const gpui = job("gpui");
  assert.equal(gpui.uses, "./.github/workflows/gpui-release.yml");
  assert.deepEqual(gpui.permissions, { contents: "read" });
  assert.equal(gpui.if, "${{ !cancelled() && (inputs.gpui-dry-run || needs.metadata.result == 'success') }}");
  // `cond && '' || x` would fall through to x, so the tag expression must put the tag first.
  assert.equal(gpui.with["release-tag"], "${{ !inputs.gpui-dry-run && github.ref_name || '' }}");
  assert.deepEqual(gpuiRelease.permissions, { contents: "read" });
  for (const [name, gpuiJob] of Object.entries(gpuiRelease.jobs)) {
    assert.equal(gpuiJob.permissions, undefined, `${name} must keep read-only contents`);
    assert.doesNotMatch(JSON.stringify(gpuiJob), /gh release|GITHUB_TOKEN|GH_TOKEN/, `${name} must not reach releases`);
  }
});

test("GPUI bundles build on all three platforms and only the signing job holds the key", () => {
  const bundle = job("bundle", gpuiRelease);
  assert.deepEqual(bundle.strategy.matrix.include.map(({ platform, "bundle-target": target, format }) => `${platform}/${target}/${format}`), [
    "ubuntu-latest/x86_64-unknown-linux-gnu/appimage",
    "macos-latest/aarch64-apple-darwin/app,dmg",
    "windows-latest/x86_64-pc-windows-msvc/nsis",
  ]);
  const built = step(bundle, "Build GPUI client").run;
  assert.match(built, /cargo build --locked --release --manifest-path src-tauri\/gpui\/Cargo\.toml --bin kubecove-gpui --target "\$BUNDLE_TARGET"/);
  assert.match(step(bundle, "Install cargo-packager").run, /^cargo install cargo-packager --version \d+\.\d+\.\d+ --locked$/);
  assert.match(step(bundle, "Bundle GPUI client").run, /cargo packager --manifest-path src-tauri\/gpui\/Cargo\.toml --release --target "\$BUNDLE_TARGET"/);
  assert.equal(step(bundle, "Bundle GPUI client").env.APPIMAGE_EXTRACT_AND_RUN, undefined);
  assert.doesNotMatch(JSON.stringify(bundle), /secrets\./);
  const signing = step(job("sign", gpuiRelease), "Sign GPUI bundles");
  assert.equal(signing.env.TAURI_SIGNING_PRIVATE_KEY, "${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}");
  // The GPUI client rejects a signature whose trusted comment lacks the manifest version.
  assert.match(signing.run, /bun x tauri signer sign --app-version "\$version" "\$file"/);
  // The DMG is a manual download outside the updater feed, so it is not signed.
  assert.match(signing.run, /\[\[ "\$file" == \*\.dmg \]\] && continue/);
  assert.match(step(bundle, "Bundle GPUI client").run, /cp "\$out\/KubeCove GPUI_\$\{version\}_aarch64\.dmg" "gpui-updater\/kubecove-gpui_\$\{version\}_aarch64\.dmg"/);
  assert.match(step(job("sign", gpuiRelease), "Verify signatures and write latest-gpui.json").run, /node scripts\/gpui-release-manifest\.mjs/);
  for (const gpuiJob of Object.values(gpuiRelease.jobs)) {
    for (const { uses } of gpuiJob.steps.filter(({ uses }) => uses)) {
      assert.match(uses, /@[a-f0-9]{40}$/, `${uses} must be pinned`);
    }
  }
});

// Produced by `tauri signer sign --app-version 1.2.3` with a throwaway key generated for these
// tests. The signer records the signed file's name, so each asset name has its own signature
// over a file of that name holding `fixtureData`.
const fixtureKey = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDc0MTIwMzA0Mjc2RjdCREMKUldUY2UyOG5CQU1TZEkveUVLZWVLU3pwUlRZTzRiTVMrRk94Q0V2U3ZTcm01TkRteEV1ZFY4b04K";
const fixtureSignatures = {
  "kubecove-gpui_1.2.3_x86_64.AppImage": "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVUY2UyOG5CQU1TZEhtdnJwZng2cG1LdU9sZDFsUFFSVUVZb0xmYTY2UE5OZzVLaW95Sm5FRlBxRHB2VWVoNEd6ZnlLTU12TlZxUDMzSnA1RWdIZlI0WDRWTnFZejdLMXdNPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0NDY4Nzk0CWZpbGU6a3ViZWNvdmUtZ3B1aV8xLjIuM194ODZfNjQuQXBwSW1hZ2UJdmVyc2lvbjoxLjIuMwpwWk1BNExYc3RFN203eWJtMkphanl1djg2Wk9xTHZsVys2UC85MGppQWJ3cE4wSGRRdHRVcGIwUDY5QnBaemVSQy9RbnExK3oxeXJNMFBxK0kwQlhCUT09Cg==",
  "kubecove-gpui_1.2.3_aarch64.app.tar.gz": "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVUY2UyOG5CQU1TZE50ZFkyQU10bXlmY1B5Yy9jTFhoVEdYTVF0M1FMcEFGQzFWdmwwMkI2MXdRbmNlaW9PbEw4aFhidTZVbkdkNURzODFyc2hmYTVSRVBXL0FLdmN6endZPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0NDY4Nzk1CWZpbGU6a3ViZWNvdmUtZ3B1aV8xLjIuM19hYXJjaDY0LmFwcC50YXIuZ3oJdmVyc2lvbjoxLjIuMwpmdXdYWVdtalFZMm9VTFJZdDZUVGdkQVVrQ3BmWUovdkFOTTU4TlBCTjJWSHcvWnFDZHU3MUJSTGlqV3ExcnF5SEwwcitHSkhRNzgxWHRwelRmV2lBZz09Cg==",
  "kubecove-gpui_1.2.3_x64-setup.exe": "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVUY2UyOG5CQU1TZERRUGV1ekNCY0M4L0Q5U1NwbnJ4djhFTmdFZTVkYmJYS0gzZ1duNXdhOXVKWEJXR2sxWmU5eU1ybkRkanB4YXRyWnNaSllIeGlVb09pME0xMlpwT3c0PQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk0NDY4Nzk1CWZpbGU6a3ViZWNvdmUtZ3B1aV8xLjIuM194NjQtc2V0dXAuZXhlCXZlcnNpb246MS4yLjMKTkdSUG9GUUhYai9Ud3YxSU0ySnZCRjcyTjVsd0ZhYW41QmxhRGY4TjhOaTJ6a0hwN0xpUkw0OWdCZm9yQUxTUXY1Z0NyeEQ2dDZxN3hSMDJsREY4QlE9PQo=",
};
const fixtureName = "kubecove-gpui_1.2.3_x86_64.AppImage";
const fixtureSignature = fixtureSignatures[fixtureName];
const fixtureData = Buffer.from("kubecove gpui fixture\n");

test("GPUI signature verification accepts only the updater key, the exact bytes, the signed version and the signed file name", () => {
  const signature = { publicKey: fixtureKey, signature: fixtureSignature, data: fixtureData, version: "1.2.3", name: fixtureName };
  assert.doesNotThrow(() => verifyUpdaterSignature(signature));
  // A signature made for another asset is refused, as the client refuses the Tauri app's.
  assert.throws(() => verifyUpdaterSignature({ ...signature, name: "kubecove-gpui_1.2.3_x64-setup.exe" }), /file:kubecove-gpui_1\.2\.3_x64-setup\.exe/);
  const productionKey = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8")).plugins.updater.pubkey;
  assert.throws(() => verifyUpdaterSignature({ ...signature, publicKey: productionKey }), /different key/);
  assert.throws(() => verifyUpdaterSignature({ ...signature, data: Buffer.from("kubecove gpui fixture!\n") }), /does not match the file/);
  assert.throws(() => verifyUpdaterSignature({ ...signature, version: "1.2.4" }), /version:1\.2\.4/);
  // Rewriting the version inside the trusted comment breaks the global signature.
  const forged = Buffer.from(Buffer.from(fixtureSignature, "base64").toString("utf8").replace("version:1.2.3", "version:1.2.4")).toString("base64");
  assert.throws(() => verifyUpdaterSignature({ ...signature, signature: forged, version: "1.2.4" }), /trusted comment signature/);
  assert.throws(() => verifyUpdaterSignature({ ...signature, signature: Buffer.from("not a signature").toString("base64") }), /not a minisign signature/);
});

test("latest-gpui.json lists every platform with its release URL and signature, or fails", () => {
  const version = "1.2.3";
  const files = Object.fromEntries([...new Set(Object.values(gpuiUpdaterAssets(version)))]
    .flatMap((name) => [[name, fixtureData], [`${name}.sig`, Buffer.from(fixtureSignatures[name])]]));
  const build = (names, overrides = {}) => buildGpuiManifest({
    version, tag: `app-v${version}`, repository: "owner/repo", notes: "Notes", publicKey: fixtureKey,
    pubDate: "2026-11-09T00:00:00Z", names, read: (name) => files[name], ...overrides,
  });
  const manifest = build(Object.keys(files));
  assert.deepEqual(Object.keys(manifest), ["version", "notes", "pub_date", "platforms"]);
  assert.deepEqual(Object.keys(manifest.platforms).sort(), ["darwin-aarch64", "linux-x86_64", "windows-x86_64"]);
  assert.equal(manifest.platforms["linux-x86_64"].url, "https://github.com/owner/repo/releases/download/app-v1.2.3/kubecove-gpui_1.2.3_x86_64.AppImage");
  assert.equal(manifest.platforms["darwin-aarch64"].url, "https://github.com/owner/repo/releases/download/app-v1.2.3/kubecove-gpui_1.2.3_aarch64.app.tar.gz");
  assert.equal(manifest.platforms["windows-x86_64"].signature, fixtureSignatures["kubecove-gpui_1.2.3_x64-setup.exe"]);
  for (const missing of Object.keys(files)) {
    assert.throws(() => build(Object.keys(files).filter((name) => name !== missing)), new RegExp(`Missing ${missing.replaceAll(".", "\\.")}\\.`));
  }
  assert.throws(() => build(Object.keys(files), { tag: "feat/branch" }), /does not match app version/);
});

test("final verification rejects every missing archive, checksum, installer or updater platform", () => {
  const publish = job("publish");
  const verification = step(publish, "Verify final updater platform coverage");
  const code = verification.run.match(/node <<'EOF'\n([\s\S]+)\nEOF/)[1];
  const version = "1.2.3";
  const assets = [
    `KubeCove_${version}_universal.app.tar.gz`, `KubeCove_${version}_universal.app.tar.gz.sig`,
    `KubeCove_${version}_amd64.AppImage`, `KubeCove_${version}_amd64.AppImage.sig`,
    `KubeCove_${version}_x64-setup.exe`, `KubeCove_${version}_x64-setup.exe.sig`, "latest.json",
    ...targets.flatMap((target) => ["", "-slim"].flatMap((suffix) => {
      const archive = `kubecove-server_${version}_${target}${suffix}.tar.gz`;
      return [archive, `${archive}.sha256`];
    })),
    ...[...new Set(Object.values(gpuiUpdaterAssets(version)))].flatMap((bundle) => [bundle, `${bundle}.sig`]),
    `kubecove-gpui_${version}_aarch64.dmg`,
    "latest-gpui.json",
  ];
  const platforms = Object.fromEntries(["darwin-aarch64", "darwin-x86_64", "linux-x86_64",
    "linux-x86_64-appimage", "windows-x86_64", "windows-x86_64-nsis"].map((key) => [key, {}]));
  const gpuiPlatforms = Object.fromEntries(Object.entries(gpuiUpdaterAssets(version)).map(([key, name]) =>
    [key, { signature: "sig", url: `https://github.com/owner/repo/releases/download/app-v${version}/${name}` }]));
  const validate = (names, coverage, gpui = { version, platforms: gpuiPlatforms }) => runInNewContext(code, {
    process: { env: { UPDATER_CHECK_DIR: "/check", RELEASE_VERSION: version } },
    require: (name) => {
      assert.equal(name, "node:fs");
      return { readFileSync: (path) => {
        if (path === "/check/assets.txt") return names.join("\n");
        if (path === "/check/latest-gpui.json") return JSON.stringify(gpui);
        assert.equal(path, "/check/latest.json");
        return JSON.stringify({ platforms: coverage });
      } };
    },
  });
  assert.doesNotThrow(() => validate(assets, platforms));
  for (const missing of assets) {
    assert.throws(() => validate(assets.filter((asset) => asset !== missing), platforms), /missing assets/);
  }
  for (const missing of Object.keys(platforms)) {
    assert.throws(() => validate(assets, Object.fromEntries(Object.entries(platforms).filter(([key]) => key !== missing))),
      /missing platforms/);
  }
  for (const missing of Object.keys(gpuiPlatforms)) {
    const coverage = Object.fromEntries(Object.entries(gpuiPlatforms).filter(([key]) => key !== missing));
    assert.throws(() => validate(assets, platforms, { version, platforms: coverage }), /missing GPUI platforms/);
  }
  assert.throws(() => validate(assets, platforms, { version: "1.2.2", platforms: gpuiPlatforms }), /missing GPUI platforms/);
  const elsewhere = { ...gpuiPlatforms, "linux-x86_64": { signature: "sig", url: "https://example.com/other.AppImage" } };
  assert.throws(() => validate(assets, platforms, { version, platforms: elsewhere }), /missing GPUI platforms: linux-x86_64/);
  const final = step(publish, "Publish completed release");
  assert.ok(publish.steps.indexOf(final) > publish.steps.indexOf(verification));
  assert.equal(final.if, "needs.prepare.outputs.release-draft == 'true'");
});
