import { expect, test } from "bun:test";
import { chmod, mkdir, mkdtemp, rm, utimes, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { type CoverageMapFailure, DRIVER_SELECTION_FORMAT, lastRerunSource, rerunSelections, selectionHash } from "./rerun";

type RunOutcome = {
	status?: "failed" | "passed";
	failedDrivers?: string[];
	coverageMapFailure?: CoverageMapFailure;
	coveragePublicationRequested?: boolean;
	runLevelFailed?: boolean;
};

function driverFiles(web: string, gpui: string, outcome: RunOutcome = {}) {
	return {
		"web-scenarios.txt": web,
		"gpui-scenarios.txt": gpui,
		"result.json": JSON.stringify({
			selectionFormat: DRIVER_SELECTION_FORMAT,
			selectionHashes: { web: selectionHash(web), gpui: selectionHash(gpui) },
			...outcome,
		}),
	};
}

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
		await expect(lastRerunSource(join(root, "missing"))).rejects.toThrow("found no finished parity run");
		const both = '{"driver":"both"}';
		const legacy = await save("legacy", { "result.json": both, "scenarios.txt": "one" }, "2026-01-01");
		expect(await lastRerunSource(root)).toBe(legacy);
		const modern = await save("modern", driverFiles("one", ""), "2026-01-02");
		const noop = await save("noop", driverFiles("", ""), "2026-01-03");
		await save("before-selection", { "result.json": versioned }, "2026-01-04");
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
		const finished = await save("finished", {
			"result.json": '{"driver":"both","interrupted":false,"error":"Interrupted system call"}',
			"scenarios.txt": "one",
		}, "2026-01-09");
		expect(await lastRerunSource(root)).toBe(finished);
		await save("unknown-driver", { "result.json": "{}", "scenarios.txt": "one" }, "2026-01-10");
		await expect(lastRerunSource(root, "web")).rejects.toThrow("Restore result.json");
		const malformed = await save("malformed", {
			"result.json": JSON.stringify({ selectionHashes: { gpui: selectionHash("two") } }),
		}, "2026-01-11");
		const diagnostic = `Invalid driver selection hashes in ${join(malformed, "result.json")}`;
		await expect(lastRerunSource(root, "web")).rejects.toThrow(diagnostic);
		await expect(rerunSelections(malformed)).rejects.toThrow(diagnostic);
		await save("marked-without-hashes", { "result.json": versioned, "scenarios.txt": "one" }, "2026-01-12");
		await expect(lastRerunSource(root)).rejects.toThrow("Missing web and GPUI selections");
		const damaged = await save("damaged-result", {
			"result.json": '{"selectionHashes":',
			"web-scenarios.txt": "one",
			"gpui-scenarios.txt": "",
		}, "2026-01-13");
		await expect(lastRerunSource(root)).rejects.toThrow(join(damaged, "result.json"));
	} finally {
		await rm(root, { recursive: true });
	}
});

test("last skips unpublished selections and unreadable legacy selections for the other driver", async () => {
	const root = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		const web = join(root, "parity-web");
		const gpui = join(root, "parity-gpui");
		const uncommitted = join(root, "parity-uncommitted");
		await Promise.all([mkdir(web), mkdir(gpui), mkdir(uncommitted)]);
		await Promise.all([
			writeFile(join(web, "result.json"), '{"driver":"web"}'),
			writeFile(join(web, "scenarios.txt"), "one"),
			writeFile(join(gpui, "result.json"), '{"driver":"gpui"}'),
			mkdir(join(gpui, "scenarios.txt")),
			writeFile(join(uncommitted, "result.json"), JSON.stringify({ selectionFormat: DRIVER_SELECTION_FORMAT, selectionRecorded: false })),
			writeFile(join(uncommitted, "web-scenarios.txt"), "one"),
		]);
		await Promise.all([web, gpui, uncommitted].map((run, index) => {
			const date = new Date(Date.UTC(2026, 1, index + 1));
			return utimes(join(run, "result.json"), date, date);
		}));
		expect(await lastRerunSource(root, "web")).toBe(web);
	} finally {
		await rm(root, { recursive: true });
	}
});

