// PARITY_CHANGED selection: the scenarios a diff can affect, from the coverage map
// (coverage-map.ts). See "Change-based selection" in e2e/parity/README.md for the rules.
// Usage: bun e2e/parity/changed-scenarios.ts <base> [<head>] [--map <path>]
import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { type CoverageMap, pairs, readMap, triples } from "./coverage-map";

export const DEFAULT_MAP = "e2e/artifacts/parity-coverage-map.json";

/** Changes coverage cannot see: build inputs, the parity harness, and its workflows. */
const FULL_SUITE = [
	/^(package\.json|bun\.lock|bunfig\.toml|vite\.config\.ts|svelte\.config\.js|index\.html)$/,
	/^tsconfig[^/]*\.json$/,
	/(^|\/)(Cargo\.toml|Cargo\.lock|build\.rs|rust-toolchain(\.toml)?)$/,
	/^(\.cargo|public)\//,
	// The runner, drivers, contract and fixture exporter; scenario files select their own scenarios.
	/^e2e\/parity\/(?!scenarios\/[^/]+\.json$)(?!.*\.md$)/,
	/^scripts\/(e2e-parity|dev-interactions-plugin|frontend-bundle-report)\.ts$/,
	/^\.agents\/skills\/verify-kubecove\/scripts\/processes\.ts$/,
	/^src-tauri\/gpui\/tests\/(parity\.rs|parity\/|common\/)/,
	/^\.github\/workflows\/parity[^/]*\.yml$/,
	// GPUI fixture consumers are not attributed to individual mock calls.
	/^src\/lib\/(tauri-dev-mock[^/]*|mcp-dev-mocks)\.ts$/,
	// Files the native crates embed, such as fonts, icons and configuration. The map lists
	// only what cargo's dep-info names, and it has no line coverage for any of them.
	/^src-tauri\/(?!server\/)(?!.*\.(rs|md)$)/,
];
const SCENARIO_FILE = /^e2e\/parity\/scenarios\/[^/]+\.json$/;
// A line that is only comment text: code after a closing `*/` makes it a code line.
const COMMENT_LINE = String.raw`\/\/.*|\/\*(?:(?!\*\/).)*(?:\*\/\s*)?|\*\/\s*|\*(?:\s(?:(?!\*\/).)*(?:\*\/\s*)?)?`;
const COMMENT = new RegExp(`^\\s*(${COMMENT_LINE})?$`);
const DECLARED = /\b(?:const|static|struct|enum|union|type|trait)\s+(?:mut\s+)?([A-Za-z_]\w*)|\bimpl(?:<[^>]*>)?\s+(?:[\w:]+\s+for\s+)?([A-Za-z_]\w*)/g;
// Git's hunk header names the enclosing item. An `impl` header says only which type a method
// belongs to, so it names no changed item.
const DECLARED_CONTEXT = /\b(?:const|static|struct|enum|union|type|trait)\s+(?:mut\s+)?([A-Za-z_]\w*)/g;
// Rust lines outside functions that have no coverage count of their own: comments, blank lines,
// braces, and `use` and `mod` declarations. IMPORT handles a change to the last two.
const SKIPPED = new RegExp(`^\\s*(${COMMENT_LINE}|[{}()[\\];,]*|(pub(\\([^)]*\\))?\\s+)?(use|mod)\\s.*)$`);
// A changed `use` or `mod` line can give a name another meaning in every item of the file.
const IMPORT = /^\s*(pub(\([^)]*\))?\s+)?(use|mod|extern\s+crate)\s/;
const REEXPORT = /^\s*pub(\([^)]*\))?\s+(use|mod)\s/;

type Hunk = { oldStart: number; oldCount: number; newStart: number; newCount: number; context: string; removed: string[]; added: string[] };
type FileDiff = { oldPath?: string; newPath?: string; hunks: Hunk[]; binary: boolean };
export type Selection = { all?: string; ids: Set<string>; because: Record<string, string> };

async function git(args: string[]): Promise<{ code: number; stdout: string }> {
	const child = Bun.spawn(["git", "-c", "core.quotePath=false", ...args], { stdout: "pipe", stderr: "pipe" });
	const stdout = await new Response(child.stdout).text();
	return { code: await child.exited, stdout };
}
async function gitOut(args: string[]): Promise<string> {
	const result = await git(args);
	if (result.code !== 0) throw new Error(`git ${args.join(" ")} exited ${result.code}`);
	return result.stdout;
}

