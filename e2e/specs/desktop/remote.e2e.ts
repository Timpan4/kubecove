import { type ChildProcess, spawn } from "node:child_process";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { createConnection } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { $, browser, expect } from "@wdio/globals";
import { after, before, describe, it } from "mocha";
import type { RemoteConnectionSummary } from "../../../src/lib/remote-runtime";
import type { ClusterContext, KubeconfigSourcesSummary, PortForwardSessionSummary } from "../../../src/lib/types";

const serverBinary = process.env.KUBECOVE_HEADLESS_BINARY;

declare global {
	interface Window {
		__KUBECOVE_REMOTE_STREAM__?: Array<{ type: string }>;
	}
}

async function portRefusesConnection(port: number): Promise<boolean> {
	return new Promise((resolveRefused) => {
		const socket = createConnection({ host: "127.0.0.1", port });
		socket.once("error", () => resolveRefused(true));
		socket.once("connect", () => { socket.destroy(); resolveRefused(false); });
	});
}

async function echoBytes(port: number): Promise<void> {
	// One chunk beyond the protocol's 16-message, 16-KiB queue checks backpressure.
	const expected = Buffer.from(Uint8Array.from({ length: 16 * 1024 * (16 + 1) }, (_, index) => index % 256));
	await new Promise<void>((resolveEcho, reject) => {
		const socket = createConnection({ host: "127.0.0.1", port });
		let received = Buffer.alloc(0);
		socket.once("connect", () => socket.write(expected));
		socket.on("data", (bytes) => {
			received = Buffer.concat([received, bytes]);
			if (received.length >= expected.length) {
				if (received.equals(expected)) socket.end();
				else reject(new Error("Laptop tunnel changed echoed bytes."));
			}
		});
		socket.once("error", reject);
		socket.once("close", () => {
			if (received.equals(expected)) resolveEcho();
			else reject(new Error("Laptop tunnel closed before delivering all bytes."));
		});
	});
}

