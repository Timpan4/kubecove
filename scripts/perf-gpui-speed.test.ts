import { expect, onTestFinished, test } from "bun:test";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { constants } from "node:os";
import { join } from "node:path";

const supported = process.platform === "linux" || process.platform === "darwin";
const artifacts = join(import.meta.dir, "..", "e2e", "artifacts");

async function startRunner(mockAction: string) {
	await mkdir(artifacts, { recursive: true });
	const out = await mkdtemp(join(artifacts, "gpui-speed-test-"));
	const binary = join(out, "mock-speed");
	await writeFile(binary, `#!${process.execPath}
import { dirname, join } from "node:path";
import { parse } from "yaml";
const config = process.env.KUBECONFIG;
const { clusters } = parse(await Bun.file(config).text());
const fixtureUrl = new URL("/api", clusters[0].cluster.server).href;
const controlUrl = "http://" + process.env.KUBECOVE_SPEED_CONTROL;
const started = Date.now();
const response = await fetch(fixtureUrl, { tls: { ca: await Bun.file(join(dirname(config), "cert.pem")).text() } });
if (!response.ok) throw new Error("mock fixture read failed");
await response.json();
console.error("mock speed diagnostic");
${mockAction}
`, { mode: 0o700 });
	const runner = Bun.spawn([process.execPath, join(import.meta.dir, "perf-gpui-speed.ts"),
		"--bin", binary, "--fixture", "normal", "--latency-ms", "0,25", "--out", out,
	], { stdout: "pipe", stderr: "pipe" });
	onTestFinished(async () => {
		if (runner.exitCode === null && runner.signalCode === null) runner.kill("SIGKILL");
		await runner.exited;
	});
	return { runner, out };
}

function processExists(pid: number) {
	try {
		process.kill(pid, 0);
		return true;
	} catch (error) {
		if (error instanceof Error && "code" in error && error.code === "ESRCH") return false;
		throw error;
	}
}

for (const signal of ["SIGINT", "SIGTERM"] as const) {
	test.skipIf(!supported)(`${signal} reaps only the stalled speed child and preserves diagnostics`, async () => {
		const sibling = Bun.spawn([process.execPath, "-e", "await Bun.stdin.text()"], { stdin: "pipe" });
		onTestFinished(async () => {
			sibling.stdin.end();
			await sibling.exited;
		});
		// Release the mock even if a broken runner exits without reaping it.
		const release = Promise.withResolvers<void>();
		const lifetime = Bun.serve({
			hostname: "127.0.0.1",
			port: 0,
			idleTimeout: 0,
			async fetch() {
				await release.promise;
				return new Response();
			},
		});
		onTestFinished(async () => {
			release.resolve();
			await lifetime.stop(true);
		});
		const { runner, out } = await startRunner(`
process.on("SIGINT", () => {});
process.on("SIGTERM", () => {});
console.log(JSON.stringify({ pid: process.pid, fixtureUrl, controlUrl }));
await fetch(${JSON.stringify(lifetime.url.href)}).catch(() => {});
`);
		const stderr = new Response(runner.stderr).text();
		const reader = runner.stdout.getReader();
		const ready = await reader.read();
		if (ready.done) throw new Error(`Mock did not start. ${await stderr}`);
		const { pid, fixtureUrl, controlUrl } = JSON.parse(new TextDecoder().decode(ready.value));
		runner.kill(signal);
		expect(await runner.exited).toBe(128 + constants.signals[signal]);
		expect(processExists(pid)).toBe(false);
		expect(sibling.exitCode).toBeNull();
		expect(sibling.signalCode).toBeNull();
		expect(await stderr).toContain(signal);
		expect(await Bun.file(join(out, "stderr-0.log")).text()).toContain("mock speed diagnostic");
		expect(await Bun.file(join(out, "requests-0.json")).json()).toMatchObject([{ method: "GET", path: "/api" }]);
		expect(await Bun.file(join(out, "fixture-25", "kubeconfig")).exists()).toBe(false);
		const ca = await Bun.file(join(out, "fixture-0", "cert.pem")).text();
		await expect(fetch(fixtureUrl, { tls: { ca } })).rejects.toThrow();
		await expect(fetch(controlUrl)).rejects.toThrow();
	});
}

test.skipIf(!supported)("child failure includes its exit code and keeps request and stderr artifacts", async () => {
	const { runner, out } = await startRunner("process.exit(7);");
	const stderr = new Response(runner.stderr).text();
	expect(await runner.exited).not.toBe(0);
	expect(await stderr).toContain("exited 7");
	expect(await Bun.file(join(out, "stderr-0.log")).text()).toContain("mock speed diagnostic");
	expect(await Bun.file(join(out, "requests-0.json")).json()).toMatchObject([{ method: "GET", path: "/api" }]);
});

test.skipIf(!supported)("successful latencies still write reports and per-action reads", async () => {
	const { runner, out } = await startRunner(`
await Bun.write(process.env.KUBECOVE_SPEED_REPORT, JSON.stringify({
  actions: [{ label: "mock action", startEpochMs: started, endEpochMs: Date.now() }],
}));
`);
	const [exitCode, stderr] = await Promise.all([runner.exited, new Response(runner.stderr).text()]);
	if (exitCode !== 0) throw new Error(stderr);
	for (const latency of [0, 25]) {
		expect(await Bun.file(join(out, `speed-${latency}.json`)).exists()).toBe(true);
		expect(await Bun.file(join(out, `reads-${latency}.json`)).json()).toEqual([
			{ label: "mock action", served: 1, watches: 0, paths: { "GET /api": 1 } },
		]);
	}
});
