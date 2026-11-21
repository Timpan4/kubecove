import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { readinessTimeoutMs, waitFor } from "../../.agents/skills/verify-kubecove/scripts/processes";

export async function mcpCall(endpoint: string, token: string, name: string, args: object = {}) {
	const response = await fetch(endpoint, {
		method: "POST", signal: AbortSignal.timeout(readinessTimeoutMs),
		headers: { Accept: "application/json, text/event-stream", "Content-Type": "application/json", Authorization: `Bearer ${token}` },
		body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name, arguments: args } }),
	});
	if (!response.ok) throw new Error(`MCP ${name} returned HTTP ${response.status}`);
	const { result } = await response.json();
	if (!result || result.isError) {
		// A changed snapshot is rejected before dispatching input, so it is safe to retry.
		if (/take a new snapshot|changed since the snapshot/.test(result?.content?.[0]?.text ?? "")) throw new StaleSnapshot();
		throw new Error(`MCP tool ${name} failed`);
	}
	return JSON.parse(result.content[0].text);
}

class StaleSnapshot extends Error {}

interface Element { path: string; ref: string; label?: string; description?: string }
export async function gpuiDriver(endpoint: string, dataDir: string) {
	const token = (await readFile(join(dataDir, "automation-token"), "utf8")).trim();
	const call = (name: string, args: object = {}) => mcpCall(endpoint, token, name, args);
	const snapshot = async (): Promise<Element[]> => (await call("snapshot")).elements;
	const find = async (description: string, matches: (el: Element) => boolean) => {
		let element: Element | undefined;
		await waitFor(description, async () => {
			element = (await snapshot()).find(matches);
			return Boolean(element);
		});
		if (!element) throw new Error(`Missing ${description}`);
		return element;
	};
	const id = (id: string) => find(id, (el) => el.path.split("/").at(-1) === id);
	const label = (label: string) => find(label, (el) => el.label === label);
	const click = async (labelText: string) => { await label(labelText); await call("click", { label: labelText }); };
	const clickId = (path: string) => waitFor(`click ${path}`, async () => {
		const element = (await snapshot()).find(el => el.path.split("/").at(-1) === path);
		if (!element) return false;
		try { await call("click", { ref: element.ref }); return true; }
		catch (error) { if (error instanceof StaleSnapshot) return false; throw new Error(`Click failed for ${path}`, { cause: error }); }
	});
	const type = async (labelText: string, text: string) => { await label(labelText); await call("type_text", { label: labelText, text }); };
	return { call, find, id, label, click, clickId, type };
}
