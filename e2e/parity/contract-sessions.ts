import { LIVE_SCRIPTS } from "../../src/lib/tauri-dev-mock-live";
import type { JsonObject } from "../../src/lib/types";
import type { Step } from "./contract-steps";
import { boolean, count, isString, oneOf, optionalBoolean, string } from "./contract-values";

/** Parses terminal, port-forward, exec and live-data steps; returns undefined for any other action. */
export function sessionStep(item: JsonObject): Step | undefined {
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
		case "expectTerminalCursor": {
			const cursor: Extract<Step, { action: "expectTerminalCursor" }> = { action: item.action, cursor: oneOf(["block", "block blink", "bar", "bar blink", "underline", "underline blink", "outline"], item.cursor, "a cursor paint") };
			if (item.row !== undefined && item.row !== null) cursor.row = count(item.row);
			if (item.col !== undefined && item.col !== null) cursor.col = count(item.col);
			return cursor;
		}
		case "configurePreset": return { action: item.action, label: string(item.label), context: string(item.context), namespace: string(item.namespace), service: string(item.service), remote: string(item.remote), local: string(item.local) };
		case "presetControl": {
			const control: Extract<Step, { action: "presetControl" }> = { action: item.action, operation: oneOf(["add", "save", "edit", "start", "remove", "absent"], item.operation, "a preset action") };
			if (item.label !== undefined) control.label = string(item.label);
			return control;
		}
		case "replaySessions": return { action: item.action, script: oneOf(["reconnecting", "failed", "foreign-output", "exec-output", "exec-exited", "exec-failed", "exec-stopped"], item.script, "a session script") };
		case "confirmExecLeave": return { action: item.action, choice: oneOf(["stay", "stop"], item.choice, "a leave choice") };
		case "configureForward": return { action: item.action, remote: string(item.remote), local: isString(item.local) ? item.local : "" };
		case "forwardControl": {
			const control: Extract<Step, { action: "forwardControl" }> = { action: item.action, operation: oneOf(["start", "save", "restore", "stop", "reconnect"], item.operation, "a forward control") };
			if (item.target !== undefined) control.target = string(item.target);
			return control;
		}
		case "expectForwardState": {
			const state: Extract<Step, { action: "expectForwardState" }> = { action: item.action, target: string(item.target), local: string(item.local), status: string(item.status), count: count(item.count) };
			if (item.resolved !== undefined) state.resolved = string(item.resolved);
			return state;
		}
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
		case "expectExecState": {
			const state: Extract<Step, { action: "expectExecState" }> = { action: item.action, container: string(item.container), status: string(item.status), contains: isString(item.contains) ? item.contains : "" };
			if (item.absent !== undefined) state.absent = string(item.absent);
			if (item.command !== undefined) state.command = string(item.command);
			return state;
		}
		case "expectLiveState": {
			const state: Extract<Step, { action: "expectLiveState" }> = { action: item.action, label: string(item.label) };
			if (item.notice !== undefined) state.notice = string(item.notice);
			return state;
		}
		case "replayLiveData": return { action: item.action, script: oneOf(LIVE_SCRIPTS, item.script, "a live script") };
		default: return undefined;
	}
}
