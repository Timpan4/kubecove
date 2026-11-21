// Verify the signed desktop artifacts and write their release channel's manifest.
//
// Each .sig contains base64-encoded minisign text with a prehashed signature.
// The GPUI client and this verifier read the same release public key.
//
// Usage: ASSETS_DIR=<dir> GH_REPO=<owner/repo> NOTES_FILE=<file> [RELEASE_TAG=app-v<version>]
//   node scripts/gpui-release-manifest.mjs
// An empty RELEASE_TAG is a dry run: URLs point at app-v<version>, which need not exist.
import { createHash, createPublicKey, verify } from "node:crypto";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import { desktopReleaseChannel } from "./desktop-release-channel.mjs";

// macOS ships for Apple silicon only, so there is no darwin-x86_64 entry.
export function gpuiUpdaterAssets(version) {
  const { prefix } = desktopReleaseChannel(version);
  return {
    "linux-x86_64": `${prefix}_${version}_x86_64.AppImage`,
    "darwin-aarch64": `${prefix}_${version}_aarch64.app.tar.gz`,
    "windows-x86_64": `${prefix}_${version}_x64-setup.exe`,
  };
}

function lines(base64) {
  return Buffer.from(base64.trim(), "base64").toString("utf8").split(/\r?\n/);
}

function fail(label, reason) {
  throw new Error(`${label}: ${reason}`);
}

// Minisign layout: the key blob is "Ed" + 8-byte key ID + 32-byte Ed25519 key. The signature
// blob is the algorithm ("ED" signs the BLAKE2b-512 hash) + key ID +
// 64-byte signature, and a global signature covers that signature plus the trusted comment.
// `name` is the asset's file name; the signer records the name of the file it signed.
export function verifyUpdaterSignature({ publicKey, signature, data, version, name, label = name }) {
  const keyBlob = Buffer.from(lines(publicKey)[1] ?? "", "base64");
  if (keyBlob.length !== 42 || keyBlob.toString("latin1", 0, 2) !== "Ed") fail(label, "the updater public key is not a minisign Ed25519 key");
  const key = createPublicKey({ key: { kty: "OKP", crv: "Ed25519", x: keyBlob.subarray(10).toString("base64url") }, format: "jwk" });

  const [untrusted = "", blob = "", trustedLine = "", globalBlob = ""] = lines(signature);
  if (!untrusted.startsWith("untrusted comment:")) fail(label, "not a minisign signature");
  const signatureBlob = Buffer.from(blob, "base64");
  const algorithm = signatureBlob.toString("latin1", 0, 2);
  if (signatureBlob.length !== 74 || algorithm !== "ED") fail(label, "unsupported signature algorithm");
  if (!signatureBlob.subarray(2, 10).equals(keyBlob.subarray(2, 10))) fail(label, "signed by a different key than the updater public key");
  const prefix = "trusted comment: ";
  if (!trustedLine.startsWith(prefix)) fail(label, "missing trusted comment");
  const trusted = trustedLine.slice(prefix.length);
  const signed = signatureBlob.subarray(10);

  const message = createHash("blake2b512").update(data).digest();
  if (!verify(null, message, key, signed)) fail(label, "the signature does not match the file");
  if (!verify(null, Buffer.concat([signed, Buffer.from(trusted, "utf8")]), key, Buffer.from(globalBlob, "base64"))) {
    fail(label, "the trusted comment signature does not match");
  }
  // The GPUI client, like tauri-plugin-updater's `requireSignedVersion`, rejects a signature
  // that does not name the manifest version.
  const fields = trusted.split("\t");
  if (!fields.includes(`version:${version}`)) fail(label, `the trusted comment does not carry version:${version}`);
  // Both desktop products use the same key. The signed file name binds the product and platform.
  if (!fields.includes(`file:${name}`)) fail(label, `the trusted comment does not carry file:${name}`);
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
    verifyUpdaterSignature({ publicKey, signature, data: read(name), version, name });
    platforms[platform] = { signature, url: `https://github.com/${repository}/releases/download/${tag}/${name}` };
  }
  return { version, notes, pub_date: pubDate, platforms };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const { ASSETS_DIR: dir, GH_REPO: repository, NOTES_FILE: notesFile, RELEASE_TAG: releaseTag = "" } = process.env;
  if (!dir || !repository || !notesFile) throw new Error("ASSETS_DIR, GH_REPO and NOTES_FILE are required.");
  const { public_key: publicKey } = JSON.parse(readFileSync(new URL("../src-tauri/gpui/updater.json", import.meta.url), "utf8"));
  const { version } = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
  const tag = releaseTag || `app-v${version}`;
  if (!releaseTag) console.log(`Dry run: manifest URLs point at ${tag}.`);
  const manifest = buildGpuiManifest({
    version,
    tag,
    repository,
    notes: readFileSync(notesFile, "utf8").trim(),
    publicKey,
    pubDate: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    names: readdirSync(dir),
    read: (name) => readFileSync(join(dir, name)),
  });
  const filename = desktopReleaseChannel(version).manifest;
  writeFileSync(join(dir, filename), `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`Verified ${Object.keys(manifest.platforms).join(", ")} and wrote ${filename}.`);
}
