// Captures reference screenshots of the Svelte web frontend with browser mocks.
// Usage: bun e2e/parity/visual/capture-web.ts <output-dir>
import { mkdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { chromium, type Page } from "playwright-core";
import { chromeExecutable, freePort, readinessTimeoutMs, startIdentity, stopGroup, waitFor } from "../../../.agents/skills/verify-kubecove/scripts/processes";
import { VIEWPORT, VIEWS, type View } from "./views";

async function show(page: Page, view: View) {
	switch (view) {
		case "overview":
			return;
		case "cluster-overview":
			// The row's accessible name also includes its "Expand" button label.
			await page.getByRole("treeitem").filter({ hasText: /^\s*Cluster Overview\s*$/ }).click();
			await page.getByRole("button", { name: /^Open resource / }).first().waitFor();
			return;
		case "resources":
			await page.getByRole("button", { name: "Resources", exact: true }).first().click();
			await page.getByRole("button", { name: /^Open resource / }).first().waitFor();
			return;
		case "detail":
		case "yaml":
		case "events":
		case "logs":
		case "revisions":
		case "actions": {
			await page.getByRole("button", { name: "Resources", exact: true }).first().click();
			await page.getByRole("textbox", { name: "Search resources" }).fill("payments-api");
			const row = page.locator("tr").filter({ has: page.getByRole("button", { name: "Open resource payments-api", exact: true }) }).filter({ has: page.getByText("Deployment", { exact: true }) });
			await row.getByRole("button", { name: "Open resource payments-api", exact: true }).click();
			await page.getByRole("button", { name: "Close resource details" }).waitFor();
			// The pane opens on a loading placeholder; capture the loaded detail.
			await page.getByRole("tab", { name: "Details", exact: true }).waitFor();
			await page.getByText("Loading resource details", { exact: false }).waitFor({ state: "hidden" });
			if (view === "yaml") {
				await page.getByRole("tab", { name: "YAML", exact: true }).click();
				await page.getByText("kind: Deployment", { exact: false }).first().waitFor();
			}
			if (view === "events") {
				await page.getByRole("tab", { name: "Events", exact: true }).click();
				await page.getByRole("textbox", { name: "Filter resource events" }).waitFor();
			}
			if (view === "logs") {
				await page.getByRole("tab", { name: "Logs", exact: true }).click();
				await page.getByText("downstream retry budget at 82%", { exact: false }).waitFor();
			}
			if (view === "revisions") {
				await page.getByRole("tab", { name: "Revisions", exact: true }).click();
				await page.getByRole("combobox", { name: "Comparison revision" }).waitFor();
			}
			if (view === "actions") {
				await page.getByRole("tab", { name: "Actions", exact: true }).click();
				await page.locator('[data-resource-action="replicas"]').waitFor();
				await page.getByText("Loading", { exact: false }).waitFor({ state: "hidden" });
			}
			return;
		}
		case "incidents":
			await page.getByRole("treeitem").filter({ has: page.getByText("Incidents", { exact: true }) }).click();
			await page.getByText("Guided investigation", { exact: true }).waitFor();
			return;
		case "port-forwards":
			await page.getByRole("treeitem").filter({ has: page.getByText("Port Forwards", { exact: true }) }).click();
			await page.getByText("Active local tunnels with guarded stop controls.", { exact: true }).waitFor();
			return;
	}
}

const output = resolve(process.argv[2] ?? "e2e/artifacts/gpui-visual/web");
await mkdir(output, { recursive: true });
const port = await freePort();
const env = { ...process.env };
delete env.KUBECONFIG;
const vite = Bun.spawn(["bun", "--bun", "vite", "--host", "127.0.0.1", "--port", String(port), "--strictPort"], { cwd: process.cwd(), env, detached: true, stdout: "ignore", stderr: "ignore" });
const identity = startIdentity(vite.pid);
let browser: Awaited<ReturnType<typeof chromium.launch>> | undefined;
try {
	await waitFor("Vite", async (timeout) => (await fetch(`http://127.0.0.1:${port}/`, { signal: timeout })).ok);
	browser = await chromium.launch({ executablePath: chromeExecutable(), headless: true });
	for (const view of VIEWS) {
		// GPUI's offscreen window renders at scale 2, so the web capture matches it.
		const page = await browser.newPage({ viewport: VIEWPORT, deviceScaleFactor: 2, colorScheme: "dark", reducedMotion: "reduce" });
		page.setDefaultTimeout(readinessTimeoutMs);
		await page.goto(`http://127.0.0.1:${port}/`);
		await page.locator("#workspace-name").fill("Parity mock workspace");
		await page.getByRole("button", { name: "Create workspace", exact: true }).click();
		await page.waitForLoadState("networkidle");
		await show(page, view);
		// Let deferred surfaces and fonts settle before the capture.
		await page.evaluate(() => document.fonts.ready);
		await page.waitForTimeout(800);
		await page.mouse.move(0, VIEWPORT.height - 1);
		await page.screenshot({ path: join(output, `${view}.png`) });
		console.log(`web ${view}`);
		await page.close();
	}
} finally {
	await browser?.close();
	if (!(await stopGroup(vite.pid, identity))) console.error(`Could not stop Vite process group ${vite.pid}`);
}
