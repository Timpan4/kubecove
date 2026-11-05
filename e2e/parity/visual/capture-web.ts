// Captures reference screenshots of the Svelte web frontend with browser mocks.
// Usage: bun e2e/parity/visual/capture-web.ts <output-dir>
import { mkdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { chromium, type Page } from "playwright-core";
import { chromeExecutable, freePort, readinessTimeoutMs, startIdentity, stopGroup, waitFor } from "../../../.agents/skills/verify-kubecove/scripts/processes";
import { VIEWPORT, VIEWS, type View } from "./views";

async function show(page: Page, view: View) {
	switch (view) {
		case "usage-footer":
            await page.getByRole("button", { name: "Open settings", exact: true }).click();
            await page.getByRole("switch", { name: "Show CPU and memory footer", exact: true }).click();
            await page.locator("footer").getByText("CPU 3.8% · Memory 370 MB · 3 processes", { exact: true }).waitFor();
            return;
        case "overview":
			return;
		case "cluster-overview":
			// The row's accessible name also includes its "Expand" button label.
			await page.getByRole("treeitem").filter({ hasText: /^\s*Cluster Overview\s*$/ }).click();
			await page.getByRole("button", { name: /^Open resource / }).first().waitFor();
			return;
		case "network-flow":
        case "metrics":
        case "resources":
			await page.getByRole("button", { name: "Resources", exact: true }).first().click();
			await page.getByRole("button", { name: /^Open resource / }).first().waitFor();
            if (view === "network-flow") await page.getByRole("button", { name: "Network Flow", exact: true }).click();
            if (view === "metrics") await page.getByRole("textbox", { name: "Search resources" }).fill("payments-api");
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
		case "rbac":
			await page.getByRole("treeitem", { name: /^(?:Expand RBAC )?RBAC$/ }).click();
			await page.getByText("Verify exact action", { exact: true }).waitFor();
			return;
		case "workspaces":
			await page.getByRole("button", { name: "Open workspaces", exact: true }).click();
			await page.getByRole("checkbox", { name: "payments", exact: true }).waitFor();
			return;
		case "settings":
			await page.getByRole("button", { name: "Open settings", exact: true }).click();
			await page.getByRole("heading", { level: 2, name: "General", exact: true }).waitFor();
			return;
		case "palette":
			await page.getByRole("button", { name: "Search views, namespaces, and resources", exact: true }).click();
			await page.getByPlaceholder("Search views, namespaces, and resources...").fill("payments");
			await page.getByRole("option").first().waitFor();
			return;
		case "gitops":
		case "gitops-list":
		case "flux":
		case "argo-app":
		case "argo-workspace":
			await page.getByRole("treeitem").filter({ hasText: /^\s*GitOps\s*$/ }).click();
			await page.getByRole("heading", { level: 2, name: "Argo CD Applications", exact: true }).waitFor();
			if (view === "gitops-list") await page.getByRole("radio", { name: "List", exact: true }).click();
			if (view === "flux") {
				await page.getByRole("button", { name: "Expand GitOps", exact: true }).click();
				await page.getByRole("treeitem", { name: /^(?:(?:Expand|Collapse) Flux )?Flux$/ }).click();
				await page.getByRole("heading", { level: 2, name: "Flux Resources", exact: true }).waitFor();
			}
			if (view === "argo-app") {
				await page.getByRole("button", { name: "Open details for Application root-application in argocd", exact: true }).click();
				await page.getByText("Change navigator", { exact: true }).waitFor();
			}
			if (view === "argo-workspace") {
				await page.getByRole("button", { name: "View resources for platform-argocd", exact: true }).first().click();
				await page.getByRole("heading", { level: 2 }).filter({ hasText: "platform-argocd" }).waitFor();
				await page.getByLabel("Argo CD resource filters").waitFor();
			}
			return;
		case "helm":
		case "helm-list":
		case "helm-release":
			await page.getByRole("treeitem").filter({ hasText: /^\s*Helm\s*$/ }).click();
			await page.getByRole("heading", { level: 2, name: "Helm Releases", exact: true }).waitFor();
			if (view === "helm-list") await page.getByRole("radio", { name: "List", exact: true }).click();
			if (view === "helm-release") {
				const card = page.locator('[data-slot="card"]').filter({ has: page.locator('[data-slot="card-title"]').getByText("metrics-gateway", { exact: true }) });
				await card.getByRole("button", { name: "Details", exact: true }).click();
				await page.getByText("4 tracked, 1 missing, 0 label-only", { exact: true }).waitFor();
				await page.getByRole("button", { name: "Close Helm details", exact: true }).evaluate((button) => {
					const details = button.closest('[data-slot="card"]');
					const viewport = details?.closest(".overflow-y-auto, .overflow-auto");
					if (!(details instanceof HTMLElement) || !(viewport instanceof HTMLElement)) throw new Error("Helm details scroll viewport missing");
					viewport.scrollTop += details.getBoundingClientRect().top - viewport.getBoundingClientRect().top - viewport.clientTop;
				});
				const header = await page.getByRole("button", { name: "Search views, namespaces, and resources", exact: true }).boundingBox();
				if (!header || header.y < 0) throw new Error("Helm details capture scrolled the workspace header out of view");
			}
			return;
	}
}

const output = resolve(process.argv[2] ?? "e2e/artifacts/gpui-visual/web");
await mkdir(output, { recursive: true });
const port = await freePort();
const env = { ...process.env };
delete env.KUBECONFIG;
delete env.ARGV0;
const vite = Bun.spawn([process.execPath, join(process.cwd(), "node_modules/vite/bin/vite.js"), "--host", "127.0.0.1", "--port", String(port), "--strictPort"], { cwd: process.cwd(), env, detached: true, stdout: "ignore", stderr: "ignore" });
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
