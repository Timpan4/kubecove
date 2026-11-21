import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { delimiter, join, resolve } from "node:path";
import { readinessTimeoutMs, startIdentity, stopGroup, waitFor } from "../../.agents/skills/verify-kubecove/scripts/processes";

test.skipIf(process.platform === "win32")("interrupted E2E escalates a stuck build, saves its log and cleans up", async () => {
	await mkdir("e2e/artifacts", { recursive: true });
	const root = await mkdtemp(resolve("e2e/artifacts/lifecycle-interrupt-"));
	const ready = join(root, "build.pid");
	try {
		await writeFile(join(root, "cargo"), "#!/bin/sh\nexit 0\n", { mode: 0o755 });
		await writeFile(join(root, "bun"), `#!/bin/sh
[ "$1 $2 $3" = "run tauri build" ] || exit 0
exec "$TEST_BUN" -e '
process.on("SIGTERM", () => console.log("ignored SIGTERM"));
await Bun.write(process.env.TEST_READY, String(process.pid));
setInterval(() => {}, 1000);
'
`, { mode: 0o755 });
		const lifecycle = Bun.spawn([process.execPath, resolve(import.meta.dir, "../lifecycle.ts"), "desktop-smoke", "--run-id", "interrupt-test"], {
			cwd: root,
			env: { ...process.env, PATH: [root, process.env.PATH].join(delimiter), TEST_BUN: process.execPath, TEST_READY: ready },
			stdout: "ignore",
			stderr: "pipe",
			timeout: 3 * readinessTimeoutMs,
			killSignal: "SIGKILL",
		});
		const stderr = new Response(lifecycle.stderr).text();
		let build: { pid: number; identity: string | undefined } | undefined;
		try {
			await waitFor("the simulated E2E build to start", () => Bun.file(ready).exists());
			const pid = Number(await Bun.file(ready).text());
			build = { pid, identity: startIdentity(pid) };
			lifecycle.kill("SIGINT");
			const code = await lifecycle.exited;
			if (code !== 130) throw new Error(`Unexpected lifecycle exit ${code}: ${await stderr}`);
			expect(() => process.kill(pid, 0)).toThrow("ESRCH");
			expect(await Bun.file(join(root, "e2e", "artifacts", "desktop-interrupt-test", "build.log")).text()).toContain("ignored SIGTERM");
			expect(await Bun.file(join(root, ".e2e", "desktop-smoke", "interrupt-test", "kubeconfig")).exists()).toBe(false);
		} finally {
			if (build) await stopGroup(build.pid, build.identity, "SIGKILL");
			if (lifecycle.exitCode === null && lifecycle.signalCode === null) lifecycle.kill("SIGKILL");
			await lifecycle.exited;
		}
	} finally {
		await rm(root, { recursive: true, force: true });
	}
}, 4 * readinessTimeoutMs);
