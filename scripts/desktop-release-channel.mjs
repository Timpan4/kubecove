import { pathToFileURL } from "node:url";

const FROZEN_LEGACY_VERSION = "0.15.1";
const [legacyMajor, legacyMinor, legacyPatch] = FROZEN_LEGACY_VERSION.split(".").map(Number);

export function desktopReleaseChannel(version, { dryRun = false } = {}) {
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) throw new Error("Expected a stable desktop release version");
  const [major, minor, patch] = version.split(".").map(Number);
  const preTransition = major === legacyMajor && (minor < legacyMinor || (minor === legacyMinor && patch < legacyPatch));
  if (version === FROZEN_LEGACY_VERSION || (dryRun && preTransition)) {
    return { prefix: "kubecove-gpui", product: "KubeCove GPUI", identifier: "com.timpan.kubecove.gpui", manifest: "latest-gpui.json" };
  }
  if (major === 0 && minor < 16) throw new Error(`Both legacy feeds are frozen at ${FROZEN_LEGACY_VERSION}`);
  return { prefix: "kubecove", product: "KubeCove", identifier: "com.timpan.kubecove", manifest: "latest-desktop.json" };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const [, , version, field, mode] = process.argv;
  const value = desktopReleaseChannel(version, { dryRun: mode === "--dry-run" })[field];
  if (!value) throw new Error("Expected a desktop release channel field");
  console.log(value);
}
