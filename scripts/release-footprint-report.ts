import { posix } from "node:path";

interface FrontendFile {
	path: string;
	rawBytes: number;
}
type FontRequests = Record<"launcher" | "workspace", readonly string[]>;
interface MutableFontRequests {
	launcher: string[];
	workspace: string[];
}

export function inspectElfSections(output: string) {
	let sectionBytes = 0;
	let removableSymbolBytes = 0;
	let runtimeSymbolBytes = 0;
	let sections = 0;
	for (const line of output.split("\n")) {
		const match = /^\s*\[\s*\d+\]\s+(\S+)\s+(\S+)\s+[\da-f]+\s+[\da-f]+\s+([\da-f]+)\s/i.exec(line);
		if (!match) continue;
		const [, name, type, hexSize] = match;
		const size = Number.parseInt(hexSize, 16);
		sections += 1;
		if (type === "NOBITS") continue;
		sectionBytes += size;
		if ([".symtab", ".strtab"].includes(name) || name.startsWith(".debug") || name.startsWith(".zdebug")) removableSymbolBytes += size;
		if ([".dynsym", ".dynstr", ".gnu.version", ".gnu.version_d", ".gnu.version_r"].includes(name)) runtimeSymbolBytes += size;
	}
	if (!sections) throw new Error("ELF section sizes are unavailable");
	return { sectionBytes, removableSymbolBytes, runtimeSymbolBytes };
}

export function summarizeFrontend(files: readonly FrontendFile[], requests: FontRequests = { launcher: [], workspace: [] }) {
	const bytes = { javascript: 0, css: 0, fonts: 0, other: 0 };
	const bundled: Array<{ file: string; bytes: number }> = [];
	for (const file of files) {
		const extension = posix.extname(file.path).toLowerCase();
		if (extension === ".js") bytes.javascript += file.rawBytes;
		else if (extension === ".css") bytes.css += file.rawBytes;
		else if ([".woff2", ".woff", ".ttf", ".otf"].includes(extension)) {
			bytes.fonts += file.rawBytes;
			bundled.push({ file: posix.basename(file.path), bytes: file.rawBytes });
		} else bytes.other += file.rawBytes;
	}
	bundled.sort((a, b) => a.file.localeCompare(b.file));
	const fontNames = new Set(bundled.map((font) => font.file));
	const requested: MutableFontRequests = { launcher: [], workspace: [] };
	const unmatchedRequests = { launcher: 0, workspace: 0 };
	for (const stage of ["launcher", "workspace"] as const) {
		for (const file of new Set(requests[stage])) {
			if (fontNames.has(file)) requested[stage].push(file);
			else unmatchedRequests[stage] += 1;
		}
		requested[stage].sort();
	}
	return { bytes, totalBytes: bytes.javascript + bytes.css + bytes.fonts + bytes.other, fonts: { bundled, requested, unmatchedRequests } };
}
