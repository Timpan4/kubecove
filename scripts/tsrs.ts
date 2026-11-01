import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import tsrs from "tsrs/package.json";

const require = createRequire(import.meta.url);
const root = fileURLToPath(new URL("../", import.meta.url));

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

async function main(): Promise<number> {
	const args = process.argv.slice(2);
	const platformPackage = `@tsrs/${process.platform}-${process.arch}`;
	const hasPrebuilt = Object.keys(tsrs.optionalDependencies).some(
		(name) =>
			name === platformPackage || name.startsWith(`${platformPackage}-`),
	);
	const env = { ...process.env };
	if (!env.TSRS_BINARY && !hasPrebuilt) {
		throw new Error(
			`tsrs publishes no binary for ${process.platform}-${process.arch}; set TSRS_BINARY to a tsrs compiler.`,
		);
	}
	if (env.TSRS_BINARY && !existsSync(env.TSRS_BINARY)) {
		throw new Error("TSRS_BINARY points to a compiler that does not exist.");
	}
	const svelte = args[0] === "--svelte";
	if (svelte) args.shift();
	if (!svelte && env.TSRS_BINARY) {
		try {
			return await run([env.TSRS_BINARY, ...args], env);
		} catch {
			throw new Error("Cannot execute the compiler selected by TSRS_BINARY.");
		}
	}
	const tsrsRoot = dirname(require.resolve("tsrs/package.json"));
	if (svelte) {
		// Use the pinned package's resolver for platform/libc selection.
		const { default: getExePath } = await import(
			pathToFileURL(join(tsrsRoot, "lib", "getExePath.js")).href
		);
		env.TSGO_BIN = env.TSRS_BINARY || getExePath();
	}
	const entry = svelte
		? fileURLToPath(new URL("./svelte-check.mjs", import.meta.url))
		: join(tsrsRoot, tsrs.bin.tsrs);
	return run([process.execPath, entry, ...args], env);
}

try {
	process.exitCode = await main();
} catch (error) {
	console.error(error instanceof Error ? error.message : error);
	process.exitCode = 1;
}
