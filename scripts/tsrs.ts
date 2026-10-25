import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import tsrs from "tsrs/package.json";

const require = createRequire(import.meta.url);
const root = fileURLToPath(new URL("../", import.meta.url));
// The published 0.18 release lacks Windows fixes present in this 0.19 candidate.
const sourceRevision = "e906c9e430f555ff66286ff81cbf0d23566ad742";
const sourceRoot = join(
	root,
	".e2e",
	"tools",
	"tsrs",
	sourceRevision,
	`${process.platform}-${process.arch}`,
);

async function run(command: string[], env = process.env): Promise<number> {
	const child = Bun.spawn(command, {
		cwd: root,
		env,
		stdin: "inherit",
		stdout: "inherit",
		stderr: "inherit",
	});
	const interrupt = () => child.kill("SIGINT");
	const terminate = () => child.kill("SIGTERM");
	process.on("SIGINT", interrupt);
	process.on("SIGTERM", terminate);
	try {
		return await child.exited;
	} finally {
		process.off("SIGINT", interrupt);
		process.off("SIGTERM", terminate);
	}
}

async function buildSource(): Promise<string> {
	const binary = join(
		sourceRoot,
		"bin",
		process.platform === "win32" ? "tsrs.exe" : "tsrs",
	);
	if (!existsSync(binary)) {
		console.error(
			`Building tsrs from ${sourceRevision}. Requires Rust 1.99 or newer and the host C/C++ build tools.`,
		);
		const code = await run([
			"cargo",
			"install",
			"--git",
			"https://github.com/oxc-project/tsrs",
			"--rev",
			sourceRevision,
			"--locked",
			"--root",
			sourceRoot,
			"--target-dir",
			join(sourceRoot, "target"),
			"--target",
			"host-tuple",
			"--bin",
			"tsrs",
			"tsrs_cli",
		]);
		if (code !== 0)
			throw new Error(`tsrs source build failed with exit code ${code}.`);
		if (!existsSync(binary))
			throw new Error(`tsrs source build did not produce ${binary}.`);
	}
	return binary;
}

async function main(): Promise<number> {
	const args = process.argv.slice(2);
	if (args[0] === "--build-source") {
		if (args.length !== 1)
			throw new Error("--build-source does not accept compiler arguments.");
		console.log(await buildSource());
		return 0;
	}
	const platformPackage = `@tsrs/${process.platform}-${process.arch}`;
	const hasPrebuilt = Object.keys(tsrs.optionalDependencies).some(
		(name) =>
			name === platformPackage || name.startsWith(`${platformPackage}-`),
	);
	const env = { ...process.env };
	if (!env.TSRS_BINARY && !hasPrebuilt) env.TSRS_BINARY = await buildSource();
	const svelte = args[0] === "--svelte";
	if (svelte) args.shift();
	const entry = svelte
		? join(
				dirname(require.resolve("svelte-check/package.json")),
				"bin",
				"svelte-check",
			)
		: join(dirname(require.resolve("tsrs/package.json")), tsrs.bin.tsrs);
	return run([process.execPath, entry, ...args], env);
}

try {
	process.exitCode = await main();
} catch (error) {
	console.error(error instanceof Error ? error.message : error);
	process.exitCode = 1;
}
