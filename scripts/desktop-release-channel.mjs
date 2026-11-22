import { pathToFileURL } from "node:url";

const FROZEN_LEGACY_VERSION = "0.15.1";
const CONSOLIDATED = {
  prefix: "kubecove",
  product: "KubeCove",
  identifier: "com.timpan.kubecove",
  manifest: "latest-desktop.json",
};

export function desktopReleaseChannel(version, { dryRun = false } = {}) {
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) throw new Error("Expected a stable desktop release version");
  const [major, minor] = version.split(".").map(Number);
  if (major === 0 && minor < 16) {
    if (dryRun && version === FROZEN_LEGACY_VERSION) return { ...CONSOLIDATED };
    throw new Error(`Both legacy feeds are frozen at ${FROZEN_LEGACY_VERSION}`);
  }
  return { ...CONSOLIDATED };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const [, , version, field, mode] = process.argv;
  const value = desktopReleaseChannel(version, { dryRun: mode === "--dry-run" })[field];
  if (!value) throw new Error("Expected a desktop release channel field");
  console.log(value);
}
