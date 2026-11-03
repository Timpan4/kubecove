#!/usr/bin/env bun
// Drives KubeCove browser dev mode (typed IPC mocks, `mock-dev` context) for verification runs.
// Every run owns one Vite process group and one headless Chrome process group, recorded by PID and leader start time.
import { existsSync, readdirSync } from "node:fs";
import { appendFile, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, join } from "node:path";
import { chromium, type Locator, type Page } from "playwright-core";
import { freePort, gitHead, ownsLeader, readinessTimeoutMs, root, startChrome, startIdentity, stopGroup, waitFor } from "./processes";

type RunState = { runId: string; head: string; url: string; vitePid: number; viteStartedAt?: string; chromePid: number; chromeStartedAt?: string; cdpUrl: string; evidenceDir: string };
type Role = Parameters<Page["getByRole"]>[0];

const stateRoot = join(root, ".e2e", "verify");
const [command, ...rest] = Bun.argv.slice(2);
const flag = (name: string) => {
	const index = rest.indexOf(`--${name}`);
	return index >= 0 ? rest[index + 1] : undefined;
};
const required = (name: string) => {
	const value = flag(name);
	if (value === undefined) throw new Error(`--${name} is required`);
	return value;
};
const hasTarget = () => Boolean(flag("role") || flag("label") || flag("text") || flag("selector"));

function runIdFromArgs() {
	const explicit = flag("run") ?? process.env.KUBECOVE_VERIFY_RUN;
	if (explicit) return explicit;
	const runs = existsSync(stateRoot) ? readdirSync(stateRoot).filter((name) => existsSync(join(stateRoot, name, "state.json"))) : [];
	if (runs.length === 1) return runs[0];
	throw new Error(runs.length ? `several runs exist (${runs.join(", ")}); pass --run <id>` : "no verification run; run `launch` first");
}

async function loadState(runId: string): Promise<RunState> {
	const file = join(stateRoot, runId, "state.json");
	if (!existsSync(file)) throw new Error(`run ${runId} has no state at ${file}; this helper did not launch it, so it will not drive it`);
	return JSON.parse(await readFile(file, "utf8")) as RunState;
}

async function saveState(state: RunState) {
	await writeFile(join(stateRoot, state.runId, "state.json"), JSON.stringify(state, null, 2));
}

async function launch() {
	const runId = flag("run") ?? `${new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14)}-${process.pid}`;
	if (!/^[a-zA-Z0-9-]{1,48}$/.test(runId)) throw new Error("run id must be 1-48 letters, digits, or hyphens");
	const runDir = join(stateRoot, runId);
	const evidenceDir = join(root, "e2e", "artifacts", `verify-${runId}`);
	if (existsSync(runDir) || existsSync(evidenceDir)) throw new Error(`run ${runId} already exists; choose a new --run id`);
	// A non-recursive mkdir claims the run. A second launch with the same id fails here, before it starts anything or
	// registers a cleanup that would remove this run's directory.
	await mkdir(stateRoot, { recursive: true });
	await mkdir(runDir);
	// PID 0 means not started yet; cleanup skips it.
	const state: RunState = { runId, head: gitHead(), url: "", vitePid: 0, chromePid: 0, cdpUrl: "", evidenceDir };
	// Vite and Chrome are detached, so an interrupt during startup runs the same cleanup as a failed launch. The
	// handlers exist before anything starts; a signal without a handler would end launch and leave Vite running.
	let stopping: Promise<void> | undefined;
	const abandon = () => (stopping ??= cleanup(state));
	const interrupt = (signal: NodeJS.Signals) => {
		console.error(`launch interrupted by ${signal}; cleaning up run ${runId}`);
		void abandon().finally(() => process.exit(1));
	};
	process.on("SIGINT", interrupt);
	process.on("SIGTERM", interrupt);
	try {
		await mkdir(join(runDir, "chrome-profile"));
		await mkdir(evidenceDir, { recursive: true });
		const url = `http://127.0.0.1:${await freePort()}/`;
		state.url = url;
		// A log file, not a pipe, receives output so Vite survives this helper exiting.
		const log = Bun.file(join(evidenceDir, "vite.log"));
		// Once cleanup has started, start nothing it would miss. Each check runs in the same tick as its spawn.
		if (stopping) throw new Error("launch interrupted before Vite started");
		// `--port` and `--strictPort` override vite.config.ts, so a server already on 1430 is never touched.
		const vite = Bun.spawn(["bun", "--bun", "vite", "--host", "127.0.0.1", "--port", new URL(url).port, "--strictPort"], { cwd: root, detached: true, stdin: "ignore", stdout: log, stderr: log, env: { ...process.env, KUBECONFIG: undefined, TAURI_DEV_HOST: undefined } });
		Object.assign(state, { vitePid: vite.pid, viteStartedAt: startIdentity(vite.pid) });
		await saveState(state);
		if (stopping) throw new Error("launch interrupted before Chrome started");
		const chrome = startChrome(join(runDir, "chrome-profile"));
		Object.assign(state, { chromePid: chrome.pid, chromeStartedAt: chrome.startedAt });
		[state.cdpUrl] = await Promise.all([chrome.cdpUrl, saveState(state)]);
		await saveState(state);
		await waitFor("Vite to serve the KubeCove index", async (signal) => (await (await fetch(url, { signal })).text()).includes("<title>KubeCove</title>"));
		await withPage(state, async (page) => {
			await page.goto(url);
			// Ready means usable: the launcher has loaded the browser mock kubeconfig context.
			await page.getByText("mock-dev", { exact: true }).first().waitFor();
		});
		// Inside the try: a launch that cannot record itself fails and stops what it started.
		await record(state, { action: "launch", url: state.url, head: state.head });
	} catch (error) {
		console.error(`launch failed; cleaning up run ${runId}. Vite log: ${join(evidenceDir, "vite.log")}`);
		await abandon();
		throw error;
	} finally {
		process.off("SIGINT", interrupt);
		process.off("SIGTERM", interrupt);
	}
	console.log(JSON.stringify({ ready: true, runId, url: state.url, evidenceDir }, null, 2));
	console.log(`export KUBECOVE_VERIFY_RUN=${runId}`);
}

