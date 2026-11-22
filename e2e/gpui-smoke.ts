import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createConnection } from "node:net";
import { join } from "node:path";
import { freePort, readinessTimeoutMs, waitFor } from "../.agents/skills/verify-kubecove/scripts/processes";
import { writeSmokeConfig } from "./harness/browser-run";
import { safeDiagnosticText } from "./harness/diagnostics";
import { gpuiDriver, mcpCall } from "./harness/gpui-automation";
import { captureProcessOutput, processOutput, trackChild } from "./harness/lifecycle-processes";
import { startMemoryFixture } from "./harness/memory-fixture";
import { startServer } from "./harness/server-process";

function required(name: string) {
	const value = process.env[name];
	if (!value) throw new Error(`Missing ${name} from desktop smoke launcher`);
	return value;
}
const artifacts = required("KUBECOVE_E2E_ARTIFACTS");
const checks: string[] = [];
const failures: string[] = [];
const skipped: { check: string; reason: string }[] = [];
const cancelled = new AbortController();
const stop = () => cancelled.abort();
process.once("SIGINT", stop);
process.once("SIGTERM", stop);
let fixture: Awaited<ReturnType<typeof startMemoryFixture>> | undefined;
let server: ReturnType<typeof startServer> | undefined;
let child: ReturnType<typeof trackChild<number>> | undefined;
let startupStderr: ReturnType<typeof captureProcessOutput> | undefined;
let startupComplete = false;
try {
	await mkdir(artifacts, { recursive: true });
	const data = required("KUBECOVE_DATA_DIR");
	const cluster = required("KUBECOVE_E2E_CLUSTER");
	const binary = required("KUBECOVE_GPUI_BINARY");
	const port = await freePort();
	await writeFile(join(data, "mcp-settings.json"), JSON.stringify({ enabled: false, port }), { flag: "wx" });
	const app = Bun.spawn([binary], { detached: process.platform !== "win32", stdout: "pipe", stderr: "pipe", env: { ...process.env, KUBECONFIG: undefined } });
	const stderr = captureProcessOutput(app.stderr);
	startupStderr = stderr;
	let endpoint: string | undefined;
	const readEndpoint = async (signal: AbortSignal) => {
		let buffer = "";
		const decoder = new TextDecoder();
		for await (const chunk of processOutput(app.stdout, signal)) {
			buffer += decoder.decode(chunk, { stream: true });
			const lines = buffer.split("\n");
			buffer = lines.pop() ?? "";
			for (const line of lines) endpoint ??= /MCP endpoint (http:\/\/127\.0\.0\.1:\d+\/mcp);/.exec(line)?.[1];
		}
	};
	child = trackChild(app, async (signal) => {
		const [code] = await Promise.all([app.exited, readEndpoint(signal), stderr.drain(signal)]);
		return code;
	});
	void child.done.catch(() => {});
	await waitFor("GPUI automation listener", async () => {
		cancelled.signal.throwIfAborted();
		const exit = app.exitCode ?? app.signalCode;
		if (exit !== null) throw new Error(`GPUI exited (${exit}) before automation was ready`);
		return Boolean(endpoint);
	}, readinessTimeoutMs, { retryErrors: false });
	if (!endpoint) throw new Error("Missing GPUI endpoint");
	const ui = await gpuiDriver(endpoint, data, cancelled.signal);
	await ui.id(`workspace-group:${cluster}-admin`);
	startupComplete = true;
	await ui.id(`workspace-group:${cluster}-restricted`);
	await ui.click("Open settings");
	await ui.clickId("settings-section:AI agents");
	checks.push("Pinned admin and restricted contexts visible; Settings opened");
	const bindSignal = AbortSignal.any([cancelled.signal, AbortSignal.timeout(readinessTimeoutMs)]);
	const bindUi = await gpuiDriver(endpoint, data, bindSignal);
	const productPort = await enableProductMcp(bindUi, port, bindSignal);
	const mcp = `http://127.0.0.1:${productPort}/mcp`;
	const productToken = (await readFile(join(data, "mcp-token"), "utf8")).trim();
	const result = await mcpCall(mcp, productToken, "list_contexts", {}, AbortSignal.any([cancelled.signal, AbortSignal.timeout(readinessTimeoutMs)]));
	assert.deepEqual(result.contexts.map((context: { name: string }) => context.name).sort(), [`${cluster}-admin`, `${cluster}-restricted`]);
	await ui.click("MCP server");
	await waitFor("disabled product listener to close", (signal) => listenerRefused(productPort, AbortSignal.any([cancelled.signal, signal])), readinessTimeoutMs, { retryErrors: false });
	checks.push("Enabled product MCP in Settings, read both contexts and closed listener");
	if (process.platform === "win32") {
		skipped.push({ check: "Remote server sign-in, Pod log stream, and disconnect", reason: "Windows does not support the private token file required by token-mode server startup." });
	} else {
		const remoteDir = join(data, "remote-fixture");
		await mkdir(remoteDir, { mode: 0o700 });
		fixture = await startMemoryFixture("normal", remoteDir, { remoteDesktop: true, signal: cancelled.signal });
		const config = await writeSmokeConfig(remoteDir, "remote", fixture);
		server = startServer(required("KUBECOVE_SERVER_BINARY"), remoteDir, { KUBECONFIG: config, HOME: remoteDir, USERPROFILE: remoteDir,
			XDG_CONFIG_HOME: remoteDir, KUBECOVE_E2E: undefined, DBUS_SESSION_BUS_ADDRESS: undefined }, cancelled.signal);
		await checkRemoteDesktop(ui, await server.ready, fixture);
	}
} catch (error) {
	failures.push(String(error));
} finally {
	try {
		const cleanup = await Promise.allSettled([child?.stop("SIGTERM"), server?.stop()]);
		for (const result of cleanup) if (result.status === "rejected") failures.push(String(result.reason));
		await fixture?.stop();
	} catch (error) {
		failures.push(String(error));
	} finally {
		process.off("SIGINT", stop);
		process.off("SIGTERM", stop);
		const stderr = startupComplete ? undefined : safeDiagnosticText(startupStderr?.text() ?? "").trim();
		if (stderr) failures.push(`GPUI startup stderr:\n${stderr}`);
		await writeFile(join(artifacts, "gpui-smoke.json"), JSON.stringify({ passed: failures.length === 0, checks, skipped, failures }, null, 2));
	}
}
if (failures.length) throw new Error(failures.join("\n"));

