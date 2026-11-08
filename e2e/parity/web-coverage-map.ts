// Records which source lines a web parity scenario executed, for the coverage map
// (coverage-map.ts). Active only when PARITY_COVERAGE=1.
import { mkdir, writeFile } from "node:fs/promises";
import { basename, join } from "node:path";
import type { Page } from "playwright-core";
import { expand, type FileCoverage, type ScenarioCoverage, toRanges } from "./coverage-map";

type V8Function = { ranges: { startOffset: number; endOffset: number; count: number }[] };
type Entry = { url: string; source?: string; functions: V8Function[] };
/** Source-map segments of one served module: generated offsets and the original 1-based lines. */
type Segments = { offsets: number[]; lines: number[] } | undefined;

const BASE64 = new Map([..."ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"].map((char, index) => [char, index]));
// Decoded maps per module; every scenario loads the same Vite output.
const decoded = new Map<string, Segments>();

/** Decodes the inline source map Vite appends in dev, keeping segments that map into `file`. */
function segments(file: string, source: string): Segments {
	const match = /\/\/# sourceMappingURL=data:application\/json;(?:charset=utf-8;)?base64,([A-Za-z0-9+/=]+)\s*$/.exec(source);
	if (!match?.[1]) return undefined;
	const map = JSON.parse(Buffer.from(match[1], "base64").toString("utf8")) as { sources: string[]; mappings: string };
	const own = new Set(map.sources.flatMap((name, index) => basename(name.split("?")[0] ?? "") === basename(file) ? [index] : []));
	const lineStarts = [0];
	for (let index = 0; index < source.length; index++) if (source.charCodeAt(index) === 10) lineStarts.push(index + 1);
	const result = { offsets: [] as number[], lines: [] as number[] };
	let [line, column, sourceIndex, originalLine, value, shift] = [0, 0, 0, 0, 0, 0];
	const fields: number[] = [];
	const flush = () => {
		if (fields.length) {
			column += fields[0] ?? 0;
			if (fields.length >= 4) {
				sourceIndex += fields[1] ?? 0;
				originalLine += fields[2] ?? 0;
				if (own.has(sourceIndex)) { result.offsets.push((lineStarts[line] ?? source.length) + column); result.lines.push(originalLine + 1); }
			}
		}
		fields.length = 0;
	};
	for (const char of map.mappings) {
		if (char === "," || char === ";") {
			flush();
			if (char === ";") { line++; column = 0; }
			continue;
		}
		const digit = BASE64.get(char) ?? 0;
		value += (digit & 31) * 2 ** shift;
		if (digit & 32) shift += 5;
		else { fields.push(value % 2 ? -Math.floor(value / 2) : Math.floor(value / 2)); value = 0; shift = 0; }
	}
	flush();
	return result;
}

/**
 * The original lines one script's executed code maps to, and whether code below its top level
 * ran. In a Svelte component the top level holds the hoisted template markup, which runs on
 * import; only code inside functions, which runs when the component renders, counts there, and
 * `code` lists the lines such code maps to.
 */
function fileCoverage(entry: Entry & { source: string }, map: Segments, svelte: boolean): FileCoverage {
	const length = entry.source.length;
	// V8 block ranges nest, so painting outer functions and blocks before inner ones leaves each
	// offset with its innermost count.
	const functions = entry.functions.filter((fn) => fn.ranges.length).sort((left, right) => {
		const [a, b] = [left.ranges[0], right.ranges[0]];
		return (a?.startOffset ?? 0) - (b?.startOffset ?? 0) || (b?.endOffset ?? 0) - (a?.endOffset ?? 0);
	});
	const executed = new Uint8Array(length);
	const inner = new Uint8Array(length);
	let active = false;
	for (const fn of functions) {
		const outer = fn.ranges[0];
		const topLevel = !outer || (outer.startOffset === 0 && outer.endOffset >= length);
		if (!topLevel) inner.fill(1, outer.startOffset, Math.min(outer.endOffset, length));
		if (!topLevel && outer.count > 0) active = true;
		for (const range of fn.ranges) executed.fill(range.count > 0 ? 1 : 0, range.startOffset, Math.min(range.endOffset, length));
	}
	if (!map) return { lines: [], active };
	if (svelte) for (let index = 0; index < length; index++) executed[index] = (executed[index] ?? 0) & (inner[index] ?? 0);
	// Unexecuted offsets before each offset: two offsets lie in one executed run when equal.
	const gaps = new Uint32Array(length + 1);
	for (let index = 0; index < length; index++) gaps[index + 1] = (gaps[index] ?? 0) + (executed[index] ? 0 : 1);
	const lines: number[] = [];
	const code: number[] = [];
	let previous: { offset: number; line: number } | undefined;
	for (const [index, offset] of map.offsets.entries()) {
		const line = map.lines[index] ?? 0;
		if (svelte && inner[offset]) code.push(line);
		if (offset >= length || !executed[offset]) { previous = undefined; continue; }
		lines.push(line);
		// Fill the lines between two mapped points of one executed run, such as a multi-line
		// template literal, whose inner lines carry no mapping of their own.
		if (previous && gaps[previous.offset] === gaps[offset] && line > previous.line) for (let fill = previous.line + 1; fill < line; fill++) lines.push(fill);
		previous = { offset, line };
	}
	return { lines: toRanges(lines), active, ...(svelte ? { code: toRanges(code) } : {}) };
}

/** Repository path of a Vite-served project module, or undefined for dependencies and styles. */
function sourcePath(url: string): string | undefined {
	const parsed = new URL(url);
	if (!parsed.pathname.startsWith("/src/") || parsed.searchParams.get("type") === "style") return undefined;
	return decodeURIComponent(parsed.pathname.slice(1));
}

/** Starts recording before the page loads; `save` writes `coverage/web-<index>.json`. */
export async function startCodeCoverage(page: Page) {
	if (process.env.PARITY_COVERAGE !== "1") return undefined;
	// Scenarios that reload the page keep the coverage from before the reload.
	await page.coverage.startJSCoverage({ resetOnNavigation: false });
	return {
		async save(artifacts: string, index: number, id: string) {
			const files: ScenarioCoverage["files"] = {};
			for (const entry of await page.coverage.stopJSCoverage() as Entry[]) {
				const path = sourcePath(entry.url);
				if (!path || entry.source === undefined) continue;
				const key = `${path}:${entry.source.length}`;
				if (!decoded.has(key)) decoded.set(key, segments(path, entry.source));
				const file = fileCoverage({ ...entry, source: entry.source }, decoded.get(key), path.endsWith(".svelte"));
				const earlier = files[path];
				files[path] = earlier ? { lines: toRanges([...earlier.lines, ...file.lines].flatMap(expand)), active: earlier.active || file.active, ...(file.code ? { code: toRanges([...(earlier.code ?? []), ...file.code].flatMap(expand)) } : {}) } : file;
			}
			await mkdir(join(artifacts, "coverage"), { recursive: true });
			await writeFile(join(artifacts, "coverage", `web-${index}.json`), JSON.stringify({ id, files } satisfies ScenarioCoverage));
		},
	};
}
