import { hasAppDetailPanel } from "./viewHelpers";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
};

describe("app view helpers", () => {
	test("does not reserve the detail panel for empty resource views", () => {
		expect(
			hasAppDetailPanel("resources", false, false, false),
		).toBe(false);
	});

	test("shows details for the selected item owned by the active view", () => {
		expect(hasAppDetailPanel("resources", false, false, true)).toBe(true);
		expect(hasAppDetailPanel("helm", true, false, true)).toBe(true);
		expect(hasAppDetailPanel("argo", false, true, true)).toBe(true);
	});

	test("ignores stale selections from inactive detail views", () => {
		expect(hasAppDetailPanel("helm", false, true, true)).toBe(false);
		expect(hasAppDetailPanel("argo", true, false, true)).toBe(false);
	});
});
