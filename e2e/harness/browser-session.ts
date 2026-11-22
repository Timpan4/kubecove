import { readFile } from "node:fs/promises";
import { browser } from "@wdio/globals";
import { BROWSER_SESSION_STORAGE_KEY } from "../../src/lib/browser-session-contract";
import type { JsonObject } from "../../src/lib/types";
import { browserLoginConfig, storeBrowserSession } from "./browser-login";
import { serverClient } from "./server-session";

export async function runnerServer() {
	const origin = process.env.KUBECOVE_SERVER_ORIGIN;
	const path = process.env.KUBECOVE_SERVER_TOKEN_FILE;
	if (!origin || !path) throw new Error("The runner must supply an isolated server");
	const token = (await readFile(path, "utf8")).trim();
	return { origin, token, client: serverClient(origin, token) };
}

export async function openServerPage() {
	const { origin, token } = await runnerServer();
	// Authenticate on a same-origin document before starting the first measured page.
	// The private credential never becomes part of a navigation URL or a screenshot.
	await browser.url(`${origin}/api/v1/auth`);
	await browser.execute(storeBrowserSession, browserLoginConfig(token)).catch(() => {
		throw new Error("Could not start the browser session");
	});
	await browser.url(origin);
}

// Execute with the page's own cookie and session so diagnostics reflect the UI.
export async function pageInvoke<Result>(command: string, args: JsonObject = {}): Promise<Result> {
	return browser.execute(async (name, payload, storageKey) => {
		const resume = sessionStorage.getItem(storageKey);
		if (!resume) throw new Error("Missing browser session");
		const response = await fetch(`/api/v1/invoke/${name}`, {
			method: "POST", headers: { "Content-Type": "application/json", "X-Kubecove-Session": resume },
			body: JSON.stringify(payload),
		});
		if (!response.ok) throw new Error(`Server command ${name} failed (${response.status})`);
		return response.json();
	}, command, args, BROWSER_SESSION_STORAGE_KEY);
}
