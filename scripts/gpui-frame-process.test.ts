import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { delimiter, join, relative } from "node:path";
import { isParityProcess } from "./gpui-frame-process";

test("detects the executing Bun parity script and native test binary", () => {
	expect(isParityProcess("/usr/bin/bun", ["bun", "scripts/e2e-parity.ts"])).toBe(true);
	expect(isParityProcess("/usr/bin/bun", ["bun", "run", "/checkout/scripts/e2e-parity.ts"])).toBe(true);
	expect(isParityProcess("/checkout/target/debug/deps/parity-ab1234", ["/checkout/target/debug/deps/parity-ab1234"])).toBe(true);
});

test("ignores shell observers, timeout, tail and lock waiters mentioning parity", () => {
	for (const executable of ["zsh", "timeout", "tail", "flock"]) {
		expect(isParityProcess(`/usr/bin/${executable}`, [executable, "bun scripts/e2e-parity.ts", "parity-ab1234"])).toBe(false);
	}
	expect(isParityProcess("/usr/bin/flock", ["flock", "heavy.lock", "bun", "scripts/e2e-parity.ts"])).toBe(false);
});

test("detects parity when its running executable has been replaced", () => {
	expect(isParityProcess("/usr/bin/bun (deleted)", ["bun", "scripts/e2e-parity.ts"])).toBe(true);
	expect(isParityProcess("/checkout/target/debug/deps/parity-ab1234 (deleted)", ["/checkout/target/debug/deps/parity-ab1234"])).toBe(true);
});

test("ignores unrelated Bun scripts and script names embedded in arguments", () => {
	expect(isParityProcess("/usr/bin/bun", ["bun", "observer.ts", "scripts/e2e-parity.ts"])).toBe(false);
	expect(isParityProcess("/usr/bin/bun", ["bun", "scripts/gpui-frames.ts"])).toBe(false);
	expect(isParityProcess("/usr/bin/bun", ["bun", "-e", "console.log('scripts/e2e-parity.ts')"])).toBe(false);
});

const linuxTest = test.skipIf(process.platform !== "linux");
const isolationModule = JSON.stringify(join(import.meta.dir, "gpui-frame-process.ts"));

async function probeIsolation(env: Record<string, string>, script: string) {
	// Bun resolves executables using the child's initial PATH.
	const child = Bun.spawn([process.execPath, "-e", `
		import { holdFrameIsolation } from ${isolationModule};
		try { ${script} }
		catch (error) { console.error(error.message); process.exitCode = 1; }
	`], { env: { ...process.env, ...env }, stdout: "pipe", stderr: "pipe" });
	const [code, stdout, stderr] = await Promise.all([
		child.exited, new Response(child.stdout).text(), new Response(child.stderr).text(),
	]);
	return { code, stdout, stderr };
}

async function isolationFixture() {
	const artifacts = join(import.meta.dir, "../e2e/artifacts");
	await mkdir(artifacts, { recursive: true });
	return mkdtemp(join(artifacts, "frame-isolation-"));
}

const requestLock = (mode: "shared" | "exclusive") => `
	const lock = await holdFrameIsolation("${mode}");
	console.log(lock.state);
	if (lock.state === "held") await lock.release();
`;

linuxTest("refuses a Linux lock request when XDG_RUNTIME_DIR is unset or relative", async () => {
	for (const XDG_RUNTIME_DIR of ["", relative(process.cwd(), "/proc/self")]) {
		const result = await probeIsolation({ XDG_RUNTIME_DIR }, requestLock("exclusive"));
		expect(result.code).toBe(1);
		expect(result.stdout).toBe("");
		expect(result.stderr).toMatch(/Cannot establish frame isolation.*XDG_RUNTIME_DIR/);
	}
});

linuxTest("refuses a Linux lock request when flock is missing", async () => {
	const result = await probeIsolation({ XDG_RUNTIME_DIR: "/proc/self", PATH: "/dev/null" }, requestLock("exclusive"));
	expect(result.code).toBe(1);
	expect(result.stdout).toBe("");
	expect(result.stderr).toMatch(/Cannot establish frame isolation.*flock.*PATH/);
});

linuxTest("reports an unwritable runtime directory without announcing a wait", async () => {
	// procfs cannot create arbitrary files, even when this test runs as root.
	const result = await probeIsolation({ XDG_RUNTIME_DIR: "/proc/self" }, requestLock("shared"));
	expect(result.code).toBe(1);
	expect(result.stdout).toBe("");
	expect(result.stderr).toMatch(/Cannot establish frame isolation.*\/proc\/self\/kubecove-frame-isolation\.lock/);
	expect(result.stderr).toContain("cannot open lock file");
});

linuxTest("reports incompatible flock as a failure for both lock modes", async () => {
	const fixture = await isolationFixture();
	try {
		await writeFile(join(fixture, "flock"), `#!/bin/sh
for arg do
	if [ "$arg" = "--close" ]; then
		echo "flock: unrecognized option '--close'" >&2
		exit 1
	fi
done
exit 0
`, { mode: 0o700 });
		for (const mode of ["exclusive", "shared"] as const) {
			const result = await probeIsolation({
				XDG_RUNTIME_DIR: fixture, PATH: [fixture, process.env.PATH].join(delimiter),
			}, requestLock(mode));
			expect(result.code).toBe(1);
			expect(result.stdout).toBe("");
			expect(result.stderr).toMatch(/Cannot establish frame isolation.*flock/);
			expect(result.stderr).toContain("unrecognized option '--close'");
		}
	} finally {
		await rm(fixture, { recursive: true });
	}
});

linuxTest("a shared parity lock blocks a frame lock until release", async () => {
	const fixture = await isolationFixture();
	try {
		const result = await probeIsolation({ XDG_RUNTIME_DIR: fixture }, `
			const parity = await holdFrameIsolation("shared");
			if (parity.state !== "held") throw new Error("Parity failed to take the lock");
			try { ${requestLock("exclusive")} }
			finally { await parity.release(); }
			${requestLock("exclusive")}
		`);
		expect(result.code).toBe(0);
		expect(result.stdout).toBe("busy\nheld\n");
		expect(result.stderr).toBe("");
	} finally {
		await rm(fixture, { recursive: true });
	}
});
