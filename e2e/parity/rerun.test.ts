import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm, utimes, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { DRIVER_SELECTION_FORMAT, lastRerunSource, rerunSelections, selectionHash } from "./rerun";

test("last picks the newest finished run with a complete nonempty selection", async () => {
	const root = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	const versioned = JSON.stringify({ selectionFormat: DRIVER_SELECTION_FORMAT });
	const save = async (name: string, files: Record<string, string>, date: string) => {
		const run = join(root, `parity-${name}`);
		await mkdir(run);
		await Promise.all(Object.entries(files).map(([file, contents]) => writeFile(join(run, file), contents)));
		await utimes(join(run, "result.json"), new Date(date), new Date(date));
		return run;
	};
	try {
		const both = '{"driver":"both"}';
		const legacy = await save("legacy", { "result.json": both, "scenarios.txt": "one" }, "2026-01-01");
		expect(await lastRerunSource(root)).toBe(legacy);
		const modern = await save("modern", { "result.json": versioned, "web-scenarios.txt": "one", "gpui-scenarios.txt": "" }, "2026-01-02");
		const noop = await save("noop", { "result.json": versioned, "web-scenarios.txt": "", "gpui-scenarios.txt": "" }, "2026-01-03");
		await save("partial", { "result.json": both, "web-scenarios.txt": "one", "scenarios.txt": "one" }, "2026-01-04");
		await save("empty-legacy", { "result.json": both, "scenarios.txt": "" }, "2026-01-05");
		await save("interrupted", { "result.json": '{"driver":"both","interrupted":true}', "scenarios.txt": "one" }, "2026-01-06");
		expect(await rerunSelections(noop)).toEqual({ web: new Set(), gpui: new Set() });
		expect(await lastRerunSource(root)).toBe(modern);
		const gpuiOnly = await save("gpui-only", {
			"result.json": JSON.stringify({ selectionHashes: { web: selectionHash(""), gpui: selectionHash("two") } }),
			"web-scenarios.txt": "",
			"gpui-scenarios.txt": "two",
		}, "2026-01-07");
		expect(await lastRerunSource(root)).toBe(gpuiOnly);
		expect(await lastRerunSource(root, "web")).toBe(modern);
		await save("corrupt", {
			"result.json": JSON.stringify({ selectionHashes: { web: selectionHash("one"), gpui: selectionHash("") } }),
			"web-scenarios.txt": "on",
			"gpui-scenarios.txt": "",
		}, "2026-01-08");
		await expect(lastRerunSource(root, "web")).rejects.toThrow("do not match result.json");
		expect(await lastRerunSource(root, "gpui")).toBe(gpuiOnly);
	} finally {
		await rm(root, { recursive: true });
	}
});

test("a rerun selects only each driver's failures, including missing results", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	const save = (file: string, contents: string) => writeFile(join(run, file), contents);
	try {
		await Promise.all([
			save("web-scenarios.txt", "shared\nweb-only\n"),
			save("gpui-scenarios.txt", "shared\ngpui-only\n"),
			save("result.json", '{"driver":'),
			save("web.json", '{"driver":"web","scenarios":[{"id":"shared","status":"passed"},{"id":"web-only","status":"failed"}]}'),
			save("gpui.json", '{"driver":"gpui","scenarios":[{"id":"shared","status":"passed"}]}'),
		]);
		expect(await rerunSelections(run)).toEqual({ web: new Set(["web-only"]), gpui: new Set(["gpui-only"]) });
	} finally {
		await rm(run, { recursive: true });
	}
});

test("unreadable reports leave selected IDs eligible and empty selections stay empty", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			writeFile(join(run, "web-scenarios.txt"), "one\ntwo"),
			writeFile(join(run, "gpui-scenarios.txt"), ""),
			mkdir(join(run, "web.json")),
			mkdir(join(run, "gpui.json")),
		]);
		expect(await rerunSelections(run)).toEqual({ web: new Set(["one", "two"]), gpui: new Set() });
	} finally {
		await rm(run, { recursive: true });
	}
});

test("rerunning an earlier web-only report has no GPUI selection", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			writeFile(join(run, "scenarios.txt"), "one\ntwo"),
			writeFile(join(run, "result.json"), '{"driver":"web"}'),
			writeFile(join(run, "web.json"), '{"driver":"web","scenarios":[{"id":"one","status":"passed"}]}'),
		]);
		expect(await rerunSelections(run)).toEqual({ web: new Set(["two"]), gpui: new Set() });
	} finally {
		await rm(run, { recursive: true });
	}
});

