// The parity coverage map: which source lines each shared scenario executed, recorded by a
// `PARITY_COVERAGE=1` run and read by `PARITY_CHANGED` selection (changed-scenarios.ts).
// Usage: bun e2e/parity/coverage-map.ts merge <out.json> <shard-map.json>...
import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { count, object, string, strings } from "./contract-values";

/** Inclusive 1-based line range. */
export type Range = [number, number];
/**
 * What one scenario executed in one file. `active` is whether code beyond the module's top level
 * ran. For Svelte, `code` lists the lines compiled component code maps to, executed or not. For
 * Rust, `functions` lists the line spans of the functions that ran.
 */
export type FileCoverage = { lines: Range[]; active: boolean; code?: Range[]; functions?: Range[] };
export type ScenarioCoverage = { id: string; files: Record<string, FileCoverage> };

/**
 * Scenario sets are indices into `scenarios`, stored once in `sets`. Per file, `lines` holds
 * flat `[set, start, end]` triples, `loaded` the scenarios that executed anything there, and
 * `active` those that ran code beyond the top level (for Svelte, rendered the component).
 * `code` holds flat `[start, end]` pairs of lines that have code of their own: Rust lines with
 * a coverage count, and Svelte lines that compiled component code maps to. `functions` holds
 * flat `[set, start, end]` triples: each Rust function's line span and the scenarios that ran it.
 */
export type CoverageMap = {
	version: 1;
	commit: string;
	scenarios: string[];
	sets: number[][];
	/** Repository files the instrumented parity binary compiled, from cargo dep-info. */
	compiled: string[];
	files: Record<string, { loaded: number; active: number; lines: number[]; code?: number[]; functions?: number[] }>;
};

export type CodeInfo = { code: Record<string, Range[]>; compiled: string[]; functions?: Record<string, Range[]> };

export function toRanges(lines: Iterable<number>): Range[] {
	const sorted = [...new Set(lines)].sort((left, right) => left - right);
	const ranges: Range[] = [];
	for (const line of sorted) {
		const last = ranges.at(-1);
		if (last && line === last[1] + 1) last[1] = line;
		else ranges.push([line, line]);
	}
	return ranges;
}

export const expand = ([start, end]: Range): number[] => Array.from({ length: end - start + 1 }, (_, offset) => start + offset);

export function encodeMap(commit: string, records: ScenarioCoverage[], info: CodeInfo): CoverageMap {
	const sets: number[][] = [];
	const setIndex = new Map<string, number>();
	const intern = (ids: number[]) => {
		const key = ids.join(",");
		let index = setIndex.get(key);
		if (index === undefined) { index = sets.length; sets.push(ids); setIndex.set(key, index); }
		return index;
	};
	const byFile = new Map<string, { loaded: number[]; active: number[]; lines: Map<number, number[]>; functions: Map<string, number[]> }>();
	for (const [index, record] of records.entries()) {
		for (const [path, file] of Object.entries(record.files)) {
			let entry = byFile.get(path);
			if (!entry) { entry = { loaded: [], active: [], lines: new Map(), functions: new Map() }; byFile.set(path, entry); }
			entry.loaded.push(index);
			if (file.active) entry.active.push(index);
			for (const range of file.functions ?? []) {
				const key = range.join(":");
				entry.functions.set(key, [...(entry.functions.get(key) ?? []), index]);
			}
			for (const [start, end] of file.lines) {
				for (let line = start; line <= end; line++) {
					const ids = entry.lines.get(line);
					if (ids) { if (ids.at(-1) !== index) ids.push(index); } else entry.lines.set(line, [index]);
				}
			}
		}
	}
	const files: CoverageMap["files"] = {};
	// Rust files no scenario ran still list their functions, so a change there selects nothing.
	const paths = new Set([...byFile.keys(), ...Object.keys(info.code), ...Object.keys(info.functions ?? {})]);
	for (const path of [...paths].sort()) {
		const entry = byFile.get(path) ?? { loaded: [], active: [], lines: new Map(), functions: new Map() };
		const lines: number[] = [];
		let run: { set: number; start: number; end: number } | undefined;
		for (const line of [...entry.lines.keys()].sort((left, right) => left - right)) {
			const set = intern(entry.lines.get(line) ?? []);
			if (run && run.set === set && run.end === line - 1) run.end = line;
			else { if (run) lines.push(run.set, run.start, run.end); run = { set, start: line, end: line }; }
		}
		if (run) lines.push(run.set, run.start, run.end);
		const code = info.code[path];
		const functions = [...new Set([...(info.functions?.[path] ?? []).map((range) => range.join(":")), ...entry.functions.keys()])]
			.sort((left, right) => Number(left.split(":")[0]) - Number(right.split(":")[0]))
			.flatMap((key) => [intern(entry.functions.get(key) ?? []), ...key.split(":").map(Number)]);
		const file: CoverageMap["files"][string] = { loaded: intern(entry.loaded), active: intern(entry.active), lines };
		if (code) file.code = code.flat();
		if (functions.length) file.functions = functions;
		files[path] = file;
	}
	return { version: 1, commit, scenarios: records.map((record) => record.id), sets, compiled: [...new Set(info.compiled)].sort(), files };
}

