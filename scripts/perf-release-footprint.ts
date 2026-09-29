import { createHash, randomUUID } from "node:crypto";
import { chmod, mkdir, readdir, readFile, stat, writeFile } from "node:fs/promises";
import { arch, cpus, release } from "node:os";
import { basename, join, relative, resolve, sep } from "node:path";
import { gzipSync } from "node:zlib";
import { observeReleaseStartup } from "./perf-release-native";
import { inspectElfSections, summarizeFrontend } from "./release-footprint-report";

const root = resolve(process.cwd());
type BuildEnvironment = Record<string, string | undefined>;
const runId = `${Date.now()}-${randomUUID()}`;
const artifacts = join(root, "e2e", "artifacts", `release-footprint-${runId}`);
const targets = join(root, ".e2e", "release-footprint", runId);
const baseline = { OPT_LEVEL: "3", LTO: "false", CODEGEN_UNITS: "16", DEBUG: "0", STRIP: "none", PANIC: "unwind" };
const variants = [
	{ name: "baseline", settings: {} },
	{ name: "thin-lto", settings: { LTO: "thin" } },
	{ name: "fat-lto", settings: { LTO: "fat" } },
	{ name: "one-codegen-unit", settings: { CODEGEN_UNITS: "1" } },
	{ name: "strip-symbols", settings: { STRIP: "symbols" } },
	{ name: "opt-level-2", settings: { OPT_LEVEL: "2" } },
	{ name: "opt-level-s", settings: { OPT_LEVEL: "s" } },
	{ name: "opt-level-z", settings: { OPT_LEVEL: "z" } },
	{ name: "panic-abort", settings: { PANIC: "abort" } },
	{ name: "remove-unused-commands", settings: {} },
];

async function command(args: string[], env: BuildEnvironment = {}, log?: string) {
	const child = Bun.spawn(args, { cwd: root, env: { ...process.env, ...env }, stdout: "pipe", stderr: "pipe" });
	const writer = log ? Bun.file(log).writer() : undefined;
	async function drain(stream: ReadableStream<Uint8Array>) {
		if (!writer) return new Response(stream).text();
		for await (const chunk of stream) writer.write(chunk);
		return "";
	}
	const [code, stdout, stderr] = await Promise.all([child.exited, drain(child.stdout), drain(child.stderr)]);
	await writer?.end();
	if (code !== 0) throw new Error(`${args[0]} failed (${code}); ${log ?? stderr}`);
	return stdout;
}

async function frontendFingerprint(directory: string) {
	const entries = await readdir(directory, { recursive: true, withFileTypes: true });
	const files = entries.filter((entry) => entry.isFile()).map((entry) => join(entry.parentPath, entry.name)).sort();
	const hash = createHash("sha256");
	for (const file of files) {
		hash.update(relative(directory, file).split(sep).join("/"));
		hash.update("\0");
		hash.update(await readFile(file));
		hash.update("\0");
	}
	return hash.digest("hex");
}

async function buildFrontend(directory: string, profile: boolean) {
	await command(["bun", "x", "vite", "build", "--outDir", directory], { KUBECOVE_PUBLIC_PROFILE: String(profile) }, join(artifacts, `frontend-${profile ? "profile" : "production"}.log`));
	const report = JSON.parse(await readFile(join(root, ".e2e", "reports", "frontend-bundle.json"), "utf8"));
	return { directory, sha256: await frontendFingerprint(directory), report, summary: summarizeFrontend(report.files) };
}

async function allowedCommands(target: string) {
	const files = await readdir(join(target, "release", "build"), { recursive: true });
	const lists = [];
	for (const file of files.filter((file) => basename(file) === "allowed-commands.json")) {
		const data = JSON.parse(await readFile(join(target, "release", "build", file), "utf8"));
		lists.push({ owner: file.split(sep)[0].replace(/-[a-f0-9]+$/, ""), ...data });
	}
	const required = ["plugin:opener|open_url", "plugin:process|restart", "plugin:updater|check", "plugin:updater|download", "plugin:updater|install", "plugin:updater|download_and_install"];
	for (const name of required) if (!lists.some((list) => list.commands.includes(name))) throw new Error(`Unused-command removal omitted ${name}`);
	if (lists.some((list) => list.has_app_acl)) throw new Error("Unexpected application ACL changes the typed-command contract");
	return lists;
}

