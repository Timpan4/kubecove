import { type AddressInfo, createServer, type Server } from "node:net";
import { $, browser, expect } from "@wdio/globals";
import { after, describe, it } from "mocha";
import type { McpStatus } from "../../../src/lib/mcp-runtime";

// Failure modes (ADR 0028): the listener starts before the user enables it; the tools
// answer without the token or to a browser Origin; an agent sees tools beyond the
// read-only set; a port conflict stops the app or goes unreported; a rotated token
// keeps working; turning MCP off leaves the port open.

const TOOL_COUNT = 11;

type JsonRpcRequest = {
	jsonrpc: "2.0";
	id: number;
	method: "tools/list" | "tools/call";
	params: { name?: string; arguments?: Record<never, never> };
};
type ToolsListResponse = { result: { tools: Array<{ name: string; annotations: { readOnlyHint: boolean } }> } };
type ToolCallResponse = { result: { content: Array<{ text: string }> } };

/** Binds a loopback port and returns its number; the caller owns the server. */
async function listenOnLoopback(server: Server): Promise<number> {
	await new Promise<void>((ready) => server.listen(0, "127.0.0.1", ready));
	// SAFETY: a TCP server listening on a host and port reports AddressInfo, never a pipe name.
	return (server.address() as AddressInfo).port;
}

async function freePort(): Promise<number> {
	const server = createServer();
	const port = await listenOnLoopback(server);
	await new Promise((closed) => server.close(closed));
	return port;
}

async function rpc(port: number, token: string | null, body: JsonRpcRequest, origin?: string): Promise<Response> {
	const headers = new Headers({ Accept: "application/json, text/event-stream", "Content-Type": "application/json" });
	if (token) headers.set("Authorization", `Bearer ${token}`);
	if (origin) headers.set("Origin", origin);
	return fetch(`http://127.0.0.1:${port}/mcp`, { method: "POST", headers, body: JSON.stringify(body) });
}

const listTools: JsonRpcRequest = { jsonrpc: "2.0", id: 1, method: "tools/list", params: {} };

async function status(): Promise<McpStatus> {
	// The WebDriver bridge treats a returned object's `error` key as a script failure.
	const json = await browser.execute(() =>
		window.__TAURI__.core.invoke<McpStatus>("get_mcp_status").then((value) => JSON.stringify(value)),
	);
	// SAFETY: `json` is JSON.stringify of the get_mcp_status result, which the command contract types as McpStatus.
	return JSON.parse(json) as McpStatus;
}

async function token(): Promise<string> {
	return browser.execute(() => window.__TAURI__.core.invoke<string>("reveal_mcp_token"));
}

async function savePort(port: number): Promise<void> {
	const input = await $("#mcp-port");
	await input.setValue(String(port));
	await $("button=Save").click();
	await browser.waitUntil(async () => (await status()).port === port, { timeoutMsg: "MCP port was not saved." });
}

describe("desktop MCP server", () => {
	let blocker: Server | undefined;
	after(async () => {
		if (blocker) await new Promise((closed) => blocker?.close(closed));
	});

	it("is off until enabled, then serves only read-only tools to the token holder", async () => {
		await $('button[aria-label="Open settings"]').click();
		await $("button*=AI agents").click();
		const toggle = await $('button[role="switch"][aria-label="MCP server"]');
		await toggle.waitForClickable();
		await expect(toggle).toHaveAttribute("aria-checked", "false");
		expect((await status()).running).toBe(false);

		const port = await freePort();
		await savePort(port);
		await toggle.click();
		await expect($("body")).toHaveText(expect.stringContaining(`Listening at http://127.0.0.1:${port}/mcp`));
		const first = await token();

		expect((await rpc(port, null, listTools)).status).toBe(401);
		expect((await rpc(port, "wrong-token", listTools)).status).toBe(401);
		expect((await rpc(port, first, listTools, "http://127.0.0.1:1430")).status).toBe(403);
		const listed = await rpc(port, first, listTools);
		expect(listed.status).toBe(200);
		// SAFETY: a 200 tools/list reply is a JSON-RPC result carrying the MCP tool list.
		const { tools } = ((await listed.json()) as ToolsListResponse).result;
		expect(tools).toHaveLength(TOOL_COUNT);
		expect(tools.every((tool) => tool.annotations.readOnlyHint)).toBe(true);

		const contexts = await rpc(port, first, {
			jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "list_contexts", arguments: {} },
		});
		// SAFETY: a successful tools/call reply is a JSON-RPC result whose first block is text.
		const result = JSON.parse(((await contexts.json()) as ToolCallResponse).result.content[0].text);
		expect(result.contexts.filter((context: { current: boolean }) => context.current)).toHaveLength(1);
		expect(result.contexts).toHaveLength(2);
	});

	it("reports a port conflict, rotates the token and closes the port when turned off", async () => {
		const working = (await status()).port;
		blocker = createServer();
		const taken = await listenOnLoopback(blocker);
		await savePort(taken);
		await expect($('p[role="alert"]')).toHaveText(expect.stringContaining(`Could not listen on 127.0.0.1:${taken}`));
		expect((await status()).running).toBe(false);
		await savePort(working);
		await browser.waitUntil(async () => (await status()).running, { timeoutMsg: "MCP did not restart." });

		const old = await token();
		await $("button=Rotate token").click();
		await browser.waitUntil(async () => (await token()) !== old, { timeoutMsg: "Token was not rotated." });
		expect((await rpc(working, old, listTools)).status).toBe(401);
		expect((await rpc(working, await token(), listTools)).status).toBe(200);

		await $('button[role="switch"][aria-label="MCP server"]').click();
		await browser.waitUntil(async () => !(await status()).running, { timeoutMsg: "MCP did not stop." });
		await expect(rpc(working, await token(), listTools)).rejects.toThrow();
	});
});
