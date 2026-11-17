import { afterEach, expect, spyOn, test } from "bun:test";
import { clearDiagnostics, setDiagnosticsEnabled } from "./diagnostics";
import { type CommandWindow, foldCommands, withUnfoldedCommands } from "./interaction-commands";
import { traceInvoke } from "./tauri-runtime";

afterEach(() => {
	clearDiagnostics();
	setDiagnosticsEnabled(false);
});

test("late command completions stay with the window containing their measured start", async () => {
	const clock = spyOn(performance, "now");
	try {
		setDiagnosticsEnabled(true);
		const earlier = Promise.withResolvers<string>();
		const later = Promise.withResolvers<never>();
		const windows: CommandWindow[] = [
			{ start: 10, end: 10.625, commands: [] },
			{ start: 10.625, end: 11.75, commands: [] },
		];

		clock.mockReturnValue(10.5625);
		const success = traceInvoke("earlier_command", undefined, () => earlier.promise);
		clock.mockReturnValue(10.6875);
		const failure = traceInvoke("later_command", undefined, () => later.promise);
		const mark = foldCommands(windows, undefined);

		clock.mockReturnValue(20.4375);
		later.reject(new Error("command failed"));
		await expect(failure).rejects.toThrow("command failed");
		clock.mockReturnValue(20.5625);
		earlier.resolve("result");
		await success;

		const expected = [
			[{ cmd: "earlier_command", ms: 10, ok: true, startOffsetMs: 1 }],
			[{ cmd: "later_command", ms: 10, ok: false, startOffsetMs: 0 }],
		];
		expect(withUnfoldedCommands(windows, mark)).toEqual(expected);
		const completedMark = foldCommands(windows, mark);
		clearDiagnostics();
		expect(withUnfoldedCommands(windows, completedMark)).toEqual(expected);
	} finally {
		clock.mockRestore();
	}
});
