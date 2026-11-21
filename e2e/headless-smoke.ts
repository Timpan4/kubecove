import assert from "node:assert/strict";
import { chmod, mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import type { JsonObject } from "../src/lib/types";
import { safeDiagnosticText } from "./harness/diagnostics";
import { serverBinary, startServer } from "./harness/server-process";
import { channelMarker, serverClient } from "./harness/server-session";
import { startPortForwardFixture } from "./harness/port-forward-fixture";

// This check exercises the packaged HTTP/WS boundary without a Kubernetes cluster.
// Live Kubernetes sessions remain covered by the isolated cluster E2E workflow.
const root = resolve(import.meta.dir, "..");
const artifactDir = join(root, "e2e/artifacts/headless");
const temporary = await mkdtemp(join(tmpdir(), "kubecove-headless-"));
const dataDir = join(temporary, "data");
const checks: string[] = [];
const fixtureConfig = join(temporary, "fixture-kubeconfig");
const interrupt = new AbortController();
const stop = () => interrupt.abort();
process.once("SIGTERM", stop);
process.once("SIGINT", stop);
let server: ReturnType<typeof startServer> | undefined;
let kubeFixture: ReturnType<typeof startPortForwardFixture> | undefined;
let client: ReturnType<typeof serverClient> | undefined;
let failure: string | null = null;
try {
	await mkdir(dataDir, { mode: 0o700 });
	await chmod(temporary, 0o700);
	kubeFixture = startPortForwardFixture();
	await writeFile(fixtureConfig, JSON.stringify({
		apiVersion: "v1", kind: "Config",
		clusters: [{ name: "fixture", cluster: { server: kubeFixture.url.toString() } }],
		contexts: [{ name: "fixture", context: { cluster: "fixture", user: "fixture" } }],
		users: [{ name: "fixture", user: {} }], "current-context": "fixture",
	}));
	// The operator selects the fixture through a configured variable; requests may name only it (ADR 0026).
	await writeFile(join(dataDir, "kubeconfig-sources.json"), JSON.stringify({ kubeconfigEnvVar: "KUBECOVE_SMOKE_CONFIG", paths: [], showSourceLabels: true }));
	server = startServer(process.env.KUBECOVE_SERVER_BINARY ?? serverBinary(), dataDir, {
		HOME: temporary, XDG_CONFIG_HOME: temporary, KUBECONFIG: join(temporary, "missing-kubeconfig"),
		KUBECOVE_SMOKE_CONFIG: fixtureConfig, KUBECOVE_SMOKE_MISSING_CONFIG: join(temporary, "missing-kubeconfig"), DBUS_SESSION_BUS_ADDRESS: undefined,
	}, interrupt.signal);
	const { origin, token } = await server.ready;
	client = serverClient(origin, token);
	const { login, post, invoke, connect } = client;
	assert.equal((await stat(dataDir)).mode & 0o777, 0o700);
	assert.equal((await stat(join(dataDir, "server-token"))).mode & 0o777, 0o600);
	assert.ok((await readFile(join(dataDir, "server-token"), "utf8")).trim() === token, "Private token file must match the startup credential.");
	checks.push("Private startup credential and data directory");
	for (const body of [{}, { token: "incorrect" }]) {
		assert.equal((await post("/api/v1/session", body)).status, 401);
	}
	assert.equal((await post("/api/v1/session", { token }, { Origin: "http://attacker.invalid" })).status, 403);
	assert.equal((await post("/api/v1/session", { token }, { Host: "attacker.invalid" })).status, 403);
	const unsigned = await post("/api/v1/invoke/get_backend_diagnostics", {});
	assert.equal(unsigned.status, 401);
	checks.push("Wrong token, Host, Origin and unsigned commands rejected");
	const page = await fetch(origin);
	const html = await page.text();
	const policy = page.headers.get("content-security-policy") ?? "";
	assert.ok(policy.includes(`connect-src 'self' ws://${new URL(origin).host};`));
	assert.ok(html.includes(`content="${policy}"`));
	assert.ok(!policy.includes("ws: wss:"));
	checks.push("Embedded web assets and exact-origin header/meta CSP");
	const first = await login();
	let channel = await connect(first.session);
	const diagnostics = await invoke(first.session, "get_backend_diagnostics", {}, first.cookie);
	assert.equal(diagnostics.status, 200);
	assert.ok(Array.isArray(await diagnostics.json()));
	for (const [command, args, status, kind] of [
		["pick_kubeconfig_paths", {}, 404, "unsupportedOperation"],
		["stop_port_forward", { sessionId: 42 }, 400, "validation"],
	] as const) {
		const response = await invoke(first.session, command, args);
		assert.equal(response.status, status);
		const error = await response.json();
		assert.equal(error.kind, kind);
		assert.ok(String(error.message) === error.message);
	}
	checks.push("Cookie/native commands preserve typed results and errors");
	const sourcesBefore = await (await invoke(first.session, "get_kubeconfig_sources", {})).json();
	const sourceMutations: Array<[string, JsonObject]> = [
		["set_kubeconfig_env_var", { envVar: "KUBECOVE_SMOKE_CONFIG" }],
		["add_kubeconfig_paths", { paths: [fixtureConfig] }],
		["remove_kubeconfig_path", { path: fixtureConfig }],
		["reorder_kubeconfig_paths", { paths: [] }],
		["set_show_kubeconfig_source_labels", { show: false }],
	];
	for (const [command, args] of sourceMutations) {
		const response = await invoke(first.session, command, args);
		assert.equal((await response.json()).kind, "unsupportedOperation", command);
	}
	assert.deepEqual(await (await invoke(first.session, "get_kubeconfig_sources", {})).json(), sourcesBefore);
	checks.push("Server users cannot change operator kubeconfig sources or labels");
	const outsideSources = await invoke(first.session, "list_kube_contexts", { kubeconfigEnvVar: "KUBECOVE_SMOKE_MISSING_CONFIG" });
	assert.equal(outsideSources.status, 422);
	assert.equal((await outsideSources.json()).kind, "kubeconfig");
	checks.push("Requests cannot select a kubeconfig variable outside the operator's sources");
	// Failure contract: a denied type or namespace cannot discard readable rows,
	// claim reconciliation coverage, or turn authentication failures into empty success.
	const unauthorized = await invoke(first.session, "list_resource_scope", { clusterContext: "fixture", requests: [{ kind: "Pod", namespace: "unauthorized" }], kubeconfigEnvVar: "KUBECOVE_SMOKE_CONFIG" });
	assert.equal(unauthorized.status, 422);
	assert.notEqual((await unauthorized.json()).kind, "forbidden");
	for (const requests of [
		[{ kind: "Pod", namespace: "readable" }, { kind: "Pod", namespace: "denied" }],
		[{ kind: "Pod" }, { kind: "Secret" }],
		[{ kind: "Secret" }],
	]) {
		const batch = await invoke(first.session, "list_resource_scope", { clusterContext: "fixture", requests, kubeconfigEnvVar: "KUBECOVE_SMOKE_CONFIG" });
		assert.equal(batch.status, 200, "Forbidden resource lists must preserve readable results.");
		const result = await batch.json();
		assert.equal(result.failures.length, 1, `Expected one denial for ${JSON.stringify(requests)}`);
		assert.equal(result.failures[0].error.kind, "forbidden");
		assert.deepEqual(result.failures[0].request, requests.at(-1));
		assert.deepEqual(result.completedRequests, requests.slice(0, -1));
		assert.deepEqual(result.resources.map((row: { name: string }) => row.name), requests.length === 1 ? [] : ["readable-pod"]);
	}
	checks.push("Denied resource types and namespaces retain readable rows without false coverage; authentication still fails");
	const response = await invoke(first.session, "start_pod_log_stream", {
		request: { clusterContext: "missing", namespace: "default", podName: "missing", kubeconfigEnvVar: "KUBECOVE_SMOKE_CONFIG" },
		channel: channelMarker(1),
	});
	assert.equal(response.status, 200);
	let streamedError = false;
	while (!streamedError) {
		const frame = await channel.next();
		assert.equal(frame.type, "channel");
		assert.equal(frame.channelId, 1);
		streamedError = frame.message?.type === "error";
	}
	checks.push("Typed channel delivery for a context missing from the operator's kubeconfig");
	await channel.close();
	const resumed = await login(first.session.resumeToken);
	assert.ok(resumed.session.resumeToken === first.session.resumeToken, "Resume must retain the existing owner.");
	channel = await connect(resumed.session);
	assert.equal((await invoke(resumed.session, "get_backend_diagnostics", {})).status, 200);
	checks.push("WebSocket reconnect resumes the same owner within grace");
	const forwardResponse = await invoke(resumed.session, "start_pod_port_forward", {
		request: { clusterContext: "fixture", namespace: "default", podName: "forward-target", remotePort: 8080, kubeconfigEnvVar: "KUBECOVE_SMOKE_CONFIG" },
	});
	assert.equal(forwardResponse.status, 200, "Mock Kubernetes port-forward must open.");
	const forward: { id: string; localPort: number; localAddress: string } = await forwardResponse.json();
	assert.equal(forward.localAddress, "127.0.0.1");
	const forwardId = crypto.randomUUID();
	const connectionId = crypto.randomUUID();
	channel.socket.send(JSON.stringify({ type: "tunnel_register", forwardId: crypto.randomUUID(), request: {
		clusterContext: "missing", namespace: "default", podName: "forward-target", remotePort: 8080, kubeconfigEnvVar: "KUBECOVE_SMOKE_CONFIG",
	} }));
	assert.equal((await channel.next()).type, "tunnel_error", "Registration must resolve the target before reporting a listening forward.");
	channel.socket.send(JSON.stringify({ type: "tunnel_register", forwardId, request: {
		clusterContext: "fixture", namespace: "default", targetKind: "Service", targetName: "forward-service", remotePort: 80, kubeconfigEnvVar: "KUBECOVE_SMOKE_CONFIG",
	} }));
	const registration = await channel.next();
	assert.equal(registration.type, "tunnel_registered");
	assert.equal(registration.resolvedPodName, "forward-target");
	assert.equal(registration.resolvedPodPort, 8080);
	channel.socket.send(JSON.stringify({ type: "tunnel_open", forwardId, connectionId }));
	assert.equal((await channel.next()).type, "tunnel_opened");
	const id = new TextEncoder().encode(connectionId);
	// Send one chunk beyond the relay's 16-message budget while reading echoed data.
	const expectedBytes = Uint8Array.from({ length: 16 * 1024 * (16 + 1) }, (_, index) => index % 256);
	for (let offset = 0; offset < expectedBytes.length; offset += 16 * 1024) {
		const frame = new Uint8Array(2 + id.length + 16 * 1024);
		new DataView(frame.buffer).setUint16(0, id.length);
		frame.set(id, 2);
		frame.set(expectedBytes.subarray(offset, offset + 16 * 1024), 2 + id.length);
		channel.socket.send(frame);
	}
	const received = new Uint8Array(expectedBytes.length);
	let receivedBytes = 0;
	while (receivedBytes < expectedBytes.length) {
		const frame = await channel.nextBinary();
		assert.equal(new DataView(frame.buffer, frame.byteOffset).getUint16(0), id.length);
		assert.equal(new TextDecoder().decode(frame.subarray(2, 2 + id.length)), connectionId);
		const bytes = frame.subarray(2 + id.length);
		received.set(bytes, receivedBytes);
		receivedBytes += bytes.length;
	}
	assert.deepEqual(received, expectedBytes);
	// EOF must end an idle Kubernetes stream without waiting for the peer to close.
	channel.socket.send(JSON.stringify({ type: "tunnel_eof", connectionId }));
	const eofFrames = new Set<string>();
	for (let index = 0; index < 2; index++) {
		const frame = await channel.next();
		assert.ok(["tunnel_eof_ack", "tunnel_closed"].includes(frame.type), "Upload EOF must be accepted and close the tunnel.");
		eofFrames.add(frame.type);
	}
	assert.deepEqual(eofFrames, new Set(["tunnel_eof_ack", "tunnel_closed"]));
	channel.socket.send(JSON.stringify({ type: "tunnel_eof", connectionId }));
	assert.equal((await channel.next()).type, "tunnel_closed");
	channel.socket.send(JSON.stringify({ type: "tunnel_unregister", forwardId }));
	assert.equal((await channel.next()).type, "tunnel_unregistered");
	checks.push("Authenticated Service tunnel resolves its Pod and named port, preserving duplex bytes beyond its bounded queue");
	checks.push("Tunnel upload EOF closes an idle Kubernetes stream; stale EOF cannot reopen it");
	for (const [podName, terminalType] of [["forward-close", "tunnel_closed"], ["forward-error", "tunnel_error"]]) {
		const forwardId = crypto.randomUUID();
		const connectionId = crypto.randomUUID();
		channel.socket.send(JSON.stringify({ type: "tunnel_register", forwardId, request: {
			clusterContext: "fixture", namespace: "default", podName, remotePort: 8080, kubeconfigEnvVar: "KUBECOVE_SMOKE_CONFIG",
		} }));
		assert.equal((await channel.next()).type, "tunnel_registered");
		channel.socket.send(JSON.stringify({ type: "tunnel_open", forwardId, connectionId }));
		assert.equal((await channel.next()).type, "tunnel_opened");
		const payload = new TextEncoder().encode("peer closes first");
		const id = new TextEncoder().encode(connectionId);
		const frame = new Uint8Array(2 + id.length + payload.length);
		new DataView(frame.buffer).setUint16(0, id.length);
		frame.set(id, 2);
		frame.set(payload, 2 + id.length);
		channel.socket.send(frame);
		if (podName === "forward-close") {
			assert.deepEqual((await channel.nextBinary()).subarray(2 + id.length), payload);
		}
		assert.equal((await channel.next()).type, terminalType, "Pod termination must reach the client without upload EOF.");
		channel.socket.send(JSON.stringify({ type: "tunnel_unregister", forwardId }));
		assert.equal((await channel.next()).type, "tunnel_unregistered");
	}
	checks.push("Pod-side EOF and errors terminate tunnels without waiting for client upload EOF");
	const second = await login();
	assert.ok(second.session.resumeToken !== first.session.resumeToken, "New login must create a distinct session");
	assert.equal((await invoke(first.session, "get_backend_diagnostics", {})).status, 200);
	assert.equal((await invoke(second.session, "get_backend_diagnostics", {})).status, 200);
	checks.push("Another login opens a second session and leaves the first one working");
	const secondForwards = await (await invoke(second.session, "list_port_forwards", {})).json();
	assert.ok(!secondForwards.some((item: { id: string }) => item.id === forward.id), "Port-forwards belong to their session.");
	await new Promise<void>((resolveEcho, reject) => {
		let echo = "";
		Bun.connect({ hostname: "127.0.0.1", port: forward.localPort, socket: {
			open(socket) { socket.write("forward survives another login"); },
			data(socket, bytes) {
				echo += bytes.toString();
				if (echo === "forward survives another login") { socket.end(); resolveEcho(); }
			},
			error() { reject(new Error("Server forward failed.")); },
			close() { if (echo !== "forward survives another login") reject(new Error("Server forward closed before echo.")); },
		} }).catch(() => reject(new Error("Server listener could not connect.")));
	});
	checks.push("Server-loopback forward stays with its session and preserves every echoed byte");
	assert.equal((await post("/api/v1/session/close", {}, { Authorization: `Bearer ${token}`, "X-Kubecove-Session": first.session.resumeToken })).status, 200);
	await channel.close();
	assert.equal((await invoke(first.session, "get_backend_diagnostics", {})).status, 409);
	assert.equal((await invoke(second.session, "get_backend_diagnostics", {})).status, 200);
	let listenerClosed = false;
	try {
		const unexpected = await Bun.connect({ hostname: "127.0.0.1", port: forward.localPort, socket: { data() {} } });
		unexpected.end();
	} catch { listenerClosed = true; }
	assert.ok(listenerClosed, "Explicit disconnect must close server forward listeners.");
	assert.equal((await post("/api/v1/session/close", {}, { Authorization: `Bearer ${token}`, "X-Kubecove-Session": second.session.resumeToken })).status, 200);
	checks.push("Explicit disconnect revokes only that session");
	if (Bun.argv.includes("--browser")) {
		const { verifyServerBrowser } = await import("./harness/server-browser");
		await verifyServerBrowser(origin, token);
		checks.push("Embedded Svelte sign-in, context invoke, resource watch delivery, server settings and WebSocket session revocation in Chromium");
	}
	if (process.env.KUBECOVE_NATIVE_REMOTE_PROBE) {
		const probe = Bun.spawn([
			process.env.CARGO ?? "cargo", "test", "--locked", "--manifest-path", "src-tauri/Cargo.toml",
			"-p", "kubecove-host", "--features", "remote-client", "native_tunnel_delivers_bytes_and_closes_after_upload_eof", "--", "--ignored",
		], {
			cwd: root,
			env: { ...process.env, KUBECOVE_REMOTE_TEST_URL: origin, KUBECOVE_REMOTE_TEST_TOKEN: token },
			stdout: "pipe", stderr: "inherit",
		});
		const [status, output] = await Promise.all([probe.exited, new Response(probe.stdout).text()]);
		const diagnostic = safeDiagnosticText(output).replaceAll(token, "[redacted]").replace(/([?#&](?:resumeToken|token)=)[^\s&"']+/g, "$1[redacted]");
		assert.equal(status, 0, `Production native Rust tunnel probe must pass.\n${diagnostic}`);
		assert.ok(output.includes("native_tunnel_delivers_bytes_and_closes_after_upload_eof ... ok"), "The tunnel probe must actually run.");
		checks.push("Native Rust desktop client preserves bytes and closes on upload EOF, pod EOF and pod errors without a GUI");
	}
} catch (error) {
	failure = error instanceof Error ? error.message : "Headless smoke failed.";
} finally {
	try {
		for (const cleanup of [
			() => client?.closeSockets(),
			() => server?.stop(),
			() => kubeFixture?.stop(true),
			() => rm(temporary, { recursive: true, force: true }),
		]) {
			try { await cleanup(); } catch (error) {
				failure = [failure, `Headless cleanup failed: ${String(error)}`].filter(Boolean).join("\n");
			}
		}
	} finally {
		process.off("SIGTERM", stop);
		process.off("SIGINT", stop);
		await mkdir(artifactDir, { recursive: true });
		await writeFile(join(artifactDir, "report.json"), `${JSON.stringify({ passed: failure === null, checks, failure }, null, 2)}\n`);
	}
}
if (failure) throw new Error(failure);
console.log(`Headless smoke passed. Report: ${join(artifactDir, "report.json")}`);
