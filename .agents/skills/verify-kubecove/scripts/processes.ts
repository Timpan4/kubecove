// Process, port, and Chrome helpers shared by the verify-kubecove scripts.
import { existsSync, readdirSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { createServer } from "node:net";
import { join, resolve } from "node:path";
import { chromium } from "playwright-core";

export const root = resolve(import.meta.dir, "../../../..");
// Reuses the fast E2E suite's 30 s Mocha budget (`e2e/wdio.fast.conf.ts`) and the lifecycle runner's 1 s poll.
export const readinessTimeoutMs = 30_000;
const pollMs = 1_000;

export async function freePort() {
	return new Promise<number>((done, fail) => {
		const server = createServer();
		server.once("error", fail);
		server.listen(0, "127.0.0.1", () => {
			const address = server.address();
			server.close(() => (typeof address === "object" && address ? done(address.port) : fail(new Error("no free port"))));
		});
	});
}

export function alive(pid: number) {
	try {
		process.kill(pid, 0);
		return true;
	} catch {
		return false;
	}
}

export async function waitFor(description: string, check: () => Promise<boolean>, timeoutMs = readinessTimeoutMs) {
	const deadline = Date.now() + timeoutMs;
	do {
		if (await check().catch(() => false)) return;
		await Bun.sleep(pollMs);
	} while (Date.now() < deadline);
	throw new Error(`timed out after ${timeoutMs} ms waiting for ${description}`);
}

// Signals only the detached process group this run created, as `e2e/harness/desktop-profile.ts` does.
export async function stopGroup(pid: number) {
	if (!alive(pid)) return true;
	if (process.platform === "win32") await Bun.spawn(["taskkill", "/PID", String(pid), "/T", "/F"], { stdout: "ignore", stderr: "ignore" }).exited;
	else {
		try {
			process.kill(-pid, "SIGTERM");
		} catch {}
	}
	return waitFor(`PID ${pid} to exit`, async () => !alive(pid)).then(() => true, () => false);
}

// Chrome comes from KUBECOVE_VERIFY_CHROME, Playwright's pinned revision, or the newest cached Playwright Chromium.
export function chromeExecutable() {
	const override = process.env.KUBECOVE_VERIFY_CHROME;
	if (override) return override;
	const pinned = chromium.executablePath();
	if (existsSync(pinned)) return pinned;
	const match = /^(.*[\\/])chromium-(\d+)([\\/].*)$/.exec(pinned);
	if (match && existsSync(match[1])) {
		const revisions = readdirSync(match[1]).flatMap((name) => /^chromium-(\d+)$/.exec(name)?.[1] ?? []).map(Number).sort((a, b) => b - a);
		for (const revision of revisions) {
			const candidate = `${match[1]}chromium-${revision}${match[3]}`;
			if (existsSync(candidate)) return candidate;
		}
	}
	throw new Error(`no Chrome found; set KUBECOVE_VERIFY_CHROME or run \`bunx playwright-core install chromium\` (expected ${pinned})`);
}

// Headless Chrome never opens a window on the user's desktop; the profile directory isolates storage.
export async function startChrome(profileDir: string) {
	const chrome = Bun.spawn([chromeExecutable(), "--headless=new", "--no-first-run", "--no-default-browser-check", "--disable-gpu", `--user-data-dir=${profileDir}`, "--remote-debugging-address=127.0.0.1", "--remote-debugging-port=0", "--window-size=1440,900", "about:blank"], { detached: true, stdin: "ignore", stdout: "ignore", stderr: "ignore" });
	const portFile = join(profileDir, "DevToolsActivePort");
	try {
		await waitFor("Chrome DevTools port", async () => existsSync(portFile));
	} catch (error) {
		await stopGroup(chrome.pid);
		throw error;
	}
	return { pid: chrome.pid, cdpUrl: `http://127.0.0.1:${(await readFile(portFile, "utf8")).split(/\r?\n/)[0]}` };
}

export function gitHead() {
	const head = Bun.spawnSync(["git", "rev-parse", "HEAD"], { cwd: root }).stdout.toString().trim();
	const dirty = Bun.spawnSync(["git", "status", "--porcelain"], { cwd: root }).stdout.toString().trim() ? "+dirty" : "";
	return `${head}${dirty}`;
}
