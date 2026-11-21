// Turns the per-scenario LLVM profiles of a PARITY_COVERAGE=1 GPUI run into coverage map
// inputs (coverage-map.ts): `coverage/gpui-<index>.json` and `coverage/gpui-code.json`.
import { existsSync } from "node:fs";
import { readdir, readFile, rm, writeFile } from "node:fs/promises";
import { join, relative, resolve, sep } from "node:path";
import type { CodeInfo, Range, ScenarioCoverage } from "./coverage-map";
import { toRanges } from "./coverage-map";
import { GPUI_CRATE_ROOT } from "./gpui-prebuilt";

/** Flags the coverage build adds; `code_coverage.rs` in the GPUI parity tests reads the cfg. */
export const COVERAGE_RUSTFLAGS = "-C instrument-coverage --cfg kubecove_coverage";

export const exitProfileDirectory = (artifacts: string) => join(artifacts, "gpui-exit-profiles");
export const gpuiProfilePattern = (artifacts: string, owner: "build" | `worker-${number}`) => join(exitProfileDirectory(artifacts), owner, "%p-%m.profraw");

async function output(command: string[], signal: AbortSignal, cwd?: string): Promise<string> {
	signal.throwIfAborted();
	const child = Bun.spawn(command, { cwd, signal, stdout: "pipe", stderr: "pipe" });
	const [stdout, stderr] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text()]);
	const code = await child.exited;
	signal.throwIfAborted();
	if (code !== 0) throw new Error(`${command.join(" ")} failed: ${stderr.trim()}`);
	return stdout;
}

/** LLVM_COV or LLVM_PROFDATA, else the rustup `llvm-tools` component, else PATH. */
async function llvmTools(signal: AbortSignal): Promise<CoverageTools> {
	const { LLVM_COV, LLVM_PROFDATA } = process.env;
	const invoke = (command: string[], abort: AbortSignal) => output(command, abort);
	if (LLVM_COV && LLVM_PROFDATA) return { cov: LLVM_COV, profdata: LLVM_PROFDATA, invoke };
	const workspace = resolve("src-tauri/gpui");
	const rustc = process.env.RUSTC ?? process.env.CARGO_BUILD_RUSTC ?? "rustc";
	const [sysroot, version] = await Promise.all([
		output([rustc, "--print", "sysroot"], signal, workspace),
		output([rustc, "-vV"], signal, workspace),
	]);
	const host = /^host: (\S+)$/m.exec(version)?.[1];
	const locate = (name: string) => {
		const bundled = join(sysroot.trim(), "lib", "rustlib", host ?? "", "bin", process.platform === "win32" ? `${name}.exe` : name);
		return existsSync(bundled) ? bundled : name;
	};
	return { cov: LLVM_COV || locate("llvm-cov"), profdata: LLVM_PROFDATA || locate("llvm-profdata"), invoke };
}

/**
 * Repository files listed in cargo dep-info (`*.d`) anywhere under `buildDir`, including
 * `include_*!` inputs. Cargo's build-dir layout keeps each unit's dep-info in
 * `<profile>/build/<package>/<hash>/out/`, so a fixed `deps` directory misses crates. Rustc
 * names workspace members' files relative to the workspace and path dependencies' files
 * absolutely. Units left from earlier builds can add files, which only selects more.
 */
export async function compiledFiles(buildDir: string, workspace: string, root: string): Promise<string[]> {
	const files = new Set<string>();
	const names = existsSync(buildDir) ? await readdir(buildDir, { recursive: true }) : [];
	for (const name of names.filter((entry) => entry.endsWith(".d"))) {
		for (const line of (await readFile(join(buildDir, name), "utf8")).split("\n")) {
			const colon = line.indexOf(": ");
			if (colon < 0) continue;
			for (const path of line.slice(colon + 2).split(/(?<!\\) /).map((part) => part.replaceAll("\\ ", " ").trim()).filter(Boolean)) {
				const repositoryPath = local(root, resolve(workspace, path));
				if (repositoryPath) files.add(repositoryPath);
			}
		}
	}
	if (!files.has(GPUI_CRATE_ROOT)) throw new Error(`Cargo dep-info under ${buildDir} does not list ${GPUI_CRATE_ROOT}, so the coverage map could not tell which Rust files the parity binary compiled`);
	return [...files].sort();
}

/** A repository-relative path, or undefined outside the repository. */
function local(root: string, path: string): string | undefined {
	const relativePath = relative(root, path);
	return relativePath.startsWith("..") || relativePath.startsWith(sep) ? undefined : relativePath.split(sep).join("/");
}

type Span = { path: string; range: Range };

/**
 * Per-file executed lines, executed function spans and coverable lines from
 * `llvm-cov export -format=lcov`. `spans` gives each function's span by mangled name.
 */
