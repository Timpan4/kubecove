import { mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { stringify } from "yaml";
import { assertDesktopProfileIdle, stopOwnedProfileProcess } from "../e2e/harness/desktop-profile";
import { startMemoryFixture } from "../e2e/harness/memory-fixture";

export async function observeReleaseStartup(binary: string, artifacts: string, fonts: string[]) {
	await assertDesktopProfileIdle();
	const owned = join(artifacts, "fixture");
	await mkdir(join(owned, "data"), { recursive: true });
	let fixture: Awaited<ReturnType<typeof startMemoryFixture>> | undefined;
	let driver: Bun.Subprocess | undefined;
	try {
		fixture = await startMemoryFixture("normal", owned);
		const cluster = "kubecove-e2e-footprint";
		const config = {
			"apiVersion": "v1", "kind": "Config",
			"clusters": [{ name: cluster, cluster: { server: fixture.url, "certificate-authority-data": fixture.authority } }],
			"contexts": ["admin", "restricted"].map((role) => ({ name: `${cluster}-${role}`, context: { cluster, user: `${cluster}-${role}` } })),
			"current-context": `${cluster}-admin`,
			"users": ["admin", "restricted"].map((role) => ({ name: `${cluster}-${role}`, user: { token: "redacted-smoke-value" } })),
		};
		const kubeconfig = join(owned, "kubeconfig");
		await writeFile(kubeconfig, stringify(config), { mode: 0o600 });
		const env = { ...process.env, KUBECONFIG: undefined, KUBECOVE_E2E: "1", KUBECOVE_KUBECONFIG: kubeconfig, KUBECOVE_DATA_DIR: join(owned, "data"), KUBECOVE_E2E_CLUSTER: cluster, KUBECOVE_E2E_ARTIFACTS: artifacts, KUBECOVE_E2E_BINARY: binary, KUBECOVE_PROFILE: "1", KUBECOVE_MEMORY_FIXTURE: undefined, KUBECOVE_FOOTPRINT_FONTS: JSON.stringify(fonts) };
		const child = Bun.spawn(["node", "node_modules/@wdio/cli/bin/wdio.js", "run", "e2e/wdio.footprint.conf.ts"], { env, detached: true, stdout: "pipe", stderr: "pipe" });
		driver = child;
		const [code, stdout, stderr] = await Promise.all([child.exited, new Response(child.stdout).text(), new Response(child.stderr).text()]);
		await writeFile(join(artifacts, "wdio-report.txt"), `${stdout}${stderr}`);
		if (code !== 0) throw new Error(`Native footprint scenario failed; see ${artifacts}`);
	} finally {
		if (driver) await stopOwnedProfileProcess(driver, "SIGTERM");
		fixture?.stop();
		await rm(owned, { recursive: true, force: true });
	}
}
