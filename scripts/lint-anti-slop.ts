import { existsSync } from "node:fs";
import { join } from "node:path";

export function isLinuxAllocatorFailure(platform: string, stderr: string): boolean {
	return platform === "linux" && stderr.includes("oxc_allocator/src/pool/fixed_size.rs") && stderr.includes("panicked");
}

async function jemallocLibrary(): Promise<string | undefined> {
	const ldconfig = Bun.which("ldconfig") ?? "/sbin/ldconfig";
	if (existsSync(ldconfig)) {
		const child = Bun.spawn([ldconfig, "-p"], { stdout: "pipe", stderr: "ignore" });
		const libraries = await new Response(child.stdout).text();
		await child.exited;
		const match = /^\s*libjemalloc\.so\.2\s+.*=>\s+(\S+)$/m.exec(libraries);
		if (match && existsSync(match[1])) return match[1];
	}
	// An unprivileged Debian package extraction can provide the same library locally.
	const root = join(import.meta.dir, "..", ".e2e", "tools", "allocator", "root");
	if (!existsSync(root)) return undefined;
	for await (const path of new Bun.Glob("usr/lib/*/libjemalloc.so.2").scan({ cwd: root, absolute: true })) return path;
	return undefined;
}

async function lint(library?: string) {
	const root = join(import.meta.dir, "..");
	const env = { ...process.env };
	if (library) env.LD_PRELOAD = [library, env.LD_PRELOAD].filter(Boolean).join(" ");
	const child = Bun.spawn(["node", join(root, "node_modules", "oxlint", "bin", "oxlint"), "--config", "oxlint.config.ts", ".", ...Bun.argv.slice(2)], {
		cwd: root, env, stdout: "pipe", stderr: "pipe",
	});
	const [code, stdout, stderr] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
	return { code, stdout, stderr };
}

if (import.meta.main) {
	let result = await lint();
	if (result.code !== 0 && isLinuxAllocatorFailure(process.platform, result.stderr)) {
		const library = await jemallocLibrary();
		if (library) {
			console.error("Oxlint hit its Linux allocation limit; retrying with jemalloc for this linter process only.");
			result = await lint(library);
		} else {
			console.error("Install the jemalloc runtime (Debian/Ubuntu: libjemalloc2) to work around Oxlint's Linux allocation limit. No lint rules were skipped.");
		}
	}
	process.stdout.write(result.stdout);
	process.stderr.write(result.stderr);
	process.exit(result.code);
}
