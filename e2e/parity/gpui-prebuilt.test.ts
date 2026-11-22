import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { verifyPrebuiltParity, writePrebuiltParity } from "./gpui-prebuilt";

const build = {
	gitSha: "f".repeat(40),
	target: "x86_64-unknown-linux-gnu",
	rustcVersion: "rustc test toolchain",
	features: [],
	profile: "test",
	instrumented: true,
	sourceRoot: resolve("."),
};

test("publishes a manifest and verifies its binary and compiled source inventory", async () => {
	await mkdir("e2e/artifacts", { recursive: true });
	const directory = await mkdtemp(resolve("e2e/artifacts/prebuilt-test-"));
	try {
		const input = join(directory, "input");
		await writeFile(input, "test binary");
		const compiled = ["rust/gpui/src/lib.rs"];
		const path = await writePrebuiltParity(directory, input, build, compiled);
		const prebuilt = await verifyPrebuiltParity(path, build);
		expect(prebuilt.compiled).toEqual(compiled);
		expect(prebuilt.executable).toBe(join(directory, process.platform === "win32" ? "parity.exe" : "parity"));
		await expect(writePrebuiltParity(directory, input, build, compiled)).rejects.toThrow("EEXIST");
		await expect(verifyPrebuiltParity(path, { ...build, gitSha: "a".repeat(40) })).rejects.toThrow("gitSha");
		await expect(verifyPrebuiltParity(path, { ...build, target: "aarch64-unknown-linux-gnu" })).rejects.toThrow("target");
		await expect(verifyPrebuiltParity(path, { ...build, profile: "release" })).rejects.toThrow("profile");
		await expect(verifyPrebuiltParity(path, { ...build, features: ["e2e"] })).rejects.toThrow("features");
		await expect(verifyPrebuiltParity(path, { ...build, instrumented: false })).rejects.toThrow("instrumented");
		await expect(verifyPrebuiltParity(path, { ...build, sourceRoot: directory })).rejects.toThrow("sourceRoot");
		await expect(verifyPrebuiltParity(path, { ...build, rustcVersion: "another toolchain" })).rejects.toThrow("rustcVersion");
		await writeFile(prebuilt.executable, "changed binary");
		await expect(verifyPrebuiltParity(path, build)).rejects.toThrow("binarySha256");
		await rm(prebuilt.executable);
		await expect(verifyPrebuiltParity(path, build)).rejects.toThrow("Missing GPUI parity binary");
	} finally {
		await rm(directory, { recursive: true, force: true });
	}
});
