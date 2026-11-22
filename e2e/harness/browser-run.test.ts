import { expect, test } from "bun:test";
import { mkdir, mkdtemp, readdir, rm } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { createBrowserBuild } from "./browser-run";

test("concurrent browser builds embed their own Vite output and launch their own binary", async () => {
	await mkdir("e2e/artifacts", { recursive: true });
	const state = await mkdtemp(resolve("e2e/artifacts/browser-build-test-"));
	const assets = new Set<string>();
	const targets = new Set<string>();
	const builds = createBrowserBuild(state, async (command, args, env) => {
		if (command === "bun") {
			assets.add(args[args.indexOf("--outDir") + 1]);
		} else {
			expect(command).toBe("cargo");
			const target = args[args.indexOf("--target-dir") + 1];
			const bundle = join(dirname(target), "dist");
			expect(assets.has(bundle)).toBe(true);
			expect(env.KUBECOVE_WEB_ASSET_DIR).toBe(bundle);
			expect(env.CARGO_BUILD_BUILD_DIR).toBe(target);
			targets.add(target);
		}
		return { stdout: "" };
	});
	try {
		const reports = join(state, "reports");
		const binaries = await Promise.all([builds.build(reports, false), builds.build(reports, true)]);
		expect(assets.size).toBe(2);
		expect(targets.size).toBe(2);
		for (const binary of binaries) expect(targets.has(dirname(dirname(binary)))).toBe(true);
		await builds.cleanup();
		expect(await readdir(state)).toEqual(["reports"]);
	} finally {
		await rm(state, { recursive: true, force: true });
	}
});
