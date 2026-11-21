import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { parse } from "yaml";

const workflow = parse(readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8"));
const stage = workflow.jobs["stage-release-assets"];
const freeze = stage.steps.find(step => step.name === "Carry forward frozen legacy feeds");

test("a consolidated draft carries the original legacy manifests with pinned GitHub asset URLs", () => {
  assert.equal(freeze.if, "needs.metadata.outputs.manifest == 'latest-desktop.json'");
  const directory = mkdtempSync(join(tmpdir(), "kubecove-feed-"));
  const repo = "Timpan4/kubecove";
  const api = `https://api.github.com/repos/${repo}/releases/assets/123`;
  const direct = `https://github.com/${repo}/releases/download/app-v0.15.1/kubecove-gpui_0.15.1_x86_64.AppImage`;
  // Only these release reads and draft upload are accepted by the fake CLI.
  const gh = `gh() {
    case "$*" in
      'release download app-v0.15.1 --pattern latest.json --pattern latest-gpui.json --dir frozen')
        cp -r source frozen ;;
      'api repos/Timpan4/kubecove/releases/tags/app-v0.15.1 --jq '*)
        cat urls.json ;;
      'release upload app-v0.16.0 frozen/latest.json frozen/latest-gpui.json --clobber')
        cp frozen/*.json uploaded/ ;;
      *) return 1 ;;
    esac
  }`;
  try {
    for (const name of ["source", "uploaded"]) mkdirSync(join(directory, name));
    const legacy = (url, version = "0.15.1") => JSON.stringify({ version, platforms: { linux: { url, signature: "old signature" } } });
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
    for (const bad of [legacy(api, "0.16.0"), legacy(direct.replace("app-v0.15.1", "latest"))]) {
      rmSync(join(directory, "frozen"), { recursive: true });
      write("latest.json", bad);
      assert.notEqual(run(), 0);
      assert.equal(readFileSync(join(directory, "uploaded/latest.json"), "utf8"), legacy(api));
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
