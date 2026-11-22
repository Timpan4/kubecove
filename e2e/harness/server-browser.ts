import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { chromium } from "playwright-core";
import { chromeExecutable, readinessTimeoutMs } from "../../.agents/skills/verify-kubecove/scripts/processes";
import type { SharedWorkspaceDocument } from "../../src/features/workspaces/workspace-sharing-schema";

/** Drive the Svelte assets embedded in the smoke server, using its private fixture context. */
export async function verifyServerBrowser(origin: string, token: string) {
	const browser = await chromium.launch({ executablePath: chromeExecutable() });
	const credentials = [token];
	try {
		const page = await browser.newPage();
		page.setDefaultTimeout(readinessTimeoutMs);
		await page.goto(origin);
		await page.getByLabel("Server token", { exact: true }).fill(token);
		const [login, contexts, socket] = await Promise.all([
			page.waitForResponse(`${origin}/api/v1/session`),
			page.waitForResponse(`${origin}/api/v1/invoke/list_kube_contexts`),
			page.waitForEvent("websocket", (socket) => new URL(socket.url()).pathname === "/api/v1/channels"),
			page.getByRole("button", { name: "Connect", exact: true }).click(),
		]);
		assert.equal(login.status(), 200);
		const session = await login.json();
		credentials.push(session.resumeToken);
		assert.equal(contexts.status(), 200);
		await page.getByText("fixture", { exact: true }).first().waitFor();
		const sharedWorkspace = {
			apiVersion: "kubecove.dev/workspace/v1",
			kind: "Workspace",
			metadata: { name: "browser-import" },
			spec: {
				displayName: "Browser import",
				scope: { clusterContext: "fixture", namespaces: ["readable"], kinds: ["Pod"], layout: "resources", argoAppFilter: "" },
				shortcuts: [],
				portForwards: [],
			},
		} satisfies SharedWorkspaceDocument;
		const [chooser] = await Promise.all([
			page.waitForEvent("filechooser"),
			page.getByRole("button", { name: "Import", exact: true }).click(),
		]);
		await chooser.setFiles({ name: "browser-import.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(sharedWorkspace)) });
		await page.getByText("Import preview", { exact: true }).waitFor();
		await page.getByRole("button", { name: "Apply import", exact: true }).click();
		await page.getByText("Imported 1 new, replaced 0, skipped 0.", { exact: true }).waitFor();
		const [download] = await Promise.all([
			page.waitForEvent("download"),
			page.getByRole("button", { name: "Export all", exact: true }).click(),
		]);
		const exported = JSON.parse(await readFile(await download.path(), "utf8"));
		assert.deepEqual(exported, sharedWorkspace);
		await page.locator("#workspace-name").fill("Server smoke");
		await page.getByRole("checkbox", { name: "readable", exact: true }).check();
		await Promise.all([
			socket.waitForEvent("framereceived", ({ payload }) => {
				const frame = JSON.parse(String(payload));
				return frame.type === "channel" && frame.message.type === "resourceChanged"
					&& frame.message.action === "added" && frame.message.target.kind === "Pod"
					&& frame.message.target.cluster === "fixture" && frame.message.target.namespace === "readable"
					&& frame.message.target.name === "watch-only-pod";
			}),
			(async () => {
				await page.getByRole("button", { name: "Create workspace", exact: true }).click();
				await page.getByRole("button", { name: "Open resources", exact: true }).click();
			})(),
		]);
		await page.getByRole("button", { name: "Open settings", exact: true }).click();
		await page.getByText("Server information", { exact: true }).waitFor();
		await page.getByText(`Server ${session.serverVersion}`, { exact: true }).waitFor();
		await page.getByText(session.credentialStore === "native" ? "Native credential storage" : "Encrypted file storage", { exact: true }).waitFor();
		await page.getByRole("navigation", { name: "Settings sections" }).getByRole("button", { name: "Kubeconfig", exact: true }).click();
		await page.getByRole("textbox", { name: "Kubeconfig path on server" }).waitFor();
		assert.equal(await page.getByRole("button", { name: "Choose files" }).count(), 0);
		// Revoke this session from outside the page. The UI can learn about it only via the live socket.
		await Promise.all([
			socket.waitForEvent("framereceived", ({ payload }) => JSON.parse(String(payload)).type === "session_moved"),
			page.request.post(`${origin}/api/v1/session/close`, {
				headers: { Origin: origin, "X-Kubecove-Session": session.resumeToken },
				data: {},
			}).then((closed) => assert.equal(closed.status(), 200, "Revoking the browser session must succeed")),
		]);
		await page.getByText("This browser's server session closed or expired.", { exact: true }).waitFor();
	} catch (cause) {
		// Playwright's failure log can include input values and authenticated URLs.
		const message = cause instanceof Error ? cause.message : String(cause);
		throw new Error(credentials.reduce((safe, secret) => safe.replaceAll(secret, "[redacted]"), message));
	} finally {
		await browser.close();
	}
}
