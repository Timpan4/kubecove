import { LIVE_SCRIPTS } from "../../src/lib/tauri-dev-mock-live";
import { type StepParsers, boolean, count, object, oneOf, optionalBoolean, plainString, string, strings, noPayload, withOptional } from "./contract-values";

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
		if (control.credentialPlugin !== undefined) timing.credentialPlugin = boolean(control.credentialPlugin);
		return { control: timing };
	},
	expectLiveSessionCount: (item) => ({ count: count(item.count) }),
	expectPortForward: (item) => ({ title: string(item.title) }),
	openLiveSessionPopover: noPayload,
	expectLiveSessionRow: (item) => {
		return withOptional({
			target: string(item.target), status: string(item.status), controls: strings(item.controls, false),
		}, {
			local: item.local !== undefined ? string(item.local) : undefined,
		});
	},
	liveSessionControl: (item) => {
		return withOptional({
			operation: oneOf(["Show", "Reconnect", "Copy", "Stop", "Manage"], item.operation, "a live-session control"),
		}, {
			target: item.target !== undefined ? string(item.target) : undefined,
			local: item.local !== undefined ? string(item.local) : undefined,
		});
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
		return withOptional({
			cursor: oneOf(["block", "block blink", "bar", "bar blink", "underline", "underline blink", "outline"], item.cursor, "a cursor paint"),
		}, {
			row: item.row !== undefined && item.row !== null ? count(item.row) : undefined,
			col: item.col !== undefined && item.col !== null ? count(item.col) : undefined,
		});
	},
	configurePreset: (item) => ({ label: string(item.label), context: string(item.context), namespace: string(item.namespace), service: string(item.service), remote: string(item.remote), local: string(item.local) }),
	presetControl: (item) => {
		return withOptional({
			operation: oneOf(["add", "save", "edit", "start", "remove", "absent"], item.operation, "a preset action"),
		}, {
			label: item.label !== undefined ? string(item.label) : undefined,
		});
	},
	replaySessions: (item) => ({ script: oneOf(["reconnecting", "failed", "foreign-output", "exec-output", "exec-exited", "exec-failed", "exec-stopped"], item.script, "a session script") }),
	confirmExecLeave: (item) => ({ choice: oneOf(["stay", "stop"], item.choice, "a leave choice") }),
	configureForward: (item) => ({ remote: string(item.remote), local: item.local === undefined ? "" : plainString(item.local) }),
	forwardControl: (item) => {
		return withOptional({
			operation: oneOf(["start", "save", "restore", "stop", "reconnect"], item.operation, "a forward control"),
		}, {
			target: item.target !== undefined ? string(item.target) : undefined,
			local: item.local !== undefined ? string(item.local) : undefined,
		});
	},
	expectForwardState: (item) => {
		return withOptional({
			target: string(item.target), local: string(item.local), status: string(item.status), count: count(item.count),
		}, {
			resolved: item.resolved !== undefined ? string(item.resolved) : undefined,
		});
	},
	expectForwardNotice: (item) => ({ title: string(item.title), text: string(item.text) }),
	execControl: (item) => {
		return withOptional({
			operation: oneOf(["start", "input", "close", "stop", "select", "custom", "arguments", "shortcut", "run"], item.operation, "an exec control"),
		}, {
			container: item.container !== undefined ? string(item.container) : undefined,
			command: item.command !== undefined ? string(item.command) : undefined,
			data: item.data !== undefined ? string(item.data) : undefined,
			tab: item.tab !== undefined ? count(item.tab) : undefined,
		});
	},
	expectExecState: (item) => {
		return withOptional({
			container: string(item.container), status: string(item.status), contains: item.contains === undefined ? "" : plainString(item.contains),
		}, {
			absent: item.absent !== undefined ? string(item.absent) : undefined,
			command: item.command !== undefined ? string(item.command) : undefined,
		});
	},
	expectLiveState: (item) => {
		return withOptional({
			label: string(item.label),
		}, {
			notice: item.notice !== undefined ? string(item.notice) : undefined,
		});
	},
	expectLiveMenu: (item) => ({ message: string(item.message) }),
	replayLiveData: (item) => ({ script: oneOf(LIVE_SCRIPTS, item.script, "a live script") }),
} satisfies StepParsers;
