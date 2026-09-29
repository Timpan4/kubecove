import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { chromium, type Browser, type Page } from "playwright-core";

const cdpUrl = process.env.OBSCURA_CDP_URL?.trim() || "ws://127.0.0.1:9223/devtools/browser";
const appUrl = process.env.E2E_FAST_URL?.trim() || "http://127.0.0.1:1430/";
const artifactRoot = process.env.KUBECOVE_E2E_ARTIFACTS?.trim() || join("e2e", "artifacts");
const artifacts = join(artifactRoot, `obscura-${new Date().toISOString().replace(/[:.]/g, "-")}`);

interface SmokeResult {
	passed: boolean;
	title?: string;
	workspaceName?: string;
	error?: string;
}

await mkdir(artifacts, { recursive: true });
let browser: Browser | undefined;
let page: Page | undefined;
let result: SmokeResult = { passed: false };
try {
	browser = await chromium.connectOverCDP(cdpUrl);
	const context = await browser.newContext();
	page = await context.newPage();
	await page.goto(appUrl);
	await page.getByText("mock-dev", { exact: true }).first().waitFor();
	await page.locator("#workspace-name").fill("Obscura Lab");
	const title = await page.title();
	const workspaceName = await page.locator("#workspace-name").inputValue();
	const bodyText = await page.locator("body").innerText();
	result = {
		passed: title === "KubeCove" && workspaceName === "Obscura Lab" && bodyText.includes("Create workspace") && bodyText.includes("mock-dev"),
		title,
		workspaceName,
	};
	await writeFile(join(artifacts, "page.txt"), bodyText);
	if (!result.passed) throw new Error("Expected the KubeCove mock workspace form and editable workspace name");
} catch (error) {
	result = { ...result, passed: false, error: String(error).replaceAll(cdpUrl, "[Obscura CDP endpoint]") };
	if (page) {
		await writeFile(join(artifacts, "page.txt"), await page.locator("body").innerText()).catch(() => {});
		await page.screenshot({ path: join(artifacts, "failure.png") }).catch(() => {});
	}
} finally {
	await page?.close().catch(() => {});
	await browser?.close().catch(() => {});
	await writeFile(join(artifacts, "result.json"), `${JSON.stringify({ appUrl, driver: "Playwright over Obscura CDP", ...result }, null, 2)}\n`);
	console.log(`${result.passed ? "Obscura + Playwright smoke passed" : "Obscura + Playwright smoke failed"}: ${artifacts}`);
	if (!result.passed) process.exitCode = 1;
}
