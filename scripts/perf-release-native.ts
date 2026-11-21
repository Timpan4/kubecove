import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { withBrowserServer, writeSmokeConfig } from "../e2e/harness/browser-run";
import { signalOwnedProcess } from "../e2e/harness/desktop-profile";
import { startMemoryFixture } from "../e2e/harness/memory-fixture";

export async function observeReleaseStartup(binary: string, artifacts: string, fonts: string[]) {
	// The fixture holds credentials. CI may collect artifacts even after a forced stop.
	const owned = await mkdtemp(join(tmpdir(), "kubecove-footprint-"));
	const cancellation = new AbortController();
	let driver: Bun.Subprocess | undefined;
	let stopping: Promise<void> | undefined;
	const interrupt = () => {
		cancellation.abort();
		if (driver) stopping ??= signalOwnedProcess(driver, "SIGTERM");
	};
	process.on("SIGINT", interrupt);
	process.on("SIGTERM", interrupt);
	let fixture: Awaited<ReturnType<typeof startMemoryFixture>> | undefined;
	try {
		fixture = await startMemoryFixture("normal", owned);
		const kubeconfig = await writeSmokeConfig(owned, "kubecove-e2e-footprint", fixture);
		await withBrowserServer(binary, join(owned, "data"), kubeconfig, cancellation.signal, async (server) => {
			const child = Bun.spawn(["node", "node_modules/@wdio/cli/bin/wdio.js", "run", "e2e/wdio.footprint.conf.ts"], {
				env: { ...process.env, ...server, KUBECOVE_E2E_ARTIFACTS: artifacts, KUBECOVE_PROFILE: "1", KUBECOVE_FOOTPRINT_FONTS: JSON.stringify(fonts) },
				detached: process.platform !== "win32", stdout: "pipe", stderr: "pipe",
			});
			driver = child;
			const [code, stdout, stderr] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
			await writeFile(join(artifacts, "wdio-report.txt"), stdout + stderr);
			cancellation.signal.throwIfAborted();
			if (code !== 0) throw new Error(`Footprint browser check failed; see ${artifacts}`);
		});
	} finally {
		try { await stopping; } finally {
			fixture?.stop();
			process.off("SIGINT", interrupt);
			process.off("SIGTERM", interrupt);
			await rm(owned, { recursive: true, force: true });
		}
	}
}
