import { pathToFileURL } from "node:url";

export function desktopReleaseChannel(version) {
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) throw new Error("Expected a stable desktop release version");
  if (version === "0.15.1") {
    return { prefix: "kubecove-gpui", product: "KubeCove GPUI", identifier: "com.timpan.kubecove.gpui", manifest: "latest-gpui.json" };
  }
  const [major, minor] = version.split(".").map(Number);
  if (major === 0 && minor < 16) throw new Error("Both legacy feeds are frozen at 0.15.1");
  return { prefix: "kubecove", product: "KubeCove", identifier: "com.timpan.kubecove", manifest: "latest-desktop.json" };
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const value = desktopReleaseChannel(process.argv[2])[process.argv[3]];
  if (!value) throw new Error("Expected a desktop release channel field");
  console.log(value);
}
