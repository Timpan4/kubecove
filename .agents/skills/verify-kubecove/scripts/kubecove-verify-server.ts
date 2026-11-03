#!/usr/bin/env bun
// One-shot check of headless server mode (ADR 0020) and its read-only MCP endpoint (ADR 0028).
// The server gets a private HOME, data directory, and fixture kubeconfig, so it never reads ~/.kube/config.
import { existsSync } from "node:fs";
import { chmod, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { chromium } from "playwright-core";
import { gitHead, root, startChrome, startIdentity, stopGroup, waitFor } from "./processes";

const args = Bun.argv.slice(2);
const flag = (name: string) => {
	const index = args.indexOf(`--${name}`);
	return index >= 0 ? args[index + 1] : undefined;
};
const runId = flag("run") ?? `${new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14)}-${process.pid}`;
if (!/^[a-zA-Z0-9-]{1,48}$/.test(runId)) throw new Error("run id must be 1-48 letters, digits, or hyphens");
const binary = flag("binary") ?? join(root, "src-tauri/server/target/debug", process.platform === "win32" ? "kubecove-server.exe" : "kubecove-server");
const scratch = join(root, ".e2e", "verify", `server-${runId}`);
const evidence = join(root, "e2e", "artifacts", `verify-server-${runId}`);
const context = "verify-fixture";
if (!existsSync(binary)) throw new Error(`missing ${binary}; build it first (see the skill's server section)`);
if (existsSync(scratch) || existsSync(evidence)) throw new Error(`run ${runId} already exists; choose a new --run id`);
await mkdir(join(scratch, "data"), { recursive: true });
await mkdir(join(scratch, "home"));
await mkdir(evidence, { recursive: true });
if (process.platform !== "win32") for (const dir of [scratch, join(scratch, "data")]) await chmod(dir, 0o700);

// The fixture points at a closed loopback port: contexts parse, cluster reads fail without network access.
const fixture = join(scratch, "fixture-kubeconfig");
// One key per line so the pre-commit credential scan's exception for this file stays narrow.
await writeFile(
	fixture,
	JSON.stringify({
		apiVersion: "v1",
		kind: "Config",
		clusters: [{ name: context, cluster: { server: "https://127.0.0.1:9" } }],
		contexts: [{ name: context, context: { cluster: context, user: context } }],
		users: [{ name: context, user: {} }],
		"current-context": context,
	}),
);
// The operator selects sources; ADR 0026 lets requests name only this variable (same pattern as e2e/headless-smoke.ts).
await writeFile(join(scratch, "data", "kubeconfig-sources.json"), JSON.stringify({ kubeconfigEnvVar: "KUBECOVE_VERIFY_CONFIG", paths: [], showSourceLabels: true }));

const results: Record<string, unknown> = { runId, head: gitHead(), binary };
const secrets: string[] = [];
const redact = (text: string) => secrets.reduce((value, secret) => value.replaceAll(secret, "[redacted]"), text);
const server = Bun.spawn([binary, "--data-dir", join(scratch, "data"), "--listen", "127.0.0.1:0", "--mcp"], {
	cwd: root,
	detached: true,
	stdin: "ignore",
	stdout: "pipe",
	stderr: "pipe",
	env: { ...process.env, HOME: join(scratch, "home"), XDG_CONFIG_HOME: join(scratch, "home"), XDG_DATA_HOME: join(scratch, "home"), KUBECONFIG: join(scratch, "missing-kubeconfig"), KUBECOVE_VERIFY_CONFIG: fixture, DBUS_SESSION_BUS_ADDRESS: undefined },
});
const serverStartedAt = startIdentity(server.pid);
let output = "";
const decoder = new TextDecoder();
const drain = async (stream: ReadableStream<Uint8Array>) => { for await (const chunk of stream) output += decoder.decode(chunk, { stream: true }); };
void drain(server.stdout);
void drain(server.stderr);
let chrome: ReturnType<typeof startChrome> | undefined;
let passed = false;

// Runs once, from the normal path or from an interrupt, and exits. Scratch holds the tokens, so it is removed only
// after both process groups stopped; otherwise it stays for a retry and the run fails.
let finishing: Promise<never> | undefined;
const finish = (interrupted?: string) =>
	(finishing ??= (async () => {
		// Its own field: stopping the processes makes the main path fail too, and that error must not hide the cause.
		if (interrupted) results.interrupted = interrupted;
		results.stoppedChrome = chrome ? await stopGroup(chrome.pid, chrome.startedAt) : true;
		results.stoppedServer = await stopGroup(server.pid, serverStartedAt);
		const stopped = results.stoppedChrome === true && results.stoppedServer === true;
		const ok = passed && stopped && !interrupted;
		results.passed = ok;
		if (!stopped) results.scratchKept = { scratch, serverPid: server.pid, chromePid: chrome?.pid };
		await writeFile(join(evidence, "server.log"), redact(output));
		await writeFile(join(evidence, "result.json"), `${redact(JSON.stringify(results, null, 2))}\n`);
		if (stopped) await rm(scratch, { recursive: true, force: true });
		console.log(JSON.stringify({ passed: ok, evidence, stopped, scratchRemoved: !existsSync(scratch), evidenceKept: existsSync(join(evidence, "result.json")) }, null, 2));
		process.exit(ok ? 0 : 1);
	})());
// The server and Chrome are detached, so an interrupt must stop them here or they outlive this check.
for (const signal of ["SIGINT", "SIGTERM"] as const) process.on(signal, () => void finish(signal));

try {
	let origin = "";
	await waitFor("the server login line", async () => {
		const match = /KubeCove login: (http:\/\/[^\s/]+)\/#token=(\S+)/.exec(output);
		if (match) { origin = match[1]; secrets.push(match[2]); }
		return Boolean(match) && output.includes("KubeCove MCP:");
	});
	const token = (/#token=(\S+)/.exec(output) ?? [])[1] ?? "";
	results.origin = origin;
	results.indexServed = (await (await fetch(`${origin}/`)).text()).includes("<title>KubeCove</title>");

	const mcpToken = (await readFile(join(scratch, "data", "mcp-token"), "utf8")).trim();
	secrets.push(mcpToken);
	const mcp = (body: unknown, bearer?: string) => fetch(`${origin}/mcp`, { method: "POST", headers: { Accept: "application/json, text/event-stream", "Content-Type": "application/json", ...(bearer ? { Authorization: `Bearer ${bearer}` } : {}) }, body: JSON.stringify(body) });
	const listTools = { jsonrpc: "2.0", id: 1, method: "tools/list", params: {} };
	results.mcpWithoutToken = (await mcp(listTools)).status;
	results.mcpWithServerToken = (await mcp(listTools, token)).status;
	const tools = (await (await mcp(listTools, mcpToken)).json()) as { result: { tools: Array<{ name: string; annotations?: { readOnlyHint?: boolean } }> } };
	results.tools = tools.result.tools.map(({ name, annotations }) => ({ name, readOnlyHint: annotations?.readOnlyHint === true }));
	const contexts = (await (await mcp({ jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "list_contexts", arguments: {} } }, mcpToken)).json()) as { result: { content: Array<{ text: string }> } };
	results.listContexts = JSON.parse(contexts.result.content[0].text);

	// Browser server mode: sign in with the startup token in the URL fragment and reach the launcher.
	chrome = startChrome(join(scratch, "chrome-profile"));
	const browser = await chromium.connectOverCDP(await chrome.cdpUrl);
	const page = browser.contexts()[0].pages()[0];
	await page.goto(`${origin}/#token=${token}`);
	await page.getByText(context, { exact: true }).first().waitFor();
	await writeFile(join(evidence, "browser-launcher.aria.txt"), redact(await page.locator("body").ariaSnapshot()));
	await page.screenshot({ path: join(evidence, "browser-launcher.png") });
	results.browserUrl = redact(page.url());
	await browser.close();

	const toolNames = (results.tools as Array<{ name: string; readOnlyHint: boolean }>);
	passed = results.indexServed === true && results.mcpWithoutToken === 401 && results.mcpWithServerToken === 401 && toolNames.length > 0 && toolNames.every((tool) => tool.readOnlyHint) && JSON.stringify(results.listContexts).includes(context);
} catch (error) {
	results.error = redact(error instanceof Error ? error.message : String(error));
}
await finish();
