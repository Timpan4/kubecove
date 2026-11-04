import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { chromium, type Locator, type Page } from "playwright-core";
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
		case "expectClusterResource": case "expectNoClusterResource": {
			const row = page.locator("tr").filter({ has: page.getByRole("button", { name: `Open resource ${step.name}`, exact: true }) }).filter({ has: page.getByText(step.kind, { exact: true }) });
			// Absence is checked after a preceding step waited for the new scope's rows.
			if (step.action === "expectNoClusterResource") { if (await row.count()) throw new Error(`${step.kind} ${step.name} is still listed`); }
			else await row.first().waitFor({ state: "visible" });
			break;
		}
		case "openPage": {
			// Rows with children also name their expand toggle.
			const label = literal(step.page);
			await page.getByRole("treeitem", { name: new RegExp(`^(?:(?:Expand|Collapse) ${label} )?${label}$`) }).click();
			break;
		}
		case "showTab": await page.getByRole("tab", { name: step.tab, exact: true }).click(); break;
		case "filterEvents": await page.getByRole("radiogroup", { name: "Filter events by type" }).getByRole("radio", { name: step.filter, exact: true }).click(); break;
		case "expectEventCount":
			if (step.count) await eventRows(page).nth(step.count - 1).waitFor();
			await eventRows(page).nth(step.count).waitFor({ state: "detached" });
			break;
		case "expectEvent":
			await eventRows(page).nth(step.index).filter({ has: page.getByText(step.message, { exact: true }) }).filter({ has: page.getByText(step.reason, { exact: true }) }).waitFor();
			break;
		case "expectLiveSessionCount": await page.getByRole("button", { name: `Open live sessions (${step.count} active)`, exact: true }).waitFor(); break;
		case "filterIncidents": await page.locator('[aria-label="Filter incident signals"]').getByRole("button", { name: new RegExp(`^${literal(step.filter)} \\d+$`) }).click(); break;
		case "expectIncidentGuide": await page.getByRole("heading", { level: 3, name: step.title, exact: true }).waitFor(); break;
		case "expectGitOpsCount":
			await page.locator("div.justify-between").filter({ has: page.getByText(step.label, { exact: true }) }).locator("strong").filter({ hasText: new RegExp(`^${step.count}$`) }).waitFor();
			break;
		case "expectPortForward": await page.getByRole("cell").getByText(step.title, { exact: true }).waitFor(); break;
		case "expectLogLines":
			// Headlines of the rendered rows, in display order.
			await page.waitForFunction((lines) => JSON.stringify([...document.querySelectorAll("[data-log-row]")].map((row) => row.querySelector('span[class*="text-[12.5px]"]')?.textContent?.trim())) === JSON.stringify(lines), step.lines);
			break;
		case "filterLogs": await page.getByRole("textbox", { name: "Filter log lines" }).fill(step.value); break;
		case "toggleLogLevel": await page.getByRole("group", { name: "Filter by level" }).getByRole("button", { name: new RegExp(`^${step.level} `) }).click(); break;
		case "expectLogsEmpty": await page.getByText(step.title, { exact: true }).waitFor(); break;
		case "selectRevision": await revisionButton(page, step.label).click(); break;
		case "expectRevisionDiff": {
			const header = page.locator("div.border-b").filter({ hasText: "compared with" });
			await header.locator("span.font-medium", { hasText: new RegExp(`^${literal(step.selected)}$`) }).waitFor();
			await header.getByRole("combobox", { name: "Comparison revision" }).filter({ hasText: new RegExp(`^\\s*${literal(step.comparison)}\\s*$`) }).waitFor();
			await containsLines(page.getByRole("tabpanel").locator("pre"), step.contains);
			break;
		}
		case "reviewAction": await page.getByRole("button", { name: step.label, exact: true }).click(); break;
		case "editReplicas":
			await page.locator('[data-resource-action="replicas"]').getByRole("button", { name: "Edit", exact: true }).click();
			await page.getByRole("spinbutton", { name: "Desired replicas" }).fill(step.value);
			break;
		case "expectReview": {
			const dialog = page.getByRole("dialog");
			await dialog.getByRole("heading", { name: step.title, exact: true }).waitFor();
			await dialog.getByText(step.description, { exact: true }).waitFor();
			await containsLines(dialog.getByLabel("Server result diff"), step.contains);
			break;
		}
		case "confirmDeletion": await page.getByRole("dialog").getByRole("textbox", { name: "Type resource name to confirm" }).fill(step.value); break;
		// Forced, so a disabled control is clicked like a user would and must do nothing.
		case "executeReview": await page.getByRole("dialog").getByRole("button", { name: /^(Apply|Delete)$/ }).click({ force: true }); break;
		case "closeReview": await page.getByRole("dialog").getByRole("button", { name: "Cancel", exact: true }).click(); break;
		case "expectNoReview": await page.getByRole("dialog").waitFor({ state: "detached" }); break;
		case "expectActionResult": await page.getByRole("status").filter({ hasText: new RegExp(`^${literal(step.text)}$`) }).waitFor(); break;
		case "expectNoActionResult": if (await page.locator('p[role="status"].text-emerald-300').count()) throw new Error("An action result is shown"); break;
		case "editYaml": await page.getByRole("button", { name: "Edit YAML", exact: true }).click({ force: true }); break;
		case "applyYaml": await page.getByRole("button", { name: "Apply", exact: true }).click({ force: true }); break;
		case "dryRunYaml": await page.getByRole("button", { name: "Dry run", exact: true }).click(); break;
		case "expectDryRun": await containsLines(page.locator("div.font-mono.text-xs.leading-relaxed"), step.contains); break;
		case "expectYamlNotice": await page.getByText(step.title, { exact: true }).first().waitFor(); break;
		case "expectNoYamlNotice": if (await page.getByText(step.title, { exact: true }).count()) throw new Error(`${step.title} is shown`); break;
		case "expectYamlReadOnly":
			await page.getByRole("button", { name: "Edit YAML", exact: true }).waitFor();
			if (await page.getByRole("button", { name: "Dry run", exact: true }).count()) throw new Error("The YAML draft editor opened");
			break;
		default: { const unhandled: never = step; throw new Error(`Unhandled parity step: ${JSON.stringify(unhandled)}`); }
	}
}

/** Event rows in the Events tab: each starts with its `<time>`. */
function eventRows(page: Page) { return page.locator("div.grid:has(> time[datetime])"); }
function literal(text: string) { return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"); }
/** A revision in the Revisions list; its comparison picker shows the same label. */
function revisionButton(page: Page, label: string) { return page.locator("button.w-full.p-3").filter({ has: page.locator("div.font-medium", { hasText: new RegExp(`^${literal(label)}$`) }) }); }
/** Waits until the element's rendered lines include every expected diff line. */
async function containsLines(locator: Locator, lines: string[]) {
	const element = await locator.first().elementHandle();
	await locator.page().waitForFunction(({ element, expected }) => {
		// biome-ignore lint/performance/useDomNodeTextContent: the YAML dry-run diff renders each line as a block <div>, so only rendered text keeps one line per row.
		const rendered = element instanceof HTMLElement ? element.innerText.split("\n") : [];
		return expected.every((line) => rendered.includes(line));
	}, { element, expected: lines });
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
