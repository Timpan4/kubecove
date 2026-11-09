// Verifies the signed GPUI updater artifacts and writes `latest-gpui.json` beside them.
//
// Each `.sig` comes from `tauri signer sign --app-version`, so it is a base64 minisign
// signature, the format tauri-plugin-updater and the GPUI client read. Verification uses
// the updater public key committed in `src-tauri/tauri.conf.json`. The manifest follows
// Tauri's static JSON format; the Tauri feed `latest.json` is not touched.
//
// Usage: ASSETS_DIR=<dir> GH_REPO=<owner/repo> NOTES_FILE=<file> [RELEASE_TAG=app-v<version>]
//   node scripts/gpui-release-manifest.mjs
// An empty RELEASE_TAG is a dry run: URLs point at app-v<version>, which need not exist.
import { createHash, createPublicKey, verify } from "node:crypto";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

export const MANIFEST_NAME = "latest-gpui.json";

// One universal macOS bundle serves both darwin keys.
export function gpuiUpdaterAssets(version) {
  const mac = `kubecove-gpui_${version}_universal.app.tar.gz`;
  return {
    "linux-x86_64": `kubecove-gpui_${version}_x86_64.AppImage`,
    "darwin-x86_64": mac,
    "darwin-aarch64": mac,
    "windows-x86_64": `kubecove-gpui_${version}_x64-setup.exe`,
  };
}

function lines(base64) {
  return Buffer.from(base64.trim(), "base64").toString("utf8").split(/\r?\n/);
}

function fail(label, reason) {
  throw new Error(`${label}: ${reason}`);
}

// Minisign layout: the key blob is "Ed" + 8-byte key ID + 32-byte Ed25519 key. The signature
// blob is the algorithm ("ED" signs the BLAKE2b-512 hash, "Ed" the raw bytes) + key ID +
// 64-byte signature, and a global signature covers that signature plus the trusted comment.
export function verifyUpdaterSignature({ publicKey, signature, data, version, label = "signature" }) {
  const keyBlob = Buffer.from(lines(publicKey)[1] ?? "", "base64");
  if (keyBlob.length !== 42 || keyBlob.toString("latin1", 0, 2) !== "Ed") fail(label, "the updater public key is not a minisign Ed25519 key");
  const key = createPublicKey({ key: { kty: "OKP", crv: "Ed25519", x: keyBlob.subarray(10).toString("base64url") }, format: "jwk" });

  const [untrusted = "", blob = "", trustedLine = "", globalBlob = ""] = lines(signature);
  if (!untrusted.startsWith("untrusted comment:")) fail(label, "not a minisign signature");
  const signatureBlob = Buffer.from(blob, "base64");
  const algorithm = signatureBlob.toString("latin1", 0, 2);
  if (signatureBlob.length !== 74 || (algorithm !== "ED" && algorithm !== "Ed")) fail(label, "unsupported signature algorithm");
  if (!signatureBlob.subarray(2, 10).equals(keyBlob.subarray(2, 10))) fail(label, "signed by a different key than the updater public key");
  const prefix = "trusted comment: ";
  if (!trustedLine.startsWith(prefix)) fail(label, "missing trusted comment");
  const trusted = trustedLine.slice(prefix.length);
  const signed = signatureBlob.subarray(10);

  const message = algorithm === "ED" ? createHash("blake2b512").update(data).digest() : data;
  if (!verify(null, message, key, signed)) fail(label, "the signature does not match the file");
  if (!verify(null, Buffer.concat([signed, Buffer.from(trusted, "utf8")]), key, Buffer.from(globalBlob, "base64"))) {
    fail(label, "the trusted comment signature does not match");
  }
  // The GPUI client, like tauri-plugin-updater's `requireSignedVersion`, rejects a signature
  // that does not name the manifest version.
  if (!trusted.split("\t").includes(`version:${version}`)) fail(label, `the trusted comment does not carry version:${version}`);
}

// `read(name)` returns a file's bytes from the assets directory, or throws when it is missing.
export function buildGpuiManifest({ version, tag, repository, notes, publicKey, pubDate, names, read }) {
  if (tag !== `app-v${version}`) throw new Error(`Release tag ${tag} does not match app version ${version}.`);
  const present = new Set(names);
  const platforms = {};
  for (const [platform, name] of Object.entries(gpuiUpdaterAssets(version))) {
    for (const required of [name, `${name}.sig`]) {
      if (!present.has(required)) throw new Error(`Missing ${required}. Found: ${[...present].sort().join(", ") || "nothing"}.`);
    }
    const signature = read(`${name}.sig`).toString("utf8");
    verifyUpdaterSignature({ publicKey, signature, data: read(name), version, label: name });
    platforms[platform] = { signature, url: `https://github.com/${repository}/releases/download/${tag}/${name}` };
  }
  return { version, notes, pub_date: pubDate, platforms };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const { ASSETS_DIR: dir, GH_REPO: repository, NOTES_FILE: notesFile, RELEASE_TAG: releaseTag = "" } = process.env;
  if (!dir || !repository || !notesFile) throw new Error("ASSETS_DIR, GH_REPO and NOTES_FILE are required.");
  const tauri = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
  const version = tauri.version;
  const tag = releaseTag || `app-v${version}`;
  if (!releaseTag) console.log(`Dry run: manifest URLs point at ${tag}.`);
  const manifest = buildGpuiManifest({
    version,
    tag,
    repository,
    notes: readFileSync(notesFile, "utf8").trim(),
    publicKey: tauri.plugins.updater.pubkey,
    pubDate: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    names: readdirSync(dir),
    read: (name) => readFileSync(join(dir, name)),
  });
  writeFileSync(join(dir, MANIFEST_NAME), `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`Verified ${Object.keys(manifest.platforms).join(", ")} and wrote ${MANIFEST_NAME}.`);
}
