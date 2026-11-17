import { LIVE_SCRIPTS } from "../../src/lib/tauri-dev-mock-live";
import { type StepParsers, boolean, count, object, oneOf, optionalBoolean, plainString, string, strings, noPayload } from "./contract-values";

export const sessionsSteps = {
	mockTiming: (item) => {
		const control = object(item.control);
		const timing: import("../../src/lib/tauri-dev-mock-timing").TimingControl = {
			operation: oneOf(["hold", "dirty", "contend", "release", "fail", "failOnce", "expectRequests", "expectPending", "expectNoRetry", "expectNoError"], control.operation, "a timing operation"),
			command: string(control.command), context: control.context === "" ? "" : string(control.context),
		};
		if (control.kind !== undefined) timing.kind = string(control.kind);
		if (control.namespace !== undefined) timing.namespace = string(control.namespace);
		if (control.count !== undefined) timing.count = count(control.count);
		return { control: timing };
	},
	expectLiveSessionCount: (item) => ({ count: count(item.count) }),
	expectPortForward: (item) => ({ title: string(item.title) }),
	openLiveSessionPopover: noPayload,
	expectLiveSessionRow: (item) => {
		return {
			target: string(item.target), status: string(item.status), controls: strings(item.controls, false),
			...(item.local !== undefined ? { local: string(item.local) } : {}),
		};
	},
	liveSessionControl: (item) => {
		return {
			operation: oneOf(["Show", "Reconnect", "Copy", "Stop", "Manage"], item.operation, "a live-session control"),
			...(item.target !== undefined ? { target: string(item.target) } : {}),
			...(item.local !== undefined ? { local: string(item.local) } : {}),
		};
	},
	expectHealthTooltip: (item) => ({ kind: string(item.kind), namespace: string(item.namespace), name: string(item.name), state: string(item.state), source: string(item.source), statuses: strings(item.statuses, true), partial: boolean(item.partial) }),
	expectTimestampTooltip: (item) => ({ kind: string(item.kind), namespace: string(item.namespace), name: string(item.name), value: string(item.value) }),
	expectEventTimeTooltip: (item) => ({ index: count(item.index) }),
	terminalFit: (item) => ({ width: count(item.width), height: count(item.height) }),
	terminalOutput: (item) => ({ data: string(item.data), container: string(item.container) }),
	terminalSelect: (item) => ({ row: count(item.row), col: count(item.col), endRow: count(item.endRow), endCol: count(item.endCol), word: optionalBoolean(item.word) ?? false, line: optionalBoolean(item.line) ?? false, alt: optionalBoolean(item.alt) ?? false }),
	terminalCopy: (item) => ({ text: item.text === "" ? "" : string(item.text) }),
	terminalPaste: (item) => ({ text: item.text === "" ? "" : string(item.text) }),
	terminalPrimaryPaste: (item) => ({ text: string(item.text) }),
	terminalScroll: (item) => ({ rows: count(item.rows) }),
	terminalDragOutside: (item) => ({ row: count(item.row), col: count(item.col), edge: oneOf(["above", "below"], item.edge, "a terminal edge") }),
	terminalRelease: (item) => ({ ms: count(item.ms) }),
	terminalFocus: (item) => {
		const focused = boolean(item.focused);
		if (item.key === undefined) return { focused };
		if (!focused) throw new Error("Only focusing the terminal takes a key");
		return { focused, key: oneOf(["Tab"], item.key, "a terminal focus key") };
	},
	terminalKey: (item) => ({ key: oneOf(["Tab"] as const, item.key, "a terminal key") }),
	terminalRightClick: (item) => ({ row: count(item.row), col: count(item.col) }),
	expectTerminalCursor: (item) => {
		return {
			cursor: oneOf(["block", "block blink", "bar", "bar blink", "underline", "underline blink", "outline"], item.cursor, "a cursor paint"),
			...(item.row !== undefined && item.row !== null ? { row: count(item.row) } : {}),
			...(item.col !== undefined && item.col !== null ? { col: count(item.col) } : {}),
		};
	},
	configurePreset: (item) => ({ label: string(item.label), context: string(item.context), namespace: string(item.namespace), service: string(item.service), remote: string(item.remote), local: string(item.local) }),
	presetControl: (item) => {
		return {
			operation: oneOf(["add", "save", "edit", "start", "remove", "absent"], item.operation, "a preset action"),
			...(item.label !== undefined ? { label: string(item.label) } : {}),
		};
	},
	replaySessions: (item) => ({ script: oneOf(["reconnecting", "failed", "foreign-output", "exec-output", "exec-exited", "exec-failed", "exec-stopped"], item.script, "a session script") }),
	confirmExecLeave: (item) => ({ choice: oneOf(["stay", "stop"], item.choice, "a leave choice") }),
	configureForward: (item) => ({ remote: string(item.remote), local: item.local === undefined ? "" : plainString(item.local) }),
	forwardControl: (item) => {
		return {
			operation: oneOf(["start", "save", "restore", "stop", "reconnect"], item.operation, "a forward control"),
			...(item.target !== undefined ? { target: string(item.target) } : {}),
		};
	},
	expectForwardState: (item) => {
		return {
			target: string(item.target), local: string(item.local), status: string(item.status), count: count(item.count),
			...(item.resolved !== undefined ? { resolved: string(item.resolved) } : {}),
		};
	},
	expectForwardNotice: (item) => ({ title: string(item.title), text: string(item.text) }),
	execControl: (item) => {
		return {
			operation: oneOf(["start", "input", "close", "stop", "select", "custom", "arguments", "shortcut", "run"], item.operation, "an exec control"),
			...(item.container !== undefined ? { container: string(item.container) } : {}),
			...(item.command !== undefined ? { command: string(item.command) } : {}),
			...(item.data !== undefined ? { data: string(item.data) } : {}),
			...(item.tab !== undefined ? { tab: count(item.tab) } : {}),
		};
	},
	expectExecState: (item) => {
		return {
			container: string(item.container), status: string(item.status), contains: item.contains === undefined ? "" : plainString(item.contains),
			...(item.absent !== undefined ? { absent: string(item.absent) } : {}),
			...(item.command !== undefined ? { command: string(item.command) } : {}),
		};
	},
	expectLiveState: (item) => {
		return {
			label: string(item.label),
			...(item.notice !== undefined ? { notice: string(item.notice) } : {}),
		};
	},
	expectLiveMenu: (item) => ({ message: string(item.message) }),
	replayLiveData: (item) => ({ script: oneOf(LIVE_SCRIPTS, item.script, "a live script") }),
} satisfies StepParsers;
