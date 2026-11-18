import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { DRIVER_SELECTION_FORMAT, rerunSelections } from "./rerun";

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

test("a truncated legacy result still allows reruns using its shared selection", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			writeFile(join(run, "scenarios.txt"), "one"),
			writeFile(join(run, "result.json"), '{"driver":'),
			writeFile(join(run, "web.json"), '{"driver":"web","scenarios":[{"id":"one","status":"passed"}]}'),
		]);
		expect(await rerunSelections(run)).toEqual({ web: new Set(), gpui: new Set(["one"]) });
	} finally {
		await rm(run, { recursive: true });
	}
});
