import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { rerunSelections } from "./rerun";

test("a rerun selects only each driver's failures, including missing results", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	const save = (file: string, contents: string) => writeFile(join(run, file), contents);
	try {
		await Promise.all([
			save("web-scenarios.txt", "shared\nweb-only\n"),
			save("gpui-scenarios.txt", "shared\ngpui-only\n"),
			save("result.json", '{"driver":'),
			save("web.json", '{"scenarios":[{"id":"shared","status":"passed"},{"id":"web-only","status":"failed"}]}'),
			save("gpui.json", '{"scenarios":[{"id":"shared","status":"passed"}]}'),
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
			writeFile(join(run, "web.json"), '{"scenarios":[{"id":"one","status":"passed"}]}'),
		]);
		expect(await rerunSelections(run)).toEqual({ web: new Set(["two"]), gpui: new Set() });
	} finally {
		await rm(run, { recursive: true });
	}
});