export function decodeMap(map: CoverageMap) {
	const records: ScenarioCoverage[] = map.scenarios.map((id) => ({ id, files: {} }));
	const code: Record<string, Range[]> = {};
	const functions: Record<string, Range[]> = {};
	const member = (set: number) => map.sets[set] ?? [];
	for (const [path, file] of Object.entries(map.files)) {
		for (const index of member(file.loaded)) {
			const record = records[index];
			if (record) record.files[path] = { lines: [], active: false };
		}
		for (const index of member(file.active)) {
			const entry = records[index]?.files[path];
			if (entry) entry.active = true;
		}
		for (let offset = 0; offset + 2 < file.lines.length; offset += 3) {
			const [set, start, end] = [file.lines[offset] ?? 0, file.lines[offset + 1] ?? 0, file.lines[offset + 2] ?? 0];
			for (const index of member(set)) records[index]?.files[path]?.lines.push([start, end]);
		}
		if (file.code) code[path] = pairs(file.code);
		for (const [set, start, end] of triples(file.functions ?? [])) {
			functions[path] = [...(functions[path] ?? []), [start, end]];
			for (const index of member(set)) {
				const entry = records[index]?.files[path];
				if (entry) entry.functions = [...(entry.functions ?? []), [start, end]];
			}
		}
	}
	return { records, info: { code, compiled: map.compiled, functions } };
}

export function triples(flat: number[]): [number, number, number][] {
	const result: [number, number, number][] = [];
	for (let offset = 0; offset + 2 < flat.length; offset += 3) result.push([flat[offset] ?? 0, flat[offset + 1] ?? 0, flat[offset + 2] ?? 0]);
	return result;
}

export function pairs(flat: number[]): Range[] {
	const ranges: Range[] = [];
	for (let offset = 0; offset + 1 < flat.length; offset += 2) ranges.push([flat[offset] ?? 0, flat[offset + 1] ?? 0]);
	return ranges;
}

/** A flat list of whole numbers in groups of `group`: a cut-off group would read as no coverage. */
function flat<Value>(value: Value, group: number): number[] {
	if (!Array.isArray(value) || value.length % group !== 0) throw new Error(`Expected whole numbers in groups of ${group}`);
	return value.map((item) => count(item));
}

/**
 * Parses a map file. A damaged map is rejected here: a record of another shape or a reference
 * past the end would read as "no scenario ran this", which selects nothing.
 */
function parseMap<Value>(value: Value): CoverageMap {
	const input = object(value);
	if (input.version !== 1 || !Array.isArray(input.sets)) throw new Error("Expected a version 1 map with scenario sets");
	const scenarios = strings(input.scenarios, true);
	const sets = input.sets.map((set) => flat(set, 1));
	const below = (limit: number, index: number) => {
		if (index >= limit) throw new Error(`Reference ${index} is past the end`);
		return index;
	};
	for (const set of sets) for (const index of set) below(scenarios.length, index);
	const spans = (list: number[]) => {
		for (const [set] of triples(list)) below(sets.length, set);
		return list;
	};
	const files: CoverageMap["files"] = {};
	for (const [path, entry] of Object.entries(object(input.files))) {
		const file = object(entry);
		files[path] = { loaded: below(sets.length, count(file.loaded)), active: below(sets.length, count(file.active)), lines: spans(flat(file.lines, 3)) };
		if (file.code !== undefined) files[path].code = flat(file.code, 2);
		if (file.functions !== undefined) files[path].functions = spans(flat(file.functions, 3));
	}
	return { version: 1, commit: string(input.commit), scenarios, sets, compiled: strings(input.compiled, true), files };
}

export async function readMap(path: string): Promise<CoverageMap> {
	try {
		return parseMap(JSON.parse(await readFile(path, "utf8")));
	} catch (error) {
		throw new Error(`${path} is not a complete version 1 parity coverage map`, { cause: error });
	}
}