async function inspectPackage(target: string, directory: string) {
	const bundle = join(target, "release", "bundle", "deb");
	const packages = (await readdir(bundle)).filter((file) => file.endsWith(".deb"));
	if (packages.length !== 1) throw new Error("Expected one comparable Debian package");
	const pkg = join(bundle, packages[0]);
	const unpacked = join(directory, "unpacked");
	await command(["dpkg-deb", "--extract", pkg, unpacked]);
	const members = (await command(["ar", "t", pkg])).trim().split("\n");
	const memberBytes = [];
	for (const name of members) {
		const child = Bun.spawn(["ar", "p", pkg, name], { stdout: "pipe", stderr: "pipe" });
		const [bytes, code, stderr] = await Promise.all([new Response(child.stdout).arrayBuffer(), child.exited, new Response(child.stderr).text()]);
		if (code !== 0) throw new Error(`Package member extraction failed: ${stderr}`);
		memberBytes.push({ name, bytes: bytes.byteLength });
	}
	const binary = join(unpacked, "usr", "bin", "kubecove");
	const sidecar = join(unpacked, "usr", "bin", "kubeconform");
	const executableBytes = (await stat(binary)).size;
	const sidecarData = await readFile(sidecar);
	const elf = inspectElfSections(await command(["readelf", "--section-headers", "--wide", binary]));
	const packageBytes = (await stat(pkg)).size;
	const entries = await readdir(unpacked, { recursive: true, withFileTypes: true });
	let installedBytes = 0;
	for (const entry of entries) if (entry.isFile()) installedBytes += (await stat(join(entry.parentPath, entry.name))).size;
	const rustOutputs = [];
	for (const file of (await readdir(join(target, "release"))).sort()) if (/^(kubecove|libkubecove_lib\.(a|so|rlib))$/.test(file)) rustOutputs.push({ file, bytes: (await stat(join(target, "release", file))).size });
	return { package: basename(pkg), packageBytes, members: memberBytes, containerOverheadBytes: packageBytes - memberBytes.reduce((sum, member) => sum + member.bytes, 0), installedBytes, otherInstalledBytes: installedBytes - executableBytes - sidecarData.length, executableBytes, rustOutputs, elf, sidecar: { rawBytes: sidecarData.length, standaloneGzipBytes: gzipSync(sidecarData).length, version: (await command([sidecar, "-v"])).trim() }, binary };
}

async function crashProbe(binary: string, directory: string) {
	const child = Bun.spawn(["prlimit", "--core=0", "--", binary], { env: { ...process.env, KUBECOVE_E2E: "1", RUST_BACKTRACE: "1" }, stdout: "pipe", stderr: "pipe" });
	const [exitCode, stderr] = await Promise.all([child.exited, new Response(child.stderr).text(), new Response(child.stdout).text()]);
	await writeFile(join(directory, "crash.txt"), stderr);
	if (!stderr.includes("KUBECOVE_E2E=1 requires the e2e Cargo feature")) throw new Error("Production crash guard did not run");
	return { exitCode, signal: child.signalCode, backtrace: stderr.includes("stack backtrace:"), namedRunFrame: stderr.includes("kubecove_lib::run"), stderrBytes: Buffer.byteLength(stderr), probe: "existing pre-initialization production E2E guard; core dumps disabled for this child" };
}

