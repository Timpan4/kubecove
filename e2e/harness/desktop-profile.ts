export async function signalOwnedProcess(child: Bun.Subprocess, signal: "SIGINT" | "SIGTERM", timeout?: number): Promise<void> {
	if (child.exitCode !== null || child.signalCode !== null) return;
	if (process.platform === "win32") {
		await Bun.spawn(["taskkill", "/PID", String(child.pid), "/T", "/F"], { stdout: "ignore", stderr: "ignore", timeout, killSignal: "SIGKILL" }).exited;
	} else {
		try {
			// Only the detached process group created by this run is signalled.
			process.kill(-child.pid, signal);
		} catch (error) {
			if (!(error instanceof Error && "code" in error && error.code === "ESRCH")) throw error;
		}
	}
}
