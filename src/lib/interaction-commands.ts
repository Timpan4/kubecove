// Tauri commands started during one recorded interaction, read from the frontend
// diagnostics buffer. Only command names and timings; never arguments or results.
// A command belongs to the interaction it started in. An interaction captures the
// commands that had finished when it ended; withLateCommands adds those that
// finished later, as src-tauri/gpui/src/interactions.rs does.
import { type FrontendDiagnosticEvent, getDiagnosticsSnapshot, isDiagnosticsEnabled } from "./diagnostics";

export interface InteractionCommand {
	cmd: string;
	ms: number;
	ok: boolean;
	startOffsetMs: number;
}

/** Commands of one interaction, captured when it ended. */
export interface CommandCapture {
	finished: InteractionCommand[];
	/** The newest diagnostic event id the capture saw; later ids finished after the interaction ended. */
	seenId: number;
}

/** One interaction's window [start, end) on the performance.now() clock. */
export interface CommandWindow {
	start: number;
	end: number;
	/** Undefined when diagnostics was off as the interaction ended. */
	capture: CommandCapture | undefined;
}

interface FinishedCommand {
	event: FrontendDiagnosticEvent;
	start: number;
	ms: number;
	ok: boolean;
}

function finishedCommand(event: FrontendDiagnosticEvent): FinishedCommand | undefined {
	if (event.event !== "tauri.invoke.done" && event.event !== "tauri.invoke.error") return undefined;
	const ms = Number(event.data.ms ?? 0);
	return { event, start: event.atMs - ms, ms, ok: event.event === "tauri.invoke.done" };
}

function toCommand(command: FinishedCommand, windowStart: number): InteractionCommand {
	return {
		cmd: String(command.event.data.cmd),
		ms: command.ms,
		ok: command.ok,
		startOffsetMs: Math.round(command.start - windowStart),
	};
}

/** Commands that started in [start, end) and have finished; undefined while diagnostics is off. */
export function captureCommands(start: number, end: number): CommandCapture | undefined {
	if (!isDiagnosticsEnabled()) return undefined;
	const events = getDiagnosticsSnapshot().frontendEvents;
	const finished: InteractionCommand[] = [];
	for (const event of events) {
		const command = finishedCommand(event);
		if (command && command.start >= start && command.start < end) finished.push(toCommand(command, start));
	}
	return { finished, seenId: events.at(-1)?.id ?? 0 };
}

/** Each window's commands, including those that finished after it ended. Windows are in start order. */
export function withLateCommands(windows: readonly CommandWindow[]): (InteractionCommand[] | undefined)[] {
	const commands = windows.map((entry) => entry.capture && [...entry.capture.finished]);
	for (const event of getDiagnosticsSnapshot().frontendEvents) {
		const command = finishedCommand(event);
		if (!command) continue;
		// The last window that started at or before the command.
		let low = 0;
		let high = windows.length;
		while (low < high) {
			const middle = (low + high) >>> 1;
			if (windows[middle].start <= command.start) low = middle + 1;
			else high = middle;
		}
		const index = low - 1;
		const entry = windows[index];
		if (!entry?.capture || command.start >= entry.end || event.id <= entry.capture.seenId) continue;
		commands[index]?.push(toCommand(command, entry.start));
	}
	return commands;
}