if (process.platform !== "linux") throw new Error("This package characterization uses Linux ELF and Debian tools; native application code remains cross-platform");
if ((await command(["git", "status", "--porcelain"])).trim()) throw new Error("Commit the profiling sources before comparing a clean SHA");
const sourceSha = (await command(["git", "rev-parse", "HEAD"])).trim();
await mkdir(artifacts, { recursive: true });
await mkdir(targets, { recursive: true });
const production = await buildFrontend(join(targets, "frontend-production"), false);
const companion = await buildFrontend(join(targets, "frontend-profile"), true);
const observations: object[] = [];
const report = { sourceSha, environment: { architecture: arch(), osRelease: release(), logicalCpus: cpus().length, rust: (await command(["rustc", "--version"])).trim(), bun: Bun.version }, format: "deb", frontend: { production: { sha256: production.sha256, ...production.summary }, companion: { sha256: companion.sha256, ...companion.summary } }, baseline, variants: observations, passed: false };
const save = () => writeFile(join(artifacts, "release-footprint.json"), `${JSON.stringify(report, null, 2)}\n`);
await save();
try {
for (const variant of variants) {
	const directory = join(artifacts, variant.name);
	const target = join(targets, variant.name);
	await mkdir(directory, { recursive: true });
	for (const file of ["kubeconform-aarch64-apple-darwin", "kubeconform-x86_64-apple-darwin", "kubeconform-x86_64-unknown-linux-gnu"]) await chmod(join(root, "src-tauri", "bin", file), 0o644);
	if ((await command(["git", "status", "--porcelain"])).trim()) throw new Error("Source changed during comparison");
	const settings = { ...baseline, ...variant.settings };
	const env = { CARGO_TARGET_DIR: target, ...Object.fromEntries(Object.entries(settings).flatMap(([name, value]) => [[`CARGO_PROFILE_RELEASE_${name}`, value], [`CARGO_PROFILE_BENCH_${name}`, value]])) };
	const unusedCommands = variant.name === "remove-unused-commands";
	const config = { build: { beforeBuildCommand: null, frontendDist: production.directory, removeUnusedCommands: unusedCommands }, bundle: { createUpdaterArtifacts: false } };
	await writeFile(join(directory, "build-config.json"), `${JSON.stringify({ settings, config }, null, 2)}\n`);
	if ((await command(["git", "rev-parse", "HEAD"])).trim() !== sourceSha || await frontendFingerprint(production.directory) !== production.sha256 || await frontendFingerprint(companion.directory) !== companion.sha256) throw new Error("Comparison input changed");
	console.log(`Building ${variant.name}`);
	const started = performance.now();
	await command(["bun", "run", "tauri", "build", "--bundles", "deb", "--config", JSON.stringify(config)], env, join(directory, "production-build.log"));
	const buildMs = performance.now() - started;
	const pkg = await inspectPackage(target, directory);
	const preservedCommands = unusedCommands ? await allowedCommands(target) : null;
	const crash = await crashProbe(pkg.binary, directory);
	await command(["cargo", "bench", "--manifest-path", "src-tauri/Cargo.toml", "--features", "bench-support", "--bench", "backend_hot_paths"], env, join(directory, "benchmarks.txt"));
	const e2eConfig = JSON.parse(await readFile(join(root, "src-tauri", "tauri.e2e.conf.json"), "utf8"));
	const companionConfig = { ...config, build: { ...config.build, frontendDist: companion.directory }, app: { ...e2eConfig.app, security: { ...e2eConfig.app.security, capabilities: ["default", ...e2eConfig.app.security.capabilities] } } };
	await command(["bun", "run", "tauri", "build", "--no-bundle", "--features", "e2e", "--config", JSON.stringify(companionConfig)], env, join(directory, "companion-build.log"));
	await observeReleaseStartup(join(target, "release", "kubecove"), directory, companion.summary.fonts.bundled.map((font) => font.file));
	const startup = JSON.parse(await readFile(join(directory, "footprint-startup.json"), "utf8"));
	const { binary: _binary, ...packageReport } = pkg;
	report.variants.push({ name: variant.name, settings, buildMs, ...packageReport, crash, preservedCommands, startup, benchmarkFile: `${variant.name}/benchmarks.txt`, benchmarkPanic: "unwind, enforced by Cargo even for panic-abort" });
	await save();
}
report.passed = true;
await save();
} catch (error) {
	await writeFile(join(artifacts, "failure.json"), `${JSON.stringify({ passed: false, error: String(error) }, null, 2)}\n`);
	throw error;
} finally {
for (const file of ["kubeconform-aarch64-apple-darwin", "kubeconform-x86_64-apple-darwin", "kubeconform-x86_64-unknown-linux-gnu"]) await chmod(join(root, "src-tauri", "bin", file), 0o644);
}
console.log(`Report: ${join(artifacts, "release-footprint.json")}`);
