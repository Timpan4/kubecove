import { queryKeys } from "./queryKeys";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe<Expected>(expected: Expected): void;
	toEqual<Expected>(expected: Expected): void;
};

describe("query key identity", () => {
	test("scopes every Argo Application variant by source, cluster, workspace, namespace, and name", () => {
		const variants = (
			cluster = "kind-dev",
			workspace = "workspace-1",
			name = "guestbook",
			namespace = "argocd",
			source = "KUBECONFIG",
		) => [
			queryKeys.argoWorkspaceApplication(cluster, workspace, name, namespace, "uid-1", true, source),
			queryKeys.argoWorkspaceApplication(cluster, workspace, name, namespace, "uid-2", false, source),
			queryKeys.argoWorkspaceInspector(cluster, workspace, name, namespace, "uid-1", true, "connected", "connection-1", source),
			queryKeys.argoWorkspaceInspector(cluster, workspace, name, namespace, "uid-2", false, "kubernetes", "ignored", source),
			queryKeys.argoWorkspaceSelectedResource(cluster, workspace, name, namespace, "uid-1", true, "connected", "connection-1", "apps", "Deployment", "default", "guestbook", source),
			queryKeys.argoWorkspaceSelectedResource(cluster, workspace, name, namespace, "uid-2", false, "kubernetes", "ignored", "apps", "Deployment", "default", "guestbook", source),
		];
		const scope = queryKeys.argoWorkspaceApplicationScope(
			"kind-dev",
			"workspace-1",
			"guestbook",
			"argocd",
			"KUBECONFIG",
		);

		expect(scope).toEqual([
			"argo-workspace",
			"kubeconfigEnv=KUBECONFIG",
			"kind-dev",
			"workspace-1",
			"argocd",
			"guestbook",
		]);
		for (const variant of variants()) expect(variant.slice(0, scope.length)).toEqual(scope);
		for (const [cluster, workspace, name, namespace, source] of [
			["kind-dev", "workspace-1", "guestbook", "argocd", "SECOND_KUBECONFIG"],
			["kind-prod", "workspace-1", "guestbook", "argocd", "KUBECONFIG"],
			["kind-dev", "workspace-2", "guestbook", "argocd", "KUBECONFIG"],
			["kind-dev", "workspace-1", "guestbook", "other", "KUBECONFIG"],
			["kind-dev", "workspace-1", "other", "argocd", "KUBECONFIG"],
		] as const) {
			for (const variant of variants(cluster, workspace, name, namespace, source)) {
				expect(JSON.stringify(variant.slice(0, scope.length)) === JSON.stringify(scope)).toBe(false);
			}
		}
	});

	test("keeps Argo status order and discovery scope canonical", () => {
		expect(
			queryKeys.argoConnectionStatuses(
				"kind-dev",
				"workspace-1",
				["primary", "backup"],
				"KUBECONFIG",
			),
		).toEqual([
			"argo-connection-status",
			"kubeconfigEnv=KUBECONFIG",
			"kind-dev",
			"workspace-1",
			["primary", "backup"],
		]);
		expect(
			JSON.stringify(
				queryKeys.argoConnectionStatuses(
					"kind-dev",
					"workspace-1",
					["primary", "backup"],
				),
			) ===
				JSON.stringify(
					queryKeys.argoConnectionStatuses(
						"kind-dev",
						"workspace-1",
						["backup", "primary"],
					),
				),
		).toBe(false);
		expect(queryKeys.argoServerDiscovery("kind-dev", "KUBECONFIG")).toEqual([
			"argo-server-discovery",
			"kubeconfigEnv=KUBECONFIG",
			"kind-dev",
		]);
	});

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
			queryKeys.argoWorkspaceInspector(
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
			queryKeys.argoWorkspaceSelectedResource(
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