test.skipIf(process.platform === "win32" || process.getuid?.() === 0)("last finds a valid run despite an inaccessible historical directory", async () => {
	const root = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	const old = join(root, "parity-inaccessible");
	const recent = join(root, "parity-recent");
	await mkdir(old);
	try {
		await mkdir(recent);
		await Promise.all([
			writeFile(join(old, "result.json"), '{"driver":"gpui"}'),
			writeFile(join(recent, "result.json"), '{"driver":"web"}'),
			writeFile(join(recent, "scenarios.txt"), "one"),
		]);
		const earlier = new Date("2026-01-01");
		await utimes(join(old, "result.json"), earlier, earlier);
		await chmod(old, 0o000);
		expect(await lastRerunSource(root)).toBe(recent);
	} finally {
		await chmod(old, 0o700);
		await rm(root, { recursive: true });
	}
});

test("a rerun selects only each driver's failures, including missing results", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	const save = (file: string, contents: string) => writeFile(join(run, file), contents);
	try {
		await Promise.all([
			...Object.entries(driverFiles("shared\nweb-only\n", "shared\ngpui-only\n", { status: "failed" })).map(([file, contents]) => save(file, contents)),
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
			...Object.entries(driverFiles("one\ntwo", "", { status: "failed" })).map(([file, contents]) => writeFile(join(run, file), contents)),
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
			writeFile(join(run, "result.json"), '{"driver":"web","status":"failed"}'),
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
			...Object.entries(driverFiles("unique\nduplicate\nmissing", "gpui-only", { status: "failed" })).map(([file, contents]) => save(file, contents)),
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
		await writeFile(join(run, "result.json"), JSON.stringify({
			driver: "both",
			selectionHashes: { web: selectionHash("web-only"), gpui: selectionHash("gpui-only") },
		}));
		await expect(rerunSelections(run)).rejects.toThrow("Missing web and GPUI selections");
	} finally {
		await rm(run, { recursive: true });
	}
});

test("an unexpected result invalidates the report even if all selected scenarios passed", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			...Object.entries(driverFiles("selected", "", { status: "failed" })).map(([file, contents]) => writeFile(join(run, file), contents)),
			writeFile(join(run, "web.json"), '{"driver":"web","scenarios":[{"id":"selected","status":"passed"},{"id":"unexpected","status":"passed"}]}'),
		]);
		expect(await rerunSelections(run)).toEqual({ web: new Set(["selected"]), gpui: new Set() });
	} finally {
		await rm(run, { recursive: true });
	}
});

test("a shared selection without trustworthy metadata cannot establish the original driver", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			writeFile(join(run, "scenarios.txt"), "one"),
			writeFile(join(run, "result.json"), '{"driver":'),
			writeFile(join(run, "web.json"), '{"driver":"web","scenarios":[{"id":"one","status":"passed"}]}'),
		]);
		await expect(rerunSelections(run)).rejects.toThrow("Restore result.json");
	} finally {
		await rm(run, { recursive: true });
	}
});

test("a driver error replays its original selection even when one scenario failed", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			...Object.entries(driverFiles("one\ntwo", "one", { status: "failed", failedDrivers: ["web"] })).map(([file, contents]) => writeFile(join(run, file), contents)),
			writeFile(join(run, "web.json"), '{"driver":"web","scenarios":[{"id":"one","status":"passed"},{"id":"two","status":"failed"}]}'),
			writeFile(join(run, "gpui.json"), '{"driver":"gpui","scenarios":[{"id":"one","status":"passed"}]}'),
		]);
		expect(await rerunSelections(run)).toEqual({ web: new Set(["one", "two"]), gpui: new Set() });
	} finally {
		await rm(run, { recursive: true });
	}
});

