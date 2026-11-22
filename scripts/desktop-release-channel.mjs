import { pathToFileURL } from "node:url";

const FROZEN_LEGACY_VERSION = "0.15.1";
const MIN_CONSOLIDATED_VERSION = "0.16.0";
const CONSOLIDATED = {
  prefix: "kubecove",
  product: "KubeCove",
  identifier: "com.timpan.kubecove",
  manifest: "latest-desktop.json",
};

export function desktopReleaseChannel(version, { dryRun = false } = {}) {
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) throw new Error("Expected a stable desktop release version");
  if (compareVersions(version, MIN_CONSOLIDATED_VERSION) < 0) {
    if (dryRun && version === FROZEN_LEGACY_VERSION) return { ...CONSOLIDATED };
    throw new Error(`Both legacy feeds are frozen at ${FROZEN_LEGACY_VERSION}`);
  }
  return { ...CONSOLIDATED };
}

function compareVersions(left, right) {
  const a = left.split(".").map(Number);
  const b = right.split(".").map(Number);
  for (let index = 0; index < 3; index++) {
    if (a[index] !== b[index]) return a[index] - b[index];
  }
  return 0;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const [, , version, field, mode] = process.argv;
  const value = desktopReleaseChannel(version, { dryRun: mode === "--dry-run" })[field];
  if (!value) throw new Error("Expected a desktop release channel field");
  console.log(value);
}
