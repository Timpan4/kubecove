import { readinessTimeoutMs, startIdentity, stopGroup, waitFor } from "../../.agents/skills/verify-kubecove/scripts/processes";
import { signalOwnedProcess } from "./desktop-profile";

export type InterruptSignal = "SIGINT" | "SIGTERM";

export function cleanEnvironment(extra: Record<string, string | undefined>) {
	const env = { ...process.env };
	delete env.KUBECONFIG;
	for (const [name, value] of Object.entries(extra)) {
		if (value === undefined) delete env[name];
		else env[name] = value;
	}
	return env;
}

export function processOutput(stream: ReadableStream<Uint8Array>, signal?: AbortSignal) {
	return signal ? stream.pipeThrough(new TransformStream(), { signal }) : stream;
}

export function captureProcessOutput(stream: ReadableStream<Uint8Array> | undefined) {
	const decoder = new TextDecoder();
	let text = "";
	return {
		async drain(signal?: AbortSignal) {
			if (!stream) return;
			for await (const chunk of processOutput(stream, signal)) text += decoder.decode(chunk, { stream: true });
		},
		text() {
			text += decoder.decode();
			return text;
		},
	};
}

export function trackChild<Result>(child: Bun.Subprocess, complete: (signal: AbortSignal) => Promise<Result>) {
	const identity = process.platform === "win32" ? undefined : startIdentity(child.pid);
	const output = new AbortController();
	const completion = complete(output.signal);
	const { promise: released, resolve: release } = Promise.withResolvers<void>();
	const done = Promise.race([completion, released.then(() => { throw output.signal.reason; })]);
	let stopping: Promise<void> | undefined;
	return {
		done,
		stop(signal: InterruptSignal) {
			stopping ??= (async () => {
				try {
					if (process.platform === "win32") await signalOwnedProcess(child, signal, readinessTimeoutMs);
					else if (!await stopGroup(child.pid, identity) && !await stopGroup(child.pid, identity, "SIGKILL")) {
						throw new Error(`E2E process group ${child.pid} did not stop`);
					}
					// A process outside the group can hold an inherited pipe open after the group exits.
					await waitFor(`E2E process ${child.pid} to exit and close its output`, () => completion.then(() => true, () => true));
				} finally {
					output.abort(new Error(`Stopped waiting for E2E process ${child.pid}`));
					child.unref();
					release();
				}
			})();
			return stopping;
		},
	};
}