test("passing rows are replayed when verified selections have an unknown outcome", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			...Object.entries(driverFiles("one", "")).map(([file, contents]) => writeFile(join(run, file), contents)),
			writeFile(join(run, "web.json"), '{"driver":"web","scenarios":[{"id":"one","status":"passed"}]}'),
		]);
		const expected = { web: new Set(["one"]), gpui: new Set<string>() };
		expect(await rerunSelections(run)).toEqual(expected);
		await writeFile(join(run, "result.json"), driverFiles("one", "", { status: "failed", failedDrivers: ["unknown"] })["result.json"]);
		expect(await rerunSelections(run)).toEqual(expected);
	} finally {
		await rm(run, { recursive: true });
	}
});

test("a legacy cleanup error replays the affected driver even when every scenario passed", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			writeFile(join(run, "scenarios.txt"), "one"),
			writeFile(join(run, "result.json"), '{"driver":"both","status":"failed","error":"Web driver: Error: Could not stop owned Vite process group"}'),
			writeFile(join(run, "web.json"), '{"driver":"web","scenarios":[{"id":"one","status":"passed"}]}'),
			writeFile(join(run, "gpui.json"), '{"driver":"gpui","scenarios":[{"id":"one","status":"passed"}]}'),
		]);
		expect(await rerunSelections(run)).toEqual({ web: new Set(["one"]), gpui: new Set() });
	} finally {
		await rm(run, { recursive: true });
	}
});

test("a matching union cannot verify hashless driver selections", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			writeFile(join(run, "result.json"), JSON.stringify({ driver: "both", selectionFormat: DRIVER_SELECTION_FORMAT })),
			writeFile(join(run, "web-scenarios.txt"), "one"),
			writeFile(join(run, "gpui-scenarios.txt"), "two"),
			writeFile(join(run, "scenarios.txt"), "one\ntwo"),
		]);
		await expect(rerunSelections(run)).rejects.toThrow("Missing driver selection hashes");
	} finally {
		await rm(run, { recursive: true });
	}
});

test("a damaged driver list cannot drop recorded failures", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all(Object.entries(driverFiles("one\ntwo", "two")).map(([file, contents]) => writeFile(join(run, file), contents)));
		await writeFile(join(run, "web-scenarios.txt"), "one");
		await expect(rerunSelections(run)).rejects.toThrow("do not match result.json");
	} finally {
		await rm(run, { recursive: true });
	}
});

test("a failed run with passing reports replays the recorded selection", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			...Object.entries(driverFiles("one", "two", { status: "failed", failedDrivers: [] })).map(([file, contents]) => writeFile(join(run, file), contents)),
			writeFile(join(run, "web.json"), '{"driver":"web","scenarios":[{"id":"one","status":"passed"}]}'),
			writeFile(join(run, "gpui.json"), '{"driver":"gpui","scenarios":[{"id":"two","status":"passed"}]}'),
		]);
		expect(await rerunSelections(run)).toEqual({ web: new Set(["one"]), gpui: new Set(["two"]) });
	} finally {
		await rm(run, { recursive: true });
	}
});

test("a coverage retry replays both selections after a coverage-map failure", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			...Object.entries(driverFiles("one\ntwo", "one", { status: "failed", coverageMapFailure: "generation" })).map(([file, contents]) => writeFile(join(run, file), contents)),
			writeFile(join(run, "web.json"), '{"driver":"web","scenarios":[{"id":"one","status":"passed"},{"id":"two","status":"failed"}]}'),
			writeFile(join(run, "gpui.json"), '{"driver":"gpui","scenarios":[{"id":"one","status":"passed"}]}'),
		]);
		expect(await rerunSelections(run)).toEqual({ web: new Set(["two"]), gpui: new Set() });
		expect(await rerunSelections(run, { coverage: true })).toEqual({ web: new Set(["one", "two"]), gpui: new Set(["one"]) });
		await writeFile(join(run, "result.json"), driverFiles("one\ntwo", "one", { status: "failed", coverageMapFailure: "publication" })["result.json"]);
		expect(await rerunSelections(run, { coverage: true })).toEqual({ web: new Set(["one", "two"]), gpui: new Set(["one"]), publishCoverageMap: true });
		await writeFile(join(run, "result.json"), driverFiles("one\ntwo", "one", {
			status: "failed", coverageMapFailure: "generation", coveragePublicationRequested: true,
		})["result.json"]);
		expect(await rerunSelections(run, { coverage: true })).toEqual({ web: new Set(["one", "two"]), gpui: new Set(["one"]), publishCoverageMap: true });
	} finally {
		await rm(run, { recursive: true });
	}
});

