import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { runInNewContext } from "node:vm";
import { parse } from "yaml";

const workflow = parse(readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8"));
const packaging = parse(readFileSync(new URL("../.github/workflows/server-packaging.yml", import.meta.url), "utf8"));
const targets = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "aarch64-apple-darwin"];

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
    "src-tauri/Cargo.lock", "src-tauri/server/Cargo.toml", "src-tauri/server/Cargo.lock", "CHANGELOG.md"]) {
    assert.ok(staged.includes(path), `Release commit omits ${path}`);
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

test("release orchestration runs after dependencies are installed in CI and releases", () => {
  const ci = parse(readFileSync(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8"));
  for (const source of [ci, workflow]) {
    const frontend = job("frontend", source);
    const installed = frontend.steps.indexOf(step(frontend, "Install frontend dependencies"));
    const tested = frontend.steps.findIndex(({ run }) => run?.includes("node --test") &&
      run.includes("scripts/release-workflow-regression.test.mjs"));
    assert.ok(tested > installed, "release tests need the installed YAML parser");
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
    TARGET=aarch64-apple-darwin
    TAG_NAME=app-v1.2.3
    ${commands}
  `], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.stdout.trim().split("\n"), [
    "cargo build --locked --release --manifest-path src-tauri/server/Cargo.toml --target aarch64-apple-darwin",
    "package scripts/package-server.sh aarch64-apple-darwin full 1.2.3",
    "cargo build --locked --release --manifest-path src-tauri/server/Cargo.toml --target aarch64-apple-darwin --no-default-features",
    "package scripts/package-server.sh aarch64-apple-darwin slim 1.2.3",
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
  for (const action of ["dtolnay/rust-toolchain", "swatinem/rust-cache"]) {
    assert.match(build.steps.find(({ uses }) => uses?.startsWith(`${action}@`))?.uses, /@[a-f0-9]{40}$/);
  }
});

test("publishing can read build artifacts and waits for all release gates", () => {
  const publish = job("publish");
  assert.equal(publish.permissions.actions, "read");
  assert.equal(publish.permissions.contents, "write");
  for (const need of ["prepare", "verify", "build", "server-packaging"]) {
    assert.ok(publish.needs.includes(need), need);
  }
  assert.match(step(publish, "Download headless server archives").run, /gh run download/);
  assert.match(step(publish, "Upload headless server release assets").run, /gh release upload[^\n]+kubecove-server-\*\/kubecove-server_\*/);
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
  ];
  const platforms = Object.fromEntries(["darwin-aarch64", "darwin-x86_64", "linux-x86_64",
    "linux-x86_64-appimage", "windows-x86_64", "windows-x86_64-nsis"].map((key) => [key, {}]));
  const validate = (names, coverage) => runInNewContext(code, {
    process: { env: { UPDATER_CHECK_DIR: "/check", RELEASE_VERSION: version } },
    require: (name) => {
      assert.equal(name, "node:fs");
      return { readFileSync: (path) => {
        if (path === "/check/assets.txt") return names.join("\n");
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
  const final = step(publish, "Publish completed release");
  assert.ok(publish.steps.indexOf(final) > publish.steps.indexOf(verification));
  assert.equal(final.if, "needs.prepare.outputs.release-draft == 'true'");
});
