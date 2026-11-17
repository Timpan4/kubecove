import { existsSync } from "node:fs";
import { readFile, readlink, writeFile } from "node:fs/promises";
import { getPriority } from "node:os";
import { basename, join } from "node:path";

export type FrameIsolation =
	| { state: "held"; path: string; release: () => Promise<void> }
	| { state: "busy" }
	| { state: "unavailable" };

/**
 * A lock between parity runs and frame measurements on one machine, in every checkout. Parity
 * holds it shared for its whole run, and each frame command holds it exclusively while its child
 * runs, so a parity run cannot start and finish unseen inside a frame case that prints nothing.
 * Linux only, like the process check below: it needs util-linux `flock` and XDG_RUNTIME_DIR,
 * which is per user and machine-wide, unlike a checkout's TMPDIR. Without them it returns
 * "unavailable" and the process check still applies.
 *
 * A shared request waits for a running measurement; `signal` stops that wait. An exclusive
 * request never waits and returns "busy" instead.
 */
export async function holdFrameIsolation(
	mode: "shared" | "exclusive",
	signal?: AbortSignal,
): Promise<FrameIsolation> {
	const flock = Bun.which("flock");
	const runtime = process.env.XDG_RUNTIME_DIR;
	if (process.platform !== "linux" || !flock || !runtime || !existsSync(runtime)) return { state: "unavailable" };
	const path = join(runtime, "kubecove-frame-isolation.lock");
	// One child makes every attempt, so while it waits it is the only process naming the lock
	// path, and a frame command can tell this waiting runner from one that runs drivers.
	// --close: only flock keeps the lock open, so nothing the holder starts can keep it.
	// The holder reads stdin until release, or until this process exits and closes it.
	const hold = (options: string) => `exec "$1" ${options} --close "$0" sh -c "echo held; exec cat >/dev/null"`;
	const script = mode === "shared"
		? `"$1" --nonblock --shared "$0" true || echo waiting; ${hold("--shared")}`
		: hold("--exclusive --nonblock");
	// Its own process group keeps a terminal Ctrl-C away from flock, so the lock lasts until
	// this process releases it after its own cleanup.
	const holder = Bun.spawn(["sh", "-c", script, path, flock], {
		stdin: "pipe", stdout: "pipe", stderr: "inherit", detached: true,
	});
	const stop = () => holder.kill();
	signal?.addEventListener("abort", stop, { once: true });
	try {
		const reader = holder.stdout.getReader();
		const decoder = new TextDecoder();
		let output = "";
		for (;;) {
			const { done, value } = await reader.read();
			if (done) break;
			output += decoder.decode(value, { stream: true });
			if (output.includes("waiting\n")) {
				output = output.replace("waiting\n", "");
				console.log(`Waiting for a frame measurement to release ${path}`);
			}
			if (output.includes("held\n")) {
				reader.releaseLock();
				return { state: "held", path, release: async () => { holder.stdin.end(); await holder.exited; } };
			}
		}
		const code = await holder.exited;
		signal?.throwIfAborted();
		// flock --nonblock exits 1 without starting the holder when the lock is taken.
		if (mode === "exclusive" && code === 1) return { state: "busy" };
		throw new Error(`flock ${path} exited ${code} before taking the lock`);
	} finally {
		signal?.removeEventListener("abort", stop);
	}
}

export function isParityProcess(executable: string, argv: string[]): boolean {
	// procfs marks an unlinked executable while its process is still running.
	const name = basename(executable).replace(/ \(deleted\)$/, "");
	if (name.startsWith("parity-") && basename(argv[0] ?? "") === name) return true;
	const script = argv[argv[1] === "run" ? 2 : 1];
	return name === "bun" && script !== undefined &&
		(script === "scripts/e2e-parity.ts" || script.endsWith("/scripts/e2e-parity.ts"));
}

/**
 * Running parity processes. With `heldLock`, the isolation lock this process holds exclusively,
 * a parity runner whose child is requesting that lock is waiting for it, not running drivers,
 * so it is left out. A runner that does not take the lock is still reported.
 */