async function withPage<T>(state: RunState, work: (page: Page) => Promise<T>) {
	// Chrome holds its DevTools port while it runs. Once the run's Chrome has exited, another browser could hold the
	// recorded port, so the helper connects only while the recorded Chrome still runs.
	if (!ownsLeader(state.chromePid, state.chromeStartedAt)) throw new Error(`run ${state.runId}'s Chrome (PID ${state.chromePid}) is no longer running, or that PID now names another process; run cleanup, then launch a new run`);
	const browser = await chromium.connectOverCDP(state.cdpUrl);
	try {
		const context = browser.contexts()[0];
		const page = context.pages().find((candidate) => candidate.url().startsWith(state.url)) ?? context.pages()[0] ?? (await context.newPage());
		if (flag("timeout")) page.setDefaultTimeout(Number(flag("timeout")));
		return await work(page);
	} finally {
		// Disconnects only; the run's Chrome and its page stay alive for the next command.
		await browser.close();
	}
}

async function record(state: RunState, entry: Record<string, unknown>) {
	await appendFile(join(state.evidenceDir, "actions.jsonl"), `${JSON.stringify({ at: new Date().toISOString(), ...entry })}\n`);
}

function locate(page: Page) {
	const exact = rest.includes("--exact");
	const name = flag("name");
	const scope = flag("within") ? page.locator(required("within")) : page;
	if (flag("role")) return scope.getByRole(required("role") as Role, name === undefined ? {} : { name, exact });
	if (flag("label")) return scope.getByLabel(required("label"), { exact });
	if (flag("text")) return scope.getByText(required("text"), { exact });
	if (flag("selector")) return scope.locator(required("selector"));
	throw new Error("choose a target with --role [--name], --label, --text, or --selector, optionally --within <css>");
}

function target(page: Page): Locator {
	const all = locate(page);
	return flag("nth") ? all.nth(Number(flag("nth"))) : all.first();
}

async function browserCommand(state: RunState, action: string | undefined) {
	await withPage(state, async (page) => {
		const result: Record<string, unknown> = { action };
		if (action === "goto") await page.goto(new URL(flag("path") ?? "/", state.url).toString());
		else if (action === "reload") await page.reload();
		else if (action === "reset") {
			await page.goto(state.url);
			await page.evaluate(() => localStorage.clear());
			await page.reload();
			await page.getByText("mock-dev", { exact: true }).first().waitFor();
		} else if (action === "click") await target(page).click();
		else if (action === "fill") await target(page).fill(required("value"));
		else if (action === "press") await (hasTarget() ? target(page).press(required("key")) : page.keyboard.press(required("key")));
		else if (action === "wait") await target(page).waitFor({ state: (flag("state") as "visible" | "hidden" | "attached" | "detached" | undefined) ?? "visible" });
		else if (action === "count") result.count = await locate(page).count();
		else if (action === "value") result.value = await target(page).inputValue();
		else if (action === "viewport") await page.setViewportSize({ width: Number(required("width")), height: Number(required("height")) });
		else if (action === "snapshot" || action === "screenshot" || action === "text") {
			const path = isAbsolute(required("path")) ? required("path") : join(state.evidenceDir, required("path"));
			await mkdir(dirname(path), { recursive: true });
			const scoped = hasTarget() ? target(page) : page.locator("body");
			if (action === "snapshot") await writeFile(path, await scoped.ariaSnapshot());
			// biome-ignore lint/performance/useDomNodeTextContent: evidence records the rendered text a user sees.
			else if (action === "text") await writeFile(path, await scoped.innerText());
			// A target captures that element, scrolled into view; `--full` captures the whole page and takes no target.
			else if (hasTarget()) {
				if (rest.includes("--full")) throw new Error("screenshot takes a target or --full, not both");
				await scoped.screenshot({ path });
			} else await page.screenshot({ path, fullPage: rest.includes("--full") });
			result.path = path;
		} else throw new Error("browser actions: goto, reload, reset, click, fill, press, wait, count, value, viewport, snapshot, screenshot, text");
		result.title = await page.title();
		result.url = page.url();
		await record(state, { ...result, args: rest });
		console.log(JSON.stringify(result));
	});
}

