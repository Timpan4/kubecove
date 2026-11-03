#!/usr/bin/env bun
// One-shot check of headless server mode (ADR 0020) and its read-only MCP endpoint (ADR 0028).
// The server gets a private HOME, data directory, and fixture kubeconfig, so it never reads ~/.kube/config.
import { existsSync } from "node:fs";
import { chmod, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { chromium } from "playwright-core";
import { gitHead, readinessTimeoutMs, root, startChrome, startIdentity, stopGroup, waitFor } from "./processes";

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
// A non-recursive mkdir claims the run. A second check with the same id fails here, before teardown could remove
// this run's scratch directory.
await mkdir(dirname(scratch), { recursive: true });
await mkdir(scratch);
const results: Record<string, unknown> = { runId, head: gitHead(), binary };
const secrets: string[] = [];
// Longest first, so a secret that contains another one is replaced whole.
const redact = (text: string) =>
	secrets
		.filter(Boolean)
		.sort((a, b) => b.length - a.length)
		.reduce((value, secret) => value.replaceAll(secret, "[redacted]"), text);
// The fixture points at a closed loopback port: contexts parse, cluster reads fail without network access.
const fixture = join(scratch, "fixture-kubeconfig");
const spawnServer = () =>
	Bun.spawn([binary, "--data-dir", join(scratch, "data"), "--listen", "127.0.0.1:0", "--mcp"], {
		cwd: root,
		detached: true,
		stdin: "ignore",
		stdout: "pipe",
		stderr: "pipe",
		env: { ...process.env, HOME: join(scratch, "home"), XDG_CONFIG_HOME: join(scratch, "home"), XDG_DATA_HOME: join(scratch, "home"), KUBECONFIG: join(scratch, "missing-kubeconfig"), KUBECOVE_VERIFY_CONFIG: fixture, DBUS_SESSION_BUS_ADDRESS: undefined },
	});
let server: ReturnType<typeof spawnServer> | undefined;
let serverStartedAt: string | undefined;
let output = "";
const outputErrors: string[] = [];
// Each pipe gets its own decoder, so a character split across chunks of one pipe is not mixed with the other's bytes.
// A failed read is recorded and fails the check.
const drain = async (stream: ReadableStream<Uint8Array>) => {
	const decoder = new TextDecoder();
	try {
		for await (const chunk of stream) output += decoder.decode(chunk, { stream: true });
		output += decoder.decode();
	} catch (error) {
		outputErrors.push(error instanceof Error ? error.message : String(error));
	}
};
let drained: Promise<unknown> = Promise.resolve();
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
		results.stoppedServer = server ? await stopGroup(server.pid, serverStartedAt) : true;
		const stopped = results.stoppedChrome === true && results.stoppedServer === true;
		// The pipes reach end of file once every process holding them has exited. A server that did not stop keeps them
		// open, so the wait is skipped then, and otherwise bounded by the readiness timeout in case a process outside
		// the group still holds a pipe.
		results.outputComplete = stopped && (await Promise.race([drained.then(() => true), Bun.sleep(readinessTimeoutMs).then(() => false)]));
		if (outputErrors.length) results.outputErrors = outputErrors;
		// Collect the tokens again from the final output and the token file: an interrupt or a startup timeout can
		// come before the readiness poll saw the login line, and the server can print it while shutting down.
		for (const match of output.matchAll(/#token=(\S+)/g)) secrets.push(match[1]);
		const errorText = (error: unknown) => redact(error instanceof Error ? error.message : String(error));
		// A failed read, removal, or write is reported and fails the check, but never skips the steps after it.
		let evidenceError: string | undefined;
		try {
			const mcpTokenFile = join(scratch, "data", "mcp-token");
			if (existsSync(mcpTokenFile)) secrets.push((await readFile(mcpTokenFile, "utf8")).trim());
		} catch (error) {
			evidenceError = errorText(error);
		}
		// Scratch is removed before the evidence is written, so result.json records whether its tokens and Chrome
		// profile are gone. A kept scratch directory fails the check.
		if (stopped) await rm(scratch, { recursive: true, force: true }).catch((error) => (results.scratchError = errorText(error)));
		const scratchRemoved = !existsSync(scratch);
		if (!scratchRemoved) results.scratchKept = { scratch, serverPid: server?.pid, chromePid: chrome?.pid };
		if (evidenceError !== undefined) results.evidenceError = evidenceError;
		const ok = passed && stopped && scratchRemoved && evidenceError === undefined && !interrupted && results.outputComplete === true && outputErrors.length === 0;
		results.passed = ok;
		// Each artifact is written on its own, so a failed server.log still leaves result.json.
		// An interrupt can arrive before setup created the evidence directory.
		await mkdir(evidence, { recursive: true }).catch((error) => (evidenceError ??= errorText(error)));
		await writeFile(join(evidence, "server.log"), redact(output)).catch((error) => (evidenceError ??= errorText(error)));
		await writeFile(join(evidence, "result.json"), `${redact(JSON.stringify(results, null, 2))}\n`).catch((error) => (evidenceError ??= errorText(error)));
		const exitOk = ok && evidenceError === undefined;
		console.log(JSON.stringify({ passed: exitOk, evidence, stopped, scratchRemoved, evidenceKept: existsSync(join(evidence, "result.json")), evidenceError, scratchError: results.scratchError }, null, 2));
		process.exit(exitOk ? 0 : 1);
	})());
