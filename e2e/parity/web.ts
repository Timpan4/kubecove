import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { chromium, type Page } from "playwright-core";
import { chromeExecutable, freePort, readinessTimeoutMs, startIdentity, stopGroup, waitFor } from "../../.agents/skills/verify-kubecove/scripts/processes";
import { mergeActionMaps } from "./action-maps";
import { type Action, type Contract, type Step, requireRegisteredActions } from "./contract";
import type { WebActions } from "./web-action";
import { startCodeCoverage } from "./web-coverage-map";
import { seedsWorkspace, seedWorkspace, storeReloadHolds, shellActions } from "./web-shell";
import { adapterActions } from "./web-adapters";
import { argoActions } from "./web-argo";
import { coverageActions } from "./web-coverage";
import { deleteReviewActions } from "./web-delete-review";
import { deliveryActions } from "./web-delivery";
import { detailStatusActions } from "./web-detail-status";
import { detailsActions } from "./web-details";
import { diagnosticsActions } from "./web-diagnostics";
import { feedbackActions } from "./web-feedback";
import { incidentDiagnosticsActions } from "./web-incident-diagnostics";
import { kubeconfigSettingsActions } from "./web-kubeconfig-settings";
import { liveMenuActions } from "./web-live-menu";
import { liveSessionActions } from "./web-live-sessions";
import { logRowAndRestoreActions } from "./web-logs-restore";
import { mcpActions } from "./web-mcp";
import { multiContextActions } from "./web-multi-context";
import { navigationActions } from "./web-navigation";
import { overlayMotionActions } from "./web-overlay-motion";
import { overviewActions } from "./web-overview";
import { paletteActions } from "./web-palette";
import { panelActions } from "./web-panels";
import { rbacActions } from "./web-rbac";
import { remoteActions } from "./web-remote";
import { resourceFilterActions } from "./web-resource-filters";
import { resourceTableActions } from "./web-resource-table";
import { resourceActions } from "./web-resources";
import { sessionHealthActions } from "./web-session-health";
import { settingsActions } from "./web-settings";
import { sidebarLiveActions } from "./web-sidebar-live";
import { terminalActions } from "./web-terminal";
import { timingActions } from "./web-timing";
import { topologySpikeActions } from "./web-topology-spike";
import { topologyActions } from "./web-topology";
import { workspaceActions } from "./web-workspaces";
import { yamlEditorActions } from "./web-yaml-editor";
import { yamlPermissionActions } from "./web-yaml-permissions";

const handlers: WebActions = mergeActionMaps(
	adapterActions,
	argoActions,
	coverageActions,
	deleteReviewActions,
	deliveryActions,
	detailStatusActions,
	detailsActions,
	diagnosticsActions,
	feedbackActions,
	incidentDiagnosticsActions,
	kubeconfigSettingsActions,
	liveMenuActions,
	liveSessionActions,
	logRowAndRestoreActions,
	mcpActions,
	multiContextActions,
	navigationActions,
	overlayMotionActions,
	overviewActions,
	paletteActions,
	panelActions,
	rbacActions,
	remoteActions,
	resourceFilterActions,
	resourceTableActions,
	resourceActions,
	sessionHealthActions,
	settingsActions,
	shellActions,
	sidebarLiveActions,
	terminalActions,
	timingActions,
	topologySpikeActions,
	topologyActions,
	workspaceActions,
	yamlEditorActions,
	yamlPermissionActions,
);

requireRegisteredActions(Object.keys(handlers));

export async function action<Name extends Action>(page: Page, step: Step<Name>, scenarioSteps: readonly Step[] = []) {
	const name: Name = step.action;
	if (!Object.hasOwn(handlers, name)) throw new Error(`Unhandled parity action: ${name}`);
	await handlers[name](page, step, scenarioSteps);
}

type WebResult = { id: string; status: "passed" | "failed"; durationMs?: number; error?: string };
// PARITY_STEP_SCREENSHOTS=0 keeps only failure screenshots. CI sets it: step screenshots took
// about ten captures per scenario and 205 MB of artifacts per shard.
const stepScreenshots = process.env.PARITY_STEP_SCREENSHOTS !== "0";
type Browser = Awaited<ReturnType<typeof chromium.launch>>;

