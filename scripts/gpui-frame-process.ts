import { readFile, readlink, writeFile } from "node:fs/promises";
import { getPriority } from "node:os";
import { basename, isAbsolute, join } from "node:path";

export type FrameIsolation =
	| { state: "held"; path: string; release: () => Promise<void> }
	| { state: "busy" }
	| { state: "unavailable" };

/**
 * A lock between parity runs and frame measurements on one machine, in every checkout. Parity
 * holds it shared through driver cleanup. Each guarded frame command holds it exclusively through
 * its final process scan and log write, so a parity run cannot go unseen inside a silent case.
 * Linux requires util-linux `flock` and a per-user XDG_RUNTIME_DIR shared across checkouts.
 * A process scan alone can miss a short parity run, so a Linux lock failure refuses the run.
 * Other platforms return "unavailable".
 *
 * A shared request waits for a running measurement; `signal` stops that wait. An exclusive
 * request never waits and returns "busy" instead.
 */
export async function holdFrameIsolation(
	mode: "shared" | "exclusive",
	signal?: AbortSignal,
): Promise<FrameIsolation> {
	if (process.platform !== "linux") return { state: "unavailable" };
	signal?.throwIfAborted();
	const runtime = process.env.XDG_RUNTIME_DIR;
	if (!runtime || !isAbsolute(runtime)) throw new Error("Cannot establish frame isolation: XDG_RUNTIME_DIR must be an absolute path");
	const flock = Bun.which("flock");
	if (!flock) throw new Error("Cannot establish frame isolation: util-linux flock is required on PATH");
	const shell = Bun.which("sh");
	if (!shell) throw new Error("Cannot establish frame isolation: sh is required on PATH");
	const path = join(runtime, "kubecove-frame-isolation.lock");
	// The direct child names the lock path during both attempts, so the frame scan can recognize
	// a parity runner that is waiting to start its drivers.
	// --close: only flock keeps the lock open, so nothing the holder starts can keep it.
	// Only EOF arrives on stdin, either from release or when this process exits.
	const hold = `"$2" -c 'printf "held\\n"; IFS= read -r line || :'`;
	// util-linux reserves the configured status for contention. 75 is EX_TEMPFAIL.
	const conflict = 75;
	const nonblocking = `"$1" --nonblock --conflict-exit-code ${conflict} --${mode} --close "$0" ${hold}`;
	const script = mode === "shared"
		? `${nonblocking}
status=$?
if [ "$status" -ne ${conflict} ]; then exit "$status"; fi
printf 'waiting\\n'
exec "$1" --shared --close "$0" ${hold}`
		: `exec ${nonblocking}`;
	// Its own process group keeps a terminal Ctrl-C away from flock, so the lock lasts until
	// this process releases it after its own cleanup.
	const holder = Bun.spawn([shell, "-c", script, path, flock, shell], {
		stdin: "pipe", stdout: "pipe", stderr: "pipe", detached: true,
	});
	const exited = Promise.all([holder.exited, new Response(holder.stderr).text()]);
	// The whole group, so a holder that took the lock as the abort arrived cannot keep it.
	const stop = () => {
		holder.stdin.end();
		try { process.kill(-holder.pid, "SIGTERM"); }
		catch (error) {
			if (!(error instanceof Error && "code" in error && error.code === "ESRCH")) throw error;
		}
	};
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
				return { state: "held", path, release: async () => { holder.stdin.end(); await exited; } };
			}
		}
		const [code, stderr] = await exited;
		signal?.throwIfAborted();
		if (mode === "exclusive" && code === conflict) return { state: "busy" };
		const diagnostic = stderr.trim();
		throw new Error(`Cannot establish frame isolation at ${path}: flock exited ${code} before taking the lock${diagnostic ? `: ${diagnostic}` : ""}`);
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