async function doctor(state: RunState) {
	// The PIDs count only when each still names the process the run started; a reused PID fails.
	const checks: Record<string, boolean> = { vitePid: ownsLeader(state.vitePid, state.viteStartedAt), chromePid: ownsLeader(state.chromePid, state.chromeStartedAt), sameHead: gitHead() === state.head };
	const port = new URL(state.url).port;
	if (process.platform === "linux" && Bun.which("ss")) {
		// `bun --bun vite` starts a child that holds the socket, so compare process groups.
		const listener = /pid=(\d+),/.exec(Bun.spawnSync(["ss", "-ltnpH", `sport = :${port}`]).stdout.toString())?.[1];
		checks.portInRunGroup = listener !== undefined && Bun.spawnSync(["ps", "-o", "pgid=", "-p", listener]).stdout.toString().trim() === String(state.vitePid);
	}
	// A request and its body read share one deadline, so a stalled server fails the check instead of hanging doctor.
	checks.indexServed = await fetch(state.url, { signal: AbortSignal.timeout(readinessTimeoutMs) }).then(async (response) => (await response.text()).includes("<title>KubeCove</title>")).catch(() => false);
	checks.cdp = checks.chromePid && await fetch(`${state.cdpUrl}/json/version`, { signal: AbortSignal.timeout(readinessTimeoutMs) }).then((response) => response.ok).catch(() => false);
	checks.browserMockRuntime = checks.cdp && (await withPage(state, async (page) => page.url().startsWith(state.url) && (await page.evaluate(() => !("__TAURI_INTERNALS__" in window)))).catch(() => false));
	const healthy = Object.values(checks).every(Boolean);
	console.log(JSON.stringify({ healthy, runId: state.runId, url: state.url, launchedAt: state.head, evidenceDir: state.evidenceDir, checks }, null, 2));
	if (!healthy) process.exitCode = 1;
}

async function cleanup(state: RunState) {
	// Stop first: a failed evidence write must not leave the run's processes running.
	// PID 0 means launch was interrupted before that process started. It must never reach a signal: on POSIX,
	// signaling 0 or -0 targets this helper's own process group.
	const stopped = { chrome: !state.chromePid || (await stopGroup(state.chromePid, state.chromeStartedAt)), vite: !state.vitePid || (await stopGroup(state.vitePid, state.viteStartedAt)) };
	await record(state, { action: "cleanup", stopped }).catch((error) => console.error(`could not record cleanup: ${error instanceof Error ? error.message : error}`));
	const done = stopped.chrome && stopped.vite;
	// A group that did not stop keeps the run's state, so cleanup can be retried. It is written again because an
	// interrupted launch may have stopped before its last state write.
	if (done) await rm(join(stateRoot, state.runId), { recursive: true, force: true });
	else await saveState(state).catch((error) => console.error(`could not save run state: ${error instanceof Error ? error.message : error}`));
	console.log(JSON.stringify({ stopped, stateKept: !done, evidenceDir: state.evidenceDir, evidenceKept: existsSync(join(state.evidenceDir, "actions.jsonl")) }, null, 2));
	if (!done) process.exitCode = 1;
}

try {
	if (command === "launch") await launch();
	else if (command === "doctor") await doctor(await loadState(runIdFromArgs()));
	else if (command === "browser") await browserCommand(await loadState(runIdFromArgs()), rest.shift());
	else if (command === "cleanup") await cleanup(await loadState(runIdFromArgs()));
	else throw new Error("usage: kubecove-verify launch [--run <id>] | doctor | browser <action> [target flags] | cleanup, each later command with [--run <id>]");
} catch (error) {
	console.error(error instanceof Error ? error.message.split("\n").slice(0, 6).join("\n") : String(error));
	process.exitCode = 1;
}
process.exit();
