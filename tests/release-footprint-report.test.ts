import { expect, it } from "bun:test";
import { inspectElfSections, summarizeFrontend } from "../scripts/release-footprint-report";

it("partitions emitted frontend files without double counting embedded assets", () => {
	const result = summarizeFrontend([
		{ path: "dist/assets/app.js", rawBytes: 20 },
		{ path: "dist/assets/app.css", rawBytes: 10 },
		{ path: "dist/assets/font.woff2", rawBytes: 30 },
		{ path: "dist/index.html", rawBytes: 5 },
	]);
	expect(result.bytes).toEqual({ javascript: 20, css: 10, fonts: 30, other: 5 });
	expect(result.totalBytes).toBe(65);
});

it("separates removable symbols from runtime ELF tables and rejects unavailable parsing", () => {
	const sections = inspectElfSections(`
  [ 1] .text PROGBITS 0000000000001000 001000 000020 00 AX 0 0 16
  [ 2] .dynsym DYNSYM 0000000000002000 002000 000010 18 A 3 1 8
  [ 3] .symtab SYMTAB 0000000000000000 002010 000008 18 4 1 8
  [ 4] .strtab STRTAB 0000000000000000 002018 000004 00 0 0 1
  [ 5] .debug_info PROGBITS 0000000000000000 00201c 000002 00 0 0 1
  [ 6] .bss NOBITS 0000000000003000 00201e 000100 00 WA 0 0 8
`);
	expect(sections.removableSymbolBytes).toBe(14);
	expect(sections.runtimeSymbolBytes).toBe(16);
	expect(sections.sectionBytes).toBe(62);
	expect(() => inspectElfSections("readelf failed")).toThrow();
});

it("distinguishes bundled fonts from each startup stage's requests", () => {
	const result = summarizeFrontend([
		{ path: "dist/assets/used.woff2", rawBytes: 10 },
		{ path: "dist/assets/deferred.woff2", rawBytes: 20 },
	], { launcher: ["used.woff2", "used.woff2"], workspace: ["used.woff2", "unknown-private-name"] });
	expect(result.fonts.bundled.length).toBe(2);
	expect(result.fonts.requested.launcher).toEqual(["used.woff2"]);
	expect(result.fonts.requested.workspace).toEqual(["used.woff2"]);
	expect(result.fonts.unmatchedRequests.workspace).toBe(1);
	expect(JSON.stringify(result)).not.toContain("private");
});
