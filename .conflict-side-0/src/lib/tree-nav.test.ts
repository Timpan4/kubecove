import {
	ARGO_CHILDREN_LABELS,
	resolveTreeScope,
	type TreeNodeId,
} from "./tree-nav";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

describe("GitOps tree scope", () => {
	test("keeps the GitOps section as an overview scope", () => {
		const node: TreeNodeId = { type: "section", section: "argo" };
		const scope = resolveTreeScope(node);

		expect(scope.argoMode).toBe(true);
		expect(scope.kinds).toEqual([]);
	});

	test("keeps legacy Argo labels mapped to provider-specific labels", () => {
		expect(ARGO_CHILDREN_LABELS.Applications).toBe("Argo CD Applications");
		expect(ARGO_CHILDREN_LABELS.ApplicationSets).toBe(
			"Argo CD ApplicationSets",
		);
		expect(ARGO_CHILDREN_LABELS.AppProjects).toBe("Argo CD AppProjects");
	});
});
