import { expect, test } from "bun:test";
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";
import { gpuiProfilePattern, recordGpuiCoverage } from "./gpui-coverage-map";

test("gives each GPUI worker and build separate exit profiles outside the scenario directory", () => {
	const artifacts = resolve("e2e/artifacts/parity-example");
	const patterns = (["build", "worker-0", "worker-1"] as const).map((owner) => gpuiProfilePattern(artifacts, owner));
	expect(new Set(patterns).size).toBe(patterns.length);
	for (const pattern of patterns) {
		expect(pattern.startsWith(join(artifacts, "gpui-exit-profiles"))).toBe(true);
		expect(basename(pattern)).toBe("%p-%m.profraw");
	}
});

test("parallel conversion preserves serial line and function coverage without merging scenario counters", async () => {
	await mkdir("e2e/artifacts", { recursive: true });
	const temporary = await mkdtemp(resolve("e2e/artifacts/coverage-test-"));
	const file = "src-tauri/gpui/src/lib.rs";
	const root = resolve(".");
	try {
		const capture = async (jobs: number) => {
			const artifacts = join(temporary, String(jobs));
			const directory = join(artifacts, "coverage");
			await mkdir(directory, { recursive: true });
			await Promise.all([0, 1].map((index) => writeFile(join(directory, `gpui-${index}.profraw`), String(index))));
			const bothStarted = Promise.withResolvers<void>();
			let active = 0;
			let peak = 0;
			let metadataExports = 0;
			const invoke = async (command: string[]) => {
				const [program, action] = command;
				if (program === "llvm-profdata" && action === "merge") {
					const raw = command.find((arg) => arg.endsWith(".profraw"));
					if (!raw) throw new Error("expected separate scenario profile");
					expect(command.filter((arg) => arg.endsWith(".profraw"))).toEqual([raw]);
					const merged = command.at(-1);
					if (!merged) throw new Error("expected indexed profile output");
					active++;
					peak = Math.max(peak, active);
					if (active === jobs) bothStarted.resolve();
					await bothStarted.promise;
					await writeFile(merged, await readFile(raw));
					active--;
					return "";
				}
				if (program !== "llvm-cov" || action !== "export") throw new Error(`unexpected command ${command}`);
				if (command.includes("-format=text")) {
					metadataExports++;
					return JSON.stringify({ data: [{ functions: [
						{ name: "f", filenames: [join(root, file)], regions: [[2, 1, 4, 1, 0, 0], [9, 1, 20, 1, 0, 1]] },
						{ name: "g", filenames: [join(root, file)], regions: [[6, 1, 8, 1, 0, 0]] },
					] }] });
				}
				const profile = command.find((arg) => arg.startsWith("-instr-profile="))?.split("=")[1];
				if (!profile) throw new Error("expected one indexed profile for this scenario");
				const first = await readFile(profile, "utf8") === "0";
				return [
					`SF:${join(root, file)}`,
					`FNDA:${first ? 1 : 0},f`,
					`FNDA:${first ? 0 : 1},g`,
					`DA:2,${first ? 1 : 0}`,
					`DA:3,${first ? 1 : 0}`,
					`DA:7,${first ? 0 : 1}`,
					"end_of_record",
				].join("\n");
			};
			await recordGpuiCoverage(artifacts, "parity", [file], ["first", "second"], jobs, new AbortController().signal, { cov: "llvm-cov", profdata: "llvm-profdata", invoke });
			expect(peak).toBe(jobs);
			expect(metadataExports).toBe(1);
			const names = (await readdir(directory)).sort();
			expect(names).toEqual(["gpui-0.json", "gpui-1.json", "gpui-code.json"]);
			return Promise.all(names.map(async (name) => JSON.parse(await readFile(join(directory, name), "utf8"))));
		};
		const serial = await capture(1);
		expect(await capture(2)).toEqual(serial);
		expect(serial).toEqual([
			{ id: "first", files: { [file]: { lines: [[2, 3]], active: true, functions: [[2, 4]] } } },
			{ id: "second", files: { [file]: { lines: [[7, 7]], active: true, functions: [[6, 8]] } } },
			{ code: { [file]: [[2, 3], [7, 7]] }, compiled: [file], functions: { [file]: [[2, 4], [6, 8]] } },
		]);
	} finally {
		await rm(temporary, { recursive: true, force: true });
	}
});
