// Turns the per-scenario LLVM profiles of a PARITY_COVERAGE=1 GPUI run into coverage map
// inputs (coverage-map.ts): `coverage/gpui-<index>.json` and `coverage/gpui-code.json`.
import { existsSync } from "node:fs";
import { readdir, readFile, rm, writeFile } from "node:fs/promises";
import { join, relative, resolve, sep } from "node:path";
import type { CodeInfo, Range, ScenarioCoverage } from "./coverage-map";
import { toRanges } from "./coverage-map";

/** Flags the coverage build adds; `code_coverage.rs` in the GPUI parity tests reads the cfg. */
export const COVERAGE_RUSTFLAGS = "-C instrument-coverage --cfg kubecove_coverage";

async function output(command: string[], cwd?: string): Promise<string> {
	const child = Bun.spawn(command, { cwd, stdout: "pipe", stderr: "pipe" });
	const [stdout, stderr] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text()]);
	if (await child.exited !== 0) throw new Error(`${command.join(" ")} failed: ${stderr.trim()}`);
	return stdout;
}

/** LLVM_COV or LLVM_PROFDATA, else the rustup `llvm-tools` component, else PATH. */
async function llvmTool(name: "llvm-cov" | "llvm-profdata", workspace: string): Promise<string> {
	const override = process.env[name === "llvm-cov" ? "LLVM_COV" : "LLVM_PROFDATA"];
	if (override) return override;
	const sysroot = (await output(["rustc", "--print", "sysroot"], workspace)).trim();
	const host = /^host: (.+)$/m.exec(await output(["rustc", "-vV"], workspace))?.[1];
	const bundled = join(sysroot, "lib", "rustlib", host ?? "", "bin", name);
	return existsSync(bundled) ? bundled : name;
}

/** Repository files listed in cargo dep-info (`*.d`) under `dirs`, including `include_*!` inputs. */
async function compiledFiles(dirs: string[], root: string): Promise<string[]> {
	const files = new Set<string>();
	for (const dir of dirs) {
		if (!existsSync(dir)) continue;
		for (const name of (await readdir(dir)).filter((entry) => entry.endsWith(".d"))) {
			for (const line of (await readFile(join(dir, name), "utf8")).split("\n")) {
				const colon = line.indexOf(": ");
				if (colon < 0) continue;
				for (const path of line.slice(colon + 2).split(/(?<!\\) /).map((part) => part.replaceAll("\\ ", " ").trim()).filter(Boolean)) {
					const local = relative(root, resolve(path));
					if (!local.startsWith("..") && !local.startsWith(sep)) files.add(local.split(sep).join("/"));
				}
			}
		}
	}
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
function parseLcov(text: string, root: string, spans: Map<string, Span>): { files: ScenarioCoverage["files"]; code: Record<string, Range[]> } {
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
			const [number, count] = line.slice(3).split(",").map(Number);
			coverable.push(number ?? 0);
			if ((count ?? 0) > 0) executed.push(number ?? 0);
		} else if (line.startsWith("FNDA:") && path) {
			const comma = line.indexOf(",");
			const span = spans.get(line.slice(comma + 1));
			if (Number(line.slice(5, comma)) > 0 && span) ran.set(span.path, [...(ran.get(span.path) ?? []), span.range]);
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
async function functionSpans(cov: string, profile: string, executable: string, root: string): Promise<Map<string, Span>> {
	type Export = { data: { functions: { name: string; filenames: string[]; regions: number[][] }[] }[] };
	const exported = JSON.parse(await output([cov, "export", "-format=text", "-skip-expansions", `-instr-profile=${profile}`, executable, IGNORED])) as Export;
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

/**
 * Converts each `coverage/gpui-<index>.profraw` the parity processes wrote and deletes it.
 * `ids` lists the selected scenario IDs by position, as in `scenarios.txt`.
 */
export async function recordGpuiCoverage(artifacts: string, executable: string, depDirs: string[], ids: string[], jobs: number, signal: AbortSignal): Promise<void> {
	const root = resolve(".");
	const workspace = resolve("src-tauri/gpui");
	const [cov, profdata] = await Promise.all([llvmTool("llvm-cov", workspace), llvmTool("llvm-profdata", workspace)]);
	const directory = join(artifacts, "coverage");
	const names = await readdir(directory);
	const pending = names.flatMap((name) => {
		const index = /^gpui-(\d+)\.profraw$/.exec(name)?.[1];
		return index === undefined ? [] : [Number(index)];
	});
	const info: CodeInfo = { code: {}, compiled: await compiledFiles(depDirs, root) };
	let spans: Promise<Map<string, Span>> | undefined;
	const worker = async () => {
		for (let index = pending.shift(); index !== undefined; index = pending.shift()) {
			signal.throwIfAborted();
			const raw = join(directory, `gpui-${index}.profraw`);
			const merged = join(directory, `gpui-${index}.profdata`);
			await output([profdata, "merge", "-sparse", raw, "-o", merged]);
			const lcov = await output([cov, "export", "-format=lcov", `-instr-profile=${merged}`, executable, IGNORED]);
			// Spans depend only on the binary, so the first profile gives them; one export at a time.
			spans ??= functionSpans(cov, merged, executable, root);
			const parsed = parseLcov(lcov, root, await spans);
			await rm(raw); await rm(merged);
			Object.assign(info.code, parsed.code);
			await writeFile(join(directory, `gpui-${index}.json`), JSON.stringify({ id: ids[index] ?? "", files: parsed.files } satisfies ScenarioCoverage));
		}
	};
	await Promise.all(Array.from({ length: Math.max(1, jobs) }, worker));
	// Every span, including functions no scenario ran, so selection knows a line is inside one.
	const all = new Map<string, Map<string, Range>>();
	for (const span of (await spans)?.values() ?? []) all.set(span.path, (all.get(span.path) ?? new Map()).set(span.range.join(":"), span.range));
	info.functions = Object.fromEntries([...all].map(([path, ranges]) => [path, [...ranges.values()]]));
	await writeFile(join(directory, "gpui-code.json"), JSON.stringify(info));
}