function parseLcov(text: string, root: string, spans: Map<string, Span>) {
	const files: ScenarioCoverage["files"] = {};
	const code: Record<string, Range[]> = {};
	const ran = new Map<string, Range[]>();
	let path: string | undefined;
	let executed: number[] = [];
	let coverable: number[] = [];
	for (const line of text.split("\n")) {
		if (line.startsWith("SF:")) {
			path = local(root, line.slice(3));
			executed = []; coverable = [];
		} else if (line.startsWith("DA:") && path) {
			const comma = line.indexOf(",", 3);
			const number = Number(line.slice(3, comma));
			coverable.push(number);
			const checksum = line.indexOf(",", comma + 1);
			if (Number(line.slice(comma + 1, checksum < 0 ? undefined : checksum)) > 0) executed.push(number);
		} else if (line.startsWith("FNDA:") && path) {
			const comma = line.indexOf(",");
			if (Number(line.slice(5, comma)) <= 0) continue;
			const span = spans.get(line.slice(comma + 1));
			if (!span) continue;
			const ranges = ran.get(span.path);
			if (ranges) ranges.push(span.range);
			else ran.set(span.path, [span.range]);
		} else if (line === "end_of_record" && path) {
			code[path] = toRanges(coverable);
			if (executed.length) files[path] = { lines: toRanges(executed), active: true };
			path = undefined;
		}
	}
	for (const [file, ranges] of ran) {
		const entry = files[file];
		if (entry) entry.functions = [...new Map(ranges.map((range) => [range.join(":"), range])).values()];
	}
	return { files, code };
}

/**
 * The line span of every instrumented function, by mangled name, from one JSON export. A line
 * without a count of its own inside a function, such as a closing brace or a match pattern, runs
 * whenever the innermost function around it runs.
 */
async function functionSpans({ cov, invoke }: CoverageTools, profile: string, executable: string, root: string, signal: AbortSignal): Promise<Map<string, Span>> {
	type Export = { data: { functions: { name: string; filenames: string[]; regions: number[][] }[] }[] };
	// SAFETY: `llvm-cov export -format=text` writes the export schema that Export describes, with `data[].functions[]` entries that carry name, filenames and regions.
	const exported = JSON.parse(await invoke([cov, "export", "-format=text", "-skip-expansions", `-instr-profile=${profile}`, executable, IGNORED, "-num-threads=1"], signal)) as Export;
	const spans = new Map<string, Span>();
	for (const fn of exported.data[0]?.functions ?? []) {
		const path = local(root, fn.filenames[0] ?? "");
		// Region: [lineStart, colStart, lineEnd, colEnd, count, fileId, expandedFileId, kind].
		const own = fn.regions.filter((region) => region[5] === 0);
		if (!path || !own.length) continue;
		spans.set(fn.name, { path, range: [Math.min(...own.map((region) => region[0] ?? 0)), Math.max(...own.map((region) => region[2] ?? 0))] });
	}
	return spans;
}

const IGNORED = "-ignore-filename-regex=/(\\.cargo|\\.rustup|rustc)/";

type CoverageTools = { cov: string; profdata: string; invoke: (command: string[], signal: AbortSignal) => Promise<string> };

/**
 * Converts each `coverage/gpui-<index>.profraw` the parity processes wrote and deletes it.
 * `ids` lists the selected scenario IDs by position, as in `scenarios.txt`.
 */
export async function recordGpuiCoverage(artifacts: string, executable: string, compiled: string[], ids: string[], jobs: number, signal: AbortSignal, tools?: CoverageTools): Promise<void> {
	const root = resolve(".");
	const directory = join(artifacts, "coverage");
	const names = await readdir(directory);
	const pending = new Map<number, string>();
	for (const name of names) {
		const digits = /^gpui-(\d+)\.profraw$/.exec(name)?.[1];
		if (digits === undefined) continue;
		const index = Number(digits);
		if (!ids[index] || pending.has(index)) throw new Error(`Unexpected or duplicate GPUI scenario profile: ${name}`);
		pending.set(index, name);
	}
	const queue = pending.entries();
	const code = new Map<string, { index: number; ranges: Range[] }>();
	const info: CodeInfo = { code: {}, compiled };
	const controller = new AbortController();
	const abort = AbortSignal.any([signal, controller.signal]);
	const conversion = tools ?? await llvmTools(abort);
	const { cov, profdata, invoke } = conversion;
	let spans: Promise<Map<string, Span>> | undefined;
	const worker = async () => {
		for (const [index, name] of queue) {
			abort.throwIfAborted();
			const raw = join(directory, name);
			const merged = join(directory, `gpui-${index}.profdata`);
			await invoke([profdata, "merge", "-sparse", "--num-threads=1", raw, "-o", merged], abort);
			// Spans depend only on the binary. Keep a single metadata export for all workers.
			spans ??= functionSpans(conversion, merged, executable, root, abort);
			const functions = await spans;
			const lcov = await invoke([cov, "export", "-format=lcov", `-instr-profile=${merged}`, executable, IGNORED, "-num-threads=1"], abort);
			const parsed = parseLcov(lcov, root, functions);
			for (const [path, ranges] of Object.entries(parsed.code)) {
				const previous = code.get(path);
				if (!previous || previous.index < index) code.set(path, { index, ranges });
			}
			await writeFile(join(directory, `gpui-${index}.json`), JSON.stringify({ id: ids[index], files: parsed.files } satisfies ScenarioCoverage));
			await Promise.all([rm(raw), rm(merged)]);
		}
	};
	await Promise.allSettled(Array.from({ length: Math.min(jobs, pending.size) }, () =>
		worker().catch((error) => { controller.abort(error); throw error; }),
	));
	abort.throwIfAborted();
	info.code = Object.fromEntries([...code].map(([path, { ranges }]) => [path, ranges]));
	// Every span, including functions no scenario ran, so selection knows a line is inside one.
	const all = new Map<string, Map<string, Range>>();
	for (const span of (await spans)?.values() ?? []) all.set(span.path, (all.get(span.path) ?? new Map()).set(span.range.join(":"), span.range));
	info.functions = Object.fromEntries([...all].map(([path, ranges]) => [path, [...ranges.values()]]));
	await writeFile(join(directory, "gpui-code.json"), JSON.stringify(info));
}