test("a failed legacy coverage run replays the recorded selection, including previously passing IDs", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			writeFile(join(run, "scenarios.txt"), "one\ntwo"),
			writeFile(join(run, "result.json"), '{"driver":"both","status":"failed"}'),
			writeFile(join(run, "web.json"), '{"driver":"web","scenarios":[{"id":"one","status":"passed"},{"id":"two","status":"failed"}]}'),
			writeFile(join(run, "gpui.json"), '{"driver":"gpui","scenarios":[{"id":"one","status":"passed"},{"id":"two","status":"passed"}]}'),
		]);
		expect(await rerunSelections(run)).toEqual({ web: new Set(["two"]), gpui: new Set() });
		expect(await rerunSelections(run, { coverage: true })).toEqual({ web: new Set(["one", "two"]), gpui: new Set(["one", "two"]) });
	} finally {
		await rm(run, { recursive: true });
	}
});

test("legacy GPUI shard infrastructure failures replay the whole driver selection", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			writeFile(join(run, "scenarios.txt"), "one\ntwo"),
			writeFile(join(run, "gpui.json"), '{"driver":"gpui","scenarios":[{"id":"one","status":"passed"},{"id":"two","status":"failed"}]}'),
			writeFile(join(run, "gpui-0.json"), '{"driver":"gpui","scenarios":[{"id":"one","status":"passed"}]}'),
			writeFile(join(run, "gpui-1.json"), '{"driver":"gpui","scenarios":[{"id":"two","status":"failed"}]}'),
		]);
		for (const error of ["GPUI shard 0/2 exited 137", "GPUI shard 0/2 wrote no report", "GPUI shard 1/2 exited 101"]) {
			await writeFile(join(run, "result.json"), JSON.stringify({ driver: "gpui", status: "failed", error }));
			expect(await rerunSelections(run)).toEqual({ web: new Set(), gpui: new Set(error.endsWith("101") ? ["two"] : ["one", "two"]) });
		}
		await writeFile(join(run, "result.json"), '{"driver":"gpui","status":"failed","error":"GPUI shard 0/2 exited 101; GPUI shard 1/2 exited 101"}');
		expect(await rerunSelections(run)).toEqual({ web: new Set(), gpui: new Set(["one", "two"]) });
		await rm(join(run, "gpui-0.json"));
		expect(await rerunSelections(run)).toEqual({ web: new Set(), gpui: new Set(["one", "two"]) });
	} finally {
		await rm(run, { recursive: true });
	}
});

test("a run-level failure replays both selections even when a scenario also failed", async () => {
	const run = await mkdtemp(join(import.meta.dir, ".rerun-test-"));
	try {
		await Promise.all([
			...Object.entries(driverFiles("one\ntwo", "one", {
				status: "failed", failedDrivers: [], runLevelFailed: true,
			})).map(([file, contents]) => writeFile(join(run, file), contents)),
			writeFile(join(run, "web.json"), '{"driver":"web","scenarios":[{"id":"one","status":"passed"},{"id":"two","status":"failed"}]}'),
			writeFile(join(run, "gpui.json"), '{"driver":"gpui","scenarios":[{"id":"one","status":"passed"}]}'),
		]);
		expect(await rerunSelections(run)).toEqual({ web: new Set(["one", "two"]), gpui: new Set(["one"]) });
	} finally {
		await rm(run, { recursive: true });
	}
});
