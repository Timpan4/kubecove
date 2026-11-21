import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { constants, createReadStream } from "node:fs";
import { access, appendFile, chmod, copyFile, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";

const workspace = "src-tauri";
const benchmark = "backend_hot_paths";
const target = "x86_64-unknown-linux-gnu";
// cargo-codspeed 5.0.1 copies the analysis executable here. Simulation uses
// only this file, the package working directory and CODSPEED_CARGO_WORKSPACE_ROOT.
const executable = join(workspace, "target", "codspeed", "analysis", "kubecove", benchmark);
const buildArgs = ["--mode", "simulation", "--profile", "bench", "--no-default-features", "--features", "bench-support", "--bench", benchmark, "--locked"];
const sha256 = /^[a-f0-9]{64}$/;

async function digest(path) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}

async function identity({ commit, toolchain }) {
  const head = execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim();
  if (!/^[a-f0-9]{40}$/.test(commit ?? "") || commit !== head) throw new Error("Benchmark checkout does not match the expected commit");
  if (!/^\S+\/[a-f0-9]{40}$/.test(toolchain ?? "")) throw new Error("Missing or invalid benchmark toolchain");
  if (process.platform !== "linux" || process.arch !== "x64") throw new Error("Unexpected benchmark runner target");
  return { commit, toolchain, target, buildArgs, cargoLockSha256: await digest(join(workspace, "Cargo.lock")) };
}

async function installBenchmark(directory, expected, manifestSha256) {
  const entries = await readdir(directory, { withFileTypes: true });
  if (entries.some((entry) => !entry.isFile()) || !isDeepStrictEqual(entries.map((entry) => entry.name).sort(), [benchmark, "manifest.json"])) {
    throw new Error("Unexpected benchmark artifact files");
  }
  const manifestPath = join(directory, "manifest.json");
  const manifestBytes = await readFile(manifestPath);
  if (!sha256.test(manifestSha256 ?? "") || createHash("sha256").update(manifestBytes).digest("hex") !== manifestSha256) {
    throw new Error("Benchmark manifest digest does not match the build job");
  }
  const { executableSha256, ...metadata } = JSON.parse(manifestBytes);
  if (!isDeepStrictEqual(metadata, await identity(expected))) throw new Error("Benchmark artifact build identity does not match");
  const binary = join(directory, benchmark);
  if (!sha256.test(executableSha256 ?? "") || await digest(binary) !== executableSha256) {
    throw new Error("Benchmark executable digest does not match the manifest");
  }
  await mkdir(dirname(executable), { recursive: true });
  await copyFile(binary, executable, constants.COPYFILE_EXCL);
  // GitHub's artifact ZIP transport does not preserve the execute bit.
  await chmod(executable, 0o755);
  console.log(`Verified ${benchmark}: ${metadata.commit}, rustc ${metadata.toolchain}, ${metadata.target}, SHA-256 ${executableSha256}`);
}

async function buildBenchmark(directory, commit) {
  const rustc = execFileSync("rustc", ["-Vv"], { encoding: "utf8" });
  if (/^host: (\S+)$/m.exec(rustc)?.[1] !== target) throw new Error("Unexpected rustc host target");
  const version = /^release: (\S+)$/m.exec(rustc)?.[1];
  const revision = /^commit-hash: ([a-f0-9]{40})$/m.exec(rustc)?.[1];
  if (!version || !revision) throw new Error("Cannot identify the benchmark compiler");
  const toolchain = `${version}/${revision}`;
  const metadata = await identity({ commit, toolchain });
  execFileSync("cargo", ["codspeed", "build", ...buildArgs], { cwd: workspace, stdio: "inherit" });
  if (!isDeepStrictEqual(metadata, await identity({ commit, toolchain }))) throw new Error("Benchmark build changed the checkout identity");
  await access(executable, constants.X_OK);

  await mkdir(directory);
  const binary = join(directory, benchmark);
  await copyFile(executable, binary, constants.COPYFILE_EXCL);
  const manifest = join(directory, "manifest.json");
  await writeFile(manifest, `${JSON.stringify({ ...metadata, executableSha256: await digest(binary) }, null, 2)}\n`, { flag: "wx" });
  await appendFile(process.env.GITHUB_OUTPUT, `toolchain=${toolchain}\nmanifest-sha256=${await digest(manifest)}\n`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [mode, directory] = process.argv.slice(2);
  const { GITHUB_SHA: commit, EXPECTED_TOOLCHAIN: toolchain, EXPECTED_MANIFEST_SHA256: manifest } = process.env;
  if (!directory) throw new Error("Expected an artifact directory");
  if (mode === "build") await buildBenchmark(directory, commit);
  else if (mode === "install") await installBenchmark(directory, { commit, toolchain }, manifest);
  else throw new Error("Expected build or install");
}
