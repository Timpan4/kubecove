import { LIVE_SCRIPTS } from "../../src/lib/tauri-dev-mock-live";
import type { Step } from "./contract-steps";
import { boolean, count, oneOf, optionalBoolean, string } from "./contract-values";

/** Parses terminal, port-forward, exec and live-data steps; returns undefined for any other action. */
export function sessionStep(item: Record<string, unknown>): Step | undefined {
	switch (item.action) {
		case "terminalFit": return { action: item.action, width: count(item.width), height: count(item.height) };
		case "terminalOutput": return { action: item.action, data: string(item.data), container: string(item.container) };
		case "terminalSelect": return { action: item.action, row: count(item.row), col: count(item.col), endRow: count(item.endRow), endCol: count(item.endCol), word: optionalBoolean(item.word) ?? false, line: optionalBoolean(item.line) ?? false, alt: optionalBoolean(item.alt) ?? false };
		case "terminalCopy": case "terminalPaste": return { action: item.action, text: item.text === "" ? "" : string(item.text) };
		case "terminalPrimaryPaste": return { action: item.action, text: string(item.text) };
		case "terminalScroll": return { action: item.action, rows: count(item.rows) };
		case "terminalDragOutside": return { action: item.action, row: count(item.row), col: count(item.col), edge: oneOf(["above", "below"], item.edge, "a terminal edge") };
		case "terminalRelease": return { action: item.action, ms: count(item.ms) };
		case "terminalFocus": return { action: item.action, focused: boolean(item.focused) };
		case "terminalRightClick": return { action: item.action, row: count(item.row), col: count(item.col) };
		case "expectTerminalCursor": return { action: item.action, cursor: oneOf(["block", "block blink", "bar", "bar blink", "underline", "underline blink", "outline"], item.cursor, "a cursor paint"), ...(item.row == null ? {} : { row: count(item.row) }), ...(item.col == null ? {} : { col: count(item.col) }) };
		case "configurePreset": return { action: item.action, label: string(item.label), context: string(item.context), namespace: string(item.namespace), service: string(item.service), remote: string(item.remote), local: string(item.local) };
		case "presetControl": return { action: item.action, operation: oneOf(["add", "save", "edit", "start", "remove", "absent"], item.operation, "a preset action"), ...(item.label === undefined ? {} : { label: string(item.label) }) };
		case "replaySessions": return { action: item.action, script: oneOf(["reconnecting", "failed", "foreign-output", "exec-output", "exec-exited", "exec-failed", "exec-stopped"], item.script, "a session script") };
		case "confirmExecLeave": return { action: item.action, choice: oneOf(["stay", "stop"], item.choice, "a leave choice") };
		case "configureForward": return { action: item.action, remote: string(item.remote), local: typeof item.local === "string" ? item.local : "" };
		case "forwardControl": return { action: item.action, operation: oneOf(["start", "save", "restore", "stop", "reconnect"], item.operation, "a forward control"), ...(item.target === undefined ? {} : { target: string(item.target) }) };
		case "expectForwardState": return { action: item.action, target: string(item.target), local: string(item.local), status: string(item.status), count: count(item.count), ...(item.resolved === undefined ? {} : { resolved: string(item.resolved) }) };
		case "expectForwardNotice": return { action: item.action, title: string(item.title), text: string(item.text) };
		case "execControl": {
			const control: Extract<Step, { action: "execControl" }> = {
				action: item.action,
				operation: oneOf(["start", "input", "close", "stop", "select", "custom", "arguments", "shortcut", "run"], item.operation, "an exec control"),
			};
			if (item.container !== undefined) control.container = string(item.container);
			if (item.command !== undefined) control.command = string(item.command);
			if (item.data !== undefined) control.data = string(item.data);
			if (item.tab !== undefined) control.tab = count(item.tab);
			return control;
		}
		case "expectExecState": return { action: item.action, container: string(item.container), status: string(item.status), contains: typeof item.contains === "string" ? item.contains : "", ...(item.absent === undefined ? {} : { absent: string(item.absent) }), ...(item.command === undefined ? {} : { command: string(item.command) }) };
		case "expectLiveState": return { action: item.action, label: string(item.label), ...(item.notice === undefined ? {} : { notice: string(item.notice) }) };
		case "replayLiveData": return { action: item.action, script: oneOf(LIVE_SCRIPTS, item.script, "a live script") };
		default: return undefined;
	}
}