async function runScenario(browser: Browser, port: number, artifacts: string, scenarioIndex: number, scenario: Contract["scenarios"][number], signal: AbortSignal): Promise<WebResult> {
	// Each page has its own browser context, so its storage and in-memory mocks stay isolated.
	const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
	const browserLog: string[] = [];
	page.on("console", (message) => browserLog.push(`${message.type()}: ${message.text()}`));
	page.on("pageerror", (error) => browserLog.push(`pageerror: ${error.message}`));
	page.setDefaultTimeout(readinessTimeoutMs);
	// The step that failed, named in the error because CI keeps only the failure screenshot.
	let current = "startup";
	try {
		signal.throwIfAborted();
		// Explicit mock holds control ordering; ordinary parity commands need no artificial delay.
		await page.addInitScript(() => { globalThis.__kubecoveParityNoLatency = true; });
		// Fake timers must exist before the app loads: the Argo tracker's recovery timer and loading indicator delays start before later steps.
		if (scenario.steps.some((step) => step.action === "controlClock" || step.action === "advanceClock" || (step.action === "mockTiming" && step.control.operation === "expectNoRetry"))) await page.clock.install();
		const coverage = await startCodeCoverage(page);
		await page.goto(`http://127.0.0.1:${port}/`);
		const prelude = scenario.steps.findIndex((step) => step.action !== "mockTiming" || step.control.operation !== "hold");
		const startupSteps = prelude < 0 ? scenario.steps.length : prelude;
		for (const step of scenario.steps.slice(0, startupSteps)) await action(page, step, scenario.steps);
		if (seedsWorkspace(scenario)) await seedWorkspace(page);
		for (const [index, step] of scenario.steps.entries()) {
			signal.throwIfAborted();
			current = `step ${index} (${step.action})`;
			if (index >= startupSteps) { await storeReloadHolds(page, scenario.steps, index); await action(page, step, scenario.steps); }
			if (stepScreenshots) await page.screenshot({ path: join(artifacts, `web-${scenarioIndex}-${index}.png`) });
		}
		await coverage?.save(artifacts, scenarioIndex, scenario.id);
		return { id: scenario.id, status: "passed" };
	} catch (error) {
		await page.screenshot({ path: join(artifacts, `web-${scenarioIndex}-failure.png`) }).catch(() => {});
		return { id: scenario.id, status: "failed", error: `${current}: ${String(error)}` };
	} finally {
		try { await writeFile(join(artifacts, `web-${scenarioIndex}.log`), browserLog.join("\n")); } finally { await page.close(); }
	}
}

/** Runs scenarios on `jobs` pages at once; results keep the contract order. */
export async function runWeb(contract: Contract, artifacts: string, signal: AbortSignal, jobs = 1) {
	const results: WebResult[] = [];
	const port = await freePort();
	signal.throwIfAborted();
	const env = { ...process.env }; delete env.KUBECONFIG; delete env.ARGV0;
	// Coverage needs Vite's per-source inline maps; normal runs load the bundled dev app.
	const command = process.env.PARITY_COVERAGE === "1"
		? [process.execPath, join(process.cwd(), "node_modules/vite/bin/vite.js"), "--host", "127.0.0.1", "--port", String(port), "--strictPort"]
		: [process.execPath, join(process.cwd(), "e2e/parity/web-bundle-server.ts"), String(port), artifacts];
	const vite = Bun.spawn(command, { cwd: process.cwd(), env, detached: true, stdout: "pipe", stderr: "pipe" });
	const identity = startIdentity(vite.pid);
	const logs = Promise.all([new Response(vite.stdout).text(), new Response(vite.stderr).text()]);
	let browser: Browser | undefined;
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
		const launched = await chromium.launch({ executablePath: chromeExecutable(), headless: true });
		browser = launched;
		signal.throwIfAborted();
		const ordered: (WebResult | undefined)[] = [];
		let next = 0;
		const worker = async () => {
			while (next < contract.scenarios.length && !signal.aborted) {
				const scenarioIndex = next++;
				const scenario = contract.scenarios[scenarioIndex];
				if (!scenario) continue;
				const started = performance.now();
				const result = await runScenario(launched, port, artifacts, scenarioIndex, scenario, signal);
				ordered[scenarioIndex] = { ...result, durationMs: Math.round(performance.now() - started) };
			}
		};
		await Promise.all(Array.from({ length: Math.max(1, Math.min(jobs, contract.scenarios.length)) }, worker));
		for (const result of ordered) if (result) results.push(result);
	} finally {
		signal.removeEventListener("abort", cancel);
		// Each cleanup step runs even when an earlier one fails, so Vite stops and the results are kept.
		let closeError: unknown;
		await browser?.close().catch((error) => { closeError = error; });
		await writeFile(join(artifacts, "web.json"), JSON.stringify({ driver: "web", scenarios: results }, null, 2));
		const stopped = await stopGroup(vite.pid, identity);
		if (!stopped) throw new Error(`Could not stop owned Vite process group ${vite.pid}`);
		await writeFile(join(artifacts, "vite.log"), (await logs).join("\n"));
		if (closeError) throw closeError;
	}
}
