import {
	createSavedPortForward,
	createWorkspaceScope,
	reconcileSavedPortForwardsForScope,
} from "./workspaces";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect(actual: unknown): {
	toEqual(expected: unknown): void;
};

describe("reconcileSavedPortForwardsForScope", () => {
	test("keeps Service forwards outside the resource namespace filter", () => {
		const scope = createWorkspaceScope({
			name: "ops",
			clusterContext: "admin@solid-k8s",
			namespaces: ["default"],
		});
		const portForward = createSavedPortForward({
			clusterContext: "admin@solid-k8s",
			namespace: "monitoring",
			serviceName: "grafana",
			servicePort: 80,
			localPort: 8081,
		});

		expect(reconcileSavedPortForwardsForScope([portForward], scope)).toEqual([
			portForward,
		]);
	});

	test("drops Service forwards from contexts outside the workspace", () => {
		const scope = createWorkspaceScope({
			name: "ops",
			clusterContext: "admin@solid-k8s",
			namespaces: [],
		});
		const portForward = createSavedPortForward({
			clusterContext: "other-context",
			namespace: "monitoring",
			serviceName: "grafana",
			servicePort: 80,
		});

		expect(reconcileSavedPortForwardsForScope([portForward], scope)).toEqual([]);
	});
});

describe("createWorkspaceScope", () => {
	test("initializes GitOps filter beside legacy Argo filter", () => {
		const scope = createWorkspaceScope({
			name: "ops",
			clusterContext: "admin@solid-k8s",
			namespaces: [],
		});

		expect({
			gitOpsFilter: scope.gitOpsFilter,
			argoAppFilter: scope.argoAppFilter,
		}).toEqual({ gitOpsFilter: "", argoAppFilter: "" });
	});
});