(serverBinary ? describe : describe.skip)("native desktop remote server", () => {
	let child: ChildProcess;
	let directory: string;
	let exited: Promise<void>;
	let origin: string;
	let token: string;
	let fixture: ChildProcess;
	let fixtureExited: Promise<void>;
	let fixtureConfig: string;
	let forward: PortForwardSessionSummary;
	before(async () => {
		directory = await mkdtemp(join(tmpdir(), "kubecove-remote-e2e-"));
		await chmod(directory, 0o700);
		fixture = spawn("bun", [join(process.cwd(), "e2e/harness/port-forward-fixture.ts")], { stdio: ["ignore", "pipe", "ignore"] });
		fixtureExited = new Promise((resolveExit) => fixture.once("exit", () => resolveExit()));
		const fixtureUrl = await new Promise<string>((resolveReady, reject) => {
			let output = "";
			fixture.once("error", reject);
			fixture.once("exit", () => reject(new Error("Port-forward fixture exited before readiness.")));
			fixture.stdout?.on("data", (chunk: Buffer) => {
				output += chunk.toString();
				if (output.includes("\n")) resolveReady(output.trim());
			});
		});
		fixtureConfig = join(directory, "fixture-kubeconfig");
		await writeFile(fixtureConfig, JSON.stringify({
			apiVersion: "v1", kind: "Config",
			clusters: [{ name: "fixture", cluster: { server: fixtureUrl } }],
			contexts: [{ name: "fixture", context: { cluster: "fixture", user: "fixture" } }],
			users: [{ name: "fixture", user: {} }], "current-context": "fixture",
		}));
		if (!serverBinary) throw new Error("Headless binary is required.");
		child = spawn(serverBinary, ["--data-dir", directory], {
			env: { ...process.env, KUBECONFIG: process.env.KUBECOVE_KUBECONFIG, KUBECOVE_REMOTE_FIXTURE_CONFIG: fixtureConfig },
			stdio: ["ignore", "pipe", "ignore"],
		});
		exited = new Promise((resolveExit) => child.once("exit", () => resolveExit()));
		await new Promise<void>((resolveReady, reject) => {
			let output = "";
			child.once("error", () => reject(new Error("Headless server could not launch.")));
			child.once("exit", () => reject(new Error("Headless server exited before readiness.")));
			child.stdout?.on("data", (chunk: Buffer) => {
				output += chunk.toString();
				const match = /KubeCove login: (http:\/\/[^\s]+)\/#token=([^\s]+)/.exec(output);
				if (!match) return;
				origin = match[1];
				token = match[2];
				resolveReady();
			});
		});
	});

	after(async () => {
		await browser.execute(() => window.__TAURI__.core.invoke("remote_disconnect")).catch(() => {});
		const back = await $('button=Back to app');
		if (await back.isDisplayed()) await back.click();
		child?.kill("SIGTERM");
		if (exited) await exited;
		fixture?.kill("SIGTERM");
		if (fixtureExited) await fixtureExited;
		if (directory) await rm(directory, { recursive: true, force: true });
	});

	it("connects through Settings and routes typed commands to the remote host", async () => {
		const settings = await $('button[aria-label="Open settings"]');
		if (await settings.isDisplayed()) await settings.click();
		await $('button=Remote server').click();
		const serverUrl = await $('#remote-server-url');
		await serverUrl.waitForDisplayed();
		await serverUrl.setValue(origin);
		await $('#remote-server-token').setValue("invalid");
		await $('button=Connect').click();
		await expect($('[role="alert"]')).toHaveText(expect.stringContaining("request Host or Origin is not allowed"));
		await $('#remote-server-token').setValue(token);
		await $('button=Connect').click();
		await browser.waitUntil(async () => {
			const status = await browser.execute(() => window.__TAURI__.core.invoke<RemoteConnectionSummary | null>("remote_connection_status"));
			return status?.connected === true;
		}, { timeoutMsg: "Remote Settings connection did not become active." });
		const contexts = await browser.execute(() => window.__TAURI__.core.invoke<ClusterContext[], { command: string; args: object }>("remote_invoke", { command: "list_kube_contexts", args: {} }));
		expect(contexts.length).toBe(2);
		const commandError = await browser.execute(async () => {
			try {
				await window.__TAURI__.core.invoke("remote_invoke", { command: "unknown_command", args: {} });
				return null;
			} catch (error) {
				return error;
			}
		});
		expect(commandError).toEqual({ message: "unknown command", kind: "unsupportedOperation" });
		await browser.saveScreenshot(join(process.env.KUBECOVE_E2E_ARTIFACTS ?? directory, "remote-connected.png"));
	});

	it("rejects client changes to operator-managed kubeconfig sources", async () => {
		const sourcesBefore = await browser.execute(() => window.__TAURI__.core.invoke<KubeconfigSourcesSummary, object>("remote_invoke", { command: "get_kubeconfig_sources", args: {} }));
		const settings = await $('button[aria-label="Open settings"]');
		if (await settings.isDisplayed()) await settings.click();
		await $('button=Kubeconfig').click();
		const serverPath = await $('input[aria-label="Kubeconfig path on server"]');
		await serverPath.waitForDisplayed();
		await expect($('button=Choose files')).not.toBeDisplayed();
		await serverPath.setValue(fixtureConfig);
		await $('button=Add server path').click();
		await expect($('body')).toHaveText(expect.stringContaining("kubeconfig sources are set by the server operator"));
		const sourcesAfter = await browser.execute(() => window.__TAURI__.core.invoke<KubeconfigSourcesSummary, object>("remote_invoke", { command: "get_kubeconfig_sources", args: {} }));
		expect(sourcesAfter).toEqual(sourcesBefore);
		await $('button=Remote server').click();
	});

	it("delivers remote stream messages through a native Tauri channel", async () => {
		await browser.execute(async () => {
			window.__KUBECOVE_REMOTE_STREAM__ = [];
			const channel = new window.__TAURI__.core.Channel<{ type: string }>();
			channel.onmessage = (message) => window.__KUBECOVE_REMOTE_STREAM__?.push(message);
			await window.__TAURI__.core.invoke<void, object>("remote_subscribe_channel", { channelId: 1, channel });
			await window.__TAURI__.core.invoke<unknown, object>("remote_invoke", {
				command: "start_pod_log_stream",
				args: { request: { clusterContext: "missing", namespace: "default", podName: "missing", kubeconfigEnvVar: "KUBECOVE_REMOTE_FIXTURE_CONFIG" }, channel: "__KUBECOVE_CHANNEL__:1" },
			});
		});
		await browser.waitUntil(async () => browser.execute(() => window.__KUBECOVE_REMOTE_STREAM__?.some((message) => message.type === "error") === true), { timeoutMsg: "Remote stream did not deliver its typed error through the native channel." });
		await browser.execute(() => window.__TAURI__.core.invoke<boolean, object>("remote_unsubscribe_channel", { channelId: 1 }));
	});

	it("binds a laptop loopback listener and preserves tunneled bytes", async () => {
		forward = await browser.execute(() => window.__TAURI__.core.invoke<PortForwardSessionSummary, object>("remote_invoke", {
			command: "start_pod_port_forward",
			args: { request: { clusterContext: "fixture", namespace: "default", podName: "forward-target", remotePort: 8080, kubeconfigEnvVar: "KUBECOVE_REMOTE_FIXTURE_CONFIG" } },
		}));
		expect(forward.localAddress).toBe("127.0.0.1");
		await echoBytes(forward.localPort);
	});

	it("keeps the desktop session and tunnel working when another session opens and closes", async () => {
		const response = await fetch(`${origin}/api/v1/session`, {
			method: "POST", headers: { Origin: origin, "Content-Type": "application/json" }, body: JSON.stringify({ token }),
		});
		expect(response.status).toBe(200);
		const session: { resumeToken: string } = await response.json();
		const headers = { Origin: origin, Authorization: `Bearer ${token}`, "Content-Type": "application/json", "X-Kubecove-Session": session.resumeToken };
		try {
			const forwardsResponse = await fetch(`${origin}/api/v1/invoke/list_port_forwards`, { method: "POST", headers, body: "{}" });
			expect(forwardsResponse.status).toBe(200);
			const otherForwards: PortForwardSessionSummary[] = await forwardsResponse.json();
			expect(otherForwards).toEqual([]);
			const status = await browser.execute(() => window.__TAURI__.core.invoke<RemoteConnectionSummary | null>("remote_connection_status"));
			expect(status?.connected).toBe(true);
			await echoBytes(forward.localPort);
		} finally {
			const closeResponse = await fetch(`${origin}/api/v1/session/close`, { method: "POST", headers, body: "{}" });
			expect(closeResponse.status).toBe(200);
		}
		const connectedStatus = await browser.execute(() => window.__TAURI__.core.invoke<RemoteConnectionSummary | null>("remote_connection_status"));
		expect(connectedStatus?.connected).toBe(true);
		await echoBytes(forward.localPort);
		await $('button=Disconnect').click();
		await browser.waitUntil(() => portRefusesConnection(forward.localPort), { timeoutMsg: "Disconnect left the laptop forward listener open." });
		const status = await browser.execute(() => window.__TAURI__.core.invoke<RemoteConnectionSummary | null>("remote_connection_status"));
		expect(status).toBeNull();
	});

	it("closes a laptop forward on explicit disconnect", async () => {
		await $('#remote-server-token').setValue(token);
		await $('button=Connect').click();
		await browser.waitUntil(async () => browser.execute(async () => (await window.__TAURI__.core.invoke<RemoteConnectionSummary | null>("remote_connection_status"))?.connected === true), { timeoutMsg: "Desktop could not reconnect for disconnect cleanup." });
		const activeForward = await browser.execute(() => window.__TAURI__.core.invoke<PortForwardSessionSummary, object>("remote_invoke", {
			command: "start_pod_port_forward",
			args: { request: { clusterContext: "fixture", namespace: "default", targetKind: "Service", targetName: "forward-service", remotePort: 80, kubeconfigEnvVar: "KUBECOVE_REMOTE_FIXTURE_CONFIG" } },
		}));
		expect(activeForward.resolvedPodName).toBe("forward-target");
		expect(activeForward.resolvedPodPort).toBe(8080);
		await echoBytes(activeForward.localPort);
		await $('button=Disconnect').click();
		await browser.waitUntil(() => portRefusesConnection(activeForward.localPort), { timeoutMsg: "Disconnect left the laptop listener open." });
	});
});
