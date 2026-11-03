// Process, port, and Chrome helpers shared by the verify-kubecove scripts.
import { existsSync, readdirSync, readFileSync } from "node:fs";
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

// Signal 0 checks existence only; a negative PID checks a process group. EPERM means it exists but belongs to another user.
export function alive(pid: number) {
	try {
		process.kill(pid, 0);
		return true;
	} catch (error) {
		return (error as NodeJS.ErrnoException).code === "EPERM";
	}
}

// A process group can outlive its leader, so a stop waits for the whole group.
function groupAlive(pid: number) {
	return alive(process.platform === "win32" ? pid : -pid);
}

const bootId = () => readFileSync("/proc/sys/kernel/random/boot_id", "utf8").trim();

// The OS reuses PIDs, so a run also records each leader's start time and checks it before signaling.
export function startIdentity(pid: number): string | undefined {
	try {
		if (process.platform === "linux") {
			// Field 22 of /proc/<pid>/stat is the start time in clock ticks since boot; the boot ID makes it unique across reboots.
			const stat = readFileSync(`/proc/${pid}/stat`, "utf8");
			const ticks = stat.slice(stat.lastIndexOf(")") + 2).split(" ")[19];
			return ticks ? `${bootId()}:${ticks}` : undefined;
		}
		const command = process.platform === "win32" ? ["powershell", "-NoProfile", "-Command", `(Get-Process -Id ${pid}).StartTime.ToFileTimeUtc()`] : ["ps", "-o", "lstart=", "-p", String(pid)];
		return Bun.spawnSync(command, { stderr: "ignore" }).stdout.toString().trim() || undefined;
	} catch {
		return undefined;
	}
}

// True only when `pid` runs and is the process the run started, by the start identity recorded at spawn. A bare PID
// check would accept a reused PID.
export function ownsLeader(pid: number, startedAt: string | undefined) {
	return startedAt !== undefined && alive(pid) && startIdentity(pid) === startedAt;
}

// Polls `check` until it returns true. Each check gets a signal that aborts at the deadline, and the deadline holds
// even when a check ignores the signal.
export async function waitFor(description: string, check: (signal: AbortSignal) => Promise<boolean>, timeoutMs = readinessTimeoutMs) {
	const signal = AbortSignal.timeout(timeoutMs);
	const expired = new Promise<false>((done) => signal.addEventListener("abort", () => done(false), { once: true }));
	do {
		if (await Promise.race([check(signal).catch(() => false), expired])) return;
		await Promise.race([Bun.sleep(pollMs), expired]);
	} while (!signal.aborted);
	throw new Error(`timed out after ${timeoutMs} ms waiting for ${description}`);
}

// Signals only the detached process group this run created, as `e2e/harness/desktop-profile.ts` does. `startedAt` is
// the leader's startIdentity at spawn. Returns true once none of the run's processes remain. Returns false when they
// still run, or when ownership cannot be established, so the caller keeps its record for a retry.
export async function stopGroup(pid: number, startedAt: string | undefined) {
	// On Windows a PID names one process, not a group, and `taskkill /T` finds children only through a live parent. A
	// leader that exited before this stop may have left children running, so there its absence does not prove the
	// run stopped.
	const leaderGoneMeansStopped = process.platform !== "win32";
	if (!groupAlive(pid)) return leaderGoneMeansStopped;
	if (startedAt === undefined) return false;
	// Nothing a run starts survives a reboot, so a record from an earlier boot names someone else's group.
	if (process.platform === "linux" && !startedAt.startsWith(`${bootId()}:`)) return true;
	if (alive(pid)) {
		const leader = startIdentity(pid);
		if (leader === undefined) return false;
		// The PID now names a process this run did not start, so the run's own leader is gone. Signal nothing.
		if (leader !== startedAt) return leaderGoneMeansStopped;
	}
	// When the leader is gone but its group lives: within one boot, `fork()` never gives a new process the ID of a live
	// process group, so the members are the run's own. The exception needs the run's whole group to end and PID
	// numbering to wrap around to this exact ID for a new group whose leader then also exits.
	if (process.platform === "win32") await Bun.spawn(["taskkill", "/PID", String(pid), "/T", "/F"], { stdout: "ignore", stderr: "ignore" }).exited;
	else {
		try {
			process.kill(-pid, "SIGTERM");
		} catch {}
	}
	return waitFor(`process group ${pid} to exit`, async () => !groupAlive(pid)).then(
		() => true,
		() => false,
	);
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
export function startChrome(profileDir: string) {
	const chrome = Bun.spawn([chromeExecutable(), "--headless=new", "--no-first-run", "--no-default-browser-check", "--disable-gpu", `--user-data-dir=${profileDir}`, "--remote-debugging-address=127.0.0.1", "--remote-debugging-port=0", "--window-size=1440,900", "about:blank"], { detached: true, stdin: "ignore", stdout: "ignore", stderr: "ignore" });
	const portFile = join(profileDir, "DevToolsActivePort");
	// The PID is returned before Chrome is ready, so the caller can record it and stop Chrome if startup fails or is interrupted.
	const cdpUrl = waitFor("Chrome DevTools port", async () => existsSync(portFile)).then(async () => `http://127.0.0.1:${(await readFile(portFile, "utf8")).split(/\r?\n/)[0]}`);
	return { pid: chrome.pid, startedAt: startIdentity(chrome.pid), cdpUrl };
}

export function gitHead() {
	const head = Bun.spawnSync(["git", "rev-parse", "HEAD"], { cwd: root }).stdout.toString().trim();
	const dirty = Bun.spawnSync(["git", "status", "--porcelain"], { cwd: root }).stdout.toString().trim() ? "+dirty" : "";
	return `${head}${dirty}`;
}
