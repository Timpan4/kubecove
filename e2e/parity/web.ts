import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { chromium, type Page } from "playwright-core";
import { chromeExecutable, freePort, readinessTimeoutMs, startIdentity, stopGroup, waitFor } from "../../.agents/skills/verify-kubecove/scripts/processes";
import type { Contract, Step } from "./contract";

async function action(page: Page, step: Step) {
	const search = page.getByRole("textbox", { name: "Search resources" });
	const close = page.getByRole("button", { name: "Close resource details" });
	switch (step.action) {
		case "openResources": await page.getByRole("button", { name: "Resources", exact: true }).first().click(); break;
		case "search": await search.fill(step.value); break;
		case "expectSearch": if (await search.inputValue() !== step.value) throw new Error("Search changed after refresh"); break;
		case "expectResource": case "openResource": {
			const row = page.locator("tr").filter({ has: page.getByRole("button", { name: `Open resource ${step.name}`, exact: true }) }).filter({ has: page.getByText(step.kind, { exact: true }) }).filter({ has: page.getByText(step.namespace, { exact: true }) });
			await row.waitFor({ state: "visible" });
			if (await row.count() !== 1) throw new Error("Resource row is ambiguous");
			if (step.action === "openResource") await row.getByRole("button", { name: `Open resource ${step.name}`, exact: true }).click();
			break;
		}
		case "expectDetail": {
			const header = close.locator("xpath=ancestor::header");
			await header.getByRole("heading", { name: step.name, exact: true }).waitFor();
			await header.getByText(`${step.kind} / ${step.namespace}`, { exact: true }).waitFor();
			break;
		}
		case "showYaml": await page.getByRole("tab", { name: "YAML", exact: true }).click(); break;
		case "expectYaml": for (const fragment of step.contains) await page.getByText(fragment, { exact: false }).first().waitFor(); break;
		case "closeDetail": await close.click(); break;
		case "expectDetailClosed": await close.waitFor({ state: "detached" }); break;
		case "expectEmpty":
			await page.getByText("No resources match current scope and filters.", { exact: true }).waitFor();
			if (await page.locator('tr:has(button[aria-label^="Open resource "])').count()) throw new Error("Rows remain in empty search");
			break;
		case "refresh":
			await page.getByRole("button", { name: "Live updates and refresh options" }).click();
			await page.getByRole("button", { name: "Refresh (clear cache)" }).click();
			await page.getByRole("button", { name: "Refresh (clear cache)" }).waitFor({ state: "visible" });
			await page.waitForFunction(() => {
				const button = [...document.querySelectorAll("button")].find((item) => item.textContent?.trim() === "Refresh (clear cache)");
				return button instanceof HTMLButtonElement && !button.disabled;
			});
			if (await page.getByText("Refresh failed:", { exact: false }).count()) throw new Error("Refresh failed");
			await page.keyboard.press("Escape"); break;
	}
}

export async function runWeb(contract: Contract, artifacts: string, signal: AbortSignal) {
	const results: { id: string; status: "passed" | "failed"; error?: string }[] = [];
	const port = await freePort();
	const env = { ...process.env }; delete env.KUBECONFIG;
	const vite = Bun.spawn(["bun", "--bun", "vite", "--host", "127.0.0.1", "--port", String(port), "--strictPort"], { cwd: process.cwd(), env, detached: true, stdout: "pipe", stderr: "pipe" });
	const identity = startIdentity(vite.pid);
	const logs = Promise.all([new Response(vite.stdout).text(), new Response(vite.stderr).text()]);
	let browser: Awaited<ReturnType<typeof chromium.launch>> | undefined;
	const cancel = () => { void browser?.close(); };
	signal.addEventListener("abort", cancel, { once: true });
	try {
		let abortReadiness = () => {};
		try {
			await Promise.race([
				waitFor("Vite", async (timeout) => (await fetch(`http://127.0.0.1:${port}/`, { signal: timeout })).ok),
				new Promise<never>((_, reject) => { abortReadiness = () => reject(signal.reason); signal.addEventListener("abort", abortReadiness, { once: true }); }),
			]);
		} finally { signal.removeEventListener("abort", abortReadiness); }
		signal.throwIfAborted();
		browser = await chromium.launch({ executablePath: chromeExecutable(), headless: true });
		signal.throwIfAborted();
		for (const [scenarioIndex, scenario] of contract.scenarios.entries()) {
			const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
			const browserLog: string[] = [];
			page.on("console", (message) => browserLog.push(`${message.type()}: ${message.text()}`));
			page.on("pageerror", (error) => browserLog.push(`pageerror: ${error.message}`));
			page.setDefaultTimeout(readinessTimeoutMs);
			try {
				signal.throwIfAborted();
				await page.goto(`http://127.0.0.1:${port}/`);
				await page.locator("#workspace-name").fill("Parity mock workspace");
				await page.getByRole("button", { name: "Create workspace", exact: true }).click();
				for (const [index, step] of scenario.steps.entries()) {
					signal.throwIfAborted();
					await action(page, step);
					await page.screenshot({ path: join(artifacts, `web-${scenarioIndex}-${index}.png`) });
				}
				results.push({ id: scenario.id, status: "passed" });
			} catch (error) {
				await page.screenshot({ path: join(artifacts, `web-${scenarioIndex}-failure.png`) }).catch(() => {});
				results.push({ id: scenario.id, status: "failed", error: String(error) });
				if (signal.aborted) break;
			} finally {
				await writeFile(join(artifacts, `web-${scenarioIndex}.log`), browserLog.join("\n"));
				await page.close();
			}
		}
	} finally {
		signal.removeEventListener("abort", cancel);
		await browser?.close();
		const stopped = await stopGroup(vite.pid, identity);
		if (!stopped) throw new Error(`Could not stop owned Vite process group ${vite.pid}`);
		await writeFile(join(artifacts, "vite.log"), (await logs).join("\n"));
		await writeFile(join(artifacts, "web.json"), JSON.stringify({ driver: "web", scenarios: results }, null, 2));
	}
}
