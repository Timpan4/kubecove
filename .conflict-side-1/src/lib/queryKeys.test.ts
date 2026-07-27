import { isFiniteKubernetesQuery, queryKeys } from "./queryKeys";
import type { ResourceSummary } from "./types";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;
declare function expect<T>(actual: T): {
	toBe(expected: unknown): void;
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
		const keys = [
			queryKeys.namespaces("kind-dev"),
			queryKeys.resourceKinds("kind-dev"),
			queryKeys.resources("kind-dev", [{ kind: "Pod" }]),
			queryKeys.resourceDetails(resource),
			queryKeys.argoApps("kind-dev"),
			queryKeys.fluxDetect("kind-dev"),
			queryKeys.helmReleases("kind-dev"),
			queryKeys.rbacInspection("kind-dev", ["default"]),
		];

		expect(keys.every(isFiniteKubernetesQuery)).toBe(true);
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