test("reruns distrust a report for another driver and a duplicate scenario result", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	const save = (file: string, contents: string) => writeFile(join(run, file), contents);
	try {
		await Promise.all([
			save("web-scenarios.txt", "unique\nduplicate\nmissing"),
			save("gpui-scenarios.txt", "gpui-only"),
			save("web.json", '{"driver":"web","scenarios":[{"id":"unique","status":"passed"},{"id":"duplicate","status":"passed"},{"id":"duplicate","status":"failed"}]}'),
			save("gpui.json", '{"driver":"web","scenarios":[{"id":"gpui-only","status":"passed"}]}'),
		]);
		expect(await rerunSelections(run)).toEqual({ web: new Set(["duplicate", "missing"]), gpui: new Set(["gpui-only"]) });
	} finally {
		await rm(run, { recursive: true });
	}
});

test("a partial per-driver selection never borrows IDs from the compatibility list", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await writeFile(join(run, "web-scenarios.txt"), "web-only");
		await expect(rerunSelections(run)).rejects.toThrow("Missing GPUI selection");
		await writeFile(join(run, "scenarios.txt"), "web-only\nnot-recorded");
		await expect(rerunSelections(run)).rejects.toThrow("Missing GPUI selection");
	} finally {
		await rm(run, { recursive: true });
	}
});

test("a versioned artifact requires driver selections even if only the shared list was copied", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			writeFile(join(run, "scenarios.txt"), "web-only\ngpui-only"),
			writeFile(join(run, "result.json"), JSON.stringify({ driver: "both", selectionFormat: DRIVER_SELECTION_FORMAT })),
		]);
		await expect(rerunSelections(run)).rejects.toThrow("Missing web and GPUI selections");
	} finally {
		await rm(run, { recursive: true });
	}
});

test("an unexpected result invalidates the report even if all selected scenarios passed", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			writeFile(join(run, "web-scenarios.txt"), "selected"),
			writeFile(join(run, "gpui-scenarios.txt"), ""),
			writeFile(join(run, "web.json"), '{"driver":"web","scenarios":[{"id":"selected","status":"passed"},{"id":"unexpected","status":"passed"}]}'),
		]);
		expect(await rerunSelections(run)).toEqual({ web: new Set(["selected"]), gpui: new Set() });
	} finally {
		await rm(run, { recursive: true });
	}
});

test("legacy results with unknown driver selection need an explicit driver", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			writeFile(join(run, "scenarios.txt"), "one"),
			writeFile(join(run, "result.json"), '{"driver":'),
			writeFile(join(run, "web.json"), '{"driver":"web","scenarios":[{"id":"one","status":"passed"}]}'),
		]);
		await expect(rerunSelections(run)).rejects.toThrow("set PARITY_DRIVER");
		expect(await rerunSelections(run, "gpui")).toEqual({ web: new Set(), gpui: new Set(["one"]) });
	} finally {
		await rm(run, { recursive: true });
	}
});

test("a driver error replays its original selection even when one scenario failed", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			writeFile(join(run, "result.json"), '{"driver":"both","failedDrivers":["web"]}'),
			writeFile(join(run, "web-scenarios.txt"), "one\ntwo"),
			writeFile(join(run, "gpui-scenarios.txt"), "one"),
			writeFile(join(run, "web.json"), '{"driver":"web","scenarios":[{"id":"one","status":"passed"},{"id":"two","status":"failed"}]}'),
			writeFile(join(run, "gpui.json"), '{"driver":"gpui","scenarios":[{"id":"one","status":"passed"}]}'),
		]);
		expect(await rerunSelections(run)).toEqual({ web: new Set(["one", "two"]), gpui: new Set() });
	} finally {
		await rm(run, { recursive: true });
	}
});

test("the shared list detects missing IDs when driver selection hashes are unavailable", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			writeFile(join(run, "result.json"), '{"driver":'),
			writeFile(join(run, "web-scenarios.txt"), "one"),
			writeFile(join(run, "gpui-scenarios.txt"), ""),
			writeFile(join(run, "scenarios.txt"), "one\ntwo"),
		]);
		await expect(rerunSelections(run)).rejects.toThrow("do not match scenarios.txt");
	} finally {
		await rm(run, { recursive: true });
	}
});

test("a damaged driver list cannot drop recorded failures", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		const metadata = { driver: "both", selectionFormat: DRIVER_SELECTION_FORMAT };
		await Promise.all([
			writeFile(join(run, "result.json"), JSON.stringify(metadata)),
			writeFile(join(run, "web-scenarios.txt"), ""),
			writeFile(join(run, "gpui-scenarios.txt"), "two"),
		]);
		await expect(rerunSelections(run)).rejects.toThrow("do not match the recorded driver");
		await Promise.all([
			writeFile(join(run, "result.json"), JSON.stringify({
				...metadata,
				selectionHashes: { web: selectionHash("one"), gpui: selectionHash("two") },
			})),
			writeFile(join(run, "web-scenarios.txt"), "on"),
		]);
		await expect(rerunSelections(run)).rejects.toThrow("do not match result.json");
	} finally {
		await rm(run, { recursive: true });
	}
});