async function passed(artifacts: string, driver: string): Promise<Set<string>> {
	try {
		// SAFETY: a driver report is written as { driver, scenarios }. A report of another shape throws in the filter below, and the catch returns no passes.
		const report = JSON.parse(await readFile(join(artifacts, `${driver}.json`), "utf8")) as { scenarios: { id: string; status: string }[] };
		return new Set(report.scenarios.filter((result) => result.status === "passed").map((result) => result.id));
	} catch { return new Set(); }
}

async function gitHead(): Promise<string> {
	const git = Bun.spawn(["git", "rev-parse", "HEAD"], { stdout: "pipe" });
	const commit = (await new Response(git.stdout).text()).trim();
	if (await git.exited !== 0) throw new Error("git rev-parse HEAD failed");
	return commit;
}

/**
 * Builds the map for one run from its per-scenario files in `<artifacts>/coverage/`. Only
 * scenarios both drivers passed and recorded count; selection always runs the others.
 */
export async function writeRunMap(artifacts: string, out: string): Promise<CoverageMap> {
	const directory = join(artifacts, "coverage");
	const ids = (await readFile(join(artifacts, "scenarios.txt"), "utf8")).split("\n").filter(Boolean);
	const [web, gpui] = await Promise.all([passed(artifacts, "web"), passed(artifacts, "gpui")]);
	const names = new Set(await readdir(directory));
	// SAFETY: each per-scenario file is written by startCodeCoverage().save as { id, files }. writeRunMap checks the id below.
	const read = async (name: string) => names.has(name) ? JSON.parse(await readFile(join(directory, name), "utf8")) as ScenarioCoverage : undefined;
	const records: ScenarioCoverage[] = [];
	for (const [index, id] of ids.entries()) {
		if (!web.has(id) || !gpui.has(id)) continue;
		const [webFiles, gpuiFiles] = await Promise.all([read(`web-${index}.json`), read(`gpui-${index}.json`)]);
		if (webFiles?.id !== id || gpuiFiles?.id !== id) continue;
		records.push({ id, files: { ...webFiles.files, ...gpuiFiles.files } });
	}
	// SAFETY: recordGpuiCoverage in gpui-coverage-map.ts writes gpui-code.json as a CodeInfo.
	const info = JSON.parse(await readFile(join(directory, "gpui-code.json"), "utf8")) as CodeInfo;
	// A function V8 never compiled in one scenario can be compiled in another, so take the union.
	const webCode = new Map<string, Set<number>>();
	for (const record of records) {
		for (const [path, file] of Object.entries(record.files)) {
			if (!file.code) continue;
			const lines = webCode.get(path) ?? new Set();
			for (const range of file.code) for (const line of expand(range)) lines.add(line);
			webCode.set(path, lines);
			delete file.code;
		}
	}
	for (const [path, lines] of webCode) info.code[path] = toRanges(lines);
	const map = encodeMap(await gitHead(), records, info);
	await writeFile(out, JSON.stringify(map));
	return map;
}

/** Merges shard maps recorded at one commit into one map. */
export async function mergeMaps(out: string, inputs: string[]): Promise<CoverageMap> {
	const maps = await Promise.all(inputs.map(readMap));
	const commits = new Set(maps.map((map) => map.commit));
	if (commits.size !== 1) throw new Error(`Shard maps come from different commits: ${[...commits].join(", ")}`);
	const records = new Map<string, ScenarioCoverage>();
	const info: CodeInfo = { code: {}, compiled: [], functions: {} };
	for (const map of maps) {
		const decoded = decodeMap(map);
		for (const record of decoded.records) records.set(record.id, record);
		for (const [path, ranges] of Object.entries(decoded.info.code)) info.code[path] = toRanges([...(info.code[path] ?? []), ...ranges].flatMap(expand));
		Object.assign(info.functions ?? {}, decoded.info.functions);
		info.compiled.push(...decoded.info.compiled);
	}
	const map = encodeMap(maps[0]?.commit ?? "", [...records.values()], info);
	// CI merges into e2e/artifacts/, which a fresh checkout lacks.
	await mkdir(dirname(out), { recursive: true });
	await writeFile(out, JSON.stringify(map));
	return map;
}

if (import.meta.main) {
	const [command, out, ...inputs] = process.argv.slice(2);
	if (command !== "merge" || !out || !inputs.length) throw new Error("Usage: bun e2e/parity/coverage-map.ts merge <out.json> <shard-map.json>...");
	const map = await mergeMaps(out, inputs);
	console.log(`Merged ${inputs.length} maps: ${map.scenarios.length} scenarios, ${Object.keys(map.files).length} files`);
}
