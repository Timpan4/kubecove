import { join, resolve } from "node:path";
import { getDefaultHighWaterMark } from "node:stream";
import { readinessTimeoutMs } from "../../.agents/skills/verify-kubecove/scripts/processes";
import { redactSensitiveErrorDetail } from "../../src/lib/error-redaction";
import { safeDiagnosticText } from "./diagnostics";
import { captureProcessOutput, cleanEnvironment, processOutput, trackChild } from "./lifecycle-processes";

export function serverBinary(release = false, target = resolve("src-tauri/server/target")) {
	return join(target, release ? "release" : "debug", process.platform === "win32" ? "kubecove-server.exe" : "kubecove-server");
}

// Consume every line. The server prints its one-time login URL on stdout, so neither
// stream may be inherited or written to a driver log.
export function startServer(binary: string, dataDir: string, env: Record<string, string | undefined>, signal?: AbortSignal) {
	signal?.throwIfAborted();
	const child = Bun.spawn([binary, "--data-dir", dataDir], {
		detached: process.platform !== "win32", env: cleanEnvironment(env), stdout: "pipe", stderr: "pipe",
	});
	// Cap retained stderr at the Node byte-stream buffer budget while continuing to drain the pipe.
	const stderr = captureProcessOutput(child.stderr, getDefaultHighWaterMark(false));
	const ready = Promise.withResolvers<{ origin: string; token: string }>();
	const readLogin = async (drain: AbortSignal) => {
		let pending = "";
		const decoder = new TextDecoder();
		for await (const chunk of processOutput(child.stdout, drain)) {
			pending += decoder.decode(chunk, { stream: true });
			const lines = pending.split("\n");
			pending = lines.pop() ?? "";
			for (const line of lines) {
				const match = /^KubeCove login: (http:\/\/127\.0\.0\.1:\d+)\/#token=(\S+)\s*$/.exec(line);
				if (match) ready.resolve({ origin: match[1], token: match[2] });
			}
		}
	};
	const tracked = trackChild(child, async (drain) => {
		const [status] = await Promise.all([child.exited, readLogin(drain), stderr.drain(drain)]);
		return status;
	});
	const failure = (message: string) => {
		const detail = redactSensitiveErrorDetail(safeDiagnosticText(stderr.text())).trim();
		return new Error(detail ? `${message}\n${detail}` : message);
	};
	const stop = () => tracked.stop("SIGTERM");
	const abort = () => { ready.reject(new Error("Server startup interrupted")); void stop().catch(() => {}); };
	signal?.addEventListener("abort", abort, { once: true });
	const timer = setTimeout(() => ready.reject(failure("Server readiness timed out")), readinessTimeoutMs);
	const exited = tracked.done.finally(() => signal?.removeEventListener("abort", abort));
	void exited.then(
		(status) => ready.reject(failure(`Server exited (${child.signalCode ?? status}) before readiness`)),
		() => ready.reject(failure("Could not read server startup output")),
	);
	return { ready: ready.promise.finally(() => clearTimeout(timer)), exited, stop };
}
