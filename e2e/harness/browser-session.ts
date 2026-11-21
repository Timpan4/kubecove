import { readFile } from "node:fs/promises";
import { browser } from "@wdio/globals";
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
	const ok = await browser.execute(async (credential: string) => {
		const response = await fetch("/api/v1/session", {
			method: "POST", headers: { "Content-Type": "application/json" },
			body: JSON.stringify({ token: credential }),
		});
		if (!response.ok) return false;
		sessionStorage.setItem("kubecove-browser-resume", (await response.json()).resumeToken);
		return true;
	}, token).catch(() => false);
	if (!ok) throw new Error("Could not start the browser session");
	await browser.url(origin);
}

// Execute with the page's own cookie and session so diagnostics reflect the UI.
export async function pageInvoke<Result>(command: string, args: object = {}): Promise<Result> {
	return browser.execute(async (name, payload) => {
		const resume = sessionStorage.getItem("kubecove-browser-resume");
		if (!resume) throw new Error("Missing browser session");
		const response = await fetch(`/api/v1/invoke/${name}`, {
			method: "POST", headers: { "Content-Type": "application/json", "X-Kubecove-Session": resume },
			body: JSON.stringify(payload),
		});
		if (!response.ok) throw new Error(`Server command ${name} failed (${response.status})`);
		return response.json();
	}, command, args);
}
