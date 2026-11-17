// Tauri commands started during one recorded interaction, read from the frontend
// diagnostics buffer. Only command names and timings; never arguments or results.
// A command belongs to the interaction it started in. Each flush, and each read while
// no interaction runs, moves the commands that finished since the last move into
// their interactions, so an interaction keeps them after the buffer drops them;
// withUnfoldedCommands adds those that finished after the last move, as
// src-tauri/gpui/src/interactions.rs does.
import { type FrontendDiagnosticEvent, getDiagnosticsSnapshot } from "./diagnostics";

export interface InteractionCommand {
	cmd: string;
	ms: number;
	ok: boolean;
	startOffsetMs: number;
}

/** One interaction's window [start, end) on the performance.now() clock. */
export interface CommandWindow {
	start: number;
	end: number;
	/** Undefined when diagnostics was off as the interaction ended. */
	commands: InteractionCommand[] | undefined;
}

/**
 * The newest diagnostic event a fold saw. Ids restart after Clear diagnostics,
 * so its time tells two events with one id apart.
 */
export interface EventMark {
	id: number;
	atMs: number;
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

/** Events logged after `mark`. A mark the buffer no longer holds was cleared or dropped, so every event left is newer. */
function eventsAfter(events: readonly FrontendDiagnosticEvent[], mark: EventMark | undefined): readonly FrontendDiagnosticEvent[] {
	if (!mark) return events;
	for (let index = events.length - 1; index >= 0; index -= 1) {
		const event = events[index];
		if (event.id === mark.id && event.atMs === mark.atMs) return events.slice(index + 1);
	}
	return events;
}

/** Commands finished after `mark`, each with the index of the window it started in. Windows are in start order. */
function owned(
	windows: readonly CommandWindow[],
	events: readonly FrontendDiagnosticEvent[],
	mark: EventMark | undefined,
): [number, InteractionCommand][] {
	const result: [number, InteractionCommand][] = [];
	for (const event of eventsAfter(events, mark)) {
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
		if (!entry?.commands || command.start >= entry.end) continue;
		result.push([index, {
			cmd: String(event.data.cmd),
			ms: command.ms,
			ok: command.ok,
			startOffsetMs: Math.round(command.start - entry.start),
		}]);
	}
	return result;
}

/** Moves the commands that finished after `mark` into their windows; returns the new mark. */
export function foldCommands(windows: readonly CommandWindow[], mark: EventMark | undefined): EventMark | undefined {
	const events = getDiagnosticsSnapshot().frontendEvents;
	for (const [index, command] of owned(windows, events, mark)) windows[index].commands?.push(command);
	const last = events.at(-1);
	return last ? { id: last.id, atMs: last.atMs } : mark;
}

/** Each window's commands, with those that finished after `mark`. Moves nothing, since the current interaction may still run. */
export function withUnfoldedCommands(
	windows: readonly CommandWindow[],
	mark: EventMark | undefined,
): (InteractionCommand[] | undefined)[] {
	const commands = windows.map((entry) => entry.commands && [...entry.commands]);
	for (const [index, command] of owned(windows, getDiagnosticsSnapshot().frontendEvents, mark)) {
		commands[index]?.push(command);
	}
	return commands;
}
