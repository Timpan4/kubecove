import { writeFile } from "node:fs/promises";
import { getPriority } from "node:os";

async function parityProcesses(): Promise<string[]> {
	if (process.platform !== "linux") return [];
	const child = Bun.spawn(["ps", "-eo", "pid=,stat=,comm="], { stdout: "pipe", stderr: "pipe" });
	const runners = Bun.spawn(["pgrep", "-f", "scripts/e2e-parity[.]ts"], { stdout: "pipe", stderr: "pipe" });
	const [code, output, error, runnerCode, runnerIds, runnerError] = await Promise.all([
		child.exited, new Response(child.stdout).text(), new Response(child.stderr).text(),
		runners.exited, new Response(runners.stdout).text(), new Response(runners.stderr).text(),
	]);
	if (code !== 0) throw new Error(`Cannot verify parity isolation: ps exited ${code}: ${error}`);
	if (runnerCode !== 0 && runnerCode !== 1)
		throw new Error(`Cannot verify parity isolation: pgrep exited ${runnerCode}: ${runnerError}`);
	const processes = output.split("\n").filter(line => {
		const [, state, name] = line.trim().split(/\s+/);
		return !state?.startsWith("Z") && name?.startsWith("parity-");
	});
	return processes.concat(runnerIds.trim().split("\n").filter(Boolean).map(pid => `parity runner PID ${pid}`));
}

export async function runFrameCommand(
	command: string[],
	env: Record<string, string | undefined>,
	log: string,
	guardParity: boolean,
) {
	if (guardParity) {
		const parity = await parityProcesses();
		if (parity.length) throw new Error(`Frame measurement blocked by parity processes: ${parity.join(", ")}`);
	}
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
