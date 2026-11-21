// Run on the Linux signing runner before it receives the release key.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { buildGpuiManifest, gpuiUpdaterAssets } from "./gpui-release-manifest.mjs";

const dir = mkdtempSync(join(tmpdir(), "kubecove-signing-"));
const version = "0.15.1";
const key = join(dir, "test.key");
const pub = join(dir, "test.pub");
const read = (name) => readFileSync(join(dir, name));
try {
  for (const name of Object.values(gpuiUpdaterAssets(version))) writeFileSync(join(dir, name), name);
  const dmg = `kubecove-gpui_${version}_aarch64.dmg`;
  writeFileSync(join(dir, dmg), "manual installer");

  // Tauri wraps the entire encrypted minisign key file in an additional layer of base64.
  const password = "throwaway test password";
  execFileSync("minisign", ["-G", "-s", key, "-p", pub], { input: `${password}\n${password}\n`, stdio: ["pipe", "ignore", "pipe"] });
  execFileSync("bash", ["scripts/sign-gpui-release.sh", dir, version], {
    env: { ...process.env, TAURI_SIGNING_PRIVATE_KEY: readFileSync(key).toString("base64"), TAURI_SIGNING_PRIVATE_KEY_PASSWORD: password },
    stdio: ["ignore", "ignore", "pipe"],
  });
  const manifest = buildGpuiManifest({
    version, tag: `app-v${version}`, repository: "owner/repo", publicKey: readFileSync(pub).toString("base64"),
    notes: "", pubDate: new Date().toISOString(), names: readdirSync(dir), read,
  });
  assert.deepEqual(Object.keys(manifest.platforms), Object.keys(gpuiUpdaterAssets(version)));
  assert.equal(existsSync(join(dir, `${dmg}.sig`)), false);
} finally {
  rmSync(dir, { recursive: true, force: true });
}
