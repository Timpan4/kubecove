import { join, resolve } from "node:path";
import { cleanEnvironment, processOutput, trackChild } from "./lifecycle-processes";
import { readinessTimeoutMs } from "../../.agents/skills/verify-kubecove/scripts/processes";

export function serverBinary(release = false, target = resolve("src-tauri/server/target")) {
	return join(target, release ? "release" : "debug", process.platform === "win32" ? "kubecove-server.exe" : "kubecove-server");
}

// Consume every line. The server prints its one-time login URL on stdout, so neither
// stream may be inherited or written to a driver log.
export function startServer(binary: string, dataDir: string, env: Record<string, string | undefined>, signal?: AbortSignal) {
	signal?.throwIfAborted();
	const child = Bun.spawn([binary, "--data-dir", dataDir], {
		detached: process.platform !== "win32", env: cleanEnvironment(env), stdout: "pipe", stderr: "ignore",
	});
	const ready = Promise.withResolvers<{ origin: string; token: string }>();
	const tracked = trackChild(child, async (drain) => {
		let pending = "";
		for await (const chunk of processOutput(child.stdout, drain)) {
			pending += new TextDecoder().decode(chunk);
			const lines = pending.split("\n");
			pending = lines.pop() ?? "";
			for (const line of lines) {
				const match = /^KubeCove login: (http:\/\/127\.0\.0\.1:\d+)\/#token=(\S+)\s*$/.exec(line);
				if (match) ready.resolve({ origin: match[1], token: match[2] });
			}
		}
		return child.exited;
	});
	const stop = () => tracked.stop("SIGTERM");
	const abort = () => { ready.reject(new Error("Server startup interrupted")); void stop().catch(() => {}); };
	signal?.addEventListener("abort", abort, { once: true });
	const timer = setTimeout(() => ready.reject(new Error("Server readiness timed out")), readinessTimeoutMs);
	const exited = tracked.done.finally(() => signal?.removeEventListener("abort", abort));
	void exited.then(() => ready.reject(new Error("Server exited before readiness")), ready.reject);
	return { ready: ready.promise.finally(() => clearTimeout(timer)), exited, stop };
}
