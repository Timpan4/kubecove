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
  for (const [version, prefix, filename, dryRun] of [
    ["0.15.0", "kubecove-gpui", "latest-gpui.json", true],
    ["0.15.1", "kubecove-gpui", "latest-gpui.json", false],
    ["0.16.0", "kubecove", "latest-desktop.json", false],
  ]) {
    const assets = join(dir, version);
    mkdirSync(assets);
    const expected = {
      "linux-x86_64": `${prefix}_${version}_x86_64.AppImage`,
      "darwin-aarch64": `${prefix}_${version}_aarch64.app.tar.gz`,
      "windows-x86_64": `${prefix}_${version}_x64-setup.exe`,
    };
    const dmg = `${prefix}_${version}_aarch64.dmg`;
    for (const name of [...Object.values(expected), dmg]) writeFileSync(join(assets, name), name);
    execFileSync("bash", ["scripts/sign-gpui-release.sh", assets, version, ...(dryRun ? ["--dry-run"] : [])], {
      env: { ...process.env, TAURI_SIGNING_PRIVATE_KEY: readFileSync(key).toString("base64"), TAURI_SIGNING_PRIVATE_KEY_PASSWORD: password },
      stdio: ["ignore", "ignore", "pipe"],
    });
    writeGpuiManifest(assets, {
      version, tag: `app-v${version}`, dryRun, repository: "owner/repo", publicKey: readFileSync(pub).toString("base64"),
      notes: "", pubDate: new Date().toISOString(),
    });
    assert.deepEqual(readdirSync(assets).filter(name => name.endsWith(".json")), [filename]);
    const { platforms } = JSON.parse(readFileSync(join(assets, filename), "utf8"));
    const assetByPlatform = Object.fromEntries(Object.entries(platforms).map(([platform, build]) => [platform, build.url.split("/").pop()]));
    assert.deepEqual(assetByPlatform, expected);
    assert.equal(existsSync(join(assets, `${dmg}.sig`)), false);
  }
} finally {
  rmSync(dir, { recursive: true, force: true });
}
