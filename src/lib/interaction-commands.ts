// Tauri commands started during one recorded interaction, read from the frontend
// diagnostics buffer. Only command names and timings; never arguments or results.
import { getDiagnosticsSnapshot, isDiagnosticsEnabled } from "./diagnostics";

export interface InteractionCommand {
	cmd: string;
	ms: number;
	ok: boolean;
	startOffsetMs: number;
}

/** Commands that started in [start, end) on the performance.now() clock; undefined while diagnostics is off. */
export function commandsDuring(start: number, end: number): InteractionCommand[] | undefined {
	if (!isDiagnosticsEnabled()) return undefined;
	const commands: InteractionCommand[] = [];
	for (const event of getDiagnosticsSnapshot().frontendEvents) {
		if (event.event !== "tauri.invoke.done" && event.event !== "tauri.invoke.error") continue;
		const ms = Number(event.data.ms ?? 0);
		const commandStart = event.atMs - ms;
		if (commandStart < start || commandStart >= end) continue;
		commands.push({
			cmd: String(event.data.cmd),
			ms,
			ok: event.event === "tauri.invoke.done",
			startOffsetMs: Math.round(commandStart - start),
		});
	}
	return commands;
}
