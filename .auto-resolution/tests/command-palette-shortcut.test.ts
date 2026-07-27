import { describe, expect, test } from "bun:test";
import { shouldToggleCommandPaletteShortcut } from "../src/features/command-palette/shortcut";

function shortcut(overrides: {
	altKey?: boolean;
	ctrlKey?: boolean;
	key?: string;
	metaKey?: boolean;
	shiftKey?: boolean;
	target?: EventTarget | null;
} = {}) {
	return shouldToggleCommandPaletteShortcut({
		altKey: overrides.altKey ?? false,
		ctrlKey: overrides.ctrlKey ?? true,
		key: overrides.key ?? "k",
		metaKey: overrides.metaKey ?? false,
		shiftKey: overrides.shiftKey ?? false,
		target: overrides.target ?? null,
	});
}

describe("command palette shortcut", () => {
	test("accepts Ctrl+K and Cmd+K without extra modifiers", () => {
		expect(shortcut()).toBe(true);
		expect(shortcut({ ctrlKey: false, metaKey: true })).toBe(true);
	});

	test("rejects shifted or alternate chords", () => {
		expect(shortcut({ shiftKey: true })).toBe(false);
		expect(shortcut({ altKey: true })).toBe(false);
		expect(shortcut({ key: "j" })).toBe(false);
		expect(shortcut({ ctrlKey: false, metaKey: false })).toBe(false);
	});

	test("allows focused inputs but leaves xterm sessions alone", () => {
		const input = { closest: () => null } as unknown as EventTarget;
		const terminal = { closest: (selector: string) => (selector === ".xterm" ? {} : null) } as unknown as EventTarget;

		expect(shortcut({ target: input })).toBe(true);
		expect(shortcut({ target: terminal })).toBe(false);
	});
});
