import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { parse } from "yaml";

import { desktopReleaseChannel } from "./desktop-release-channel.mjs";

test("historical release tags cannot publish a latest feed", () => {
  for (const version of ["0.14.9", "0.15.0", "0.15.1", "0.15.2"]) {
    assert.throws(() => desktopReleaseChannel(version), /0\.15\.2 pitstop/);
  }
});

test("a pre-0.16.0 dry run on 0.15.1 uses the consolidated channel while other legacy versions stay frozen", () => {
  assert.deepEqual(desktopReleaseChannel("0.15.1", { dryRun: true }), desktopReleaseChannel("0.16.0"));
  assert.throws(() => desktopReleaseChannel("0.15.0", { dryRun: true }), /0\.15\.2 pitstop/);
  assert.throws(() => desktopReleaseChannel("0.15.1"), /0\.15\.2 pitstop/);
  assert.throws(() => desktopReleaseChannel("0.15.2", { dryRun: true }), /0\.15\.2 pitstop/);
});

const workflow = parse(readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8"));
const stage = workflow.jobs["stage-release-assets"];
const freeze = stage.steps.find(step => step.name === "Carry forward the 0.15.2 pitstop feeds");

test("the bundle step stages cargo-packager outputs under the public channel names", () => {
  const bundles = parse(readFileSync(new URL("../.github/workflows/gpui-release.yml", import.meta.url), "utf8"));
  const step = bundles.jobs.bundle.steps.find(step => step.name === "Bundle GPUI client");
  for (const [version, releaseTag, prefix, product, identifier] of [
    ["0.15.1", "", "kubecove", "KubeCove", "com.timpan.kubecove"],
    ["0.16.0", "app-v0.16.0", "kubecove", "KubeCove", "com.timpan.kubecove"],
  ]) {
    const directory = mkdtempSync(join(tmpdir(), "kubecove-bundle-"));
    try {
      mkdirSync(join(directory, "scripts"));
      cpSync(new URL("./desktop-release-channel.mjs", import.meta.url), join(directory, "scripts/desktop-release-channel.mjs"));
      writeFileSync(join(directory, "package.json"), JSON.stringify({ version }));
      const metadata = { packages: [{ name: "kubecove-gpui", metadata: { packager: { productName: product, identifier } } }] };
      writeFileSync(join(directory, "metadata.json"), JSON.stringify(metadata));
      const out = join(directory, "rust/gpui/target/gpui-bundle");
      const bundle = join(out, product + ".app", "Contents/MacOS");
      mkdirSync(bundle, { recursive: true });
      const sidecar = '#!/usr/bin/env bash\n[[ "$*" == "-v" ]]\n';
      writeFileSync(join(bundle, "kubeconform"), sidecar, { mode: 0o755 });
      writeFileSync(join(out, `kubecove-gpui_${version}_x86_64.AppImage`),
        '#!/usr/bin/env bash\nset -e\n[[ "$*" == "--appimage-extract usr/bin/kubeconform" ]]\n' +
        'mkdir -p squashfs-root/usr/bin\n' +
        'cp "$EXPECTED_PRODUCT.app/Contents/MacOS/kubeconform" squashfs-root/usr/bin/kubeconform\n',
        { mode: 0o755 });
      for (const name of [`kubecove-gpui_${version}_x64-setup.exe`, `${product}_${version}_aarch64.dmg`]) {
        writeFileSync(join(out, name), name);
      }
      for (const format of ["appimage", "nsis", "app,dmg"]) {
        const result = spawnSync("bash", ["-ec", 'cargo() { case "$1" in metadata) cat metadata.json ;; packager) return 0 ;; *) return 1 ;; esac; }\n' + step.run], {
          cwd: directory, encoding: "utf8", env: { ...process.env, BUNDLE_TARGET: "fixture", FORMAT: format, RELEASE_TAG: releaseTag },
        });
        assert.equal(result.status, 0, result.stderr);
      }
      const staged = join(directory, "gpui-updater");
      const names = ["x86_64.AppImage", "x64-setup.exe", "aarch64.app.tar.gz", "aarch64.dmg"].map(suffix => `${prefix}_${version}_${suffix}`);
      assert.deepEqual(readdirSync(staged).sort(), names.sort());
      const archive = spawnSync("tar", ["-tzf", join(staged, `${prefix}_${version}_aarch64.app.tar.gz`)], { encoding: "utf8" });
      assert.equal(archive.status, 0, archive.stderr);
      assert.ok(archive.stdout.split("\n").includes(product + ".app/Contents/MacOS/kubeconform"));
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  }
});

test("a consolidated draft carries the 0.15.2 pitstop manifests with pinned GitHub asset URLs", () => {
  assert.equal(freeze.if, "needs.metadata.outputs.manifest == 'latest-desktop.json'");
  const directory = mkdtempSync(join(tmpdir(), "kubecove-feed-"));
  const repo = "Timpan4/kubecove";
  const api = `https://api.github.com/repos/${repo}/releases/assets/123`;
  const direct = `https://github.com/${repo}/releases/download/app-v0.15.2/kubecove-gpui_0.15.2_x86_64.AppImage`;
  // Only these release reads and draft upload are accepted by the fake CLI.
  const gh = `gh() {
    case "$*" in
      'release download app-v0.15.2 --pattern latest.json --pattern latest-gpui.json --dir frozen')
        cp -r source frozen ;;
      'api repos/Timpan4/kubecove/releases/tags/app-v0.15.2 --jq '*)
        cat urls.json ;;
      'release upload app-v0.16.0 frozen/latest.json frozen/latest-gpui.json --clobber')
        cp frozen/*.json uploaded/ ;;
      *) return 1 ;;
    esac
  }`;
  try {
    for (const name of ["source", "uploaded"]) mkdirSync(join(directory, name));
    const legacy = (url, version = "0.15.2") => JSON.stringify({ version, platforms: { linux: { url, signature: "old signature" } } });
    const write = (name, value) => writeFileSync(join(directory, "source", name), value);
    const run = () => spawnSync("bash", ["-ec", gh + "\n" + freeze.run], {
      cwd: directory, env: { ...process.env, GH_REPO: repo, TAG_NAME: "app-v0.16.0" },
    }).status;
    writeFileSync(join(directory, "urls.json"), JSON.stringify([api, direct]));
    write("latest.json", legacy(api));
    write("latest-gpui.json", legacy(direct));
    assert.equal(run(), 0);
    for (const name of ["latest.json", "latest-gpui.json"]) {
      assert.deepEqual(readFileSync(join(directory, "uploaded", name)), readFileSync(join(directory, "source", name)));
    }
    for (const bad of [legacy(api, "0.16.0"), legacy(direct.replace("app-v0.15.2", "latest"))]) {
      rmSync(join(directory, "frozen"), { recursive: true });
      write("latest.json", bad);
      assert.notEqual(run(), 0);
      assert.equal(readFileSync(join(directory, "uploaded/latest.json"), "utf8"), legacy(api));
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
