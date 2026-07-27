import { resolveYamlForceConflicts } from "./yamlApplyModel";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
};

describe("resolveYamlForceConflicts", () => {
	test("ignores click-event-shaped overrides", () => {
		const clickLike = { currentTarget: "dry-run-button" };

		expect(resolveYamlForceConflicts(clickLike, false)).toBe(false);
		expect(resolveYamlForceConflicts(clickLike, true)).toBe(true);
		expect(resolveYamlForceConflicts(true, false)).toBe(true);
	});
});
