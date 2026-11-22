import { spawnSync } from "node:child_process";

const workspaces = ["rust", "rust/gpui", "rust/server"];
for (const workspace of workspaces) {
  const result = spawnSync("cargo", ["fmt", "--manifest-path", `${workspace}/Cargo.toml`, "--all", "--", "--check"], {
    cwd: new URL("../", import.meta.url),
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exitCode = 1;
}
