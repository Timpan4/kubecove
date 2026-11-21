import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { withBrowserServer, writeSmokeConfig } from "../e2e/harness/browser-run";
import { captureProcessOutput, trackChild } from "../e2e/harness/lifecycle-processes";
import { startMemoryFixture } from "../e2e/harness/memory-fixture";

export async function observeReleaseStartup(binary: string, artifacts: string, fonts: string[]) {
	// The fixture holds credentials. CI may collect artifacts even after a forced stop.
	const owned = await mkdtemp(join(tmpdir(), "kubecove-footprint-"));
	const cancellation = new AbortController();
	const interrupt = () => cancellation.abort();
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
			const stdout = captureProcessOutput(child.stdout);
			const stderr = captureProcessOutput(child.stderr);
			const driver = trackChild(child, async (signal) => {
				const [code] = await Promise.all([child.exited, stdout.drain(signal), stderr.drain(signal)]);
				return code;
			});
			const stopDriver = () => { void driver.stop("SIGTERM").catch(() => {}); };
			// A descendant can still hold the output pipes after WDIO exits.
			void child.exited.then(stopDriver);
			cancellation.signal.addEventListener("abort", stopDriver, { once: true });
			if (cancellation.signal.aborted) stopDriver();
			try {
				const code = await driver.done;
				cancellation.signal.throwIfAborted();
				if (code !== 0) throw new Error(`Footprint browser check failed; see ${artifacts}`);
			} finally {
				cancellation.signal.removeEventListener("abort", stopDriver);
				try { await driver.stop("SIGTERM"); } finally {
					await writeFile(join(artifacts, "wdio-report.txt"), stdout.text() + stderr.text());
				}
			}
		});
	} finally {
		process.off("SIGINT", interrupt);
		process.off("SIGTERM", interrupt);
		try { await fixture?.stop(); } finally { await rm(owned, { recursive: true, force: true }); }
	}
}
