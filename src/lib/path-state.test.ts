import {
	decodePathStateSnapshot,
	sanitizePathStateTreeNode,
} from "./path-state";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

describe("path state sanitization", () => {
	test("drops invalid tree nodes", () => {
		expect(sanitizePathStateTreeNode({ type: "kind", kind: "Pod" })).toBe(null);
	});

	test("falls back invalid workspace path filters", () => {
		const snapshot = decodePathStateSnapshot(
			JSON.stringify({
				version: 1,
				runtime: "svelte",
				launcherView: "workspaces",
				workspace: {
					workspaceId: "workspace-a",
					viewMode: "wat",
					selectedNode: { type: "kind", kind: "Pod" },
					expandedSections: ["one", 2, "two"],
					resourceInitialHealthFilter: "broken",
					resourceNamespaceOverride: ["default", false],
				},
			}),
		);

		expect(snapshot?.workspace?.viewMode).toBe("overview");
		expect(snapshot?.workspace?.selectedNode).toBe(null);
		expect(snapshot?.workspace?.expandedSections).toEqual(["one", "two"]);
		expect(snapshot?.workspace?.resourceInitialHealthFilter).toBe("all");
		expect(snapshot?.workspace?.resourceNamespaceOverride).toEqual(["default"]);
	});
});
