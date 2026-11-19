import { afterEach, beforeEach, expect, spyOn, test } from "bun:test";
import { clearDiagnostics, diagnosticLog, getDiagnosticsSnapshot, MAX_FRONTEND_EVENTS, setDiagnosticsEnabled } from "./diagnostics";
import {
	clearRecordedInteractions,
	getRecordedInteractions,
	startInteractionRecording,
	stopInteractionRecording,
} from "./interaction-recorder";
import { traceInvoke } from "./tauri-runtime";

const input = new EventTarget();
const browserGlobals = {
	window: input,
	document: new EventTarget(),
	Element: class {},
	MutationObserver: class {
		observe() {}
		disconnect() {}
	},
	requestAnimationFrame: () => 0,
	cancelAnimationFrame: () => {},
};
const previousGlobals = new Map<string, PropertyDescriptor | undefined>();

beforeEach(() => {
	for (const [name, value] of Object.entries(browserGlobals)) {
		previousGlobals.set(name, Object.getOwnPropertyDescriptor(globalThis, name));
		Object.defineProperty(globalThis, name, { configurable: true, value });
	}
	setDiagnosticsEnabled(true);
	startInteractionRecording();
});

afterEach(() => {
	stopInteractionRecording();
	clearRecordedInteractions();
	clearDiagnostics();
	setDiagnosticsEnabled(false);
	for (const [name, descriptor] of previousGlobals) {
		if (descriptor) Object.defineProperty(globalThis, name, descriptor);
		else Reflect.deleteProperty(globalThis, name);
	}
});

test("a late completion evicted before the next flush stays with its first interaction", async () => {
	const clock = spyOn(performance, "now");
	try {
		clock.mockReturnValue(10);
		input.dispatchEvent(new Event("pointerdown"));
		const pending = Promise.withResolvers<string>();
		clock.mockReturnValue(10.5625);
		const read = traceInvoke("list_resource_topology", { fixtureArgument: "private input" }, () => pending.promise);
		clock.mockReturnValue(10.625);
		input.dispatchEvent(new Event("keydown"));
		clock.mockReturnValue(20.5625);
		pending.resolve("private result");
		await read;
		expect(getDiagnosticsSnapshot().frontendEvents.at(-1)?.event).toBe("tauri.invoke.done");

		for (let index = 0; index < MAX_FRONTEND_EVENTS; index += 1) {
			diagnosticLog("resources.render");
		}
		const trace = getDiagnosticsSnapshot().frontendEvents;
		expect(trace).toHaveLength(MAX_FRONTEND_EVENTS);
		expect(trace.every(({ event }) => event === "resources.render")).toBe(true);
		stopInteractionRecording();

		const report = getRecordedInteractions();
		expect(report.map(({ kind, commands }) => ({ kind, commands }))).toEqual([
			{ kind: "click", commands: [{ cmd: "list_resource_topology", ms: 10, ok: true, startOffsetMs: 1 }] },
			{ kind: "key", commands: [] },
		]);
		expect(JSON.stringify(report)).not.toContain("fixtureArgument");
		expect(JSON.stringify(report)).not.toContain("private result");
	} finally {
		clock.mockRestore();
	}
});

test("completions after stop keep their measured start windows through Clear diagnostics", async () => {
	const clock = spyOn(performance, "now");
	try {
		const earlier = Promise.withResolvers<string>();
		const later = Promise.withResolvers<never>();
		clock.mockReturnValue(10);
		input.dispatchEvent(new Event("pointerdown"));
		clock.mockReturnValue(10.5625);
		const success = traceInvoke("earlier_command", undefined, () => earlier.promise);
		clock.mockReturnValue(10.625);
		input.dispatchEvent(new Event("keydown"));
		clock.mockReturnValue(10.6875);
		const failure = traceInvoke("later_command", undefined, () => later.promise);
		clock.mockReturnValue(11.75);
		stopInteractionRecording();

		clearDiagnostics();
		clock.mockReturnValue(20.4375);
		later.reject(new Error("private command error"));
		await expect(failure).rejects.toThrow("private command error");
		clock.mockReturnValue(20.5625);
		earlier.resolve("private result");
		await success;
		expect(getDiagnosticsSnapshot().frontendEvents.map(({ event }) => event)).toEqual([
			"tauri.invoke.error",
			"tauri.invoke.done",
		]);
		clearDiagnostics();

		const expected = [
			[{ cmd: "earlier_command", ms: 10, ok: true, startOffsetMs: 1 }],
			[{ cmd: "later_command", ms: 10, ok: false, startOffsetMs: 0 }],
		];
		const report = getRecordedInteractions();
		expect(report.map(({ commands }) => commands)).toEqual(expected);
		expect(JSON.stringify(report)).not.toContain("private command error");
		expect(getRecordedInteractions().map(({ commands }) => commands)).toEqual(expected);
	} finally {
		clock.mockRestore();
	}
});
