import { createServer } from "node:net";

export async function stopOwnedProfileProcess(child: Bun.Subprocess, signal: "SIGINT" | "SIGTERM"): Promise<void> {
	if (child.exitCode !== null || child.signalCode !== null) return;
	if (process.platform === "win32") {
		await Bun.spawn(["taskkill", "/PID", String(child.pid), "/T", "/F"], { stdout: "ignore", stderr: "ignore" }).exited;
	} else {
		try {
			// Only the detached process group created by this profiling run is signalled.
			process.kill(-child.pid, signal);
		} catch (error) {
			if (!(error instanceof Error && "code" in error && error.code === "ESRCH")) throw error;
		}
	}
}

export function hasRunningApp(processNames: string): boolean {
	return processNames.split(/\r?\n/).some((line) => /(?:^|[/\\])kubecove(?:\.exe)?$/i.test(line.trim()));
}

export async function assertPortAvailable(port: number): Promise<void> {
	await new Promise<void>((resolve, reject) => {
		const server = createServer();
		server.once("error", () => reject(new Error(`Port ${port} is occupied; profiling will not stop or reuse an existing server`)));
		server.listen({ port, host: "127.0.0.1", exclusive: true }, () => server.close(() => resolve()));
	});
}

export async function assertDesktopProfileIdle(): Promise<void> {
	const command = process.platform === "win32"
		? ["powershell", "-NoProfile", "-NonInteractive", "-Command", "Get-Process | Select-Object -ExpandProperty ProcessName"]
		: ["ps", "-A", "-o", "comm="];
	const child = Bun.spawn(command, { stdout: "pipe", stderr: "ignore" });
	const names = await new Response(child.stdout).text();
	if (await child.exited !== 0) throw new Error("Could not check running apps; profiling stopped");
	if (hasRunningApp(names)) throw new Error("KubeCove is already running; profiling requires an isolated launch and will not stop it");
	for (const port of [1420, 1430, 4445]) await assertPortAvailable(port);
}
