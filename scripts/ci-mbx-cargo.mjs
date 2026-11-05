import { execFileSync } from "node:child_process";
import { accessSync, appendFileSync, constants } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const { GITHUB_PATH, GITHUB_ENV } = process.env;
if (!GITHUB_PATH || !GITHUB_ENV) throw new Error("Cargo activation requires a GitHub Actions job");

// mbx 1.29.2 installs its Cargo shim under the platform's local data directory.
const dataDirectory = process.platform === "win32"
  ? process.env.LOCALAPPDATA
  : process.platform === "darwin"
    ? join(homedir(), "Library", "Application Support")
    : process.env.XDG_DATA_HOME || join(homedir(), ".local", "share");
if (!dataDirectory) throw new Error("The runner has no local application data directory");
const shimDirectory = join(dataDirectory, "mbx", "bin");
execFileSync("mbx", ["setup", "--yes"], { stdio: "inherit" });
accessSync(join(shimDirectory, process.platform === "win32" ? "cargo.exe" : "cargo"), constants.X_OK);
appendFileSync(GITHUB_PATH, `${shimDirectory}\n`);
// E2E runners and release packagers consume binaries at Cargo's normal target paths.
appendFileSync(GITHUB_ENV, "MBX_TARGET_VIEWS=0\n");
