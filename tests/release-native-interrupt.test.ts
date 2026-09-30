import { expect, it } from "bun:test";
import { existsSync, readFileSync, watch } from "node:fs";
import { chmod, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

// Failure modes: a parent interrupt must reap its detached driver and remove
// the owned fixture certificate, private key, kubeconfig, and data directory.
it.skipIf(process.platform !== "linux").each(["SIGINT", "SIGTERM"] as const)("cleans native profiling ownership after %s", async (signal) => {
	const root = process.env.KUBECOVE_PROFILE_REPO ?? resolve(import.meta.dir, "..");
	const artifacts = join(root, "e2e", "artifacts");
	await mkdir(artifacts, { recursive: true });
	const directory = await mkdtemp(join(artifacts, "footprint-interrupt-"));
	const marker = join(directory, "driver-ready.json");
	const nativeArtifacts = join(directory, "native");
	await mkdir(nativeArtifacts);
	const fakeNode = join(directory, "node");
	await writeFile(fakeNode, `#!${process.execPath}\nconst server = Bun.serve({hostname: "127.0.0.1", port: 0, fetch: () => new Response("owned test driver")});\nawait Bun.write(process.env.PROFILE_READY + ".tmp", JSON.stringify({pid:process.pid}));\nawait import("node:fs/promises").then(fs => fs.rename(process.env.PROFILE_READY + ".tmp", process.env.PROFILE_READY));\n`);
	await chmod(fakeNode, 0o700);
	let resolveReady!: () => void;
	const ready = new Promise<void>((resolve) => { resolveReady = resolve; });
	const watcher = watch(directory, (_event, name) => { if (name === "driver-ready.json" && existsSync(marker)) resolveReady(); });
	const helper = join(root, "scripts", "perf-release-native.ts");
	const child = Bun.spawn([process.execPath, "-e", `import {observeReleaseStartup} from ${JSON.stringify(helper)}; await observeReleaseStartup("unused-owned-test-binary", ${JSON.stringify(nativeArtifacts)}, []);`], {
		cwd: root, env: { ...process.env, PATH: `${directory}:${process.env.PATH}`, PROFILE_READY: marker }, stdout: "pipe", stderr: "pipe",
	});
	let driverPid: number | undefined;
	let cleanupError: unknown;
	try {
		await Promise.race([ready, child.exited.then((code) => { throw new Error(`Helper exited before its owned driver was ready (${code})`); })]);
		const readyDriverPid: number = JSON.parse(readFileSync(marker, "utf8")).pid;
		driverPid = readyDriverPid;
		const fixture = join(nativeArtifacts, "fixture");
		expect(existsSync(join(fixture, "kubeconfig"))).toBe(true);
		child.kill(signal);
		const code = await child.exited;
		let driverAlive = true;
		try { process.kill(readyDriverPid, 0); } catch (error) {
			if (!(error instanceof Error && "code" in error && error.code === "ESRCH")) throw error;
			driverAlive = false;
		}
		const fixtureRemoved = !existsSync(fixture);
		await writeFile(join(directory, "report.json"), `${JSON.stringify({ signal, code, driverAlive, fixtureRemoved }, null, 2)}\n`);
		expect(driverAlive).toBe(false);
		expect(fixtureRemoved).toBe(true);
	} finally {
		watcher.close();
		if (child.exitCode === null && child.signalCode === null) { child.kill("SIGTERM"); await child.exited; }
		if (driverPid) try { process.kill(-driverPid, "SIGTERM"); } catch (error) {
			if (!(error instanceof Error && "code" in error && error.code === "ESRCH")) cleanupError = error;
		}
		await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text()]);
		await rm(nativeArtifacts, { recursive: true, force: true });
		await rm(fakeNode, { force: true });
		await rm(marker, { force: true });
	}
	if (cleanupError) throw cleanupError;
});
