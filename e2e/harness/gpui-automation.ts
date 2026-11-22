import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { readinessTimeoutMs, waitFor } from "../../.agents/skills/verify-kubecove/scripts/processes";

type AutomationArgs = { label?: string; ref?: string; text?: string; key?: string };

export async function mcpCall(endpoint: string, token: string, name: string, args: AutomationArgs = {}, signal = AbortSignal.timeout(readinessTimeoutMs)) {
	const response = await fetch(endpoint, {
		method: "POST", signal,
		headers: { Accept: "application/json, text/event-stream", "Content-Type": "application/json", Authorization: `Bearer ${token}` },
		body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name, arguments: args } }),
	});
	if (!response.ok) throw new Error(`MCP ${name} returned HTTP ${response.status}`);
	const envelope = await response.json().catch(() => {
		throw new Error(`MCP tool ${name} returned invalid JSON`);
	});
	const result = envelope?.result;
	const block = Array.isArray(result?.content) ? result.content[0] : undefined;
	// oxlint-disable-next-line anti-slop/no-runtime-typeof -- Validate MCP content before passing it to JSON.parse.
	const text = block?.type === "text" && typeof block.text === "string" ? block.text : undefined;
	if (result?.isError) {
		// A changed snapshot is rejected before dispatching input, so it is safe to retry.
		if (text && /take a new snapshot|changed since the snapshot/.test(text)) throw new StaleSnapshot();
		throw new Error(`MCP tool ${name} failed`);
	}
	if (text === undefined) throw new Error(`MCP tool ${name} did not return text content`);
	try { return JSON.parse(text); }
	catch { throw new Error(`MCP tool ${name} returned invalid JSON text`); }
}

class StaleSnapshot extends Error {}

interface Element { path: string; ref: string; label?: string; description?: string }
export async function gpuiDriver(endpoint: string, dataDir: string, runSignal: AbortSignal) {
	const token = (await readFile(join(dataDir, "automation-token"), "utf8")).trim();
	const call = (name: string, args: AutomationArgs = {}, signal = AbortSignal.timeout(readinessTimeoutMs)) =>
		mcpCall(endpoint, token, name, args, AbortSignal.any([runSignal, signal]));
	const snapshot = async (signal: AbortSignal): Promise<Element[]> => (await call("snapshot", {}, signal)).elements;
	const find = async (description: string, matches: (el: Element) => boolean) => {
		let element: Element | undefined;
		await waitFor(description, async (signal) => {
			element = (await snapshot(signal)).find(matches);
			return Boolean(element);
		}, readinessTimeoutMs, { retryErrors: false });
		if (!element) throw new Error(`Missing ${description}`);
		return element;
	};
	const id = (id: string) => find(id, (el) => el.path.split("/").at(-1) === id);
	const label = (label: string) => find(label, (el) => el.label === label);
	const click = async (labelText: string) => { await label(labelText); await call("click", { label: labelText }); };
	const clickId = (path: string) => waitFor(`click ${path}`, async (signal) => {
		const element = (await snapshot(signal)).find(el => el.path.split("/").at(-1) === path);
		if (!element) return false;
		try { await call("click", { ref: element.ref }, signal); return true; }
		catch (error) { if (error instanceof StaleSnapshot) return false; throw new Error(`Click failed for ${path}`, { cause: error }); }
	}, readinessTimeoutMs, { retryErrors: false });
	const type = async (labelText: string, text: string) => { await label(labelText); await call("type_text", { label: labelText, text }); };
	return { call, find, id, label, click, clickId, type };
}
