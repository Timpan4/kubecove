import { readFile, readlink, writeFile } from "node:fs/promises";
import { getPriority } from "node:os";
import { basename, join } from "node:path";

/**
 * A lock between parity runs and frame measurements on one machine, in every checkout. Parity
 * holds it shared for its whole run, and each frame command holds it exclusively while its child
 * runs, so a parity run cannot start and finish unseen inside a frame case that prints nothing.
 * Linux only, like the process check below: it needs util-linux `flock` and XDG_RUNTIME_DIR,
 * which is per user and machine-wide, unlike a checkout's TMPDIR. Without them it returns
 * "unavailable" and the process check still applies.
 */
export async function holdFrameIsolation(
	mode: "shared" | "exclusive",
): Promise<{ release: () => Promise<void> } | "busy" | "unavailable"> {
	const flock = Bun.which("flock");
	const runtime = process.env.XDG_RUNTIME_DIR;
	if (process.platform !== "linux" || !flock || !runtime) return "unavailable";
	const path = join(runtime, "kubecove-frame-isolation.lock");
	const attempt = async (wait: boolean) => {
		// --close: only flock keeps the lock open, so nothing the holder starts can keep it.
		// The holder reads stdin until release or until this process exits and closes it.
		const holder = Bun.spawn(
			[flock, `--${mode}`, ...(wait ? [] : ["--nonblock"]), "--close", path, "sh", "-c", "echo held; exec cat >/dev/null"],
			{ stdin: "pipe", stdout: "pipe", stderr: "inherit" },
		);
		const { done } = await holder.stdout.getReader().read();
		if (!done) return { release: async () => { holder.stdin.end(); await holder.exited; } };
		// flock --nonblock exits 1 without starting the holder when the lock is taken.
		const code = await holder.exited;
		if (!wait && code === 1) return "busy" as const;
		throw new Error(`flock ${path} exited ${code} before taking the lock`);
	};
	const held = await attempt(false);
	if (held !== "busy" || mode === "exclusive") return held;
	console.log(`Waiting for a frame measurement to release ${path}`);
	return attempt(true);
}

export function isParityProcess(executable: string, argv: string[]): boolean {
	// procfs marks an unlinked executable while its process is still running.
	const name = basename(executable).replace(/ \(deleted\)$/, "");
	if (name.startsWith("parity-") && basename(argv[0] ?? "") === name) return true;
	const script = argv[argv[1] === "run" ? 2 : 1];
	return name === "bun" && script !== undefined &&
		(script === "scripts/e2e-parity.ts" || script.endsWith("/scripts/e2e-parity.ts"));
}

async function parityProcesses(): Promise<string[]> {
	if (process.platform !== "linux") return [];
	const child = Bun.spawn(["ps", "-eo", "pid=,stat=,comm="], { stdout: "pipe", stderr: "pipe" });
	const [code, output, error] = await Promise.all([
		child.exited, new Response(child.stdout).text(), new Response(child.stderr).text(),
	]);
	if (code !== 0) throw new Error(`Cannot verify parity isolation: ps exited ${code}: ${error}`);
	const candidates = output.split("\n").flatMap(line => {
		const [pid, state, name] = line.trim().split(/\s+/);
		return pid && !state?.startsWith("Z") && (name === "bun" || name?.startsWith("parity-")) ? [pid] : [];
	});
	const processes = await Promise.all(candidates.map(async pid => {
		try {
			const [executable, command] = await Promise.all([
				readlink(`/proc/${pid}/exe`), readFile(`/proc/${pid}/cmdline`, "utf8"),
			]);
			return isParityProcess(executable, command.split("\0")) ? [`parity process PID ${pid}`] : [];
		} catch (error) {
			// Processes may exit between the snapshot and their procfs reads.
			if (error instanceof Error && "code" in error && (error.code === "ENOENT" || error.code === "ESRCH")) return [];
			throw error;
		}
	}));
	return processes.flat();
}

export async function runFrameCommand(
	command: string[],
	env: Record<string, string | undefined>,
	log: string,
	guardParity: boolean,
) {
	const isolation = guardParity ? await holdFrameIsolation("exclusive") : "unavailable";
	if (isolation === "busy") throw new Error("Frame measurement blocked by a running parity run");
	try {
		if (guardParity) {
			const parity = await parityProcesses();
			if (parity.length) throw new Error(`Frame measurement blocked by parity processes: ${parity.join(", ")}`);
		}
		return await runIsolated(command, env, log, guardParity);
	} finally {
		if (isolation !== "unavailable") await isolation.release();
	}
}

async function runIsolated(
	command: string[],
	env: Record<string, string | undefined>,
	log: string,
	guardParity: boolean,
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
			const parity = await parityProcesses();
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
