import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { freePort, waitFor } from "../.agents/skills/verify-kubecove/scripts/processes";
import { writeSmokeConfig } from "./harness/browser-run";
import { gpuiDriver, mcpCall } from "./harness/gpui-automation";
import { startMemoryFixture } from "./harness/memory-fixture";
import { startServer } from "./harness/server-process";

function required(name: string) {
	const value = process.env[name];
	if (!value) throw new Error(`Missing ${name} from desktop smoke launcher`);
	return value;
}
const data = required("KUBECOVE_DATA_DIR");
const artifacts = required("KUBECOVE_E2E_ARTIFACTS");
const cluster = required("KUBECOVE_E2E_CLUSTER");
const binary = required("KUBECOVE_GPUI_BINARY");
const serverBinary = required("KUBECOVE_SERVER_BINARY");
const port = await freePort();
await writeFile(join(data, "mcp-settings.json"), JSON.stringify({ enabled: false, port }), { flag: "wx" });
const remoteDir = join(data, "remote-fixture");
await mkdir(remoteDir, { mode: 0o700 });
const fixture = await startMemoryFixture("normal", remoteDir, { remoteDesktop: true });
const checks: string[] = [];
const cancelled = new AbortController();
let child: Bun.Subprocess | undefined;
const stop = () => { cancelled.abort(); child?.kill("SIGTERM"); };
process.once("SIGINT", stop);
process.once("SIGTERM", stop);
let server: ReturnType<typeof startServer> | undefined;
let passed = false;
try {
	const config = await writeSmokeConfig(remoteDir, "remote", fixture);
	server = startServer(serverBinary, remoteDir, { KUBECONFIG: config, HOME: remoteDir, USERPROFILE: remoteDir,
		XDG_CONFIG_HOME: remoteDir, KUBECOVE_E2E: undefined, DBUS_SESSION_BUS_ADDRESS: undefined }, cancelled.signal);
	const remote = await server.ready;
	const app = Bun.spawn([binary], { stdout: "pipe", stderr: "ignore", env: { ...process.env, KUBECONFIG: undefined } });
	child = app;
	let endpoint: string | undefined;
	const drain = (async () => {
		let buffer = "";
		for await (const chunk of app.stdout) {
			buffer += new TextDecoder().decode(chunk);
			const lines = buffer.split("\n");
			buffer = lines.pop() ?? "";
			for (const line of lines) endpoint ??= /MCP endpoint (http:\/\/127\.0\.0\.1:\d+\/mcp);/.exec(line)?.[1];
		}
	})();
	await Promise.race([waitFor("GPUI automation listener", async () => Boolean(endpoint)),
		app.exited.then(() => { throw new Error("GPUI exited before automation was ready"); })]);
	if (!endpoint) throw new Error("Missing GPUI endpoint");
	const ui = await gpuiDriver(endpoint, data);
	await ui.id(`workspace-group:${cluster}-admin`);
	await ui.id(`workspace-group:${cluster}-restricted`);
	await ui.click("Open settings");
	await ui.clickId("settings-section:AI agents");
	checks.push("Pinned admin and restricted contexts visible; Settings opened");
	await ui.find("MCP toggle ready", (el) => el.label === "MCP server" && el.description === "enabled");
	await ui.click("MCP server");
	await ui.find("Product MCP listening", (el) => Boolean(el.label?.includes(`Listening at http://127.0.0.1:${port}/mcp`)));
	const mcp = `http://127.0.0.1:${port}/mcp`;
	const productToken = (await readFile(join(data, "mcp-token"), "utf8")).trim();
	const result = await mcpCall(mcp, productToken, "list_contexts");
	assert.deepEqual(result.contexts.map((context: { name: string }) => context.name).sort(), [`${cluster}-admin`, `${cluster}-restricted`]);
	await ui.click("MCP server");
	await waitFor("disabled product listener to close", async () => fetch(mcp).then(() => false, () => true));
	checks.push("Enabled product MCP in Settings, read both contexts and closed listener");
	await ui.click("Remote server");
	await ui.type("Server URL", remote.origin);
	await ui.type("Server token", remote.token);
	await ui.click("Connect");
	await ui.label("Disconnect");
	await ui.click("Back to app");
	await ui.id("workspace-group:remote-admin");
	await ui.clickId("workspace-context");
	await ui.clickId("workspace-context:remote-restricted");
	await ui.type("Name", "Remote smoke");
	await ui.id("workspace-namespace:profile");
	await ui.click("Create workspace");
	const view = await ui.find("created workspace", (el) => el.path.endsWith("/open-resources") || el.label === "Open resource profile-pod-0");
	if (view.path.endsWith("/open-resources")) await ui.clickId("open-resources");
	await ui.click("Open resource profile-pod-0");
	await ui.clickId("show-logs");
	await ui.find("Remote log line", (el) => Boolean(el.label?.includes("kubecove-remote-log-marker")));
	checks.push("Signed into remote server and received real Pod stream feedback");
	await ui.click("Open settings");
	await ui.click("Remote server");
	await ui.click("Disconnect");
	await ui.label("Connect");
	checks.push("Disconnected the remote GPUI session");
	app.kill("SIGTERM");
	await drain;
	passed = true;
} finally {
	child?.kill("SIGTERM");
	if (child) await child.exited;
	if (server) await server.stop();
	fixture.stop();
	process.off("SIGINT", stop);
	process.off("SIGTERM", stop);
	await writeFile(join(artifacts, "gpui-smoke.json"), JSON.stringify({ passed, checks }, null, 2));
}
