import { createHash } from "node:crypto";
import { constants, createReadStream } from "node:fs";
import { chmod, copyFile, lstat, readFile, realpath, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { object, strings } from "./contract-values";

export type ParityBuild = {
	gitSha: string;
	target: string;
	rustcVersion: string;
	features: string[];
	profile: string;
	instrumented: boolean;
	sourceRoot: string;
};

const binaryFile = process.platform === "win32" ? "parity.exe" : "parity";
export const GPUI_CRATE_ROOT = "src-tauri/gpui/src/lib.rs";

export async function parityBuild(instrumented: boolean): Promise<ParityBuild> {
	const sourceRoot = await realpath(".");
	const get = async (args: string[], cwd: string) => {
		const child = Bun.spawn(args, { cwd, stdout: "pipe", stderr: "inherit" });
		const [code, output] = await Promise.all([child.exited, new Response(child.stdout).text()]);
		if (code !== 0) throw new Error(`${args[0]} could not determine the parity build identity (exit ${code})`);
		return output.trim();
	};
	const [gitSha, rustcVersion, modified] = await Promise.all([
		get(["git", "rev-parse", "HEAD"], sourceRoot),
		get([process.env.RUSTC ?? process.env.CARGO_BUILD_RUSTC ?? "rustc", "-vV"], join(sourceRoot, "src-tauri", "gpui")),
		get(["git", "status", "--porcelain", "--untracked-files=no"], sourceRoot),
	]);
	if (modified) throw new Error("Prebuilt GPUI parity requires a clean tracked checkout");
	const target = /^host: (\S+)$/m.exec(rustcVersion)?.[1];
	if (!target || !/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/.test(gitSha)) throw new Error("Could not determine the GPUI parity target or Git SHA");
	// The parity test target has no optional GPUI features and uses Cargo's test profile.
	return { gitSha, target, rustcVersion, features: [], profile: "test", instrumented, sourceRoot };
}

async function digest(file: string): Promise<string> {
	const hash = createHash("sha256");
	for await (const chunk of createReadStream(file)) hash.update(chunk);
	return hash.digest("hex");
}

function compiledPaths(value: unknown): string[] {
	const files = strings(value, true);
	if (!files.includes(GPUI_CRATE_ROOT) || files.some((file) => isAbsolute(file) || file.includes("\\") || file.split("/").some((part) => part === ".." || part === "." || !part))) {
		throw new Error("Invalid compiled source inventory for prebuilt GPUI coverage");
	}
	return files;
}

export async function writePrebuiltParity(directory: string, executable: string, build: ParityBuild, compiled?: string[]): Promise<string> {
	if (build.instrumented) compiledPaths(compiled);
	else if (compiled !== undefined) throw new Error("Uninstrumented GPUI parity has no compiled source inventory");
	const binary = join(directory, binaryFile);
	await copyFile(executable, binary, constants.COPYFILE_EXCL);
	const manifest = join(directory, "manifest.json");
	await writeFile(manifest, `${JSON.stringify({ ...build, binaryFile, binarySha256: await digest(binary), compiled }, null, 2)}\n`, { flag: "wx" });
	return manifest;
}

export async function verifyPrebuiltParity(path: string, expected: ParityBuild): Promise<{ executable: string; compiled?: string[] }> {
	const manifest = object(JSON.parse(await readFile(path, "utf8")));
	for (const key of ["gitSha", "target", "rustcVersion", "profile", "instrumented", "sourceRoot"] as const) {
		if (manifest[key] !== expected[key]) throw new Error(`GPUI parity manifest mismatch: ${key}`);
	}
	const features = strings(manifest.features, true);
	if (JSON.stringify(features) !== JSON.stringify(expected.features)) throw new Error("GPUI parity manifest mismatch: features");
	if (manifest.binaryFile !== binaryFile) throw new Error("GPUI parity manifest mismatch: binaryFile");
	const compiled = expected.instrumented ? compiledPaths(manifest.compiled) : undefined;
	if (!expected.instrumented && manifest.compiled !== undefined) throw new Error("Uninstrumented GPUI parity has no compiled source inventory");
	const executable = resolve(dirname(path), binaryFile);
	const file = await lstat(executable).catch((error: unknown) => {
		if (error instanceof Error && "code" in error && error.code === "ENOENT") throw new Error(`Missing GPUI parity binary: ${executable}`, { cause: error });
		throw error;
	});
	if (!file.isFile()) throw new Error(`Expected a regular GPUI parity binary: ${executable}`);
	if (await digest(executable) !== manifest.binarySha256) throw new Error("GPUI parity manifest mismatch: binarySha256");
	// Artifact download may strip executable permissions.
	if (process.platform !== "win32" && !(file.mode & 0o100)) await chmod(executable, file.mode | 0o100);
	return { executable, compiled };
}
