import { createHash, randomUUID } from "node:crypto";
import { mkdir, readdir, readFile, stat, writeFile } from "node:fs/promises";
import { arch, cpus, release } from "node:os";
import { join, relative, resolve, sep } from "node:path";
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

async function buildFrontend(target: string, artifacts: string, profile: boolean) {
	const mode = profile ? "profile" : "production";
	const directory = join(target, `web-${mode}`);
	const reportPath = join(artifacts, `frontend-${mode}.json`);
	await command(["bun", "x", "vite", "build", "--outDir", directory], {
		KUBECOVE_PUBLIC_RUNTIME: "web", KUBECOVE_PUBLIC_PROFILE: String(profile), KUBECOVE_FRONTEND_BUNDLE_REPORT: reportPath,
	}, join(artifacts, `frontend-${mode}.log`));
	const report = JSON.parse(await readFile(reportPath, "utf8"));
	return { directory, sha256: await frontendFingerprint(directory), summary: summarizeFrontend(report.files) };
}

if (process.platform !== "linux") throw new Error("This package characterization measures the Linux ELF server");
if ((await command(["git", "status", "--porcelain"])).trim()) throw new Error("Commit the profiling sources before comparing a clean SHA");
const sourceSha = (await command(["git", "rev-parse", "HEAD"])).trim();
await mkdir(artifacts, { recursive: true });
await mkdir(targets, { recursive: true });
const observations: object[] = [];
const report = { sourceSha, environment: { architecture: arch(), osRelease: release(), logicalCpus: cpus().length,
	rust: (await command(["rustc", "--version"])).trim(), bun: Bun.version }, format: "server-elf-with-embedded-web-ui",
	baseline, variants: observations, passed: false };
const save = () => writeFile(join(artifacts, "release-footprint.json"), `${JSON.stringify(report, null, 2)}\n`);
await save();
let productionHash: string | undefined;
let companionHash: string | undefined;
try {
	for (const variant of variants) {
		const directory = join(artifacts, variant.name);
		const target = join(targets, variant.name);
		await mkdir(directory);
		if ((await command(["git", "status", "--porcelain"])).trim() || (await command(["git", "rev-parse", "HEAD"])).trim() !== sourceSha) throw new Error("Source changed during comparison");
		const settings = { ...baseline, ...variant.settings };
		const env = { CARGO_TARGET_DIR: target, CARGO_BUILD_BUILD_DIR: target, ...Object.fromEntries(Object.entries(settings).flatMap(([name, value]) =>
			[[`CARGO_PROFILE_RELEASE_${name}`, value], [`CARGO_PROFILE_BENCH_${name}`, value]])) };
		const build = ["cargo", "build", "--release", "--locked", "--manifest-path", "rust/server/Cargo.toml"];
		const production = await buildFrontend(target, directory, false);
		productionHash ??= production.sha256;
		if (production.sha256 !== productionHash) throw new Error("Production frontend changed");
		const started = performance.now();
		await command(build, { ...env, KUBECOVE_WEB_ASSET_DIR: production.directory }, join(directory, "production-build.log"));
		const buildMs = performance.now() - started;
		const binary = join(target, "release", "kubecove-server");
		const executableBytes = (await stat(binary)).size;
		const elf = inspectElfSections(await command(["readelf", "--section-headers", "--wide", binary]));
		await command(["cargo", "bench", "--manifest-path", "rust/Cargo.toml", "--features", "bench-support", "--bench", "backend_hot_paths"], env, join(directory, "benchmarks.txt"));
		const companion = await buildFrontend(target, directory, true);
		companionHash ??= companion.sha256;
		if (companion.sha256 !== companionHash) throw new Error("Profile frontend changed");
		await command(build, { ...env, KUBECOVE_WEB_ASSET_DIR: companion.directory }, join(directory, "companion-build.log"));
		await observeReleaseStartup(binary, directory, companion.summary.fonts.bundled.map((font) => font.file));
		const startup = JSON.parse(await readFile(join(directory, "footprint-startup.json"), "utf8"));
		observations.push({ name: variant.name, settings, buildMs, executableBytes, elf, production: { sha256: productionHash, ...production.summary },
			companion: { sha256: companionHash, ...companion.summary }, startup, benchmarkPanic: "unwind", benchmarkFile: `${variant.name}/benchmarks.txt` });
		await save();
	}
	report.passed = true;
	await save();
} catch (error) {
	await writeFile(join(artifacts, "failure.json"), JSON.stringify({ passed: false, error: String(error) }));
	throw error;
}
console.log(`Report: ${join(artifacts, "release-footprint.json")}`);
