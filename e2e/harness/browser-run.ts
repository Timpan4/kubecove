import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { stringify } from "yaml";
import type { startMemoryFixture } from "./memory-fixture";
import { serverBinary, startServer } from "./server-process";

type Execute = (command: string, args: string[], env: Record<string, string | undefined>, capture?: boolean, logFile?: string) => Promise<{ stdout: string }>;
type RunWdio = ReturnType<typeof createWdioRunner>;

export function createWdioRunner(root: string, spec: string | undefined, execute: Execute) {
	return async (config: string, extra: Record<string, string | undefined>, logFile?: string) => {
		const cli = join(root, "node_modules", "@wdio", "cli", "bin", "wdio.js");
		const command = [cli, "run", config, ...(spec ? ["--spec", spec] : [])];
		const version = (await execute("node", ["--version"], {}, true)).stdout.trim();
		const match = /^v(\d+)\.(\d+)\.(\d+)/.exec(version);
		if (!match) throw new Error(`could not parse Node version ${version}`);
		const [major, minor] = match.slice(1, 3).map(Number);
		if ((major === 18 && minor >= 20) || (major > 18 && major <= 24)) return execute("node", command, extra, false, logFile);
		if (major < 18 || (major === 18 && minor < 20)) throw new Error(`WDIO requires Node 18.20-24; found ${version}`);
		const fnm = Bun.which("fnm");
		if (!fnm) throw new Error(`WDIO requires Node 18-24; found ${version} and no fnm fallback`);
		return execute(fnm, ["exec", "--using", "24", "node", ...command], extra, false, logFile);
	};
}

export async function buildBrowserServer(root: string, artifacts: string, profiling: boolean, execute: Execute) {
	const target = join(root, "src-tauri/server/target");
	await execute("bun", ["--bun", "vite", "build"], {
		KUBECOVE_PUBLIC_RUNTIME: "web", KUBECOVE_PUBLIC_PROFILE: String(profiling),
		KUBECOVE_FRONTEND_BUNDLE_REPORT: profiling ? join(artifacts, "frontend-bundle.json") : process.env.KUBECOVE_FRONTEND_BUNDLE_REPORT,
	});
	await execute("cargo", ["build", "--locked", "--manifest-path", "src-tauri/server/Cargo.toml", "--target-dir", target, ...(profiling ? ["--release"] : [])], {}, false, join(artifacts, "build.log"));
	await mkdir(artifacts, { recursive: true });
	return serverBinary(profiling, target);
}

export async function runServerWdio(binary: string, dataDir: string, kubeconfig: string, env: Record<string, string | undefined>, artifacts: string, signal: AbortSignal, runWdio: RunWdio) {
	await withBrowserServer(binary, dataDir, kubeconfig, signal, async (server) => {
		await runWdio("e2e/wdio.real.conf.ts", { ...server, ...env, KUBECOVE_E2E_ARTIFACTS: artifacts }, join(artifacts, "wdio.log"));
	});
}

export async function runDevBrowser(binary: string, sessionsDir: string, kubeconfig: string, signal: AbortSignal) {
	await mkdir(sessionsDir, { recursive: true });
	const dataDir = await mkdtemp(join(sessionsDir, "session-"));
	try {
		await withBrowserServer(binary, dataDir, kubeconfig, signal, async (env, exited) => {
			const { chromium } = await import("playwright-core");
			const { chromeExecutable } = await import("../../.agents/skills/verify-kubecove/scripts/processes");
			const chrome = await chromium.launchPersistentContext(env.E2E_FAST_CHROME_PROFILES, { executablePath: chromeExecutable(), headless: true });
			try {
				const page = chrome.pages()[0] ?? await chrome.newPage();
				const token = (await readFile(env.KUBECOVE_SERVER_TOKEN_FILE, "utf8")).trim();
				await page.goto(`${env.KUBECOVE_SERVER_ORIGIN}/api/v1/auth`);
				await page.evaluate(async (token) => {
					const response = await fetch("/api/v1/session", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ token }) });
					if (!response.ok) throw new Error("Dev session login failed");
					sessionStorage.setItem("kubecove-browser-resume", (await response.json()).resumeToken);
				}, token);
				await page.goto(env.KUBECOVE_SERVER_ORIGIN);
				await page.locator("#workspace-name").waitFor();
				console.log(`Kind workspace ready at ${env.KUBECOVE_SERVER_ORIGIN}. Login token file: ${env.KUBECOVE_SERVER_TOKEN_FILE}`);
				if (await exited !== 0 && !signal.aborted) throw new Error("Kind server stopped unexpectedly");
			} finally { await chrome.close(); }
		});
	} finally { await rm(dataDir, { recursive: true, force: true }); }
}

export async function writeSmokeConfig(directory: string, cluster: string, fixture?: Awaited<ReturnType<typeof startMemoryFixture>>) {
	const roles = ["admin", "restricted"];
	const path = join(directory, "kubeconfig");
	const config = {
		apiVersion: "v1", kind: "Config",
		clusters: [{ name: cluster, cluster: fixture ? { server: fixture.url, "certificate-authority-data": fixture.authority } : { server: "https://127.0.0.1:65535" } }],
		contexts: roles.map((role) => ({ name: `${cluster}-${role}`, context: { cluster, user: `${cluster}-${role}` } })),
		"current-context": `${cluster}-admin`,
		users: roles.map((name) => ({ name: `${cluster}-${name}`, user: { token: "redacted-smoke-value" } })),
	};
	await writeFile(path, stringify(config), { mode: 0o600, flag: "wx" });
	return path;
}

export async function withBrowserServer(binary: string, dataDir: string, kubeconfig: string, signal: AbortSignal,
	drive: (env: Record<string, string>, exited: Promise<number>) => Promise<void>) {
	await mkdir(dataDir, { mode: 0o700, recursive: true });
	await chmod(dataDir, 0o700);
	const profile = await mkdtemp(join(tmpdir(), "kc-chrome-"));
	try {
		const server = startServer(binary, dataDir, { KUBECONFIG: kubeconfig, HOME: dataDir, USERPROFILE: dataDir,
			XDG_CONFIG_HOME: dataDir, KUBECOVE_E2E: undefined, DBUS_SESSION_BUS_ADDRESS: undefined }, signal);
		try {
			const { origin } = await server.ready;
			signal.throwIfAborted();
			await drive({ KUBECOVE_SERVER_ORIGIN: origin, KUBECOVE_SERVER_TOKEN_FILE: join(dataDir, "server-token"),
				KUBECONFIG: kubeconfig, E2E_FAST_CHROME_PROFILES: profile }, server.exited);
		} finally { await server.stop(); }
	} finally { await rm(profile, { recursive: true, force: true }); }
}
