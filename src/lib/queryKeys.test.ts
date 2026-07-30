import { isFiniteKubernetesQuery, queryKeys } from "./queryKeys";
import type { ResourceSummary } from "./types";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

const resource: ResourceSummary = {
	kind: "Pod",
	cluster: "kind-dev",
	name: "api",
	namespace: "default",
	age: "1m",
	health: "healthy",
};

describe("finite Kubernetes query classification", () => {
	test("includes workspace Kubernetes reads", () => {
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
		const keys = [
			queryKeys.namespaces("kind-dev"),
			queryKeys.resourceKinds("kind-dev"),
			queryKeys.resources("kind-dev", [{ kind: "Pod" }]),
			queryKeys.resourceDetails(resource),
			queryKeys.argoApps("kind-dev"),
			queryKeys.argoWorkspaceInspector(
				"kind-dev",
				"workspace-1",
				"guestbook",
				"argocd",
				"uid-1",
				true,
				"kubernetes",
				undefined,
				"KUBECONFIG",
			),
			queryKeys.fluxDetect("kind-dev"),
			queryKeys.helmReleases("kind-dev"),
			queryKeys.rbacInspection("kind-dev", ["default"]),
		];

		expect(JSON.stringify(argoWorkspaceScope) === JSON.stringify(recreatedArgoWorkspaceScope)).toBe(false);
		expect(keys.every(isFiniteKubernetesQuery)).toBe(true);
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

	test("excludes live sessions and local app queries", () => {
		const keys = [
			queryKeys.portForwards(),
			queryKeys.podExecSessions(),
			queryKeys.appUsageMetrics(),
			queryKeys.backendDiagnostics(),
			queryKeys.kubeContexts(),
			["kubeconfig-sources"],
		];

		expect(keys.some(isFiniteKubernetesQuery)).toBe(false);
	});
});
