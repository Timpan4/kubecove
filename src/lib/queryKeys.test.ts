import { queryKeys } from "./queryKeys";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

describe("query key identity", () => {
	test("keeps workspace and Application identity in Argo keys", () => {
		const argoWorkspaceScope = queryKeys.argoWorkspaceApplication(
			"kind-dev",
			"workspace-1",
			"guestbook",
			"argocd",
			"uid-1",
			true,
			"KUBECONFIG",
		);
		const recreatedArgoWorkspaceScope = queryKeys.argoWorkspaceApplication(
			"kind-dev",
			"workspace-1",
			"guestbook",
			"argocd",
			"uid-2",
			true,
			"KUBECONFIG",
		);

		expect(JSON.stringify(argoWorkspaceScope) === JSON.stringify(recreatedArgoWorkspaceScope)).toBe(false);
		expect(
			queryKeys.argoWorkspaceManagedResources(
				"kind-dev",
				"workspace-1",
				"guestbook",
				"argocd",
				"uid-1",
				true,
				"connected",
				"connection-1",
				"KUBECONFIG",
			).slice(0, argoWorkspaceScope.length),
		).toEqual(argoWorkspaceScope);
		expect(
			queryKeys.argoWorkspaceComparison(
				"kind-dev",
				"workspace-1",
				"guestbook",
				"argocd",
				"uid-1",
				true,
				"kubernetes",
				"ignored-for-fallback",
				"apps",
				"Deployment",
				"default",
				"guestbook",
				"KUBECONFIG",
			).slice(0, argoWorkspaceScope.length),
		).toEqual(argoWorkspaceScope);
	});
});
