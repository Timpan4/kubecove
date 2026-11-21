// Run on the Linux signing runner before it receives the release key.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { writeGpuiManifest } from "./gpui-release-manifest.mjs";

const dir = mkdtempSync(join(tmpdir(), "kubecove-signing-"));
try {
  const key = join(dir, "test.key");
  const pub = join(dir, "test.pub");
  const password = "throwaway test password";
  execFileSync("minisign", ["-G", "-s", key, "-p", pub], { input: `${password}\n${password}\n`, stdio: ["pipe", "ignore", "pipe"] });
  for (const [version, prefix, filename] of [
    ["0.15.1", "kubecove-gpui", "latest-gpui.json"],
    ["0.16.0", "kubecove", "latest-desktop.json"],
  ]) {
    const assets = join(dir, version);
    mkdirSync(assets);
    const names = ["x86_64.AppImage", "aarch64.app.tar.gz", "x64-setup.exe"].map(suffix => `${prefix}_${version}_${suffix}`);
    const dmg = `${prefix}_${version}_aarch64.dmg`;
    for (const name of [...names, dmg]) writeFileSync(join(assets, name), name);
    execFileSync("bash", ["scripts/sign-gpui-release.sh", assets, version], {
      env: { ...process.env, TAURI_SIGNING_PRIVATE_KEY: readFileSync(key).toString("base64"), TAURI_SIGNING_PRIVATE_KEY_PASSWORD: password },
      stdio: ["ignore", "ignore", "pipe"],
    });
    writeGpuiManifest(assets, {
      version, tag: `app-v${version}`, repository: "owner/repo", publicKey: readFileSync(pub).toString("base64"),
      notes: "", pubDate: new Date().toISOString(),
    });
    assert.deepEqual(readdirSync(assets).filter(name => name.endsWith(".json")), [filename]);
    const { platforms } = JSON.parse(readFileSync(join(assets, filename), "utf8"));
    assert.deepEqual(Object.values(platforms).map(build => build.url.split("/").pop()).sort(), names.sort());
    assert.equal(existsSync(join(assets, `${dmg}.sig`)), false);
  }
} finally {
  rmSync(dir, { recursive: true, force: true });
}