async function enableProductMcp(ui: Awaited<ReturnType<typeof gpuiDriver>>, port: number, signal: AbortSignal) {
	await ui.find("MCP toggle ready", (el) => el.label === "MCP server" && el.description === "enabled");
	await ui.click("MCP server");
	let previousError: string | undefined;
	while (!signal.aborted) {
		const outcome = await ui.find("Product MCP listener outcome", (el) =>
			el.label?.startsWith(`Listening at http://127.0.0.1:${port}/mcp `) === true ||
			(el.path.endsWith("/mcp-error") && el.label !== previousError));
		if (outcome.label?.startsWith("Listening at ")) return port;
		if (!outcome.label?.startsWith(`Could not listen on 127.0.0.1:${port}:`) || !portInUse(port)) {
			throw new Error(outcome.label ?? "Product MCP listener failed");
		}
		previousError = outcome.label;
		let replacement = await freePort();
		while (replacement === port) {
			signal.throwIfAborted();
			replacement = await freePort();
		}
		port = replacement;
		await ui.click("MCP port");
		await ui.call("press_key", { key: "secondary-a" });
		await ui.call("type_text", { text: String(port) });
		await ui.click("Save");
	}
	throw signal.reason;
}

function portInUse(port: number) {
	try {
		const listener = Bun.listen({ hostname: "127.0.0.1", port, socket: { data() {} } });
		listener.stop(true);
		return false;
	} catch (error) {
		if (error instanceof Error && "code" in error && error.code === "EADDRINUSE") return true;
		throw error;
	}
}

function listenerRefused(port: number, signal: AbortSignal): Promise<boolean> {
	return new Promise((resolve, reject) => {
		const socket = createConnection({ host: "127.0.0.1", port, signal });
		socket.once("connect", () => { socket.destroy(); resolve(false); });
		socket.once("error", (error) => {
			const code = "code" in error ? error.code : undefined;
			if (code === "ECONNREFUSED") resolve(true);
			else if (code === "ECONNRESET") resolve(false);
			else reject(error);
		});
	});
}

async function checkRemoteDesktop(ui: Awaited<ReturnType<typeof gpuiDriver>>, remote: { origin: string; token: string }, fixture: Awaited<ReturnType<typeof startMemoryFixture>>) {
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
	assert.deepEqual(fixture.logStreams(), { active: 1, cancelled: 0 });
	checks.push("Signed into remote server and received real Pod stream feedback");
	// Opening Settings closes the resource detail and its log stream before Disconnect is visible.
	await ui.click("Open settings");
	await waitFor("leaving Pod logs to cancel the upstream stream", async () => {
		cancelled.signal.throwIfAborted();
		const logs = fixture.logStreams();
		return logs.active === 0 && logs.cancelled > 0;
	}, readinessTimeoutMs, { retryErrors: false });
	checks.push("Leaving the Pod log view cancelled its live stream at the fixture");
	await ui.click("Remote server");
	await ui.click("Disconnect");
	await ui.label("Connect");
	checks.push("Disconnected the remote GPUI session");
}