// The server and Chrome are detached, so an interrupt must stop them here or they outlive this check. The handlers
// exist before anything is created or started; a signal without a handler would end this script and leave them running.
for (const signal of ["SIGINT", "SIGTERM"] as const) process.on(signal, () => void finish(signal));

try {
	await mkdir(join(scratch, "data"));
	await mkdir(join(scratch, "home"));
	await mkdir(evidence, { recursive: true });
	if (process.platform !== "win32") for (const dir of [scratch, join(scratch, "data")]) await chmod(dir, 0o700);
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

	// Once teardown has started, start nothing it would miss. The check and the spawn run in one tick.
	if (finishing) throw new Error("interrupted before the server started");
	server = spawnServer();
	serverStartedAt = startIdentity(server.pid);
	drained = Promise.all([drain(server.stdout), drain(server.stderr)]);

	let origin = "";
	let token = "";
	// The login line counts only once its newline arrived: a pipe chunk can end inside the token.
	await waitFor("the server login line", async () => {
		const match = /KubeCove login: (http:\/\/[^\s/]+)\/#token=(\S+)\r?\n/.exec(output);
		if (match) [, origin, token] = match;
		return Boolean(match) && output.includes("KubeCove MCP:");
	});
	secrets.push(token);
	results.origin = origin;
	// Each request and its body read share one deadline, so a server that stalls fails the check instead of hanging it.
	const deadline = () => AbortSignal.timeout(readinessTimeoutMs);
	results.indexServed = (await (await fetch(`${origin}/`, { signal: deadline() })).text()).includes("<title>KubeCove</title>");

	const mcpToken = (await readFile(join(scratch, "data", "mcp-token"), "utf8")).trim();
	secrets.push(mcpToken);
	const mcp = (body: unknown, bearer?: string) => fetch(`${origin}/mcp`, { method: "POST", headers: { Accept: "application/json, text/event-stream", "Content-Type": "application/json", ...(bearer ? { Authorization: `Bearer ${bearer}` } : {}) }, body: JSON.stringify(body), signal: deadline() });
	const listTools = { jsonrpc: "2.0", id: 1, method: "tools/list", params: {} };
	results.mcpWithoutToken = (await mcp(listTools)).status;
	results.mcpWithServerToken = (await mcp(listTools, token)).status;
	const tools = (await (await mcp(listTools, mcpToken)).json()) as { result: { tools: Array<{ name: string; annotations?: { readOnlyHint?: boolean } }> } };
	results.tools = tools.result.tools.map(({ name, annotations }) => ({ name, readOnlyHint: annotations?.readOnlyHint === true }));
	const contexts = (await (await mcp({ jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "list_contexts", arguments: {} } }, mcpToken)).json()) as { result: { content: Array<{ text: string }> } };
	results.listContexts = JSON.parse(contexts.result.content[0].text);

	// Browser server mode: sign in with the startup token in the URL fragment and reach the launcher.
	// Once an interrupt has started teardown, start nothing it would miss. The check and the spawn run in one tick.
	if (finishing) throw new Error("interrupted before Chrome started");
	chrome = startChrome(join(scratch, "chrome-profile"));
	const browser = await chromium.connectOverCDP(await chrome.cdpUrl);
	const page = browser.contexts()[0].pages()[0];
	// Explicit, so every wait has the check's own bound instead of relying on a library default.
	page.setDefaultTimeout(readinessTimeoutMs);
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