function parseDiff(text: string): FileDiff[] {
	const files: FileDiff[] = [];
	let file: FileDiff | undefined;
	let hunk: Hunk | undefined;
	const path = (value: string) => value === "/dev/null" ? undefined : value.replace(/^[ab]\//, "");
	// With --no-renames both sides of `diff --git a/<path> b/<path>` name one path. A binary
	// diff has no ---/+++ lines, so its paths come from this header.
	let headerPath: string | undefined;
	for (const line of text.split("\n")) {
		if (line.startsWith("diff --git ")) {
			file = { hunks: [], binary: false }; files.push(file); hunk = undefined;
			const sides = line.slice("diff --git ".length);
			const half = (sides.length - 1) / 2;
			headerPath = sides.startsWith("a/") && sides.slice(half) === ` b/${sides.slice(2, half)}` ? sides.slice(2, half) : undefined;
			continue;
		}
		if (!file) continue;
		const header = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@ ?(.*)$/.exec(line);
		if (header) {
			hunk = { oldStart: Number(header[1]), oldCount: Number(header[2] ?? 1), newStart: Number(header[3]), newCount: Number(header[4] ?? 1), context: header[5] ?? "", removed: [], added: [] };
			file.hunks.push(hunk);
		} else if (!hunk && line.startsWith("--- ")) file.oldPath = path(line.slice(4));
		else if (!hunk && line.startsWith("+++ ")) file.newPath = path(line.slice(4));
		else if (!hunk && line.startsWith("Binary files ")) {
			file.binary = true;
			if (!line.startsWith("Binary files /dev/null ")) file.oldPath = headerPath;
			if (!line.endsWith(" and /dev/null differ")) file.newPath = headerPath;
		}
		else if (hunk && line.startsWith("-")) hunk.removed.push(line.slice(1));
		else if (hunk && line.startsWith("+")) hunk.added.push(line.slice(1));
	}
	return files;
}

/** Old-side lines a hunk changes; a pure insertion touches the lines on both sides of it. */
function touched(hunk: Hunk): number[] {
	if (hunk.oldCount > 0) return Array.from({ length: hunk.oldCount }, (_, offset) => hunk.oldStart + offset);
	return [hunk.oldStart, hunk.oldStart + 1].filter((line) => line > 0);
}

/** Maps a line at one commit to its lines at another; lines changed between them map to the whole changed range. */
function translateLine(hunks: Hunk[], line: number): number[] {
	let delta = 0;
	for (const hunk of hunks) {
		const end = hunk.oldCount > 0 ? hunk.oldStart + hunk.oldCount - 1 : hunk.oldStart;
		if (hunk.oldCount > 0 && line >= hunk.oldStart && line <= end) return hunk.newCount > 0 ? Array.from({ length: hunk.newCount }, (_, offset) => hunk.newStart + offset) : [hunk.newStart, hunk.newStart + 1];
		if (line <= end) break;
		delta += hunk.newCount - hunk.oldCount;
	}
	return [line + delta];
}

const within = (ranges: number[], line: number) => pairs(ranges).some(([start, end]) => line >= start && line <= end);

/**
 * The scenarios to run for the changes from `base`'s merge base to `head`, or to the working
 * tree and untracked files when `head` is omitted. `all` names why the full suite must run.
 */
export async function changedScenarios(base: string, scenarioIds: string[], mapPath = DEFAULT_MAP, head?: string): Promise<Selection> {
	const selection: Selection = { ids: new Set(), because: {} };
	const add = (id: string, cause: string) => { if (!selection.ids.has(id)) { selection.ids.add(id); selection.because[id] = cause; } };
	const full = (reason: string): Selection => ({ all: reason, ids: new Set(scenarioIds), because: {} });
	const mergeBase = (await gitOut(["merge-base", base, head ?? "HEAD"])).trim();
	const diffs = parseDiff(await gitOut(["diff", "-U0", "--no-renames", "--no-color", "--no-ext-diff", mergeBase, ...(head ? [head] : [])]));
	if (!head) for (const path of (await gitOut(["ls-files", "--others", "--exclude-standard"])).split("\n").filter(Boolean)) diffs.push({ newPath: path, hunks: [], binary: false });
	const show = async (path: string) => head ? gitOut(["show", `${head}:${path}`]) : readFile(path, "utf8");
	const code: FileDiff[] = [];
	for (const diff of diffs) {
		if (!diff.oldPath && !diff.newPath) return full("a changed file's path could not be read from the diff");
		const fullMatch = [diff.oldPath, diff.newPath].find((path) => path && FULL_SUITE.some((rule) => rule.test(path)));
		if (fullMatch) return full(`${fullMatch} changed, which coverage cannot see`);
		if (diff.newPath && SCENARIO_FILE.test(diff.newPath)) {
			// SAFETY: a scenario file reaches a parity run only through parseContract in contract.ts, which requires each scenario's string id. This scan reads nothing else.
			const contract = JSON.parse(await show(diff.newPath)) as { scenarios?: { id?: string }[] };
			for (const scenario of contract.scenarios ?? []) if (scenario.id) add(scenario.id, `${diff.newPath} changed`);
		} else if (!diff.oldPath || !SCENARIO_FILE.test(diff.oldPath)) code.push(diff);
	}
	if (!existsSync(mapPath)) return full(`no coverage map at ${mapPath}`);
	const map = await readMap(mapPath);
	if ((await git(["cat-file", "-e", `${map.commit}^{commit}`])).code !== 0) return full(`coverage map commit ${map.commit} is not in this clone`);
	const recorded = new Set(map.scenarios);
	for (const id of scenarioIds) if (!recorded.has(id)) add(id, "no recorded coverage");
	const pick = (set: number, cause: string) => { for (const index of map.sets[set] ?? []) { const id = map.scenarios[index]; if (id) add(id, cause); } };
	const translations = new Map<string, Hunk[]>();
	const translate = async (path: string, line: number) => {
		if (mergeBase === map.commit) return [line];
		let hunks = translations.get(path);
		if (!hunks) { hunks = parseDiff(await gitOut(["diff", "-U0", "--no-renames", "--no-ext-diff", mergeBase, map.commit, "--", path]))[0]?.hunks ?? []; translations.set(path, hunks); }
		return translateLine(hunks, line);
	};
	const compiled = new Set(map.compiled);
	for (const diff of code) {
		const reason = await selectFile(diff, map, compiled, mergeBase, translate, pick, head);
		if (reason) return full(reason);
	}
	return selection;
}

/** Adds the scenarios one changed file can affect through `pick`, or returns why the full suite must run. */
async function selectFile(diff: FileDiff, map: CoverageMap, compiled: Set<string>, mergeBase: string, translate: (path: string, line: number) => Promise<number[]>, pick: (set: number, cause: string) => void, head?: string): Promise<string | undefined> {
	const webAsset = (path: string) => path.startsWith("src/") && !/\.(ts|js|mjs|svelte|css|json|md)$/.test(path);
	const path = diff.oldPath;
	if (!path) return diff.newPath && webAsset(diff.newPath) ? `new asset ${diff.newPath} has no recorded coverage` : undefined;
	const rust = path.endsWith(".rs");
	const record = map.files[path];
	if (compiled.has(path) && !rust) return `${path} is compiled into the GPUI parity binary`;
	if (!record && !(rust && compiled.has(path))) return webAsset(path) ? `${path} has no recorded coverage` : undefined;
	if (record && (diff.binary || !diff.hunks.length || (!record.lines.length && !record.code))) { pick(record.loaded, path); return undefined; }
	const base = (await gitOut(["show", `${mergeBase}:${path}`])).split("\n");
	const code = record?.code ?? [];
	/** Picks the scenarios that executed any of `targets`, and says whether any did. */
	const lineLevel = (targets: number[], cause: string): boolean => {
		if (!record) return false;
		let hit = false;
		for (let offset = 0; offset + 2 < record.lines.length; offset += 3) {
			const [set, start, end] = [record.lines[offset] ?? 0, record.lines[offset + 1] ?? 0, record.lines[offset + 2] ?? 0];
			if (targets.some((target) => target >= start && target <= end)) { pick(set, cause); hit = true; }
		}
		return hit;
	};
	// A Rust line without a count of its own inside a function, such as a closing brace, a match
	// pattern or a comment, runs whenever the innermost function around it runs.
	const innermost = (target: number): number | undefined => {
		let best: [number, number, number] | undefined;
		for (const span of triples(record?.functions ?? [])) {
			if (target >= span[1] && target <= span[2] && (!best || span[2] - span[1] < best[2] - best[1])) best = span;
		}
		return best?.[0];
	};
	const declared: string[] = [];
	let outside = false;
	if (rust) {
		const imports = diff.hunks.filter((hunk) => [...hunk.removed, ...hunk.added].some((line) => IMPORT.test(line)));
		// Other files reach a re-exported name through this one, so a removed or replaced
		// re-export can change code that this file's coverage says nothing about.
		if (imports.some((hunk) => hunk.removed.some((line) => REEXPORT.test(line)))) return `${path} changed a re-export, which other files can name`;
		if (imports.length) {
			if (!record) return `${path} changed its imports and has no recorded coverage`;
			pick(record.loaded, `${path} (imports)`);
		}
	}
	for (const hunk of diff.hunks) {
		const commentOnly = [...hunk.removed, ...hunk.added].every((line) => COMMENT.test(line));
		if (commentOnly && /\.(rs|ts|js|mjs)$/.test(path)) continue;
		let [reached, hunkOutside, hit] = [false, false, false];
		for (const line of touched(hunk)) {
			const targets = await translate(path, line);
			const cause = `${path}:${line}`;
			if (path.endsWith(".svelte") && !targets.some((target) => within(code, target))) {
				// Markup with no component code of its own, such as static elements or styles.
				if (record) pick(record.active, `${cause} (markup)`);
				reached = hit = true;
				continue;
			}
			if (!rust) { hit = lineLevel(targets, cause) || hit; reached = true; continue; }
			for (const target of targets) {
				const set = innermost(target);
				if (within(code, target)) lineLevel([target], cause);
				else if (set !== undefined) pick(set, `${cause} (function)`);
				else if (!SKIPPED.test(base[line - 1] ?? "")) hunkOutside = true;
				else continue;
				reached = true;
			}
		}
		// Lines inserted where no recorded code ran, such as top-level code between two
		// functions no scenario called, run for every scenario that loads the module.
		if (!rust && !hit && record && hunk.removed.every((line) => COMMENT.test(line))) {
			pick(path.endsWith(".svelte") ? record.active : record.loaded, `${path}:${hunk.oldStart} (insertion outside recorded code)`);
		}
		// Rust lines outside functions: items such as constants, types and trait impls. A hunk
		// that touches only skipped lines there still removes what it contains; a new function
		// it inserts runs only through callers, whose own changes select their scenarios.
		if (rust && (!reached || hunkOutside)) {
			for (const text of [...hunk.removed, ...hunk.added]) for (const match of text.matchAll(DECLARED)) declared.push(match[1] ?? match[2] ?? "");
			for (const match of hunk.context.matchAll(DECLARED_CONTEXT)) declared.push(match[1] ?? "");
			outside ||= hunkOutside || hunk.removed.some((line) => !SKIPPED.test(line));
		}
	}
	const names = [...new Set(declared.filter(Boolean))];
	if (names.length) {
		// Code that names a changed item can change with it.
		const grep = await git(["grep", "-lwF", ...names.flatMap((name) => ["-e", name]), ...(head ? [head] : []), "--", "*.rs"]);
		const users = grep.stdout.split("\n").filter(Boolean).map((line) => head ? line.slice(head.length + 1) : line);
		for (const user of users) { const file = map.files[user]; if (file && compiled.has(user)) pick(file.loaded, `${path} item used in ${user}`); }
	} else if (outside) {
		if (!record) return `${path} changed items outside recorded code`;
		pick(record.loaded, `${path} (item)`);
	}
	return undefined;
}

if (import.meta.main) {
	const args = process.argv.slice(2);
	const mapFlag = args.indexOf("--map");
	const mapPath = mapFlag >= 0 ? args.splice(mapFlag, 2)[1] : undefined;
	const [base, head] = args;
	if (!base) throw new Error("Usage: bun e2e/parity/changed-scenarios.ts <base> [<head>] [--map <path>]");
	const { loadContract } = await import("./contract");
	const ids = (await loadContract()).scenarios.map((scenario) => scenario.id);
	const selection = await changedScenarios(base, ids, mapPath ?? DEFAULT_MAP, head);
	console.log(JSON.stringify({ total: ids.length, selected: selection.ids.size, all: selection.all, because: selection.because }, null, 2));
}