async function parityProcesses(heldLock?: string): Promise<string[]> {
	if (process.platform !== "linux") return [];
	const child = Bun.spawn(["ps", "-eo", "pid=,ppid=,stat=,comm="], { stdout: "pipe", stderr: "pipe" });
	const [code, output, error] = await Promise.all([
		child.exited, new Response(child.stdout).text(), new Response(child.stderr).text(),
	]);
	if (code !== 0) throw new Error(`Cannot verify parity isolation: ps exited ${code}: ${error}`);
	const candidates: string[] = [];
	const children = new Map<string, string[]>();
	for (const line of output.split("\n")) {
		const [pid, parent, state, name] = line.trim().split(/\s+/);
		if (!pid || !parent || state?.startsWith("Z")) continue;
		children.set(parent, [...(children.get(parent) ?? []), pid]);
		if (name === "bun" || name?.startsWith("parity-")) candidates.push(pid);
	}
	// Processes may exit between the snapshot and their procfs reads; such a read gives undefined.
	const procfs = async <Value>(read: () => Promise<Value>) => {
		try { return await read(); }
		catch (error) {
			if (error instanceof Error && "code" in error && (error.code === "ENOENT" || error.code === "ESRCH")) return undefined;
			throw error;
		}
	};
	const argv = async (pid: string) => (await readFile(`/proc/${pid}/cmdline`, "utf8")).split("\0");
	const waiting = async (pid: string) => {
		if (!heldLock) return false;
		const requests = await Promise.all((children.get(pid) ?? []).map((request) => procfs(() => argv(request))));
		return requests.some((command) => command?.includes(heldLock));
	};
	const processes = await Promise.all(candidates.map(async pid => {
		const found = await procfs(async () => {
			const [executable, command] = await Promise.all([readlink(`/proc/${pid}/exe`), argv(pid)]);
			return isParityProcess(executable, command) && !await waiting(pid);
		});
		return found ? [`parity process PID ${pid}`] : [];
	}));
	return processes.flat();
}

export async function runFrameCommand(
	command: string[],
	env: Record<string, string | undefined>,
	log: string,
	guardParity: boolean,
) {
	const isolation: FrameIsolation = guardParity ? await holdFrameIsolation("exclusive") : { state: "unavailable" };
	if (isolation.state === "busy") throw new Error("Frame measurement blocked: a parity run or another frame measurement holds the isolation lock");
	const heldLock = isolation.state === "held" ? isolation.path : undefined;
	try {
		if (guardParity) {
			const parity = await parityProcesses(heldLock);
			if (parity.length) throw new Error(`Frame measurement blocked by parity processes: ${parity.join(", ")}`);
		}
		return await runIsolated(command, env, log, guardParity, heldLock);
	} finally {
		if (isolation.state === "held") await isolation.release();
	}
}

async function runIsolated(
	command: string[],
	env: Record<string, string | undefined>,
	log: string,
	guardParity: boolean,
	heldLock: string | undefined,
) {
	const child = Bun.spawn(command, {
		env,
		detached: process.platform !== "win32",
		stdout: "pipe",
		stderr: "pipe",
	});
	let priority: number | null = null;
	try { priority = getPriority(child.pid); } catch { /* Optional process metadata. */ }
	const interrupt = () => {
		if (process.platform === "win32") child.kill("SIGTERM");
		else {
			try { process.kill(-child.pid, "SIGTERM"); }
			catch (error) {
				if (!(error instanceof Error && "code" in error && error.code === "ESRCH")) throw error;
			}
		}
	};
	process.once("SIGINT", interrupt);
	process.once("SIGTERM", interrupt);
	let contamination: string | undefined;
	const checkParity = async () => {
		try {
			const parity = await parityProcesses(heldLock);
			if (parity.length) contamination = `Concurrent parity processes: ${parity.join(", ")}`;
		} catch (error) { contamination = String(error); }
		if (contamination) interrupt();
	};
	let code: number;
	let stdout: string;
	let stderr: string;
	try {
		const collect = async (stream: ReadableStream<Uint8Array>, echo: boolean) => {
			const decoder = new TextDecoder();
			let text = "";
			for await (const chunk of stream) {
				const part = decoder.decode(chunk, { stream: true });
				text += part;
				if (echo) {
					process.stdout.write(part);
					if (guardParity && !contamination) await checkParity();
				}
			}
			return text + decoder.decode();
		};
		[code, stdout, stderr] = await Promise.all([
			child.exited, collect(child.stdout, true), collect(child.stderr, false),
		]);
		if (guardParity && !contamination) await checkParity();
	} catch (error) {
		interrupt();
		await child.exited;
		throw error;
	} finally {
		process.removeListener("SIGINT", interrupt);
		process.removeListener("SIGTERM", interrupt);
	}
	await writeFile(log, `${stdout}\n${stderr}${contamination ? `\nINVALID: ${contamination}\n` : ""}`);
	if (contamination) throw new Error(`Frame measurement invalid: ${contamination}; see ${log}`);
	if (code !== 0) throw new Error(`${command.join(" ")} exited ${code}; see ${log}`);
	return { stderr, priority };
}
